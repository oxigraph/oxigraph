use crate::model::{BlankNode, Literal, NamedNode, Term, Triple};
use crate::sparql::Variable;
use crate::storage::numeric_encoder::EncodedTerm;
use json_event_parser::{JsonEvent, JsonWriter};
use regex::Regex;
use spargebra::algebra::GraphPattern;
use spargebra::term::GroundTerm;
use std::cell::Cell;
use std::cmp::max;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::Duration;
use std::{fmt, io};

#[derive(Debug, Clone)]
pub enum PlanNode {
    StaticBindings {
        encoded_tuples: Vec<EncodedTuple>,
        variables: Vec<PlanVariable>,
        plain_bindings: Vec<Vec<Option<GroundTerm>>>,
    },
    Service {
        service_name: PatternValue,
        variables: Rc<[Variable]>,
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
    /// Streams left and materializes right join
    HashJoin {
        left: Rc<Self>,
        right: Rc<Self>,
    },
    /// Right nested in left loop
    ForLoopJoin {
        left: Rc<Self>,
        right: Rc<Self>,
    },
    /// Streams left and materializes right anti join
    AntiJoin {
        left: Rc<Self>,
        right: Rc<Self>,
    },
    Filter {
        child: Rc<Self>,
        expression: Box<PlanExpression>,
    },
    Union {
        children: Vec<Rc<Self>>,
    },
    /// hash left join
    HashLeftJoin {
        left: Rc<Self>,
        right: Rc<Self>,
        expression: Box<PlanExpression>,
    },
    /// right nested in left loop
    ForLoopLeftJoin {
        left: Rc<Self>,
        right: Rc<Self>,
        possible_problem_vars: Rc<[usize]>, //Variables that should not be part of the entry of the left join
    },
    Extend {
        child: Rc<Self>,
        variable: PlanVariable,
        expression: Box<PlanExpression>,
    },
    Sort {
        child: Rc<Self>,
        by: Vec<Comparator>,
    },
    HashDeduplicate {
        child: Rc<Self>,
    },
    /// Removes duplicated consecutive elements
    Reduced {
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
        mapping: Rc<[(PlanVariable, PlanVariable)]>, // pairs of (variable key in child, variable key in output)
    },
    Aggregate {
        // By definition the group by key are the range 0..key_mapping.len()
        child: Rc<Self>,
        key_variables: Rc<[PlanVariable]>,
        aggregates: Rc<[(PlanAggregation, PlanVariable)]>,
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
            Self::StaticBindings { encoded_tuples, .. } => {
                for tuple in encoded_tuples {
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
                subject.lookup_variables(callback);
                predicate.lookup_variables(callback);
                object.lookup_variables(callback);
                graph_name.lookup_variables(callback);
            }
            Self::PathPattern {
                subject,
                object,
                graph_name,
                ..
            } => {
                subject.lookup_variables(callback);
                object.lookup_variables(callback);
                graph_name.lookup_variables(callback);
            }
            Self::Filter { child, expression } => {
                expression.lookup_used_variables(callback);
                child.lookup_used_variables(callback);
            }
            Self::Union { children } => {
                for child in children {
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
                variable,
                expression,
            } => {
                callback(variable.encoded);
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
                service_name.lookup_variables(callback);
                child.lookup_used_variables(callback);
            }
            Self::Project { mapping, child } => {
                let child_bound = child.used_variables();
                for (child_i, output_i) in mapping.iter() {
                    if child_bound.contains(&child_i.encoded) {
                        callback(output_i.encoded);
                    }
                }
            }
            Self::Aggregate {
                key_variables,
                aggregates,
                ..
            } => {
                for var in key_variables.iter() {
                    callback(var.encoded);
                }
                for (_, var) in aggregates.iter() {
                    callback(var.encoded);
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
            Self::StaticBindings { encoded_tuples, .. } => {
                let mut variables = BTreeMap::default(); // value true iff always bound
                let max_tuple_length = encoded_tuples
                    .iter()
                    .map(EncodedTuple::capacity)
                    .fold(0, max);
                for tuple in encoded_tuples {
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
                subject.lookup_variables(callback);
                predicate.lookup_variables(callback);
                object.lookup_variables(callback);
                graph_name.lookup_variables(callback);
            }
            Self::PathPattern {
                subject,
                object,
                graph_name,
                ..
            } => {
                subject.lookup_variables(callback);
                object.lookup_variables(callback);
                graph_name.lookup_variables(callback);
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
                variable,
                expression,
            } => {
                if matches!(
                    expression.as_ref(),
                    PlanExpression::NamedNode(_) | PlanExpression::Literal(_)
                ) {
                    // TODO: more cases?
                    callback(variable.encoded);
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
                    if child_bound.contains(&child_i.encoded) {
                        callback(output_i.encoded);
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
pub struct PlanTerm<T> {
    pub encoded: EncodedTerm,
    pub plain: T,
}

impl<T: fmt::Display> fmt::Display for PlanTerm<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.plain)
    }
}

#[derive(Debug, Clone)]
pub enum PatternValue {
    Constant(PlanTerm<PatternValueConstant>),
    Variable(PlanVariable),
    TriplePattern(Box<TriplePatternValue>),
}

impl PatternValue {
    pub fn lookup_variables(&self, callback: &mut impl FnMut(usize)) {
        if let Self::Variable(v) = self {
            callback(v.encoded)
        } else if let Self::TriplePattern(p) = self {
            p.subject.lookup_variables(callback);
            p.predicate.lookup_variables(callback);
            p.object.lookup_variables(callback);
        }
    }
}

impl fmt::Display for PatternValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constant(c) => write!(f, "{c}"),
            Self::Variable(v) => write!(f, "{v}"),
            Self::TriplePattern(p) => write!(f, "{p}"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PatternValueConstant {
    NamedNode(NamedNode),
    Literal(Literal),
    Triple(Box<Triple>),
    DefaultGraph,
}

impl fmt::Display for PatternValueConstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(n) => write!(f, "{n}"),
            Self::Literal(l) => write!(f, "{l}"),
            Self::Triple(t) => write!(f, "<< {t} >>"),
            Self::DefaultGraph => f.write_str("DEFAULT"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TriplePatternValue {
    pub subject: PatternValue,
    pub predicate: PatternValue,
    pub object: PatternValue,
}

impl fmt::Display for TriplePatternValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}

#[derive(Debug, Clone)]
pub struct PlanVariable<P = Variable> {
    pub encoded: usize,
    pub plain: P,
}

impl<P: fmt::Display> fmt::Display for PlanVariable<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.plain)
    }
}

#[derive(Debug, Clone)]
pub enum PlanExpression {
    NamedNode(PlanTerm<NamedNode>),
    Literal(PlanTerm<Literal>),
    Variable(PlanVariable),
    Exists(Rc<PlanNode>),
    Or(Vec<Self>),
    And(Vec<Self>),
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
    Bound(PlanVariable),
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
                callback(v.encoded);
            }
            Self::NamedNode(_)
            | Self::Literal(_)
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
            Self::Equal(a, b)
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
            Self::Or(es)
            | Self::And(es)
            | Self::Concat(es)
            | Self::Coalesce(es)
            | Self::CustomFunction(_, es) => {
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

impl fmt::Display for PlanExpression {
    #[allow(clippy::many_single_char_names)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Variable(v) => {
                write!(f, "{v}")
            }
            Self::Bound(v) => {
                write!(f, "Bound({v})")
            }
            Self::NamedNode(n) => write!(f, "{n}"),
            Self::Literal(l) => write!(f, "{l}"),
            Self::Rand => write!(f, "Rand()"),
            Self::Now => write!(f, "Now()"),
            Self::Uuid => write!(f, "Uuid()"),
            Self::StrUuid => write!(f, "StrUuid()"),
            Self::UnaryPlus(e) => write!(f, "UnaryPlus({e})"),
            Self::UnaryMinus(e) => write!(f, "UnaryMinus({e})"),
            Self::Not(e) => write!(f, "Not({e})"),
            Self::BNode(e) => {
                if let Some(e) = e {
                    write!(f, "BNode({e})")
                } else {
                    write!(f, "BNode()")
                }
            }
            Self::Str(e) => write!(f, "Str({e})"),
            Self::Lang(e) => write!(f, "Lang({e})"),
            Self::Datatype(e) => write!(f, "Datatype({e})"),
            Self::Iri(e) => write!(f, "Iri({e})"),
            Self::Abs(e) => write!(f, "Abs({e})"),
            Self::Ceil(e) => write!(f, "Ceil({e})"),
            Self::Floor(e) => write!(f, "Floor({e})"),
            Self::Round(e) => write!(f, "Round({e})"),
            Self::UCase(e) => write!(f, "UCase({e})"),
            Self::LCase(e) => write!(f, "LCase({e})"),
            Self::StrLen(e) => write!(f, "StrLen({e})"),
            Self::EncodeForUri(e) => write!(f, "EncodeForUri({e})"),
            Self::StaticRegex(e, r) => write!(f, "StaticRegex({e}, {r})"),
            Self::Year(e) => write!(f, "Year({e})"),
            Self::Month(e) => write!(f, "Month({e})"),
            Self::Day(e) => write!(f, "Day({e})"),
            Self::Hours(e) => write!(f, "Hours({e})"),
            Self::Minutes(e) => write!(f, "Minutes({e})"),
            Self::Seconds(e) => write!(f, "Seconds({e})"),
            Self::Timezone(e) => write!(f, "Timezone({e})"),
            Self::Tz(e) => write!(f, "Tz({e})"),
            Self::Md5(e) => write!(f, "Md5({e})"),
            Self::Sha1(e) => write!(f, "Sha1({e})"),
            Self::Sha256(e) => write!(f, "Sha256({e})"),
            Self::Sha384(e) => write!(f, "Sha384({e})"),
            Self::Sha512(e) => write!(f, "Sha512({e})"),
            Self::IsIri(e) => write!(f, "IsIri({e})"),
            Self::IsBlank(e) => write!(f, "IsBlank({e})"),
            Self::IsLiteral(e) => write!(f, "IsLiteral({e})"),
            Self::IsNumeric(e) => write!(f, "IsNumeric({e})"),
            Self::IsTriple(e) => write!(f, "IsTriple({e})"),
            Self::Subject(e) => write!(f, "Subject({e})"),
            Self::Predicate(e) => write!(f, "Predicate({e})"),
            Self::Object(e) => write!(f, "Object({e})"),
            Self::BooleanCast(e) => write!(f, "BooleanCast({e})"),
            Self::DoubleCast(e) => write!(f, "DoubleCast({e})"),
            Self::FloatCast(e) => write!(f, "FloatCast({e})"),
            Self::DecimalCast(e) => write!(f, "DecimalCast({e})"),
            Self::IntegerCast(e) => write!(f, "IntegerCast({e})"),
            Self::DateCast(e) => write!(f, "DateCast({e})"),
            Self::TimeCast(e) => write!(f, "TimeCast({e})"),
            Self::DateTimeCast(e) => write!(f, "DateTimeCast({e})"),
            Self::DurationCast(e) => write!(f, "DurationCast({e})"),
            Self::YearMonthDurationCast(e) => write!(f, "YearMonthDurationCast({e})"),
            Self::DayTimeDurationCast(e) => write!(f, "DayTimeDurationCast({e})"),
            Self::StringCast(e) => write!(f, "StringCast({e})"),
            Self::Or(es) => {
                write!(f, "Or(")?;
                for (i, e) in es.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, ")")
            }
            Self::And(es) => {
                write!(f, "And(")?;
                for (i, e) in es.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, ")")
            }
            Self::Equal(a, b) => write!(f, "Equal({a}, {b})"),
            Self::Greater(a, b) => write!(f, "Greater({a}, {b})"),
            Self::GreaterOrEqual(a, b) => write!(f, "GreaterOrEqual({a}, {b})"),
            Self::Less(a, b) => write!(f, "Less({a}, {b})"),
            Self::LessOrEqual(a, b) => write!(f, "LessOrEqual({a}, {b})"),
            Self::Add(a, b) => write!(f, "Add({a}, {b})"),
            Self::Subtract(a, b) => write!(f, "Subtract({a}, {b})"),
            Self::Multiply(a, b) => write!(f, "Multiply({a}, {b})"),
            Self::Divide(a, b) => write!(f, "Divide({a}, {b})"),
            Self::LangMatches(a, b) => write!(f, "LangMatches({a}, {b})"),
            Self::Contains(a, b) => write!(f, "Contains({a}, {b})"),
            Self::StaticReplace(a, b, c) => write!(f, "StaticReplace({a}, {b}, {c})"),
            Self::StrStarts(a, b) => write!(f, "StrStarts({a}, {b})"),
            Self::StrEnds(a, b) => write!(f, "StrEnds({a}, {b})"),
            Self::StrBefore(a, b) => write!(f, "StrBefore({a}, {b})"),
            Self::StrAfter(a, b) => write!(f, "StrAfter({a}, {b})"),
            Self::StrLang(a, b) => write!(f, "StrLang({a}, {b})"),
            Self::StrDt(a, b) => write!(f, "StrDt({a}, {b})"),
            Self::SameTerm(a, b) => write!(f, "SameTerm({a}, {b})"),
            Self::SubStr(a, b, None) => write!(f, "SubStr({a}, {b})"),
            Self::DynamicRegex(a, b, None) => write!(f, "DynamicRegex({a}, {b})"),
            Self::Adjust(a, b) => write!(f, "Adjust({a}, {b})"),
            Self::If(a, b, c) => write!(f, "If({a}, {b}, {c})"),
            Self::SubStr(a, b, Some(c)) => write!(f, "SubStr({a}, {b}, {c})"),
            Self::DynamicRegex(a, b, Some(c)) => write!(f, "DynamicRegex({a}, {b}, {c})"),
            Self::DynamicReplace(a, b, c, None) => write!(f, "DynamicReplace({a}, {b}, {c})"),
            Self::Triple(a, b, c) => write!(f, "Triple({a}, {b}, {c})"),
            Self::DynamicReplace(a, b, c, Some(d)) => {
                write!(f, "DynamicReplace({a}, {b}, {c}, {d})")
            }
            Self::Concat(es) => {
                write!(f, "Concat(")?;
                for (i, e) in es.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, ")")
            }
            Self::Coalesce(es) => {
                write!(f, "Coalesce(")?;
                for (i, e) in es.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, ")")
            }
            Self::CustomFunction(name, es) => {
                write!(f, "{name}(")?;
                for (i, e) in es.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, ")")
            }
            Self::Exists(_) => write!(f, "Exists()"), //TODO
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanAggregation {
    pub function: PlanAggregationFunction,
    pub parameter: Option<PlanExpression>,
    pub distinct: bool,
}

impl fmt::Display for PlanAggregation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.function {
            PlanAggregationFunction::Count => {
                write!(f, "Count")
            }
            PlanAggregationFunction::Sum => {
                write!(f, "Sum")
            }
            PlanAggregationFunction::Min => {
                write!(f, "Min")
            }
            PlanAggregationFunction::Max => {
                write!(f, "Max")
            }
            PlanAggregationFunction::Avg => {
                write!(f, "Avg")
            }
            PlanAggregationFunction::GroupConcat { .. } => {
                write!(f, "GroupConcat")
            }
            PlanAggregationFunction::Sample => write!(f, "Sample"),
        }?;
        if self.distinct {
            write!(f, "Distinct")?;
        }
        write!(f, "(")?;
        if let Some(expr) = &self.parameter {
            write!(f, "{expr}")?;
        }
        if let PlanAggregationFunction::GroupConcat { separator } = &self.function {
            write!(f, "; separator={separator}")?;
        }
        write!(f, ")")
    }
}

