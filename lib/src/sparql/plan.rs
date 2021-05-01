use crate::sparql::model::Variable;
use crate::storage::numeric_encoder::EncodedTerm;
use spargebra::algebra::GraphPattern;
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanNode {
    Init,
    StaticBindings {
        tuples: Vec<EncodedTuple>,
    },
    Service {
        service_name: PatternValue,
        variables: Rc<Vec<Variable>>,
        child: Rc<PlanNode>,
        graph_pattern: Rc<GraphPattern>,
        silent: bool,
    },
    QuadPatternJoin {
        child: Rc<PlanNode>,
        subject: PatternValue,
        predicate: PatternValue,
        object: PatternValue,
        graph_name: PatternValue,
    },
    PathPatternJoin {
        child: Rc<PlanNode>,
        subject: PatternValue,
        path: Rc<PlanPropertyPath>,
        object: PatternValue,
        graph_name: PatternValue,
    },
    Join {
        left: Rc<PlanNode>,
        right: Rc<PlanNode>,
    },
    AntiJoin {
        left: Rc<PlanNode>,
        right: Rc<PlanNode>,
    },
    Filter {
        child: Rc<PlanNode>,
        expression: Rc<PlanExpression>,
    },
    Union {
        children: Vec<Rc<PlanNode>>,
    },
    LeftJoin {
        left: Rc<PlanNode>,
        right: Rc<PlanNode>,
        possible_problem_vars: Rc<Vec<usize>>, //Variables that should not be part of the entry of the left join
    },
    Extend {
        child: Rc<PlanNode>,
        position: usize,
        expression: Rc<PlanExpression>,
    },
    Sort {
        child: Rc<PlanNode>,
        by: Vec<Comparator>,
    },
    HashDeduplicate {
        child: Rc<PlanNode>,
    },
    Skip {
        child: Rc<PlanNode>,
        count: usize,
    },
    Limit {
        child: Rc<PlanNode>,
        count: usize,
    },
    Project {
        child: Rc<PlanNode>,
        mapping: Rc<Vec<(usize, usize)>>, // pairs of (variable key in child, variable key in output)
    },
    Aggregate {
        // By definition the group by key are the range 0..key_mapping.len()
        child: Rc<PlanNode>,
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
            PlanNode::Filter { child, expression } => {
                expression.add_maybe_bound_variables(set);
                child.add_maybe_bound_variables(set);
            }
            PlanNode::Union { children } => {
                for child in children.iter() {
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
    Or(Box<PlanExpression>, Box<PlanExpression>),
    And(Box<PlanExpression>, Box<PlanExpression>),
    Equal(Box<PlanExpression>, Box<PlanExpression>),
    Greater(Box<PlanExpression>, Box<PlanExpression>),
    GreaterOrEqual(Box<PlanExpression>, Box<PlanExpression>),
    Less(Box<PlanExpression>, Box<PlanExpression>),
    LessOrEqual(Box<PlanExpression>, Box<PlanExpression>),
    In(Box<PlanExpression>, Vec<PlanExpression>),
    Add(Box<PlanExpression>, Box<PlanExpression>),
    Subtract(Box<PlanExpression>, Box<PlanExpression>),
    Multiply(Box<PlanExpression>, Box<PlanExpression>),
    Divide(Box<PlanExpression>, Box<PlanExpression>),
    UnaryPlus(Box<PlanExpression>),
    UnaryMinus(Box<PlanExpression>),
    Not(Box<PlanExpression>),
    Str(Box<PlanExpression>),
    Lang(Box<PlanExpression>),
    LangMatches(Box<PlanExpression>, Box<PlanExpression>),
    Datatype(Box<PlanExpression>),
    Bound(usize),
    Iri(Box<PlanExpression>),
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
    EncodeForUri(Box<PlanExpression>),
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
    Uuid,
    StrUuid,
    Md5(Box<PlanExpression>),
    Sha1(Box<PlanExpression>),
    Sha256(Box<PlanExpression>),
    Sha384(Box<PlanExpression>),
    Sha512(Box<PlanExpression>),
    Coalesce(Vec<PlanExpression>),
    If(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Box<PlanExpression>,
    ),
    StrLang(Box<PlanExpression>, Box<PlanExpression>),
    StrDt(Box<PlanExpression>, Box<PlanExpression>),
    SameTerm(Box<PlanExpression>, Box<PlanExpression>),
    IsIri(Box<PlanExpression>),
    IsBlank(Box<PlanExpression>),
    IsLiteral(Box<PlanExpression>),
    IsNumeric(Box<PlanExpression>),
    Regex(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Option<Box<PlanExpression>>,
    ),
    Triple(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Box<PlanExpression>,
    ),
    Subject(Box<PlanExpression>),
    Predicate(Box<PlanExpression>),
    Object(Box<PlanExpression>),
    IsTriple(Box<PlanExpression>),
    BooleanCast(Box<PlanExpression>),
    DoubleCast(Box<PlanExpression>),
    FloatCast(Box<PlanExpression>),
    DecimalCast(Box<PlanExpression>),
    IntegerCast(Box<PlanExpression>),
    DateCast(Box<PlanExpression>),
    TimeCast(Box<PlanExpression>),
    DateTimeCast(Box<PlanExpression>),
    DurationCast(Box<PlanExpression>),
    YearMonthDurationCast(Box<PlanExpression>),
    DayTimeDurationCast(Box<PlanExpression>),
    StringCast(Box<PlanExpression>),
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
    Reverse(Rc<PlanPropertyPath>),
    Sequence(Rc<PlanPropertyPath>, Rc<PlanPropertyPath>),
    Alternative(Rc<PlanPropertyPath>, Rc<PlanPropertyPath>),
    ZeroOrMore(Rc<PlanPropertyPath>),
    OneOrMore(Rc<PlanPropertyPath>),
    ZeroOrOne(Rc<PlanPropertyPath>),
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
                        None => result[key] = Some(self_value.clone()),
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
                        None => result[key] = Some(other_value.clone()),
                    }
                }
            }
            Some(EncodedTuple { inner: result })
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
