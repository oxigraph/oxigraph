use crate::sparql::model::Variable;
use crate::sparql::GraphPattern;
use crate::store::numeric_encoder::{
    EncodedQuad, EncodedTerm, Encoder, MemoryStrStore, StrContainer, StrHash, StrLookup,
    ENCODED_DEFAULT_GRAPH,
};
use crate::store::StoreConnection;
use crate::Result;
use std::cell::{RefCell, RefMut};
use std::collections::BTreeSet;

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanNode {
    Init,
    StaticBindings {
        tuples: Vec<EncodedTuple>,
    },
    Service {
        service_name: PatternValue,
        variables: Vec<Variable>,
        child: Box<PlanNode>,
        graph_pattern: Box<GraphPattern>,
        silent: bool,
    },
    QuadPatternJoin {
        child: Box<PlanNode>,
        subject: PatternValue,
        predicate: PatternValue,
        object: PatternValue,
        graph_name: PatternValue,
    },
    PathPatternJoin {
        child: Box<PlanNode>,
        subject: PatternValue,
        path: PlanPropertyPath,
        object: PatternValue,
        graph_name: PatternValue,
    },
    Join {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
    },
    AntiJoin {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
    },
    Filter {
        child: Box<PlanNode>,
        expression: PlanExpression,
    },
    Union {
        children: Vec<PlanNode>,
    },
    LeftJoin {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        possible_problem_vars: Vec<usize>, //Variables that should not be part of the entry of the left join
    },
    Extend {
        child: Box<PlanNode>,
        position: usize,
        expression: PlanExpression,
    },
    Sort {
        child: Box<PlanNode>,
        by: Vec<Comparator>,
    },
    HashDeduplicate {
        child: Box<PlanNode>,
    },
    Skip {
        child: Box<PlanNode>,
        count: usize,
    },
    Limit {
        child: Box<PlanNode>,
        count: usize,
    },
    Project {
        child: Box<PlanNode>,
        mapping: Vec<(usize, usize)>, // pairs of (variable key in child, variable key in output)
    },
    Aggregate {
        // By definition the group by key are the range 0..key_mapping.len()
        child: Box<PlanNode>,
        key_mapping: Vec<(usize, usize)>, // aggregate key pairs of (variable key in child, variable key in output)
        aggregates: Vec<(PlanAggregation, usize)>,
    },
}

impl PlanNode {
    /// Returns variables that might be bound in the result set
    pub fn maybe_bound_variables(&self) -> BTreeSet<usize> {
        let mut set = BTreeSet::default();
        self.add_maybe_bound_variables(&mut set);
        set
    }

