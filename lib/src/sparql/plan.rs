use crate::model::NamedNode;
use crate::storage::numeric_encoder::EncodedTerm;
use oxrdf::Variable;
use regex::Regex;
use spargebra::algebra::GraphPattern;
use std::cmp::max;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

#[derive(Debug, Clone)]
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
    /// hash left join
    HashLeftJoin {
        left: Box<Self>,
        right: Box<Self>,
        expression: Box<PlanExpression>,
    },
    /// right nested in left loop
    ForLoopLeftJoin {
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
        key_variables: Rc<Vec<usize>>,
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
            Self::StaticBindings { tuples } => {
                for tuple in tuples {
                    for (key, value) in tuple.iter().enumerate() {
                        if value.is_some() {
                            callback(key);
                        }
                    }
                }
            }
            Self::QuadPattern {
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
            Self::PathPattern {
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
            Self::Filter { child, expression } => {
                expression.lookup_used_variables(callback);
                child.lookup_used_variables(callback);
            }
            Self::Union { children } => {
                for child in children.iter() {
                    child.lookup_used_variables(callback);
                }
            }
            Self::HashJoin { left, right }
            | Self::ForLoopJoin { left, right, .. }
            | Self::AntiJoin { left, right }
            | Self::ForLoopLeftJoin { left, right, .. } => {
                left.lookup_used_variables(callback);
                right.lookup_used_variables(callback);
            }
            Self::HashLeftJoin {
                left,
                right,
                expression,
            } => {
                left.lookup_used_variables(callback);
                right.lookup_used_variables(callback);
                expression.lookup_used_variables(callback);
            }
            Self::Extend {
                child,
                position,
                expression,
            } => {
                callback(*position);
                expression.lookup_used_variables(callback);
                child.lookup_used_variables(callback);
            }
            Self::Sort { child, .. }
            | Self::HashDeduplicate { child }
            | Self::Reduced { child }
            | Self::Skip { child, .. }
            | Self::Limit { child, .. } => child.lookup_used_variables(callback),
            Self::Service {
                child,
                service_name,
                ..
            } => {
                if let PatternValue::Variable(v) = service_name {
                    callback(*v);
                }
                child.lookup_used_variables(callback);
            }
            Self::Project { mapping, child } => {
                let child_bound = child.used_variables();
                for (child_i, output_i) in mapping.iter() {
                    if child_bound.contains(child_i) {
                        callback(*output_i);
                    }
                }
            }
            Self::Aggregate {
                key_variables,
                aggregates,
                ..
            } => {
                for var in key_variables.iter() {
                    callback(*var);
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
            Self::StaticBindings { tuples } => {
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
            Self::QuadPattern {
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
            Self::PathPattern {
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
            Self::Filter { child, .. } => {
                //TODO: have a look at the expression to know if it filters out unbound variables
                child.lookup_always_bound_variables(callback);
            }
            Self::Union { children } => {
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
            Self::HashJoin { left, right } | Self::ForLoopJoin { left, right, .. } => {
                left.lookup_always_bound_variables(callback);
                right.lookup_always_bound_variables(callback);
            }
            Self::AntiJoin { left, .. }
            | Self::HashLeftJoin { left, .. }
            | Self::ForLoopLeftJoin { left, .. } => {
                left.lookup_always_bound_variables(callback);
            }
            Self::Extend {
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
            Self::Sort { child, .. }
            | Self::HashDeduplicate { child }
            | Self::Reduced { child }
            | Self::Skip { child, .. }
            | Self::Limit { child, .. } => child.lookup_always_bound_variables(callback),
            Self::Service { child, silent, .. } => {
                if *silent {
                    // none, might return a null tuple
                } else {
                    child.lookup_always_bound_variables(callback)
                }
            }
            Self::Project { mapping, child } => {
                let child_bound = child.always_bound_variables();
                for (child_i, output_i) in mapping.iter() {
                    if child_bound.contains(child_i) {
                        callback(*output_i);
                    }
                }
            }
            Self::Aggregate { .. } => {
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

#[derive(Debug, Clone)]
pub enum PatternValue {
    Constant(EncodedTerm),
    Variable(usize),
    Triple(Box<TriplePatternValue>),
}

#[derive(Debug, Clone)]
pub struct TriplePatternValue {
    pub subject: PatternValue,
    pub predicate: PatternValue,
    pub object: PatternValue,
}

#[derive(Debug, Clone)]
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
    StaticReplace(Box<Self>, Regex, Box<Self>),
    DynamicReplace(Box<Self>, Box<Self>, Box<Self>, Option<Box<Self>>),
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
    StaticRegex(Box<Self>, Regex),
    DynamicRegex(Box<Self>, Box<Self>, Option<Box<Self>>),
    Triple(Box<Self>, Box<Self>, Box<Self>),
    Subject(Box<Self>),
    Predicate(Box<Self>),
    Object(Box<Self>),
    IsTriple(Box<Self>),
    Adjust(Box<Self>, Box<Self>),
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
            Self::Variable(v) | Self::Bound(v) => {
                callback(*v);
            }
            Self::Constant(_)
            | Self::Rand
            | Self::Now
            | Self::Uuid
            | Self::StrUuid
            | Self::BNode(None) => (),
            Self::UnaryPlus(e)
            | Self::UnaryMinus(e)
            | Self::Not(e)
            | Self::BNode(Some(e))
            | Self::Str(e)
            | Self::Lang(e)
            | Self::Datatype(e)
            | Self::Iri(e)
            | Self::Abs(e)
            | Self::Ceil(e)
            | Self::Floor(e)
            | Self::Round(e)
            | Self::UCase(e)
            | Self::LCase(e)
            | Self::StrLen(e)
            | Self::EncodeForUri(e)
            | Self::StaticRegex(e, _)
            | Self::Year(e)
            | Self::Month(e)
            | Self::Day(e)
            | Self::Hours(e)
            | Self::Minutes(e)
            | Self::Seconds(e)
            | Self::Timezone(e)
            | Self::Tz(e)
            | Self::Md5(e)
            | Self::Sha1(e)
            | Self::Sha256(e)
            | Self::Sha384(e)
            | Self::Sha512(e)
            | Self::IsIri(e)
            | Self::IsBlank(e)
            | Self::IsLiteral(e)
            | Self::IsNumeric(e)
            | Self::IsTriple(e)
            | Self::Subject(e)
            | Self::Predicate(e)
            | Self::Object(e)
            | Self::BooleanCast(e)
            | Self::DoubleCast(e)
            | Self::FloatCast(e)
            | Self::DecimalCast(e)
            | Self::IntegerCast(e)
            | Self::DateCast(e)
            | Self::TimeCast(e)
            | Self::DateTimeCast(e)
            | Self::DurationCast(e)
            | Self::YearMonthDurationCast(e)
            | Self::DayTimeDurationCast(e)
            | Self::StringCast(e) => e.lookup_used_variables(callback),
            Self::Or(a, b)
            | Self::And(a, b)
            | Self::Equal(a, b)
            | Self::Greater(a, b)
            | Self::GreaterOrEqual(a, b)
            | Self::Less(a, b)
            | Self::LessOrEqual(a, b)
            | Self::Add(a, b)
            | Self::Subtract(a, b)
            | Self::Multiply(a, b)
            | Self::Divide(a, b)
            | Self::LangMatches(a, b)
            | Self::Contains(a, b)
            | Self::StaticReplace(a, _, b)
            | Self::StrStarts(a, b)
            | Self::StrEnds(a, b)
            | Self::StrBefore(a, b)
            | Self::StrAfter(a, b)
            | Self::StrLang(a, b)
            | Self::StrDt(a, b)
            | Self::SameTerm(a, b)
            | Self::SubStr(a, b, None)
            | Self::DynamicRegex(a, b, None)
            | Self::Adjust(a, b) => {
                a.lookup_used_variables(callback);
                b.lookup_used_variables(callback);
            }
            Self::If(a, b, c)
            | Self::SubStr(a, b, Some(c))
            | Self::DynamicRegex(a, b, Some(c))
            | Self::DynamicReplace(a, b, c, None)
            | Self::Triple(a, b, c) => {
                a.lookup_used_variables(callback);
                b.lookup_used_variables(callback);
                c.lookup_used_variables(callback);
            }
            Self::DynamicReplace(a, b, c, Some(d)) => {
                a.lookup_used_variables(callback);
                b.lookup_used_variables(callback);
                c.lookup_used_variables(callback);
                d.lookup_used_variables(callback);
            }
            Self::Concat(es) | Self::Coalesce(es) | Self::CustomFunction(_, es) => {
                for e in es {
                    e.lookup_used_variables(callback);
                }
            }
            Self::Exists(e) => {
                e.lookup_used_variables(callback);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanAggregation {
    pub function: PlanAggregationFunction,
    pub parameter: Option<PlanExpression>,
    pub distinct: bool,
}

#[derive(Debug, Clone)]
pub enum PlanAggregationFunction {
    Count,
    Sum,
    Min,
    Max,
    Avg,
    Sample,
    GroupConcat { separator: Rc<String> },
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum Comparator {
    Asc(PlanExpression),
    Desc(PlanExpression),
}

#[derive(Debug, Clone)]
pub struct TripleTemplate {
    pub subject: TripleTemplateValue,
    pub predicate: TripleTemplateValue,
    pub object: TripleTemplateValue,
}

#[derive(Debug, Clone)]
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
