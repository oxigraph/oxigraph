use crate::model::NamedNode;
use crate::storage::numeric_encoder::EncodedTerm;
use oxrdf::Variable;
use spargebra::algebra::GraphPattern;
use std::cmp::max;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanNode {
    StaticBindings {
        tuples: Vec<EncodedTuple>,
    },
    Service {
        service_name: PatternValue,
        variables: Rc<Vec<Variable>>,
        child: Box<Self>,
        graph_pattern: Rc<GraphPattern>,
        silent: bool,
    },
    QuadPattern {
        subject: PatternValue,
        predicate: PatternValue,
        object: PatternValue,
        graph_name: PatternValue,
    },
    PathPattern {
        subject: PatternValue,
        path: Rc<PlanPropertyPath>,
        object: PatternValue,
        graph_name: PatternValue,
    },
    /// Streams left and materializes right join
    HashJoin {
        left: Box<Self>,
        right: Box<Self>,
    },
    /// Right nested in left loop
    ForLoopJoin {
        left: Box<Self>,
        right: Box<Self>,
    },
    /// Streams left and materializes right anti join
    AntiJoin {
        left: Box<Self>,
        right: Box<Self>,
    },
    Filter {
        child: Box<Self>,
        expression: Box<PlanExpression>,
    },
    Union {
        children: Vec<Self>,
    },
    /// right nested in left loop
    LeftJoin {
        left: Box<Self>,
        right: Box<Self>,
        possible_problem_vars: Rc<Vec<usize>>, //Variables that should not be part of the entry of the left join
    },
    Extend {
        child: Box<Self>,
        position: usize,
        expression: Box<PlanExpression>,
    },
    Sort {
        child: Box<Self>,
        by: Vec<Comparator>,
    },
    HashDeduplicate {
        child: Box<Self>,
    },
    /// Removes duplicated consecutive elements
    Reduced {
        child: Box<Self>,
    },
    Skip {
        child: Box<Self>,
        count: usize,
    },
    Limit {
        child: Box<Self>,
        count: usize,
    },
    Project {
        child: Box<Self>,
        mapping: Rc<Vec<(usize, usize)>>, // pairs of (variable key in child, variable key in output)
    },
    Aggregate {
        // By definition the group by key are the range 0..key_mapping.len()
        child: Box<Self>,
        key_mapping: Rc<Vec<(usize, usize)>>, // aggregate key pairs of (variable key in child, variable key in output)
        aggregates: Rc<Vec<(PlanAggregation, usize)>>,
    },
}

impl PlanNode {
    /// Returns variables that might be bound in the result set
    pub fn used_variables(&self) -> BTreeSet<usize> {
        let mut set = BTreeSet::default();
        self.lookup_used_variables(&mut |v| {
            set.insert(v);
        });
        set
    }