#[derive(Debug, Clone)]
pub enum PlanAggregationFunction {
    Count,
    Sum,
    Min,
    Max,
    Avg,
    Sample,
    GroupConcat { separator: Rc<str> },
}

#[derive(Debug, Clone)]
pub enum PlanPropertyPath {
    Path(PlanTerm<NamedNode>),
    Reverse(Rc<Self>),
    Sequence(Rc<Self>, Rc<Self>),
    Alternative(Rc<Self>, Rc<Self>),
    ZeroOrMore(Rc<Self>),
    OneOrMore(Rc<Self>),
    ZeroOrOne(Rc<Self>),
    NegatedPropertySet(Rc<[PlanTerm<NamedNode>]>),
}

impl fmt::Display for PlanPropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(p) => write!(f, "{p}"),
            Self::Reverse(p) => write!(f, "Reverse({p})"),
            Self::Sequence(a, b) => write!(f, "Sequence{a}, {b}"),
            Self::Alternative(a, b) => write!(f, "Alternative{a}, {b}"),
            Self::ZeroOrMore(p) => write!(f, "ZeroOrMore({p})"),
            Self::OneOrMore(p) => write!(f, "OneOrMore({p})"),
            Self::ZeroOrOne(p) => write!(f, "ZeroOrOne({p})"),
            Self::NegatedPropertySet(ps) => {
                write!(f, "NegatedPropertySet(")?;
                for (i, p) in ps.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ")")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Comparator {
    Asc(PlanExpression),
    Desc(PlanExpression),
}

