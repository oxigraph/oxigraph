use crate::sparql::model::Variable;
use crate::storage::numeric_encoder::EncodedTerm;
use spargebra::algebra::GraphPattern;
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanNode {
    StaticBindings {
        tuples: Vec<EncodedTuple>,
    },
    Service {
        service_name: PatternValue,
        variables: Rc<Vec<Variable>>,
        child: Rc<Self>,
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
    HashJoin {
        left: Rc<Self>,
        right: Rc<Self>,
    },
    ForLoopJoin {
        left: Rc<Self>,
        right: Rc<Self>,
    },
    AntiJoin {
        left: Rc<Self>,
        right: Rc<Self>,
    },
    Filter {
        child: Rc<Self>,
        expression: Rc<PlanExpression>,
    },
    Union {
        children: Vec<Rc<Self>>,
    },
    LeftJoin {
        left: Rc<Self>,
        right: Rc<Self>,
        possible_problem_vars: Rc<Vec<usize>>, //Variables that should not be part of the entry of the left join
    },
    Extend {
        child: Rc<Self>,
        position: usize,
        expression: Rc<PlanExpression>,
    },
    Sort {
        child: Rc<Self>,
        by: Vec<Comparator>,
    },
    HashDeduplicate {
        child: Rc<Self>,
    },
    Skip {
        child: Rc<Self>,
        count: usize,
    },
    Limit {
        child: Rc<Self>,
        count: usize,
    },
    Project {
        child: Rc<Self>,
        mapping: Rc<Vec<(usize, usize)>>, // pairs of (variable key in child, variable key in output)
    },
    Aggregate {
        // By definition the group by key are the range 0..key_mapping.len()
        child: Rc<Self>,
        key_mapping: Rc<Vec<(usize, usize)>>, // aggregate key pairs of (variable key in child, variable key in output)
        aggregates: Rc<Vec<(PlanAggregation, usize)>>,
    },
}

impl PlanNode {
    /// Returns variables that might be bound in the result set
    pub fn maybe_bound_variables(&self) -> BTreeSet<usize> {
        let mut set = BTreeSet::default();
        self.add_maybe_bound_variables(&mut set);
        set
    }

    pub fn add_maybe_bound_variables(&self, set: &mut BTreeSet<usize>) {
        match self {
            PlanNode::StaticBindings { tuples } => {
                for tuple in tuples {
                    for (key, value) in tuple.iter().enumerate() {
                        if value.is_some() {
                            set.insert(key);
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
            }
            PlanNode::PathPattern {
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
            }
            PlanNode::Filter { child, expression } => {
                expression.add_maybe_bound_variables(set);
                child.add_maybe_bound_variables(set);
            }
            PlanNode::Union { children } => {
                for child in children.iter() {
                    child.add_maybe_bound_variables(set);
                }
            }
            PlanNode::HashJoin { left, right }
            | PlanNode::ForLoopJoin { left, right, .. }
            | PlanNode::AntiJoin { left, right }
            | PlanNode::LeftJoin { left, right, .. } => {
                left.add_maybe_bound_variables(set);
                right.add_maybe_bound_variables(set);
            }
            PlanNode::Extend {
                child,
                position,
                expression,
            } => {
                set.insert(*position);
                expression.add_maybe_bound_variables(set);
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
                for (_, var) in aggregates.iter() {
                    set.insert(*var);
                }
            }
        }
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
    In(Box<Self>, Vec<Self>),
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
}

impl PlanExpression {
    pub fn add_maybe_bound_variables(&self, set: &mut BTreeSet<usize>) {
        match self {
            PlanExpression::Variable(v) | PlanExpression::Bound(v) => {
                set.insert(*v);
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
            | PlanExpression::StringCast(e) => e.add_maybe_bound_variables(set),
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
                a.add_maybe_bound_variables(set);
                b.add_maybe_bound_variables(set);
            }
            PlanExpression::If(a, b, c)
            | PlanExpression::SubStr(a, b, Some(c))
            | PlanExpression::Regex(a, b, Some(c))
            | PlanExpression::Replace(a, b, c, None)
            | PlanExpression::Triple(a, b, c) => {
                a.add_maybe_bound_variables(set);
                b.add_maybe_bound_variables(set);
                c.add_maybe_bound_variables(set);
            }
            PlanExpression::Replace(a, b, c, Some(d)) => {
                a.add_maybe_bound_variables(set);
                b.add_maybe_bound_variables(set);
                c.add_maybe_bound_variables(set);
                d.add_maybe_bound_variables(set);
            }

            PlanExpression::Concat(es) | PlanExpression::Coalesce(es) => {
                for e in es {
                    e.add_maybe_bound_variables(set);
                }
            }
            PlanExpression::In(a, bs) => {
                a.add_maybe_bound_variables(set);
                for b in bs {
                    b.add_maybe_bound_variables(set);
                }
            }
            PlanExpression::Exists(e) => {
                e.add_maybe_bound_variables(set);
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