    pub fn lookup_used_variables(&self, callback: &mut impl FnMut(usize)) {
        match self {
            PlanNode::StaticBindings { tuples } => {
                for tuple in tuples {
                    for (key, value) in tuple.iter().enumerate() {
                        if value.is_some() {
                            callback(key);
                        }
                    }
                }
            }
            PlanNode::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => {
                if let PatternValue::Variable(var) = subject {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = predicate {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = object {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = graph_name {
                    callback(*var);
                }
            }
            PlanNode::PathPattern {
                subject,
                object,
                graph_name,
                ..
            } => {
                if let PatternValue::Variable(var) = subject {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = object {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = graph_name {
                    callback(*var);
                }
            }
            PlanNode::Filter { child, expression } => {
                expression.lookup_used_variables(callback);
                child.lookup_used_variables(callback);
            }
            PlanNode::Union { children } => {
                for child in children.iter() {
                    child.lookup_used_variables(callback);
                }
            }
            PlanNode::HashJoin { left, right }
            | PlanNode::ForLoopJoin { left, right, .. }
            | PlanNode::AntiJoin { left, right }
            | PlanNode::LeftJoin { left, right, .. } => {
                left.lookup_used_variables(callback);
                right.lookup_used_variables(callback);
            }
            PlanNode::Extend {
                child,
                position,
                expression,
            } => {
                callback(*position);
                expression.lookup_used_variables(callback);
                child.lookup_used_variables(callback);
            }
            PlanNode::Sort { child, .. }
            | PlanNode::HashDeduplicate { child }
            | PlanNode::Reduced { child }
            | PlanNode::Skip { child, .. }
            | PlanNode::Limit { child, .. } => child.lookup_used_variables(callback),
            PlanNode::Service {
                child,
                service_name,
                ..
            } => {
                if let PatternValue::Variable(v) = service_name {
                    callback(*v);
                }
                child.lookup_used_variables(callback);
            }
            PlanNode::Project { mapping, child } => {
                let child_bound = child.used_variables();
                for (child_i, output_i) in mapping.iter() {
                    if child_bound.contains(child_i) {
                        callback(*output_i);
                    }
                }
            }
            PlanNode::Aggregate {
                key_mapping,
                aggregates,
                ..
            } => {
                for (_, o) in key_mapping.iter() {
                    callback(*o);
                }
                for (_, var) in aggregates.iter() {
                    callback(*var);
                }
            }
        }
    }

    /// Returns subset of the set of variables that are always bound in the result set
    ///
    /// (subset because this function is not perfect yet)
    pub fn always_bound_variables(&self) -> BTreeSet<usize> {
        let mut set = BTreeSet::default();
        self.lookup_always_bound_variables(&mut |v| {
            set.insert(v);
        });
        set
    }

    pub fn lookup_always_bound_variables(&self, callback: &mut impl FnMut(usize)) {
        match self {
            PlanNode::StaticBindings { tuples } => {
                let mut variables = BTreeMap::default(); // value true iff always bound
                let max_tuple_length = tuples.iter().map(|t| t.capacity()).fold(0, max);
                for tuple in tuples {
                    for key in 0..max_tuple_length {
                        match variables.entry(key) {
                            Entry::Vacant(e) => {
                                e.insert(tuple.contains(key));
                            }
                            Entry::Occupied(mut e) => {
                                if !tuple.contains(key) {
                                    e.insert(false);
                                }
                            }
                        }
                    }
                }
                for (k, v) in variables {
                    if v {
                        callback(k);
                    }
                }
            }
            PlanNode::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => {
                if let PatternValue::Variable(var) = subject {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = predicate {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = object {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = graph_name {
                    callback(*var);
                }
            }
            PlanNode::PathPattern {
                subject,
                object,
                graph_name,
                ..
            } => {
                if let PatternValue::Variable(var) = subject {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = object {
                    callback(*var);
                }
                if let PatternValue::Variable(var) = graph_name {
                    callback(*var);
                }
            }
            PlanNode::Filter { child, .. } => {
                //TODO: have a look at the expression to know if it filters out unbound variables
                child.lookup_always_bound_variables(callback);
            }
            PlanNode::Union { children } => {
                if let Some(vars) = children
                    .iter()
                    .map(|c| c.always_bound_variables())
                    .reduce(|a, b| a.intersection(&b).copied().collect())
                {
                    for v in vars {
                        callback(v);
                    }
                }
            }
            PlanNode::HashJoin { left, right } | PlanNode::ForLoopJoin { left, right, .. } => {
                left.lookup_always_bound_variables(callback);
                right.lookup_always_bound_variables(callback);
            }
            PlanNode::AntiJoin { left, .. } | PlanNode::LeftJoin { left, .. } => {
                left.lookup_always_bound_variables(callback);
            }
            PlanNode::Extend {
                child,
                position,
                expression,
            } => {
                if matches!(expression.as_ref(), PlanExpression::Constant(_)) {
                    // TODO: more cases?
                    callback(*position);
                }
                child.lookup_always_bound_variables(callback);
            }
            PlanNode::Sort { child, .. }
            | PlanNode::HashDeduplicate { child }
            | PlanNode::Reduced { child }
            | PlanNode::Skip { child, .. }
            | PlanNode::Limit { child, .. } => child.lookup_always_bound_variables(callback),
            PlanNode::Service { child, silent, .. } => {
                if *silent {
                    // none, might return a null tuple
                } else {
                    child.lookup_always_bound_variables(callback)
                }
            }
            PlanNode::Project { mapping, child } => {
                let child_bound = child.always_bound_variables();
                for (child_i, output_i) in mapping.iter() {
                    if child_bound.contains(child_i) {
                        callback(*output_i);
                    }
                }
            }
            PlanNode::Aggregate { .. } => {
                //TODO
            }
        }
    }

    pub fn is_variable_bound(&self, variable: usize) -> bool {
        let mut found = false;
        self.lookup_always_bound_variables(&mut |v| {
            if v == variable {
                found = true;
            }
        });
        found
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PatternValue {
    Constant(EncodedTerm),
    Variable(usize),
    Triple(Box<TriplePatternValue>),
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct TriplePatternValue {
    pub subject: PatternValue,
    pub predicate: PatternValue,
    pub object: PatternValue,
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanExpression {
    Constant(EncodedTerm),
    Variable(usize),
    Exists(Rc<PlanNode>),
    Or(Box<Self>, Box<Self>),
    And(Box<Self>, Box<Self>),
    Equal(Box<Self>, Box<Self>),
    Greater(Box<Self>, Box<Self>),
    GreaterOrEqual(Box<Self>, Box<Self>),
    Less(Box<Self>, Box<Self>),
    LessOrEqual(Box<Self>, Box<Self>),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    Divide(Box<Self>, Box<Self>),
    UnaryPlus(Box<Self>),
    UnaryMinus(Box<Self>),
    Not(Box<Self>),
    Str(Box<Self>),
    Lang(Box<Self>),
    LangMatches(Box<Self>, Box<Self>),
    Datatype(Box<Self>),
    Bound(usize),
    Iri(Box<Self>),
    BNode(Option<Box<Self>>),
    Rand,
    Abs(Box<Self>),
    Ceil(Box<Self>),
    Floor(Box<Self>),
    Round(Box<Self>),
    Concat(Vec<Self>),
    SubStr(Box<Self>, Box<Self>, Option<Box<Self>>),
    StrLen(Box<Self>),
    Replace(Box<Self>, Box<Self>, Box<Self>, Option<Box<Self>>),
    UCase(Box<Self>),
    LCase(Box<Self>),
    EncodeForUri(Box<Self>),
    Contains(Box<Self>, Box<Self>),
    StrStarts(Box<Self>, Box<Self>),
    StrEnds(Box<Self>, Box<Self>),
    StrBefore(Box<Self>, Box<Self>),
    StrAfter(Box<Self>, Box<Self>),
    Year(Box<Self>),
    Month(Box<Self>),
    Day(Box<Self>),
    Hours(Box<Self>),
    Minutes(Box<Self>),
    Seconds(Box<Self>),
    Timezone(Box<Self>),
    Tz(Box<Self>),
    Now,
    Uuid,
    StrUuid,
    Md5(Box<Self>),
    Sha1(Box<Self>),
    Sha256(Box<Self>),
    Sha384(Box<Self>),
    Sha512(Box<Self>),
    Coalesce(Vec<Self>),
    If(Box<Self>, Box<Self>, Box<Self>),
    StrLang(Box<Self>, Box<Self>),
    StrDt(Box<Self>, Box<Self>),
    SameTerm(Box<Self>, Box<Self>),
    IsIri(Box<Self>),
    IsBlank(Box<Self>),
    IsLiteral(Box<Self>),
    IsNumeric(Box<Self>),
    Regex(Box<Self>, Box<Self>, Option<Box<Self>>),
    Triple(Box<Self>, Box<Self>, Box<Self>),
    Subject(Box<Self>),
    Predicate(Box<Self>),
    Object(Box<Self>),
    IsTriple(Box<Self>),
    BooleanCast(Box<Self>),
    DoubleCast(Box<Self>),
    FloatCast(Box<Self>),
    DecimalCast(Box<Self>),
    IntegerCast(Box<Self>),
    DateCast(Box<Self>),
    TimeCast(Box<Self>),
    DateTimeCast(Box<Self>),
    DurationCast(Box<Self>),
    YearMonthDurationCast(Box<Self>),
    DayTimeDurationCast(Box<Self>),
    StringCast(Box<Self>),
    CustomFunction(NamedNode, Vec<Self>),
}

impl PlanExpression {
    pub fn lookup_used_variables(&self, callback: &mut impl FnMut(usize)) {
        match self {
            PlanExpression::Variable(v) | PlanExpression::Bound(v) => {
                callback(*v);
            }
            PlanExpression::Constant(_)
            | PlanExpression::Rand
            | PlanExpression::Now
            | PlanExpression::Uuid
            | PlanExpression::StrUuid
            | PlanExpression::BNode(None) => (),
            PlanExpression::UnaryPlus(e)
            | PlanExpression::UnaryMinus(e)
            | PlanExpression::Not(e)
            | PlanExpression::BNode(Some(e))
            | PlanExpression::Str(e)
            | PlanExpression::Lang(e)
            | PlanExpression::Datatype(e)
            | PlanExpression::Iri(e)
            | PlanExpression::Abs(e)
            | PlanExpression::Ceil(e)
            | PlanExpression::Floor(e)
            | PlanExpression::Round(e)
            | PlanExpression::UCase(e)
            | PlanExpression::LCase(e)
            | PlanExpression::StrLen(e)
            | PlanExpression::EncodeForUri(e)
            | PlanExpression::Year(e)
            | PlanExpression::Month(e)
            | PlanExpression::Day(e)
            | PlanExpression::Hours(e)
            | PlanExpression::Minutes(e)
            | PlanExpression::Seconds(e)
            | PlanExpression::Timezone(e)
            | PlanExpression::Tz(e)
            | PlanExpression::Md5(e)
            | PlanExpression::Sha1(e)
            | PlanExpression::Sha256(e)
            | PlanExpression::Sha384(e)
            | PlanExpression::Sha512(e)
            | PlanExpression::IsIri(e)
            | PlanExpression::IsBlank(e)
            | PlanExpression::IsLiteral(e)
            | PlanExpression::IsNumeric(e)
            | PlanExpression::IsTriple(e)
            | PlanExpression::Subject(e)
            | PlanExpression::Predicate(e)
            | PlanExpression::Object(e)
            | PlanExpression::BooleanCast(e)
            | PlanExpression::DoubleCast(e)
            | PlanExpression::FloatCast(e)
            | PlanExpression::DecimalCast(e)
            | PlanExpression::IntegerCast(e)
            | PlanExpression::DateCast(e)
            | PlanExpression::TimeCast(e)
            | PlanExpression::DateTimeCast(e)
            | PlanExpression::DurationCast(e)
            | PlanExpression::YearMonthDurationCast(e)
            | PlanExpression::DayTimeDurationCast(e)
            | PlanExpression::StringCast(e) => e.lookup_used_variables(callback),
            PlanExpression::Or(a, b)
            | PlanExpression::And(a, b)
            | PlanExpression::Equal(a, b)
            | PlanExpression::Greater(a, b)
            | PlanExpression::GreaterOrEqual(a, b)
            | PlanExpression::Less(a, b)
            | PlanExpression::LessOrEqual(a, b)
            | PlanExpression::Add(a, b)
            | PlanExpression::Subtract(a, b)
            | PlanExpression::Multiply(a, b)
            | PlanExpression::Divide(a, b)
            | PlanExpression::LangMatches(a, b)
            | PlanExpression::Contains(a, b)
            | PlanExpression::StrStarts(a, b)
            | PlanExpression::StrEnds(a, b)
            | PlanExpression::StrBefore(a, b)
            | PlanExpression::StrAfter(a, b)
            | PlanExpression::StrLang(a, b)
            | PlanExpression::StrDt(a, b)
            | PlanExpression::SameTerm(a, b)
            | PlanExpression::SubStr(a, b, None)
            | PlanExpression::Regex(a, b, None) => {
                a.lookup_used_variables(callback);
                b.lookup_used_variables(callback);
            }
            PlanExpression::If(a, b, c)
            | PlanExpression::SubStr(a, b, Some(c))
            | PlanExpression::Regex(a, b, Some(c))
            | PlanExpression::Replace(a, b, c, None)
            | PlanExpression::Triple(a, b, c) => {
                a.lookup_used_variables(callback);
                b.lookup_used_variables(callback);
                c.lookup_used_variables(callback);
            }
            PlanExpression::Replace(a, b, c, Some(d)) => {
                a.lookup_used_variables(callback);
                b.lookup_used_variables(callback);
                c.lookup_used_variables(callback);
                d.lookup_used_variables(callback);
            }
            PlanExpression::Concat(es)
            | PlanExpression::Coalesce(es)
            | PlanExpression::CustomFunction(_, es) => {
                for e in es {
                    e.lookup_used_variables(callback);
                }
            }
            PlanExpression::Exists(e) => {
                e.lookup_used_variables(callback);
            }
        }
    }
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
    GroupConcat { separator: Rc<String> },
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanPropertyPath {
    Path(EncodedTerm),
    Reverse(Rc<Self>),
    Sequence(Rc<Self>, Rc<Self>),
    Alternative(Rc<Self>, Rc<Self>),
    ZeroOrMore(Rc<Self>),
    OneOrMore(Rc<Self>),
    ZeroOrOne(Rc<Self>),
    NegatedPropertySet(Rc<Vec<EncodedTerm>>),
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Comparator {
    Asc(PlanExpression),
    Desc(PlanExpression),
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct TripleTemplate {
    pub subject: TripleTemplateValue,
    pub predicate: TripleTemplateValue,
    pub object: TripleTemplateValue,
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum TripleTemplateValue {
    Constant(EncodedTerm),
    BlankNode(usize),
    Variable(usize),
    Triple(Box<TripleTemplate>),
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

    pub fn get(&self, index: usize) -> Option<&EncodedTerm> {
        self.inner.get(index).unwrap_or(&None).as_ref()
    }

    pub fn iter(&self) -> impl Iterator<Item = Option<EncodedTerm>> + '_ {
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

    pub fn combine_with(&self, other: &Self) -> Option<Self> {
        if self.inner.len() < other.inner.len() {
            let mut result = other.inner.clone();
            for (key, self_value) in self.inner.iter().enumerate() {
                if let Some(self_value) = self_value {
                    match &other.inner[key] {
                        Some(other_value) => {
                            if self_value != other_value {
                                return None;
                            }
                        }
                        None => result[key] = Some(self_value.clone()),
                    }
                }
            }
            Some(Self { inner: result })
        } else {
            let mut result = self.inner.clone();
            for (key, other_value) in other.inner.iter().enumerate() {
                if let Some(other_value) = other_value {
                    match &self.inner[key] {
                        Some(self_value) => {
                            if self_value != other_value {
                                return None;
                            }
                        }
                        None => result[key] = Some(other_value.clone()),
                    }
                }
            }
            Some(Self { inner: result })
        }
    }
}

impl IntoIterator for EncodedTuple {
    type Item = Option<EncodedTerm>;
    type IntoIter = std::vec::IntoIter<Option<EncodedTerm>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}
