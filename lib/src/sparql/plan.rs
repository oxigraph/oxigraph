use crate::sparql::algebra::GraphPattern;
use crate::sparql::model::Variable;
use crate::store::numeric_encoder::{EncodedTerm, StrId};
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanNode<I: StrId> {
    Init,
    StaticBindings {
        tuples: Vec<EncodedTuple<I>>,
    },
    Service {
        service_name: PatternValue<I>,
        variables: Rc<Vec<Variable>>,
        child: Rc<PlanNode<I>>,
        graph_pattern: Rc<GraphPattern>,
        silent: bool,
    },
    QuadPatternJoin {
        child: Rc<PlanNode<I>>,
        subject: PatternValue<I>,
        predicate: PatternValue<I>,
        object: PatternValue<I>,
        graph_name: PatternValue<I>,
    },
    PathPatternJoin {
        child: Rc<PlanNode<I>>,
        subject: PatternValue<I>,
        path: Rc<PlanPropertyPath<I>>,
        object: PatternValue<I>,
        graph_name: PatternValue<I>,
    },
    Join {
        left: Rc<PlanNode<I>>,
        right: Rc<PlanNode<I>>,
    },
    AntiJoin {
        left: Rc<PlanNode<I>>,
        right: Rc<PlanNode<I>>,
    },
    Filter {
        child: Rc<PlanNode<I>>,
        expression: Rc<PlanExpression<I>>,
    },
    Union {
        children: Vec<Rc<PlanNode<I>>>,
    },
    LeftJoin {
        left: Rc<PlanNode<I>>,
        right: Rc<PlanNode<I>>,
        possible_problem_vars: Rc<Vec<usize>>, //Variables that should not be part of the entry of the left join
    },
    Extend {
        child: Rc<PlanNode<I>>,
        position: usize,
        expression: Rc<PlanExpression<I>>,
    },
    Sort {
        child: Rc<PlanNode<I>>,
        by: Vec<Comparator<I>>,
    },
    HashDeduplicate {
        child: Rc<PlanNode<I>>,
    },
    Skip {
        child: Rc<PlanNode<I>>,
        count: usize,
    },
    Limit {
        child: Rc<PlanNode<I>>,
        count: usize,
    },
    Project {
        child: Rc<PlanNode<I>>,
        mapping: Rc<Vec<(usize, usize)>>, // pairs of (variable key in child, variable key in output)
    },
    Aggregate {
        // By definition the group by key are the range 0..key_mapping.len()
        child: Rc<PlanNode<I>>,
        key_mapping: Rc<Vec<(usize, usize)>>, // aggregate key pairs of (variable key in child, variable key in output)
        aggregates: Rc<Vec<(PlanAggregation<I>, usize)>>,
    },
}