impl fmt::Display for Comparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asc(c) => write!(f, "Asc({c})"),
            Self::Desc(c) => write!(f, "Desc({c})"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TripleTemplate {
    pub subject: TripleTemplateValue,
    pub predicate: TripleTemplateValue,
    pub object: TripleTemplateValue,
}

impl fmt::Display for TripleTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}

#[derive(Debug, Clone)]
pub enum TripleTemplateValue {
    Constant(PlanTerm<Term>),
    BlankNode(PlanVariable<BlankNode>),
    Variable(PlanVariable),
    Triple(Box<TripleTemplate>),
}

impl fmt::Display for TripleTemplateValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constant(c) => write!(f, "{c}"),
            Self::BlankNode(bn) => write!(f, "{bn}"),
            Self::Variable(v) => write!(f, "{v}"),
            Self::Triple(t) => write!(f, "<< {t} >>"),
        }
    }
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

pub struct PlanNodeWithStats {
    pub node: Rc<PlanNode>,
    pub children: Vec<Rc<PlanNodeWithStats>>,
    pub exec_count: Cell<usize>,
    pub exec_duration: Cell<Duration>,
}

impl PlanNodeWithStats {
    pub fn json_node(
        &self,
        writer: &mut JsonWriter<impl io::Write>,
        with_stats: bool,
    ) -> io::Result<()> {
        writer.write_event(JsonEvent::StartObject)?;
        writer.write_event(JsonEvent::ObjectKey("name"))?;
        writer.write_event(JsonEvent::String(&self.node_label()))?;
        if with_stats {
            writer.write_event(JsonEvent::ObjectKey("number of results"))?;
            writer.write_event(JsonEvent::Number(&self.exec_count.get().to_string()))?;
            writer.write_event(JsonEvent::ObjectKey("duration in seconds"))?;
            writer.write_event(JsonEvent::Number(
                &self.exec_duration.get().as_secs_f32().to_string(),
            ))?;
        }
        writer.write_event(JsonEvent::ObjectKey("children"))?;
        writer.write_event(JsonEvent::StartArray)?;
        for child in &self.children {
            child.json_node(writer, with_stats)?;
        }
        writer.write_event(JsonEvent::EndArray)?;
        writer.write_event(JsonEvent::EndObject)
    }