    fn add_maybe_bound_variables(&self, set: &mut BTreeSet<usize>) {
        match self {
            PlanNode::Init => (),
            PlanNode::StaticBindings { tuples } => {
                for tuple in tuples {
                    for (key, value) in tuple.iter().enumerate() {
                        if value.is_some() {
                            set.insert(key);
                        }
                    }
                }
            }
            PlanNode::QuadPatternJoin {
                child,
                subject,
                predicate,
                object,
                graph_name,
            } => {
                if let PatternValue::Variable(var) = subject {
                    set.insert(*var);
                }
                if let PatternValue::Variable(var) = predicate {
                    set.insert(*var);
                }
                if let PatternValue::Variable(var) = object {
                    set.insert(*var);
                }
                if let PatternValue::Variable(var) = graph_name {
                    set.insert(*var);
                }
                child.add_maybe_bound_variables(set);
            }
            PlanNode::PathPatternJoin {
                child,
                subject,
                object,
                graph_name,
                ..
            } => {
                if let PatternValue::Variable(var) = subject {
                    set.insert(*var);
                }
                if let PatternValue::Variable(var) = object {
                    set.insert(*var);
                }
                if let PatternValue::Variable(var) = graph_name {
                    set.insert(*var);
                }
                child.add_maybe_bound_variables(set);
            }
            PlanNode::Filter { child, .. } => child.add_maybe_bound_variables(set),
            PlanNode::Union { children } => {
                for child in children {
                    child.add_maybe_bound_variables(set);
                }
            }
            PlanNode::Join { left, right, .. }
            | PlanNode::AntiJoin { left, right, .. }
            | PlanNode::LeftJoin { left, right, .. } => {
                left.add_maybe_bound_variables(set);
                right.add_maybe_bound_variables(set);
            }
            PlanNode::Extend {
                child, position, ..
            } => {
                set.insert(*position);
                child.add_maybe_bound_variables(set);
            }
            PlanNode::Service { child, .. }
            | PlanNode::Sort { child, .. }
            | PlanNode::HashDeduplicate { child }
            | PlanNode::Skip { child, .. }
            | PlanNode::Limit { child, .. } => child.add_maybe_bound_variables(set),
            PlanNode::Project { mapping, child } => {
                let child_bound = child.maybe_bound_variables();
                for (child_i, output_i) in mapping.iter() {
                    if child_bound.contains(child_i) {
                        set.insert(*output_i);
                    }
                }
            }
            PlanNode::Aggregate {
                key_mapping,
                aggregates,
                ..
            } => {
                set.extend(key_mapping.iter().map(|(_, o)| o));
                for (_, var) in aggregates {
                    set.insert(*var);
                }
            }
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum PatternValue {
    Constant(EncodedTerm),
    Variable(usize),
}

impl PatternValue {
    pub fn is_var(&self) -> bool {
        match self {
            PatternValue::Constant(_) => false,
            PatternValue::Variable(_) => true,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanExpression {
    Constant(EncodedTerm),
    Variable(usize),
    Exists(Box<PlanNode>),
    Or(Box<PlanExpression>, Box<PlanExpression>),
    And(Box<PlanExpression>, Box<PlanExpression>),
    Equal(Box<PlanExpression>, Box<PlanExpression>),
    NotEqual(Box<PlanExpression>, Box<PlanExpression>),
    Greater(Box<PlanExpression>, Box<PlanExpression>),
    GreaterOrEq(Box<PlanExpression>, Box<PlanExpression>),
    Lower(Box<PlanExpression>, Box<PlanExpression>),
    LowerOrEq(Box<PlanExpression>, Box<PlanExpression>),
    In(Box<PlanExpression>, Vec<PlanExpression>),
    Add(Box<PlanExpression>, Box<PlanExpression>),
    Sub(Box<PlanExpression>, Box<PlanExpression>),
    Mul(Box<PlanExpression>, Box<PlanExpression>),
    Div(Box<PlanExpression>, Box<PlanExpression>),
    UnaryPlus(Box<PlanExpression>),
    UnaryMinus(Box<PlanExpression>),
    UnaryNot(Box<PlanExpression>),
    Str(Box<PlanExpression>),
    Lang(Box<PlanExpression>),
    LangMatches(Box<PlanExpression>, Box<PlanExpression>),
    Datatype(Box<PlanExpression>),
    Bound(usize),
    IRI(Box<PlanExpression>),
    BNode(Option<Box<PlanExpression>>),
    Rand,
    Abs(Box<PlanExpression>),
    Ceil(Box<PlanExpression>),
    Floor(Box<PlanExpression>),
    Round(Box<PlanExpression>),
    Concat(Vec<PlanExpression>),
    SubStr(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Option<Box<PlanExpression>>,
    ),
    StrLen(Box<PlanExpression>),
    Replace(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Box<PlanExpression>,
        Option<Box<PlanExpression>>,
    ),
    UCase(Box<PlanExpression>),
    LCase(Box<PlanExpression>),
    EncodeForURI(Box<PlanExpression>),
    Contains(Box<PlanExpression>, Box<PlanExpression>),
    StrStarts(Box<PlanExpression>, Box<PlanExpression>),
    StrEnds(Box<PlanExpression>, Box<PlanExpression>),
    StrBefore(Box<PlanExpression>, Box<PlanExpression>),
    StrAfter(Box<PlanExpression>, Box<PlanExpression>),
    Year(Box<PlanExpression>),
    Month(Box<PlanExpression>),
    Day(Box<PlanExpression>),
    Hours(Box<PlanExpression>),
    Minutes(Box<PlanExpression>),
    Seconds(Box<PlanExpression>),
    Timezone(Box<PlanExpression>),
    Tz(Box<PlanExpression>),
    Now,
    UUID,
    StrUUID,
    MD5(Box<PlanExpression>),
    SHA1(Box<PlanExpression>),
    SHA256(Box<PlanExpression>),
    SHA384(Box<PlanExpression>),
    SHA512(Box<PlanExpression>),
    Coalesce(Vec<PlanExpression>),
    If(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Box<PlanExpression>,
    ),
    StrLang(Box<PlanExpression>, Box<PlanExpression>),
    StrDT(Box<PlanExpression>, Box<PlanExpression>),
    SameTerm(Box<PlanExpression>, Box<PlanExpression>),
    IsIRI(Box<PlanExpression>),
    IsBlank(Box<PlanExpression>),
    IsLiteral(Box<PlanExpression>),
    IsNumeric(Box<PlanExpression>),
    Regex(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Option<Box<PlanExpression>>,
    ),
    BooleanCast(Box<PlanExpression>),
    DoubleCast(Box<PlanExpression>),
    FloatCast(Box<PlanExpression>),
    DecimalCast(Box<PlanExpression>),
    IntegerCast(Box<PlanExpression>),
    DateCast(Box<PlanExpression>),
    TimeCast(Box<PlanExpression>),
    DateTimeCast(Box<PlanExpression>),
    DurationCast(Box<PlanExpression>),
    StringCast(Box<PlanExpression>),
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct PlanAggregation {
    pub function: PlanAggregationFunction,
    pub parameter: Option<PlanExpression>,
    pub distinct: bool,
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanAggregationFunction {
    Count,
    Sum,
    Min,
    Max,
    Avg,
    Sample,
    GroupConcat { separator: String },
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanPropertyPath {
    PredicatePath(EncodedTerm),
    InversePath(Box<PlanPropertyPath>),
    SequencePath(Box<PlanPropertyPath>, Box<PlanPropertyPath>),
    AlternativePath(Box<PlanPropertyPath>, Box<PlanPropertyPath>),
    ZeroOrMorePath(Box<PlanPropertyPath>),
    OneOrMorePath(Box<PlanPropertyPath>),
    ZeroOrOnePath(Box<PlanPropertyPath>),
    NegatedPropertySet(Vec<EncodedTerm>),
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Comparator {
    Asc(PlanExpression),
    Desc(PlanExpression),
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct TripleTemplate {
    pub subject: TripleTemplateValue,
    pub predicate: TripleTemplateValue,
    pub object: TripleTemplateValue,
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum TripleTemplateValue {
    Constant(EncodedTerm),
    BlankNode(usize),
    Variable(usize),
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct EncodedTuple {
    inner: Vec<Option<EncodedTerm>>,
}

impl EncodedTuple {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    pub fn contains(&self, index: usize) -> bool {
        self.inner.get(index).map_or(false, Option::is_some)
    }

    pub fn get(&self, index: usize) -> Option<EncodedTerm> {
        self.inner.get(index).cloned().unwrap_or(None)
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = Option<EncodedTerm>> + 'a {
        self.inner.iter().cloned()
    }

    pub fn set(&mut self, index: usize, value: EncodedTerm) {
        if self.inner.len() <= index {
            self.inner.resize(index + 1, None);
        }
        self.inner[index] = Some(value);
    }

    pub fn unset(&mut self, index: usize) {
        if let Some(v) = self.inner.get_mut(index) {
            *v = None;
        }
    }

    pub fn combine_with(&self, other: &EncodedTuple) -> Option<Self> {
        if self.inner.len() < other.inner.len() {
            let mut result = other.inner.to_owned();
            for (key, self_value) in self.inner.iter().enumerate() {
                if let Some(self_value) = self_value {
                    match other.inner[key] {
                        Some(ref other_value) => {
                            if self_value != other_value {
                                return None;
                            }
                        }
                        None => result[key] = Some(*self_value),
                    }
                }
            }
            Some(EncodedTuple { inner: result })
        } else {
            let mut result = self.inner.to_owned();
            for (key, other_value) in other.inner.iter().enumerate() {
                if let Some(other_value) = other_value {
                    match self.inner[key] {
                        Some(ref self_value) => {
                            if self_value != other_value {
                                return None;
                            }
                        }
                        None => result[key] = Some(*other_value),
                    }
                }
            }
            Some(EncodedTuple { inner: result })
        }
    }
}

pub struct DatasetView<S: StoreConnection> {
    store: S,
    extra: RefCell<MemoryStrStore>,
    default_graph_as_union: bool,
}

impl<S: StoreConnection> DatasetView<S> {
    pub fn new(store: S, default_graph_as_union: bool) -> Self {
        Self {
            store,
            extra: RefCell::new(MemoryStrStore::default()),
            default_graph_as_union,
        }
    }

    pub fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'a> {
        if graph_name == None {
            Box::new(
                self.store
                    .quads_for_pattern(subject, predicate, object, None)
                    .filter(|quad| match quad {
                        Err(_) => true,
                        Ok(quad) => quad.graph_name != ENCODED_DEFAULT_GRAPH,
                    }),
            )
        } else if graph_name == Some(ENCODED_DEFAULT_GRAPH) && self.default_graph_as_union {
            Box::new(
                self.store
                    .quads_for_pattern(subject, predicate, object, None)
                    .map(|quad| {
                        let quad = quad?;
                        Ok(EncodedQuad::new(
                            quad.subject,
                            quad.predicate,
                            quad.object,
                            ENCODED_DEFAULT_GRAPH,
                        ))
                    }),
            )
        } else {
            self.store
                .quads_for_pattern(subject, predicate, object, graph_name)
        }
    }

    pub fn encoder<'a>(&'a self) -> impl Encoder + StrContainer + 'a {
        DatasetViewStrContainer {
            store: &self.store,
            extra: self.extra.borrow_mut(),
        }
    }
}

impl<S: StoreConnection> StrLookup for DatasetView<S> {
    fn get_str(&self, id: StrHash) -> Result<Option<String>> {
        if let Some(value) = self.extra.borrow().get_str(id)? {
            Ok(Some(value))
        } else {
            self.store.get_str(id)
        }
    }
}

struct DatasetViewStrContainer<'a, S: StoreConnection> {
    store: &'a S,
    extra: RefMut<'a, MemoryStrStore>,
}

impl<'a, S: StoreConnection> StrContainer for DatasetViewStrContainer<'a, S> {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<()> {
        if self.store.get_str(key)?.is_none() {
            self.extra.insert_str(key, value)
        } else {
            Ok(())
        }
    }
}