impl<I: StrId> PlanNode<I> {
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

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum PatternValue<I: StrId> {
    Constant(EncodedTerm<I>),
    Variable(usize),
}

impl<I: StrId> PatternValue<I> {
    pub fn is_var(&self) -> bool {
        match self {
            PatternValue::Constant(_) => false,
            PatternValue::Variable(_) => true,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanExpression<I: StrId> {
    Constant(EncodedTerm<I>),
    Variable(usize),
    Exists(Rc<PlanNode<I>>),
    Or(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    And(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    Equal(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    Greater(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    GreaterOrEqual(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    Less(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    LessOrEqual(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    In(Box<PlanExpression<I>>, Vec<PlanExpression<I>>),
    Add(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    Subtract(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    Multiply(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    Divide(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    UnaryPlus(Box<PlanExpression<I>>),
    UnaryMinus(Box<PlanExpression<I>>),
    Not(Box<PlanExpression<I>>),
    Str(Box<PlanExpression<I>>),
    Lang(Box<PlanExpression<I>>),
    LangMatches(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    Datatype(Box<PlanExpression<I>>),
    Bound(usize),
    Iri(Box<PlanExpression<I>>),
    BNode(Option<Box<PlanExpression<I>>>),
    Rand,
    Abs(Box<PlanExpression<I>>),
    Ceil(Box<PlanExpression<I>>),
    Floor(Box<PlanExpression<I>>),
    Round(Box<PlanExpression<I>>),
    Concat(Vec<PlanExpression<I>>),
    SubStr(
        Box<PlanExpression<I>>,
        Box<PlanExpression<I>>,
        Option<Box<PlanExpression<I>>>,
    ),
    StrLen(Box<PlanExpression<I>>),
    Replace(
        Box<PlanExpression<I>>,
        Box<PlanExpression<I>>,
        Box<PlanExpression<I>>,
        Option<Box<PlanExpression<I>>>,
    ),
    UCase(Box<PlanExpression<I>>),
    LCase(Box<PlanExpression<I>>),
    EncodeForUri(Box<PlanExpression<I>>),
    Contains(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    StrStarts(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    StrEnds(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    StrBefore(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    StrAfter(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    Year(Box<PlanExpression<I>>),
    Month(Box<PlanExpression<I>>),
    Day(Box<PlanExpression<I>>),
    Hours(Box<PlanExpression<I>>),
    Minutes(Box<PlanExpression<I>>),
    Seconds(Box<PlanExpression<I>>),
    Timezone(Box<PlanExpression<I>>),
    Tz(Box<PlanExpression<I>>),
    Now,
    Uuid,
    StrUuid,
    Md5(Box<PlanExpression<I>>),
    Sha1(Box<PlanExpression<I>>),
    Sha256(Box<PlanExpression<I>>),
    Sha384(Box<PlanExpression<I>>),
    Sha512(Box<PlanExpression<I>>),
    Coalesce(Vec<PlanExpression<I>>),
    If(
        Box<PlanExpression<I>>,
        Box<PlanExpression<I>>,
        Box<PlanExpression<I>>,
    ),
    StrLang(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    StrDt(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    SameTerm(Box<PlanExpression<I>>, Box<PlanExpression<I>>),
    IsIri(Box<PlanExpression<I>>),
    IsBlank(Box<PlanExpression<I>>),
    IsLiteral(Box<PlanExpression<I>>),
    IsNumeric(Box<PlanExpression<I>>),
    Regex(
        Box<PlanExpression<I>>,
        Box<PlanExpression<I>>,
        Option<Box<PlanExpression<I>>>,
    ),
    BooleanCast(Box<PlanExpression<I>>),
    DoubleCast(Box<PlanExpression<I>>),
    FloatCast(Box<PlanExpression<I>>),
    DecimalCast(Box<PlanExpression<I>>),
    IntegerCast(Box<PlanExpression<I>>),
    DateCast(Box<PlanExpression<I>>),
    TimeCast(Box<PlanExpression<I>>),
    DateTimeCast(Box<PlanExpression<I>>),
    DurationCast(Box<PlanExpression<I>>),
    YearMonthDurationCast(Box<PlanExpression<I>>),
    DayTimeDurationCast(Box<PlanExpression<I>>),
    StringCast(Box<PlanExpression<I>>),
}

impl<I: StrId> PlanExpression<I> {
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
            | PlanExpression::Replace(a, b, c, None) => {
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
pub struct PlanAggregation<I: StrId> {
    pub function: PlanAggregationFunction,
    pub parameter: Option<PlanExpression<I>>,
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
pub enum PlanPropertyPath<I: StrId> {
    Path(EncodedTerm<I>),
    Reverse(Rc<PlanPropertyPath<I>>),
    Sequence(Rc<PlanPropertyPath<I>>, Rc<PlanPropertyPath<I>>),
    Alternative(Rc<PlanPropertyPath<I>>, Rc<PlanPropertyPath<I>>),
    ZeroOrMore(Rc<PlanPropertyPath<I>>),
    OneOrMore(Rc<PlanPropertyPath<I>>),
    ZeroOrOne(Rc<PlanPropertyPath<I>>),
    NegatedPropertySet(Rc<Vec<EncodedTerm<I>>>),
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Comparator<I: StrId> {
    Asc(PlanExpression<I>),
    Desc(PlanExpression<I>),
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct TripleTemplate<I: StrId> {
    pub subject: TripleTemplateValue<I>,
    pub predicate: TripleTemplateValue<I>,
    pub object: TripleTemplateValue<I>,
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum TripleTemplateValue<I: StrId> {
    Constant(EncodedTerm<I>),
    BlankNode(usize),
    Variable(usize),
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct EncodedTuple<I: StrId> {
    inner: Vec<Option<EncodedTerm<I>>>,
}

impl<I: StrId> EncodedTuple<I> {
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

    pub fn get(&self, index: usize) -> Option<EncodedTerm<I>> {
        self.inner.get(index).cloned().unwrap_or(None)
    }

    pub fn iter(&self) -> impl Iterator<Item = Option<EncodedTerm<I>>> + '_ {
        self.inner.iter().cloned()
    }

    pub fn set(&mut self, index: usize, value: EncodedTerm<I>) {
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

    pub fn combine_with(&self, other: &EncodedTuple<I>) -> Option<Self> {
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

impl<I: StrId> IntoIterator for EncodedTuple<I> {
    type Item = Option<EncodedTerm<I>>;
    type IntoIter = std::vec::IntoIter<Option<EncodedTerm<I>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}