    fn node_label(&self) -> String {
        match self.node.as_ref() {
            PlanNode::Aggregate {
                key_variables,
                aggregates,
                ..
            } => format!(
                "Aggregate({})",
                key_variables
                    .iter()
                    .map(ToString::to_string)
                    .chain(aggregates.iter().map(|(agg, v)| format!("{agg} -> {v}")))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PlanNode::AntiJoin { .. } => "AntiJoin".to_owned(),
            PlanNode::Extend {
                expression,
                variable,
                ..
            } => format!("Extend({expression} -> {variable})"),
            PlanNode::Filter { expression, .. } => format!("Filter({expression})"),
            PlanNode::ForLoopJoin { .. } => "ForLoopJoin".to_owned(),
            PlanNode::ForLoopLeftJoin { .. } => "ForLoopLeftJoin".to_owned(),
            PlanNode::HashDeduplicate { .. } => "HashDeduplicate".to_owned(),
            PlanNode::HashJoin { .. } => "HashJoin".to_owned(),
            PlanNode::HashLeftJoin { expression, .. } => format!("HashLeftJoin({expression})"),
            PlanNode::Limit { count, .. } => format!("Limit({count})"),
            PlanNode::PathPattern {
                subject,
                path,
                object,
                graph_name,
            } => format!("PathPattern({subject} {path} {object} {graph_name})"),
            PlanNode::Project { mapping, .. } => {
                format!(
                    "Project({})",
                    mapping
                        .iter()
                        .map(|(f, t)| if f.plain == t.plain {
                            f.to_string()
                        } else {
                            format!("{f} -> {t}")
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            PlanNode::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => format!("QuadPattern({subject} {predicate} {object} {graph_name})"),
            PlanNode::Reduced { .. } => "Reduced".to_owned(),
            PlanNode::Service {
                service_name,
                silent,
                ..
            } => {
                if *silent {
                    format!("SilentService({service_name})")
                } else {
                    format!("Service({service_name})")
                }
            }
            PlanNode::Skip { count, .. } => format!("Skip({count})"),
            PlanNode::Sort { by, .. } => {
                format!(
                    "Sort({})",
                    by.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            PlanNode::StaticBindings { variables, .. } => {
                format!(
                    "StaticBindings({})",
                    variables
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            PlanNode::Union { .. } => "Union".to_owned(),
        }
    }
}

impl fmt::Debug for PlanNodeWithStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut obj = f.debug_struct("Node");
        obj.field("name", &self.node_label());
        if self.exec_duration.get() > Duration::default() {
            obj.field("number of results", &self.exec_count.get());
            obj.field("duration in seconds", &self.exec_duration.get());
        }
        if !self.children.is_empty() {
            obj.field("children", &self.children);
        }
        obj.finish()
    }
}
