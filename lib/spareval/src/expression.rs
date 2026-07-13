#[cfg(feature = "sparql-12")]
use crate::ExpressionTriple;
use crate::dataset::ExpressionTerm;
use md5::{Digest, Md5};
use oxiri::Iri;
#[cfg(feature = "sparql-12")]
use oxrdf::BaseDirection;
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{BlankNode, Literal, NamedNode, OxString, Term, Variable};
#[cfg(feature = "sep-0002")]
use oxsdatatypes::{Date, DayTimeDuration, Duration, Time, TimezoneOffset, YearMonthDuration};
use oxsdatatypes::{DateTime, Decimal, Double, Float, Integer};
use rand::random;
use regex::{Regex, RegexBuilder};
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};
use spargebra::algebra::Function;
use sparopt::algebra::{Expression, GraphPattern};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::sync::Arc;
use thiserror::Error;

pub type CustomFunctionRegistry =
    HashMap<NamedNode, Arc<dyn (Fn(&[Term]) -> Option<Term>) + Send + Sync>>;

const REGEX_SIZE_LIMIT: usize = 1_000_000;

pub trait ExpressionEvaluatorContext<'a> {
    type Term: Clone + Eq + 'a;
    type Tuple: 'a;
    type Error;

    fn build_variable_lookup(
        &mut self,
        variable: &Variable,
    ) -> impl Fn(&Self::Tuple) -> Option<Self::Term> + 'a;
    fn build_is_variable_bound(
        &mut self,
        variable: &Variable,
    ) -> impl Fn(&Self::Tuple) -> bool + 'a;
    fn build_exists(
        &mut self,
        plan: &GraphPattern,
    ) -> Result<impl Fn(&Self::Tuple) -> bool + 'a, Self::Error>;
    fn internalize_named_node(&mut self, term: &NamedNode) -> Result<Self::Term, Self::Error>;
    fn internalize_literal(&mut self, term: &Literal) -> Result<Self::Term, Self::Error>;
    fn build_internalize_expression_term(
        &mut self,
    ) -> impl Fn(ExpressionTerm) -> Result<Self::Term, Self::Error> + 'a;
    fn build_externalize_expression_term(
        &mut self,
    ) -> impl Fn(Self::Term) -> Result<ExpressionTerm, Self::Error> + 'a;
    fn build_externalize_term(&mut self) -> impl Fn(Self::Term) -> Result<Term, Self::Error> + 'a;
    fn now(&mut self) -> DateTime;
    fn base_iri(&mut self) -> Option<Iri<OxString>>;
    fn custom_functions(&mut self) -> &CustomFunctionRegistry;
}

pub type ExpressionEvaluator<'a, I, O, E> = Rc<dyn (Fn(&I) -> Result<Option<O>, E>) + 'a>;

macro_rules! try_or_ok {
    ($value:expr) => {
        if let Some(value) = $value {
            value
        } else {
            return Ok(None);
        }
    };
}

#[derive(Debug, Error)]
pub enum ExpressionEvaluationError<C> {
    /// Error from the evaluation context
    #[error(transparent)]
    Context(C),
    /// The given custom function is not supported
    #[error("The custom function {0} is not supported")]
    UnsupportedCustomFunction(NamedNode),
    /// The given custom function arity is not supported
    #[error("The custom function {name} requires between {} and {} arguments, but {actual} were given", .expected.start(), .expected.end())]
    UnsupportedCustomFunctionArity {
        name: NamedNode,
        expected: RangeInclusive<usize>,
        actual: usize,
    },
}

pub fn build_expression_evaluator<'a, C: ExpressionEvaluatorContext<'a>>(
    expression: &Expression,
    context: &mut C,
) -> Result<
    ExpressionEvaluator<'a, C::Tuple, ExpressionTerm, C::Error>,
    ExpressionEvaluationError<C::Error>,
>
where
    C::Error: 'a,
{
    Ok(match expression {
        Expression::NamedNode(t) => {
            let t = ExpressionTerm::from(Term::from(t.clone()));
            Rc::new(move |_| Ok(Some(t.clone())))
        }
        Expression::Literal(t) => {
            let t = ExpressionTerm::from(Term::from(t.clone()));
            Rc::new(move |_| Ok(Some(t.clone())))
        }
        Expression::Variable(v) => {
            let lookup = context.build_variable_lookup(v);
            let externalize = context.build_externalize_expression_term();
            Rc::new(move |t| externalize(try_or_ok!(lookup(t))).map(Some))
        }
        Expression::Bound(v) => {
            let lookup = context.build_is_variable_bound(v);
            Rc::new(move |tuple| Ok(Some(lookup(tuple).into())))
        }
        Expression::Exists(plan) => {
            let exists = context
                .build_exists(plan)
                .map_err(ExpressionEvaluationError::Context)?;
            Rc::new(move |tuple| Ok(Some(exists(tuple).into())))
        }
        Expression::Or(children) => {
            let children = children
                .iter()
                .map(|i| build_expression_evaluator(i, context))
                .collect::<Result<Vec<_>, _>>()?;
            Rc::new(move |tuple| {
                let mut error = false;
                for child in &children {
                    match child(tuple)?.and_then(|e| e.effective_boolean_value()) {
                        Some(true) => return Ok(Some(true.into())),
                        Some(false) => (),
                        None => error = true,
                    }
                }
                Ok(if error { None } else { Some(false.into()) })
            })
        }
        Expression::And(children) => {
            let children = children
                .iter()
                .map(|i| build_expression_evaluator(i, context))
                .collect::<Result<Vec<_>, _>>()?;
            Rc::new(move |tuple| {
                let mut error = false;
                for child in &children {
                    match child(tuple)?.and_then(|e| e.effective_boolean_value()) {
                        Some(true) => (),
                        Some(false) => return Ok(Some(false.into())),
                        None => error = true,
                    }
                }
                Ok(if error { None } else { Some(true.into()) })
            })
        }
        Expression::Equal(a, b) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            Rc::new(move |tuple| {
                Ok(equals(&try_or_ok!(a(tuple)?), &try_or_ok!(b(tuple)?)).map(Into::into))
            })
        }
        Expression::SameTerm(a, b) => {
            match (
                try_build_internal_expression_evaluator(a, context)?,
                try_build_internal_expression_evaluator(b, context)?,
            ) {
                (Some(a), Some(b)) => Rc::new(move |tuple| {
                    Ok(Some(
                        (try_or_ok!(a(tuple)?) == try_or_ok!(b(tuple)?)).into(),
                    ))
                }),
                (Some(a), None) => {
                    let b = build_expression_evaluator(b, context)?;
                    let internalize = context.build_internalize_expression_term();
                    Rc::new(move |tuple| {
                        Ok(Some(
                            (try_or_ok!(a(tuple)?) == internalize(try_or_ok!(b(tuple)?))?).into(),
                        ))
                    })
                }
                (None, Some(b)) => {
                    let a = build_expression_evaluator(a, context)?;
                    let internalize = context.build_internalize_expression_term();
                    Rc::new(move |tuple| {
                        Ok(Some(
                            (internalize(try_or_ok!(a(tuple)?))? == try_or_ok!(b(tuple)?)).into(),
                        ))
                    })
                }
                (None, None) => {
                    let a = build_expression_evaluator(a, context)?;
                    let b = build_expression_evaluator(b, context)?;
                    Rc::new(move |tuple| {
                        Ok(Some(
                            (try_or_ok!(a(tuple)?) == try_or_ok!(b(tuple)?)).into(),
                        ))
                    })
                }
            }
        }
        Expression::Greater(a, b) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            Rc::new(move |tuple| {
                Ok(Some(
                    (try_or_ok!(partial_cmp(&try_or_ok!(a(tuple)?), &try_or_ok!(b(tuple)?)))
                        == Ordering::Greater)
                        .into(),
                ))
            })
        }
        Expression::GreaterOrEqual(a, b) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            Rc::new(move |tuple| {
                Ok(Some(
                    match try_or_ok!(partial_cmp(&try_or_ok!(a(tuple)?), &try_or_ok!(b(tuple)?))) {
                        Ordering::Greater | Ordering::Equal => true,
                        Ordering::Less => false,
                    }
                    .into(),
                ))
            })
        }
        Expression::Less(a, b) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            Rc::new(move |tuple| {
                Ok(Some(
                    (try_or_ok!(partial_cmp(&try_or_ok!(a(tuple)?), &try_or_ok!(b(tuple)?)))
                        == Ordering::Less)
                        .into(),
                ))
            })
        }
        Expression::LessOrEqual(a, b) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            Rc::new(move |tuple| {
                Ok(Some(
                    match try_or_ok!(partial_cmp(&try_or_ok!(a(tuple)?), &try_or_ok!(b(tuple)?))) {
                        Ordering::Less | Ordering::Equal => true,
                        Ordering::Greater => false,
                    }
                    .into(),
                ))
            })
        }
        Expression::Add(a, b) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            Rc::new(move |tuple| {
                Ok(Some(
                    match try_or_ok!(NumericBinaryOperands::new(
                        try_or_ok!(a(tuple)?),
                        try_or_ok!(b(tuple)?),
                    )) {
                        NumericBinaryOperands::Float(v1, v2) => {
                            ExpressionTerm::FloatLiteral(v1 + v2)
                        }
                        NumericBinaryOperands::Double(v1, v2) => {
                            ExpressionTerm::DoubleLiteral(v1 + v2)
                        }
                        NumericBinaryOperands::Integer(v1, v2) => {
                            ExpressionTerm::IntegerLiteral(try_or_ok!(v1.checked_add(v2)))
                        }
                        NumericBinaryOperands::Decimal(v1, v2) => {
                            ExpressionTerm::DecimalLiteral(try_or_ok!(v1.checked_add(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::Duration(v1, v2) => {
                            ExpressionTerm::DurationLiteral(try_or_ok!(v1.checked_add(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::YearMonthDuration(v1, v2) => {
                            ExpressionTerm::YearMonthDurationLiteral(try_or_ok!(v1.checked_add(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DayTimeDuration(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(try_or_ok!(v1.checked_add(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(try_or_ok!(v1.checked_add_duration(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeYearMonthDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(try_or_ok!(
                                v1.checked_add_year_month_duration(v2)
                            ))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeDayTimeDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(try_or_ok!(
                                v1.checked_add_day_time_duration(v2)
                            ))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(try_or_ok!(v1.checked_add_duration(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateYearMonthDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(try_or_ok!(
                                v1.checked_add_year_month_duration(v2)
                            ))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateDayTimeDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(try_or_ok!(
                                v1.checked_add_day_time_duration(v2)
                            ))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::TimeDuration(v1, v2) => {
                            ExpressionTerm::TimeLiteral(try_or_ok!(v1.checked_add_duration(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::TimeDayTimeDuration(v1, v2) => {
                            ExpressionTerm::TimeLiteral(try_or_ok!(
                                v1.checked_add_day_time_duration(v2)
                            ))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTime(_, _)
                        | NumericBinaryOperands::Time(_, _)
                        | NumericBinaryOperands::Date(_, _) => return Ok(None),
                    },
                ))
            })
        }
        Expression::Subtract(a, b) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            Rc::new(move |tuple| {
                Ok(Some(
                    match try_or_ok!(NumericBinaryOperands::new(
                        try_or_ok!(a(tuple)?),
                        try_or_ok!(b(tuple)?),
                    )) {
                        NumericBinaryOperands::Float(v1, v2) => {
                            ExpressionTerm::FloatLiteral(v1 - v2)
                        }
                        NumericBinaryOperands::Double(v1, v2) => {
                            ExpressionTerm::DoubleLiteral(v1 - v2)
                        }
                        NumericBinaryOperands::Integer(v1, v2) => {
                            ExpressionTerm::IntegerLiteral(try_or_ok!(v1.checked_sub(v2)))
                        }
                        NumericBinaryOperands::Decimal(v1, v2) => {
                            ExpressionTerm::DecimalLiteral(try_or_ok!(v1.checked_sub(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTime(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(try_or_ok!(v1.checked_sub(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::Date(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(try_or_ok!(v1.checked_sub(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::Time(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(try_or_ok!(v1.checked_sub(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::Duration(v1, v2) => {
                            ExpressionTerm::DurationLiteral(try_or_ok!(v1.checked_sub(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::YearMonthDuration(v1, v2) => {
                            ExpressionTerm::YearMonthDurationLiteral(try_or_ok!(v1.checked_sub(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DayTimeDuration(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(try_or_ok!(v1.checked_sub(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(try_or_ok!(v1.checked_sub_duration(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeYearMonthDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(try_or_ok!(
                                v1.checked_sub_year_month_duration(v2)
                            ))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeDayTimeDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(try_or_ok!(
                                v1.checked_sub_day_time_duration(v2)
                            ))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(try_or_ok!(v1.checked_sub_duration(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateYearMonthDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(try_or_ok!(
                                v1.checked_sub_year_month_duration(v2)
                            ))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateDayTimeDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(try_or_ok!(
                                v1.checked_sub_day_time_duration(v2)
                            ))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::TimeDuration(v1, v2) => {
                            ExpressionTerm::TimeLiteral(try_or_ok!(v1.checked_sub_duration(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::TimeDayTimeDuration(v1, v2) => {
                            ExpressionTerm::TimeLiteral(try_or_ok!(
                                v1.checked_sub_day_time_duration(v2)
                            ))
                        }
                    },
                ))
            })
        }
        Expression::Multiply(a, b) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            Rc::new(move |tuple| {
                Ok(Some(
                    match try_or_ok!(NumericBinaryOperands::new(
                        try_or_ok!(a(tuple)?),
                        try_or_ok!(b(tuple)?),
                    )) {
                        NumericBinaryOperands::Float(v1, v2) => {
                            ExpressionTerm::FloatLiteral(v1 * v2)
                        }
                        NumericBinaryOperands::Double(v1, v2) => {
                            ExpressionTerm::DoubleLiteral(v1 * v2)
                        }
                        NumericBinaryOperands::Integer(v1, v2) => {
                            ExpressionTerm::IntegerLiteral(try_or_ok!(v1.checked_mul(v2)))
                        }
                        NumericBinaryOperands::Decimal(v1, v2) => {
                            ExpressionTerm::DecimalLiteral(try_or_ok!(v1.checked_mul(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        _ => return Ok(None),
                    },
                ))
            })
        }
        Expression::Divide(a, b) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            Rc::new(move |tuple| {
                Ok(Some(
                    match try_or_ok!(NumericBinaryOperands::new(
                        try_or_ok!(a(tuple)?),
                        try_or_ok!(b(tuple)?),
                    )) {
                        NumericBinaryOperands::Float(v1, v2) => {
                            ExpressionTerm::FloatLiteral(v1 / v2)
                        }
                        NumericBinaryOperands::Double(v1, v2) => {
                            ExpressionTerm::DoubleLiteral(v1 / v2)
                        }
                        NumericBinaryOperands::Integer(v1, v2) => ExpressionTerm::DecimalLiteral(
                            try_or_ok!(Decimal::from(v1).checked_div(v2)),
                        ),
                        NumericBinaryOperands::Decimal(v1, v2) => {
                            ExpressionTerm::DecimalLiteral(try_or_ok!(v1.checked_div(v2)))
                        }
                        #[cfg(feature = "sep-0002")]
                        _ => return Ok(None),
                    },
                ))
            })
        }
        Expression::UnaryPlus(e) => {
            let e = build_expression_evaluator(e, context)?;
            Rc::new(move |tuple| {
                Ok(Some(match try_or_ok!(e(tuple)?) {
                    ExpressionTerm::FloatLiteral(value) => ExpressionTerm::FloatLiteral(value),
                    ExpressionTerm::DoubleLiteral(value) => ExpressionTerm::DoubleLiteral(value),
                    ExpressionTerm::IntegerLiteral(value) => ExpressionTerm::IntegerLiteral(value),
                    ExpressionTerm::DecimalLiteral(value) => ExpressionTerm::DecimalLiteral(value),
                    #[cfg(feature = "sep-0002")]
                    ExpressionTerm::DurationLiteral(value) => {
                        ExpressionTerm::DurationLiteral(value)
                    }
                    #[cfg(feature = "sep-0002")]
                    ExpressionTerm::YearMonthDurationLiteral(value) => {
                        ExpressionTerm::YearMonthDurationLiteral(value)
                    }
                    #[cfg(feature = "sep-0002")]
                    ExpressionTerm::DayTimeDurationLiteral(value) => {
                        ExpressionTerm::DayTimeDurationLiteral(value)
                    }
                    _ => return Ok(None),
                }))
            })
        }
        Expression::UnaryMinus(e) => {
            let e = build_expression_evaluator(e, context)?;
            Rc::new(move |tuple| {
                Ok(Some(match try_or_ok!(e(tuple)?) {
                    ExpressionTerm::FloatLiteral(value) => ExpressionTerm::FloatLiteral(-value),
                    ExpressionTerm::DoubleLiteral(value) => ExpressionTerm::DoubleLiteral(-value),
                    ExpressionTerm::IntegerLiteral(value) => {
                        ExpressionTerm::IntegerLiteral(try_or_ok!(value.checked_neg()))
                    }
                    ExpressionTerm::DecimalLiteral(value) => {
                        ExpressionTerm::DecimalLiteral(try_or_ok!(value.checked_neg()))
                    }
                    #[cfg(feature = "sep-0002")]
                    ExpressionTerm::DurationLiteral(value) => {
                        ExpressionTerm::DurationLiteral(try_or_ok!(value.checked_neg()))
                    }
                    #[cfg(feature = "sep-0002")]
                    ExpressionTerm::YearMonthDurationLiteral(value) => {
                        ExpressionTerm::YearMonthDurationLiteral(try_or_ok!(value.checked_neg()))
                    }
                    #[cfg(feature = "sep-0002")]
                    ExpressionTerm::DayTimeDurationLiteral(value) => {
                        ExpressionTerm::DayTimeDurationLiteral(try_or_ok!(value.checked_neg()))
                    }
                    _ => return Ok(None),
                }))
            })
        }
        Expression::Not(e) => {
            let e = build_expression_evaluator(e, context)?;
            Rc::new(move |tuple| {
                Ok(Some(
                    (!try_or_ok!(try_or_ok!(e(tuple)?).effective_boolean_value())).into(),
                ))
            })
        }
        Expression::Coalesce(l) => {
            let l = l
                .iter()
                .map(|e| build_expression_evaluator(e, context))
                .collect::<Result<Vec<_>, _>>()?;
            Rc::new(move |tuple| {
                for e in &l {
                    if let Some(result) = e(tuple)? {
                        return Ok(Some(result));
                    }
                }
                Ok(None)
            })
        }
        Expression::If(a, b, c) => {
            let a = build_expression_evaluator(a, context)?;
            let b = build_expression_evaluator(b, context)?;
            let c = build_expression_evaluator(c, context)?;
            Rc::new(move |tuple| {
                if try_or_ok!(try_or_ok!(a(tuple)?).effective_boolean_value()) {
                    b(tuple)
                } else {
                    c(tuple)
                }
            })
        }
        Expression::FunctionCall(function, parameters) => match function {
            Function::Str => {
                if let Some(e) = try_build_internal_expression_evaluator(&parameters[0], context)? {
                    let externalize = context.build_externalize_term();
                    Rc::new(move |tuple| {
                        Ok(Some(ExpressionTerm::StringLiteral(
                            match externalize(try_or_ok!(e(tuple)?))? {
                                Term::NamedNode(term) => term.into_string(),
                                Term::BlankNode(_) => return Ok(None),
                                Term::Literal(term) => term.into_value(),
                                #[cfg(feature = "sparql-12")]
                                Term::Triple(_) => return Ok(None),
                            },
                        )))
                    })
                } else {
                    let e = build_expression_evaluator(&parameters[0], context)?;
                    Rc::new(move |tuple| {
                        Ok(Some(ExpressionTerm::StringLiteral(
                            match try_or_ok!(e(tuple)?).into() {
                                Term::NamedNode(term) => term.into_string(),
                                Term::BlankNode(_) => return Ok(None),
                                Term::Literal(term) => term.into_value(),
                                #[cfg(feature = "sparql-12")]
                                Term::Triple(_) => return Ok(None),
                            },
                        )))
                    })
                }
            }
            Function::Lang => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::StringLiteral(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::LangStringLiteral { language, .. } => language,
                            #[cfg(feature = "sparql-12")]
                            ExpressionTerm::DirLangStringLiteral { language, .. } => language,
                            ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                                return Ok(None);
                            }
                            #[cfg(feature = "sparql-12")]
                            ExpressionTerm::Triple(_) => return Ok(None),
                            _ => OxString::default(),
                        },
                    )))
                })
            }
            Function::LangMatches => {
                let language_tag = build_expression_evaluator(&parameters[0], context)?;
                let language_range = build_expression_evaluator(&parameters[1], context)?;
                Rc::new(move |tuple| {
                    let ExpressionTerm::StringLiteral(mut language_tag) =
                        try_or_ok!(language_tag(tuple)?)
                    else {
                        return Ok(None);
                    };
                    language_tag.make_mut().make_ascii_lowercase();
                    let ExpressionTerm::StringLiteral(mut language_range) =
                        try_or_ok!(language_range(tuple)?)
                    else {
                        return Ok(None);
                    };
                    language_range.make_mut().make_ascii_lowercase();
                    Ok(Some(
                        if &*language_range == "*" {
                            !language_tag.is_empty()
                        } else {
                            !ZipLongest::new(language_range.split('-'), language_tag.split('-'))
                                .any(|parts| match parts {
                                    (Some(range_subtag), Some(language_subtag)) => {
                                        range_subtag != language_subtag
                                    }
                                    (Some(_), None) => true,
                                    (None, _) => false,
                                })
                        }
                        .into(),
                    ))
                })
            }
            #[cfg(feature = "sparql-12")]
            Function::LangDir => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::StringLiteral(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::DirLangStringLiteral { direction, .. } => {
                                match direction {
                                    BaseDirection::Ltr => "ltr".into(),
                                    BaseDirection::Rtl => "rtl".into(),
                                }
                            }
                            ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                                return Ok(None);
                            }
                            #[cfg(feature = "sparql-12")]
                            ExpressionTerm::Triple(_) => return Ok(None),
                            _ => OxString::default(),
                        },
                    )))
                })
            }
            Function::Datatype => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::NamedNode(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::StringLiteral(_) => xsd::STRING,
                            ExpressionTerm::LangStringLiteral { .. } => rdf::LANG_STRING,
                            #[cfg(feature = "sparql-12")]
                            ExpressionTerm::DirLangStringLiteral { .. } => rdf::DIR_LANG_STRING,
                            ExpressionTerm::BooleanLiteral(_) => xsd::BOOLEAN,
                            ExpressionTerm::IntegerLiteral(_) => xsd::INTEGER,
                            ExpressionTerm::DecimalLiteral(_) => xsd::DECIMAL,
                            ExpressionTerm::FloatLiteral(_) => xsd::FLOAT,
                            ExpressionTerm::DoubleLiteral(_) => xsd::DOUBLE,
                            ExpressionTerm::DateTimeLiteral(_) => xsd::DATE_TIME,
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DateLiteral(_) => xsd::DATE,
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::TimeLiteral(_) => xsd::TIME,
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearLiteral(_) => xsd::G_YEAR,
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearMonthLiteral(_) => xsd::G_YEAR_MONTH,
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthLiteral(_) => xsd::G_MONTH,
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthDayLiteral(_) => xsd::G_MONTH_DAY,
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GDayLiteral(_) => xsd::G_DAY,
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DurationLiteral(_) => xsd::DURATION,
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::YearMonthDurationLiteral(_) => xsd::YEAR_MONTH_DURATION,
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DayTimeDurationLiteral(_) => xsd::DAY_TIME_DURATION,
                            ExpressionTerm::OtherTypedLiteral { datatype, .. } => datatype,
                            ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                                return Ok(None);
                            }
                            #[cfg(feature = "sparql-12")]
                            ExpressionTerm::Triple(_) => return Ok(None),
                        },
                    )))
                })
            }
            Function::Iri => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                let base_iri = context.base_iri();
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::NamedNode(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::NamedNode(iri) => iri,
                            ExpressionTerm::StringLiteral(iri) => {
                                NamedNode::new_unchecked(if let Some(base_iri) = &base_iri {
                                    OxString::new_owned(
                                        &try_or_ok!(base_iri.resolve(&iri).ok()).into_inner(),
                                    )
                                } else {
                                    try_or_ok!(Iri::parse(iri).ok()).into_inner()
                                })
                            }
                            _ => return Ok(None),
                        },
                    )))
                })
            }
            Function::BNode => match parameters.first() {
                Some(id) => {
                    let id = build_expression_evaluator(id, context)?;
                    Rc::new(move |tuple| {
                        let ExpressionTerm::StringLiteral(id) = try_or_ok!(id(tuple)?) else {
                            return Ok(None);
                        };
                        Ok(Some(ExpressionTerm::BlankNode(try_or_ok!(
                            BlankNode::new(id).ok()
                        ))))
                    })
                }
                None => Rc::new(|_| Ok(Some(ExpressionTerm::BlankNode(BlankNode::default())))),
            },
            Function::Rand => {
                Rc::new(|_| Ok(Some(ExpressionTerm::DoubleLiteral(random::<f64>().into()))))
            }
            Function::Abs => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(match try_or_ok!(e(tuple)?) {
                        ExpressionTerm::IntegerLiteral(value) => Some(
                            ExpressionTerm::IntegerLiteral(try_or_ok!(value.checked_abs())),
                        ),
                        ExpressionTerm::DecimalLiteral(value) => Some(
                            ExpressionTerm::DecimalLiteral(try_or_ok!(value.checked_abs())),
                        ),
                        ExpressionTerm::FloatLiteral(value) => {
                            Some(ExpressionTerm::FloatLiteral(value.abs()))
                        }
                        ExpressionTerm::DoubleLiteral(value) => {
                            Some(ExpressionTerm::DoubleLiteral(value.abs()))
                        }
                        _ => None,
                    })
                })
            }
            Function::Ceil => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(match try_or_ok!(e(tuple)?) {
                        ExpressionTerm::IntegerLiteral(value) => {
                            Some(ExpressionTerm::IntegerLiteral(value))
                        }
                        ExpressionTerm::DecimalLiteral(value) => Some(
                            ExpressionTerm::DecimalLiteral(try_or_ok!(value.checked_ceil())),
                        ),
                        ExpressionTerm::FloatLiteral(value) => {
                            Some(ExpressionTerm::FloatLiteral(value.ceil()))
                        }
                        ExpressionTerm::DoubleLiteral(value) => {
                            Some(ExpressionTerm::DoubleLiteral(value.ceil()))
                        }
                        _ => None,
                    })
                })
            }
            Function::Floor => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(match try_or_ok!(e(tuple)?) {
                        ExpressionTerm::IntegerLiteral(value) => {
                            Some(ExpressionTerm::IntegerLiteral(value))
                        }
                        ExpressionTerm::DecimalLiteral(value) => Some(
                            ExpressionTerm::DecimalLiteral(try_or_ok!(value.checked_floor())),
                        ),
                        ExpressionTerm::FloatLiteral(value) => {
                            Some(ExpressionTerm::FloatLiteral(value.floor()))
                        }
                        ExpressionTerm::DoubleLiteral(value) => {
                            Some(ExpressionTerm::DoubleLiteral(value.floor()))
                        }
                        _ => None,
                    })
                })
            }
            Function::Round => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(match try_or_ok!(e(tuple)?) {
                        ExpressionTerm::IntegerLiteral(value) => {
                            Some(ExpressionTerm::IntegerLiteral(value))
                        }
                        ExpressionTerm::DecimalLiteral(value) => Some(
                            ExpressionTerm::DecimalLiteral(try_or_ok!(value.checked_round())),
                        ),
                        ExpressionTerm::FloatLiteral(value) => {
                            Some(ExpressionTerm::FloatLiteral(value.round()))
                        }
                        ExpressionTerm::DoubleLiteral(value) => {
                            Some(ExpressionTerm::DoubleLiteral(value.round()))
                        }
                        _ => None,
                    })
                })
            }
            Function::Concat => {
                let l = parameters
                    .iter()
                    .map(|e| build_expression_evaluator(e, context))
                    .collect::<Result<Vec<_>, _>>()?;
                Rc::new(move |tuple| {
                    let mut args = Vec::with_capacity(l.len());
                    let mut language = None;
                    for e in &l {
                        let (value, e_language) =
                            try_or_ok!(to_string_and_language(try_or_ok!(e(tuple)?)));
                        if let Some(lang) = &language {
                            if *lang != e_language {
                                language = Some(None)
                            }
                        } else {
                            language = Some(e_language)
                        }
                        args.push(value);
                    }
                    Ok(Some(build_plain_literal(
                        OxString::concat(args),
                        language.flatten(),
                    )))
                })
            }
            Function::SubStr => {
                let source = build_expression_evaluator(&parameters[0], context)?;
                let starting_loc = build_expression_evaluator(&parameters[1], context)?;
                let length = parameters
                    .get(2)
                    .map(|l| build_expression_evaluator(l, context))
                    .transpose()?;
                Rc::new(move |tuple| {
                    let (source, language) =
                        try_or_ok!(to_string_and_language(try_or_ok!(source(tuple)?)));

                    let starting_location: usize = if let ExpressionTerm::IntegerLiteral(v) =
                        try_or_ok!(starting_loc(tuple)?)
                    {
                        try_or_ok!(usize::try_from(i64::from(v)).ok())
                    } else {
                        return Ok(None);
                    };
                    let length = if let Some(length) = &length {
                        if let ExpressionTerm::IntegerLiteral(v) = try_or_ok!(length(tuple)?) {
                            Some(try_or_ok!(usize::try_from(i64::from(v)).ok()))
                        } else {
                            return Ok(None);
                        }
                    } else {
                        None
                    };

                    // We want to slice on char indices, not byte indices
                    let mut start_iter = source
                        .char_indices()
                        .skip(try_or_ok!(starting_location.checked_sub(1)))
                        .peekable();
                    let result = if let Some((start_position, _)) = start_iter.peek().copied() {
                        OxString::new_owned(if let Some(length) = length {
                            let mut end_iter = start_iter.skip(length).peekable();
                            if let Some((end_position, _)) = end_iter.peek() {
                                &source[start_position..*end_position]
                            } else {
                                &source[start_position..]
                            }
                        } else {
                            &source[start_position..]
                        })
                    } else {
                        OxString::default()
                    };
                    Ok(Some(build_plain_literal(result, language)))
                })
            }
            Function::StrLen => {
                let arg = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    let (string, _) = try_or_ok!(to_string_and_language(try_or_ok!(arg(tuple)?)));
                    Ok(Some(ExpressionTerm::IntegerLiteral(
                        try_or_ok!(i64::try_from(string.chars().count()).ok()).into(),
                    )))
                })
            }
            Function::Replace => {
                let arg = build_expression_evaluator(&parameters[0], context)?;
                let replacement = build_expression_evaluator(&parameters[2], context)?;
                if let Some(regex) =
                    compile_static_pattern_if_exists(&parameters[1], parameters.get(3))
                {
                    Rc::new(move |tuple| {
                        let (text, language) =
                            try_or_ok!(to_string_and_language(try_or_ok!(arg(tuple)?)));
                        let ExpressionTerm::StringLiteral(replacement) =
                            try_or_ok!(replacement(tuple)?)
                        else {
                            return Ok(None);
                        };
                        Ok(Some(build_plain_literal(
                            match regex.replace_all(text.as_str(), replacement.as_str()) {
                                Cow::Owned(replaced) => OxString::new_owned(&replaced),
                                Cow::Borrowed(_) => text,
                            },
                            language,
                        )))
                    })
                } else {
                    let pattern = build_expression_evaluator(&parameters[1], context)?;
                    let flags = parameters
                        .get(3)
                        .map(|flags| build_expression_evaluator(flags, context))
                        .transpose()?;
                    Rc::new(move |tuple| {
                        let ExpressionTerm::StringLiteral(pattern) = try_or_ok!(pattern(tuple)?)
                        else {
                            return Ok(None);
                        };
                        let options = if let Some(flags) = &flags {
                            let ExpressionTerm::StringLiteral(options) = try_or_ok!(flags(tuple)?)
                            else {
                                return Ok(None);
                            };
                            Some(options)
                        } else {
                            None
                        };
                        let regex = try_or_ok!(compile_pattern(&pattern, options.as_deref()));
                        let (text, language) =
                            try_or_ok!(to_string_and_language(try_or_ok!(arg(tuple)?)));
                        let ExpressionTerm::StringLiteral(replacement) =
                            try_or_ok!(replacement(tuple)?)
                        else {
                            return Ok(None);
                        };
                        Ok(Some(build_plain_literal(
                            match regex.replace_all(text.as_str(), replacement.as_str()) {
                                Cow::Owned(replaced) => OxString::new_owned(&replaced),
                                Cow::Borrowed(_) => text,
                            },
                            language,
                        )))
                    })
                }
            }
            Function::UCase => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    let (mut value, language) =
                        try_or_ok!(to_string_and_language(try_or_ok!(e(tuple)?)));
                    let value = if value.is_ascii() {
                        value.make_mut().make_ascii_uppercase();
                        value
                    } else {
                        OxString::new_owned(&value.to_uppercase())
                    };
                    Ok(Some(build_plain_literal(value, language)))
                })
            }
            Function::LCase => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    let (mut value, language) =
                        try_or_ok!(to_string_and_language(try_or_ok!(e(tuple)?)));
                    let value = if value.is_ascii() {
                        value.make_mut().make_ascii_lowercase();
                        value
                    } else {
                        OxString::new_owned(&value.to_lowercase())
                    };
                    Ok(Some(build_plain_literal(value, language)))
                })
            }
            Function::StrStarts => {
                let arg1 = build_expression_evaluator(&parameters[0], context)?;
                let arg2 = build_expression_evaluator(&parameters[1], context)?;
                Rc::new(move |tuple| {
                    let (arg1, arg2, _) = try_or_ok!(to_argument_compatible_strings(
                        try_or_ok!(arg1(tuple)?),
                        try_or_ok!(arg2(tuple)?),
                    ));
                    Ok(Some(arg1.starts_with(arg2.as_str()).into()))
                })
            }
            Function::EncodeForUri => {
                let ltrl = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    let (ltlr, _) = try_or_ok!(to_string_and_language(try_or_ok!(ltrl(tuple)?)));
                    let mut result = Vec::with_capacity(ltlr.len());
                    for c in ltlr.bytes() {
                        match c {
                            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                                result.push(c)
                            }
                            _ => {
                                result.push(b'%');
                                let high = c / 16;
                                let low = c % 16;
                                result.push(if high < 10 {
                                    b'0' + high
                                } else {
                                    b'A' + (high - 10)
                                });
                                result.push(if low < 10 {
                                    b'0' + low
                                } else {
                                    b'A' + (low - 10)
                                });
                            }
                        }
                    }
                    Ok(Some(ExpressionTerm::StringLiteral(OxString::new_owned(
                        try_or_ok!(str::from_utf8(&result).ok()),
                    ))))
                })
            }
            Function::StrEnds => {
                let arg1 = build_expression_evaluator(&parameters[0], context)?;
                let arg2 = build_expression_evaluator(&parameters[1], context)?;
                Rc::new(move |tuple| {
                    let (arg1, arg2, _) = try_or_ok!(to_argument_compatible_strings(
                        try_or_ok!(arg1(tuple)?),
                        try_or_ok!(arg2(tuple)?),
                    ));
                    Ok(Some(arg1.ends_with(arg2.as_str()).into()))
                })
            }
            Function::Contains => {
                let arg1 = build_expression_evaluator(&parameters[0], context)?;
                let arg2 = build_expression_evaluator(&parameters[1], context)?;
                Rc::new(move |tuple| {
                    let (arg1, arg2, _) = try_or_ok!(to_argument_compatible_strings(
                        try_or_ok!(arg1(tuple)?),
                        try_or_ok!(arg2(tuple)?),
                    ));
                    Ok(Some(arg1.contains(arg2.as_str()).into()))
                })
            }
            Function::StrBefore => {
                let arg1 = build_expression_evaluator(&parameters[0], context)?;
                let arg2 = build_expression_evaluator(&parameters[1], context)?;
                Rc::new(move |tuple| {
                    let (arg1, arg2, language) = try_or_ok!(to_argument_compatible_strings(
                        try_or_ok!(arg1(tuple)?),
                        try_or_ok!(arg2(tuple)?),
                    ));
                    Ok(Some(if let Some(position) = arg1.find(arg2.as_str()) {
                        build_plain_literal(OxString::new_owned(&arg1[..position]), language)
                    } else {
                        ExpressionTerm::StringLiteral(OxString::default())
                    }))
                })
            }
            Function::StrAfter => {
                let arg1 = build_expression_evaluator(&parameters[0], context)?;
                let arg2 = build_expression_evaluator(&parameters[1], context)?;
                Rc::new(move |tuple| {
                    let (arg1, arg2, language) = try_or_ok!(to_argument_compatible_strings(
                        try_or_ok!(arg1(tuple)?),
                        try_or_ok!(arg2(tuple)?),
                    ));
                    Ok(Some(if let Some(position) = arg1.find(arg2.as_str()) {
                        build_plain_literal(
                            OxString::new_owned(&arg1[position + arg2.len()..]),
                            language,
                        )
                    } else {
                        ExpressionTerm::StringLiteral(OxString::default())
                    }))
                })
            }
            Function::Year => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::IntegerLiteral(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::DateTimeLiteral(date_time) => date_time.year(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DateLiteral(date) => date.year(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearMonthLiteral(year_month) => year_month.year(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearLiteral(year) => year.year(),
                            _ => return Ok(None),
                        }
                        .into(),
                    )))
                })
            }
            Function::Month => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::IntegerLiteral(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::DateTimeLiteral(date_time) => date_time.month(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DateLiteral(date) => date.month(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearMonthLiteral(year_month) => year_month.month(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthDayLiteral(month_day) => month_day.month(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthLiteral(month) => month.month(),
                            _ => return Ok(None),
                        }
                        .into(),
                    )))
                })
            }
            Function::Day => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::IntegerLiteral(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::DateTimeLiteral(date_time) => date_time.day(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DateLiteral(date) => date.day(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthDayLiteral(month_day) => month_day.day(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GDayLiteral(day) => day.day(),
                            _ => return Ok(None),
                        }
                        .into(),
                    )))
                })
            }
            Function::Hours => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::IntegerLiteral(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::DateTimeLiteral(date_time) => date_time.hour(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::TimeLiteral(time) => time.hour(),
                            _ => return Ok(None),
                        }
                        .into(),
                    )))
                })
            }
            Function::Minutes => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::IntegerLiteral(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::DateTimeLiteral(date_time) => date_time.minute(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::TimeLiteral(time) => time.minute(),
                            _ => return Ok(None),
                        }
                        .into(),
                    )))
                })
            }
            Function::Seconds => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(ExpressionTerm::DecimalLiteral(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::DateTimeLiteral(date_time) => date_time.second(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::TimeLiteral(time) => time.second(),
                            _ => return Ok(None),
                        },
                    )))
                })
            }
            Function::Timezone => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    let result = try_or_ok!(match try_or_ok!(e(tuple)?) {
                        ExpressionTerm::DateTimeLiteral(date_time) => date_time.timezone(),
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::TimeLiteral(time) => time.timezone(),
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DateLiteral(date) => date.timezone(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GYearMonthLiteral(year_month) => {
                            year_month.timezone()
                        }
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GYearLiteral(year) => year.timezone(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GMonthDayLiteral(month_day) => month_day.timezone(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GDayLiteral(day) => day.timezone(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GMonthLiteral(month) => month.timezone(),
                        _ => None,
                    });
                    #[cfg(feature = "sep-0002")]
                    {
                        Ok(Some(ExpressionTerm::DayTimeDurationLiteral(result)))
                    }
                    #[cfg(not(feature = "sep-0002"))]
                    {
                        Ok(Some(ExpressionTerm::OtherTypedLiteral {
                            value: OxString::new_owned(&result.to_string()),
                            datatype: xsd::DAY_TIME_DURATION,
                        }))
                    }
                })
            }
            Function::Tz => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    let timezone_offset = match try_or_ok!(e(tuple)?) {
                        ExpressionTerm::DateTimeLiteral(date_time) => date_time.timezone_offset(),
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::TimeLiteral(time) => time.timezone_offset(),
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DateLiteral(date) => date.timezone_offset(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GYearMonthLiteral(year_month) => {
                            year_month.timezone_offset()
                        }
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GYearLiteral(year) => year.timezone_offset(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GMonthDayLiteral(month_day) => month_day.timezone_offset(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GDayLiteral(day) => day.timezone_offset(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GMonthLiteral(month) => month.timezone_offset(),
                        _ => return Ok(None),
                    };
                    Ok(Some(ExpressionTerm::StringLiteral(
                        timezone_offset.map_or_else(OxString::default, |o| {
                            OxString::new_owned(o.to_string().as_str())
                        }),
                    )))
                })
            }
            #[cfg(feature = "sep-0002")]
            Function::Adjust => {
                let dt = build_expression_evaluator(&parameters[0], context)?;
                let tz = build_expression_evaluator(&parameters[1], context)?;
                Rc::new(move |tuple| {
                    let timezone_offset = Some(try_or_ok!(
                        match try_or_ok!(tz(tuple)?) {
                            ExpressionTerm::DayTimeDurationLiteral(tz) => {
                                TimezoneOffset::try_from(tz)
                            }
                            ExpressionTerm::DurationLiteral(tz) => {
                                TimezoneOffset::try_from(tz)
                            }
                            _ => return Ok(None),
                        }
                        .ok()
                    ));
                    Ok(Some(match try_or_ok!(dt(tuple)?) {
                        ExpressionTerm::DateTimeLiteral(date_time) => {
                            ExpressionTerm::DateTimeLiteral(try_or_ok!(
                                date_time.adjust(timezone_offset)
                            ))
                        }
                        ExpressionTerm::TimeLiteral(time) => {
                            ExpressionTerm::TimeLiteral(try_or_ok!(time.adjust(timezone_offset)))
                        }
                        ExpressionTerm::DateLiteral(date) => {
                            ExpressionTerm::DateLiteral(try_or_ok!(date.adjust(timezone_offset)))
                        }
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GYearMonthLiteral(year_month) => {
                            ExpressionTerm::GYearMonthLiteral(try_or_ok!(
                                year_month.adjust(timezone_offset)
                            ))
                        }
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GYearLiteral(year) => {
                            ExpressionTerm::GYearLiteral(try_or_ok!(year.adjust(timezone_offset)))
                        }
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GMonthDayLiteral(month_day) => {
                            ExpressionTerm::GMonthDayLiteral(try_or_ok!(
                                month_day.adjust(timezone_offset)
                            ))
                        }
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GDayLiteral(day) => {
                            ExpressionTerm::GDayLiteral(try_or_ok!(day.adjust(timezone_offset)))
                        }
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GMonthLiteral(month) => {
                            ExpressionTerm::GMonthLiteral(try_or_ok!(month.adjust(timezone_offset)))
                        }
                        _ => return Ok(None),
                    }))
                })
            }
            Function::Now => {
                let now = context.now();
                Rc::new(move |_| Ok(Some(ExpressionTerm::DateTimeLiteral(now))))
            }
            Function::Uuid => Rc::new(move |_| {
                let mut buffer = String::with_capacity(44);
                buffer.push_str("urn:uuid:");
                generate_uuid(&mut buffer);
                Ok(Some(ExpressionTerm::NamedNode(NamedNode::new_unchecked(
                    OxString::new_owned(&buffer),
                ))))
            }),
            Function::StrUuid => Rc::new(move |_| {
                let mut buffer = String::with_capacity(36);
                generate_uuid(&mut buffer);
                Ok(Some(ExpressionTerm::StringLiteral(OxString::new_owned(
                    &buffer,
                ))))
            }),
            Function::Md5 => build_hash_expression_evaluator::<_, Md5>(parameters, context)?,
            Function::Sha1 => build_hash_expression_evaluator::<_, Sha1>(parameters, context)?,
            Function::Sha256 => build_hash_expression_evaluator::<_, Sha256>(parameters, context)?,
            Function::Sha384 => build_hash_expression_evaluator::<_, Sha384>(parameters, context)?,
            Function::Sha512 => build_hash_expression_evaluator::<_, Sha512>(parameters, context)?,
            Function::StrLang => {
                let lexical_form = build_expression_evaluator(&parameters[0], context)?;
                let lang_tag = build_expression_evaluator(&parameters[1], context)?;
                Rc::new(move |tuple| {
                    let ExpressionTerm::StringLiteral(value) = try_or_ok!(lexical_form(tuple)?)
                    else {
                        return Ok(None);
                    };
                    let ExpressionTerm::StringLiteral(language) = try_or_ok!(lang_tag(tuple)?)
                    else {
                        return Ok(None);
                    };
                    Ok(Some(
                        Term::from(try_or_ok!(
                            Literal::new_language_tagged_literal(value, language).ok()
                        ))
                        .into(),
                    ))
                })
            }
            #[cfg(feature = "sparql-12")]
            Function::StrLangDir => {
                let lexical_form = build_expression_evaluator(&parameters[0], context)?;
                let lang_tag = build_expression_evaluator(&parameters[1], context)?;
                let direction = build_expression_evaluator(&parameters[2], context)?;
                Rc::new(move |tuple| {
                    let ExpressionTerm::StringLiteral(value) = try_or_ok!(lexical_form(tuple)?)
                    else {
                        return Ok(None);
                    };
                    let ExpressionTerm::StringLiteral(language) = try_or_ok!(lang_tag(tuple)?)
                    else {
                        return Ok(None);
                    };
                    let ExpressionTerm::StringLiteral(direction) = try_or_ok!(direction(tuple)?)
                    else {
                        return Ok(None);
                    };
                    let direction = match direction.as_str() {
                        "ltr" => BaseDirection::Ltr,
                        "rtl" => BaseDirection::Rtl,
                        _ => return Ok(None),
                    };
                    Ok(Some(
                        Term::from(try_or_ok!(
                            Literal::new_directional_language_tagged_literal(
                                value, language, direction,
                            )
                            .ok()
                        ))
                        .into(),
                    ))
                })
            }
            Function::StrDt => {
                let lexical_form = build_expression_evaluator(&parameters[0], context)?;
                let datatype = build_expression_evaluator(&parameters[1], context)?;
                Rc::new(move |tuple| {
                    let ExpressionTerm::StringLiteral(value) = try_or_ok!(lexical_form(tuple)?)
                    else {
                        return Ok(None);
                    };
                    let ExpressionTerm::NamedNode(datatype) = try_or_ok!(datatype(tuple)?) else {
                        return Ok(None);
                    };
                    Ok(Some(
                        Term::from(Literal::new_typed_literal(value, datatype)).into(),
                    ))
                })
            }

            Function::IsIri => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(
                        matches!(try_or_ok!(e(tuple)?), ExpressionTerm::NamedNode(_)).into(),
                    ))
                })
            }
            Function::IsBlank => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(
                        matches!(try_or_ok!(e(tuple)?), ExpressionTerm::BlankNode(_)).into(),
                    ))
                })
            }
            Function::IsLiteral => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(
                        match try_or_ok!(e(tuple)?) {
                            ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => false,
                            #[cfg(feature = "sparql-12")]
                            ExpressionTerm::Triple(_) => false,
                            _ => true,
                        }
                        .into(),
                    ))
                })
            }
            Function::IsNumeric => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(
                        matches!(
                            try_or_ok!(e(tuple)?),
                            ExpressionTerm::IntegerLiteral(_)
                                | ExpressionTerm::DecimalLiteral(_)
                                | ExpressionTerm::FloatLiteral(_)
                                | ExpressionTerm::DoubleLiteral(_)
                        )
                        .into(),
                    ))
                })
            }
            #[cfg(feature = "sparql-12")]
            Function::HasLang => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(
                        matches!(
                            try_or_ok!(e(tuple)?),
                            ExpressionTerm::LangStringLiteral { .. }
                                | ExpressionTerm::DirLangStringLiteral { .. }
                        )
                        .into(),
                    ))
                })
            }
            #[cfg(feature = "sparql-12")]
            Function::HasLangDir => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(
                        matches!(
                            try_or_ok!(e(tuple)?),
                            ExpressionTerm::DirLangStringLiteral { .. }
                        )
                        .into(),
                    ))
                })
            }
            Function::Regex => {
                let text = build_expression_evaluator(&parameters[0], context)?;
                if let Some(regex) =
                    compile_static_pattern_if_exists(&parameters[1], parameters.get(2))
                {
                    Rc::new(move |tuple| {
                        let (text, _) =
                            try_or_ok!(to_string_and_language(try_or_ok!(text(tuple)?)));
                        Ok(Some(regex.is_match(&text).into()))
                    })
                } else {
                    let pattern = build_expression_evaluator(&parameters[1], context)?;
                    let flags = parameters
                        .get(2)
                        .map(|flags| build_expression_evaluator(flags, context))
                        .transpose()?;
                    Rc::new(move |tuple| {
                        let ExpressionTerm::StringLiteral(pattern) = try_or_ok!(pattern(tuple)?)
                        else {
                            return Ok(None);
                        };
                        let options = if let Some(flags) = &flags {
                            let ExpressionTerm::StringLiteral(options) = try_or_ok!(flags(tuple)?)
                            else {
                                return Ok(None);
                            };
                            Some(options)
                        } else {
                            None
                        };
                        let regex = try_or_ok!(compile_pattern(&pattern, options.as_deref()));
                        let (text, _) =
                            try_or_ok!(to_string_and_language(try_or_ok!(text(tuple)?)));
                        Ok(Some(regex.is_match(&text).into()))
                    })
                }
            }
            #[cfg(feature = "sparql-12")]
            Function::Triple => {
                let s = build_expression_evaluator(&parameters[0], context)?;
                let p = build_expression_evaluator(&parameters[1], context)?;
                let o = build_expression_evaluator(&parameters[2], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(
                        try_or_ok!(ExpressionTriple::new(
                            try_or_ok!(s(tuple)?),
                            try_or_ok!(p(tuple)?),
                            try_or_ok!(o(tuple)?),
                        ))
                        .into(),
                    ))
                })
            }
            #[cfg(feature = "sparql-12")]
            Function::Subject => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(if let ExpressionTerm::Triple(t) = try_or_ok!(e(tuple)?) {
                        Some(t.subject.into())
                    } else {
                        None
                    })
                })
            }
            #[cfg(feature = "sparql-12")]
            Function::Predicate => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(if let ExpressionTerm::Triple(t) = try_or_ok!(e(tuple)?) {
                        Some(t.predicate.into())
                    } else {
                        None
                    })
                })
            }
            #[cfg(feature = "sparql-12")]
            Function::Object => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(if let ExpressionTerm::Triple(t) = try_or_ok!(e(tuple)?) {
                        Some(t.object)
                    } else {
                        None
                    })
                })
            }
            #[cfg(feature = "sparql-12")]
            Function::IsTriple => {
                let e = build_expression_evaluator(&parameters[0], context)?;
                Rc::new(move |tuple| {
                    Ok(Some(
                        matches!(try_or_ok!(e(tuple)?), ExpressionTerm::Triple(_)).into(),
                    ))
                })
            }
            Function::Custom(function_name) => {
                if let Some(function) = context.custom_functions().get(function_name).cloned() {
                    let args = parameters
                        .iter()
                        .map(|e| build_expression_evaluator(e, context))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Rc::new(move |tuple| {
                        let mut values = Vec::with_capacity(args.len());
                        for evaluator in &args {
                            values.push(try_or_ok!(evaluator(tuple)?).into());
                        }
                        Ok(function(&values).map(Into::into))
                    }));
                }

                macro_rules! cast_fn {
                    ($name:expr, $eval:expr) => {{
                        if *function_name == $name {
                            if parameters.len() != 1 {
                                return Err(
                                    ExpressionEvaluationError::UnsupportedCustomFunctionArity {
                                        name: function_name.clone(),
                                        expected: 1..=1,
                                        actual: parameters.len(),
                                    },
                                );
                            }
                            let e = build_expression_evaluator(&parameters[0], context)?;
                            return Ok(Rc::new(move |tuple| Ok(($eval)(try_or_ok!(e(tuple)?)))));
                        }
                    }};
                }

                cast_fn!(xsd::STRING, |t: ExpressionTerm| Some(
                    ExpressionTerm::StringLiteral(match t {
                        ExpressionTerm::NamedNode(term) => term.into_string(),
                        ExpressionTerm::BlankNode(_) => return None,
                        ExpressionTerm::StringLiteral(value)
                        | ExpressionTerm::LangStringLiteral { value, .. }
                        | ExpressionTerm::OtherTypedLiteral { value, .. } => value,
                        #[cfg(feature = "sparql-12")]
                        ExpressionTerm::DirLangStringLiteral { value, .. } => value,
                        ExpressionTerm::BooleanLiteral(value) => Literal::from(value).into_value(),
                        // TODO: avoid the intermediate allocation
                        ExpressionTerm::IntegerLiteral(value) => Literal::from(value).into_value(),
                        ExpressionTerm::DecimalLiteral(value) => Literal::from(value).into_value(),
                        ExpressionTerm::FloatLiteral(value) => {
                            // TODO: -0
                            if Float::from(0.000_001) <= value.abs()
                                && value.abs() < Float::from(1_000_000.)
                                || Float::from(-0.) <= value && value <= Float::from(0.)
                            {
                                OxString::new_owned(&f32::from(value).to_string())
                            } else {
                                Literal::from(value).into_value()
                            }
                        }
                        ExpressionTerm::DoubleLiteral(value) => {
                            // TODO: -0
                            if Double::from(0.000_001) <= value.abs()
                                && value.abs() < Double::from(1_000_000.)
                                || Double::from(-0.) <= value && value <= Double::from(0.)
                            {
                                OxString::new_owned(&f64::from(value).to_string())
                            } else {
                                Literal::from(value).into_value()
                            }
                        }
                        ExpressionTerm::DateTimeLiteral(value) => Literal::from(value).into_value(),
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DateLiteral(value) => Literal::from(value).into_value(),
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::TimeLiteral(value) => Literal::from(value).into_value(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GYearLiteral(value) => Literal::from(value).into_value(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GYearMonthLiteral(value) =>
                            Literal::from(value).into_value(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GMonthLiteral(value) => Literal::from(value).into_value(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GMonthDayLiteral(value) =>
                            Literal::from(value).into_value(),
                        #[cfg(feature = "calendar-ext")]
                        ExpressionTerm::GDayLiteral(value) => Literal::from(value).into_value(),
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DurationLiteral(value) => Literal::from(value).into_value(),
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::YearMonthDurationLiteral(value) =>
                            Literal::from(value).into_value(),
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DayTimeDurationLiteral(value) =>
                            Literal::from(value).into_value(),
                        #[cfg(feature = "sparql-12")]
                        ExpressionTerm::Triple(_) => return None,
                    })
                ));
                cast_fn!(xsd::BOOLEAN, |t: ExpressionTerm| Some(
                    ExpressionTerm::BooleanLiteral(match t {
                        ExpressionTerm::BooleanLiteral(value) => value,
                        ExpressionTerm::FloatLiteral(value) => value.into(),
                        ExpressionTerm::DoubleLiteral(value) => value.into(),
                        ExpressionTerm::IntegerLiteral(value) => value.into(),
                        ExpressionTerm::DecimalLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                cast_fn!(xsd::DOUBLE, |t: ExpressionTerm| Some(
                    ExpressionTerm::DoubleLiteral(match t {
                        ExpressionTerm::FloatLiteral(value) => value.into(),
                        ExpressionTerm::DoubleLiteral(value) => value,
                        ExpressionTerm::IntegerLiteral(value) => value.into(),
                        ExpressionTerm::DecimalLiteral(value) => value.into(),
                        ExpressionTerm::BooleanLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                cast_fn!(xsd::FLOAT, |t: ExpressionTerm| Some(
                    ExpressionTerm::FloatLiteral(match t {
                        ExpressionTerm::FloatLiteral(value) => value,
                        ExpressionTerm::DoubleLiteral(value) => value.into(),
                        ExpressionTerm::IntegerLiteral(value) => value.into(),
                        ExpressionTerm::DecimalLiteral(value) => value.into(),
                        ExpressionTerm::BooleanLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));

                cast_fn!(xsd::INTEGER, |t: ExpressionTerm| Some(
                    ExpressionTerm::IntegerLiteral(match t {
                        ExpressionTerm::FloatLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::DoubleLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::IntegerLiteral(value) => value,
                        ExpressionTerm::DecimalLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::BooleanLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                cast_fn!(xsd::DECIMAL, |t: ExpressionTerm| Some(
                    ExpressionTerm::DecimalLiteral(match t {
                        ExpressionTerm::FloatLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::DoubleLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::IntegerLiteral(value) => value.into(),
                        ExpressionTerm::DecimalLiteral(value) => value,
                        ExpressionTerm::BooleanLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "sep-0002")]
                cast_fn!(xsd::DATE, |t: ExpressionTerm| Some(
                    ExpressionTerm::DateLiteral(match t {
                        ExpressionTerm::DateLiteral(value) => value,
                        ExpressionTerm::DateTimeLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "sep-0002")]
                cast_fn!(xsd::TIME, |t: ExpressionTerm| Some(
                    ExpressionTerm::TimeLiteral(match t {
                        ExpressionTerm::TimeLiteral(value) => value,
                        ExpressionTerm::DateTimeLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                cast_fn!(xsd::DATE_TIME, |t: ExpressionTerm| Some(
                    ExpressionTerm::DateTimeLiteral(match t {
                        ExpressionTerm::DateTimeLiteral(value) => value,
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DateLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "sep-0002")]
                cast_fn!(xsd::DURATION, |t: ExpressionTerm| Some(
                    ExpressionTerm::DurationLiteral(match t {
                        ExpressionTerm::DurationLiteral(value) => value,
                        ExpressionTerm::YearMonthDurationLiteral(value) => value.into(),
                        ExpressionTerm::DayTimeDurationLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "sep-0002")]
                cast_fn!(xsd::YEAR_MONTH_DURATION, |t: ExpressionTerm| Some(
                    ExpressionTerm::YearMonthDurationLiteral(match t {
                        ExpressionTerm::DurationLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::YearMonthDurationLiteral(value) => value,
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "sep-0002")]
                cast_fn!(xsd::DAY_TIME_DURATION, |t: ExpressionTerm| Some(
                    ExpressionTerm::DayTimeDurationLiteral(match t {
                        ExpressionTerm::DurationLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::DayTimeDurationLiteral(value) => value,
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "calendar-ext")]
                cast_fn!(xsd::G_YEAR, |t: ExpressionTerm| Some(
                    ExpressionTerm::GYearLiteral(match t {
                        ExpressionTerm::GYearLiteral(value) => value,
                        ExpressionTerm::GYearMonthLiteral(value) => {
                            value.try_into().ok()?
                        }
                        ExpressionTerm::DateLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::DateTimeLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "calendar-ext")]
                cast_fn!(xsd::G_YEAR_MONTH, |t: ExpressionTerm| Some(
                    ExpressionTerm::GYearMonthLiteral(match t {
                        ExpressionTerm::GYearMonthLiteral(value) => value,
                        ExpressionTerm::DateLiteral(value) => value.into(),
                        ExpressionTerm::DateTimeLiteral(value) => value.try_into().ok()?,
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "calendar-ext")]
                cast_fn!(xsd::G_MONTH, |t: ExpressionTerm| Some(
                    ExpressionTerm::GMonthLiteral(match t {
                        ExpressionTerm::GMonthLiteral(value) => value,
                        ExpressionTerm::GYearMonthLiteral(value) => value.into(),
                        ExpressionTerm::GMonthDayLiteral(value) => value.into(),
                        ExpressionTerm::DateLiteral(value) => value.into(),
                        ExpressionTerm::DateTimeLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "calendar-ext")]
                cast_fn!(xsd::G_MONTH_DAY, |t: ExpressionTerm| Some(
                    ExpressionTerm::GMonthDayLiteral(match t {
                        ExpressionTerm::GMonthDayLiteral(value) => value,
                        ExpressionTerm::DateLiteral(value) => value.into(),
                        ExpressionTerm::DateTimeLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                #[cfg(feature = "calendar-ext")]
                cast_fn!(xsd::G_DAY, |t: ExpressionTerm| Some(
                    ExpressionTerm::GDayLiteral(match t {
                        ExpressionTerm::GDayLiteral(value) => value,
                        ExpressionTerm::GMonthDayLiteral(value) => value.into(),
                        ExpressionTerm::DateLiteral(value) => value.into(),
                        ExpressionTerm::DateTimeLiteral(value) => value.into(),
                        ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                        _ => return None,
                    })
                ));
                return Err(ExpressionEvaluationError::UnsupportedCustomFunction(
                    function_name.clone(),
                ));
            }
        },
    })
}

/// Evaluates an expression and returns an internal term
///
/// Returns None if building such expression implies to convert into an [`ExpressionTerm`] then back.
pub fn try_build_internal_expression_evaluator<'a, C: ExpressionEvaluatorContext<'a>>(
    expression: &Expression,
    context: &mut C,
) -> Result<
    Option<ExpressionEvaluator<'a, C::Tuple, C::Term, C::Error>>,
    ExpressionEvaluationError<C::Error>,
>
where
    C::Error: 'a,
{
    Ok(Some(match expression {
        Expression::NamedNode(t) => {
            let t = context
                .internalize_named_node(t)
                .map_err(ExpressionEvaluationError::Context)?;
            Rc::new(move |_| Ok(Some(t.clone())))
        }
        Expression::Literal(t) => {
            let t = context
                .internalize_literal(t)
                .map_err(ExpressionEvaluationError::Context)?;
            Rc::new(move |_| Ok(Some(t.clone())))
        }
        Expression::Variable(v) => {
            let lookup = context.build_variable_lookup(v);
            Rc::new(move |tuple| Ok(lookup(tuple)))
        }
        Expression::Coalesce(l) => {
            let Some(l) = l
                .iter()
                .map(|e| try_build_internal_expression_evaluator(e, context))
                .collect::<Result<Option<Vec<_>>, _>>()?
            else {
                return Ok(None);
            };
            Rc::new(move |tuple| {
                for e in &l {
                    if let Some(result) = e(tuple)? {
                        return Ok(Some(result));
                    }
                }
                Ok(None)
            })
        }
        Expression::If(a, b, c) => {
            let a = build_expression_evaluator(a, context)?;
            let Some(b) = try_build_internal_expression_evaluator(b, context)? else {
                return Ok(None);
            };
            let Some(c) = try_build_internal_expression_evaluator(c, context)? else {
                return Ok(None);
            };
            Rc::new(move |tuple| {
                if try_or_ok!(try_or_ok!(a(tuple)?).effective_boolean_value()) {
                    b(tuple)
                } else {
                    c(tuple)
                }
            })
        }
        _ => return Ok(None),
    }))
}

fn build_hash_expression_evaluator<'a, C: ExpressionEvaluatorContext<'a>, H: Digest>(
    parameters: &[Expression],
    context: &mut C,
) -> Result<
    ExpressionEvaluator<'a, C::Tuple, ExpressionTerm, C::Error>,
    ExpressionEvaluationError<C::Error>,
>
where
    C::Error: 'a,
{
    let arg = build_expression_evaluator(&parameters[0], context)?;
    Ok(Rc::new(move |tuple| {
        let ExpressionTerm::StringLiteral(input) = try_or_ok!(arg(tuple)?) else {
            return Ok(None);
        };
        let hash = hex::encode(H::new().chain_update(input.as_str()).finalize());
        Ok(Some(ExpressionTerm::StringLiteral(OxString::new_owned(
            &hash,
        ))))
    }))
}

#[cfg(feature = "sparql-12")]
type LanguageWithMaybeBaseDirection = (OxString, Option<BaseDirection>);
#[cfg(not(feature = "sparql-12"))]
type LanguageWithMaybeBaseDirection = OxString;

#[cfg(feature = "sparql-12")]
fn to_string_and_language(
    term: ExpressionTerm,
) -> Option<(OxString, Option<LanguageWithMaybeBaseDirection>)> {
    match term {
        ExpressionTerm::StringLiteral(value) => Some((value, None)),
        ExpressionTerm::LangStringLiteral { value, language } => {
            Some((value, Some((language, None))))
        }
        ExpressionTerm::DirLangStringLiteral {
            value,
            language,
            direction,
        } => Some((value, Some((language, Some(direction))))),
        _ => None,
    }
}

#[cfg(not(feature = "sparql-12"))]
fn to_string_and_language(
    term: ExpressionTerm,
) -> Option<(OxString, Option<LanguageWithMaybeBaseDirection>)> {
    match term {
        ExpressionTerm::StringLiteral(value) => Some((value, None)),
        ExpressionTerm::LangStringLiteral { value, language } => Some((value, Some(language))),
        _ => None,
    }
}

#[cfg(feature = "sparql-12")]
fn build_plain_literal(
    value: OxString,
    language: Option<LanguageWithMaybeBaseDirection>,
) -> ExpressionTerm {
    if let Some((language, direction)) = language {
        if let Some(direction) = direction {
            ExpressionTerm::DirLangStringLiteral {
                value,
                language,
                direction,
            }
        } else {
            ExpressionTerm::LangStringLiteral { value, language }
        }
    } else {
        ExpressionTerm::StringLiteral(value)
    }
}

#[cfg(not(feature = "sparql-12"))]
fn build_plain_literal(
    value: OxString,
    language: Option<LanguageWithMaybeBaseDirection>,
) -> ExpressionTerm {
    if let Some(language) = language {
        ExpressionTerm::LangStringLiteral { value, language }
    } else {
        ExpressionTerm::StringLiteral(value)
    }
}

fn to_argument_compatible_strings(
    arg1: ExpressionTerm,
    arg2: ExpressionTerm,
) -> Option<(OxString, OxString, Option<LanguageWithMaybeBaseDirection>)> {
    let (value1, language1) = to_string_and_language(arg1)?;
    let (value2, language2) = to_string_and_language(arg2)?;
    (language2.is_none() || language1 == language2).then_some((value1, value2, language1))
}

fn compile_static_pattern_if_exists(
    pattern: &Expression,
    options: Option<&Expression>,
) -> Option<Regex> {
    let static_pattern = if let Expression::Literal(pattern) = pattern {
        (*pattern.datatype() == xsd::STRING).then(|| pattern.value())
    } else {
        None
    };
    let static_options = if let Some(options) = options {
        if let Expression::Literal(options) = options {
            (*options.datatype() == xsd::STRING).then(|| Some(options.value()))
        } else {
            None
        }
    } else {
        Some(None)
    };
    if let (Some(static_pattern), Some(static_options)) = (static_pattern, static_options) {
        compile_pattern(static_pattern, static_options)
    } else {
        None
    }
}

fn compile_pattern(pattern: &str, flags: Option<&str>) -> Option<Regex> {
    let mut pattern = Cow::Borrowed(pattern);
    let flags = flags.unwrap_or_default();
    if flags.contains('q') {
        pattern = regex::escape(&pattern).into();
    }
    let mut regex_builder = RegexBuilder::new(&pattern);
    regex_builder.size_limit(REGEX_SIZE_LIMIT);
    for flag in flags.chars() {
        match flag {
            's' => {
                regex_builder.dot_matches_new_line(true);
            }
            'm' => {
                regex_builder.multi_line(true);
            }
            'i' => {
                regex_builder.case_insensitive(true);
            }
            'x' => {
                regex_builder.ignore_whitespace(true);
            }
            'q' => (),        // Already supported
            _ => return None, // invalid option
        }
    }
    regex_builder.build().ok()
}

/// Equality operator (=)
fn equals(a: &ExpressionTerm, b: &ExpressionTerm) -> Option<bool> {
    match a {
        ExpressionTerm::NamedNode(_)
        | ExpressionTerm::BlankNode(_)
        | ExpressionTerm::LangStringLiteral { .. } => Some(a == b),
        #[cfg(feature = "sparql-12")]
        ExpressionTerm::DirLangStringLiteral { .. } => Some(a == b),
        ExpressionTerm::StringLiteral(a) => match b {
            ExpressionTerm::StringLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::OtherTypedLiteral { .. } => match b {
            ExpressionTerm::OtherTypedLiteral { .. } if a == b => Some(true),
            ExpressionTerm::NamedNode(_)
            | ExpressionTerm::BlankNode(_)
            | ExpressionTerm::LangStringLiteral { .. } => Some(false),
            #[cfg(feature = "sparql-12")]
            ExpressionTerm::DirLangStringLiteral { .. } => Some(false),
            #[cfg(feature = "sparql-12")]
            ExpressionTerm::Triple(_) => Some(false),
            _ => None,
        },
        ExpressionTerm::BooleanLiteral(a) => match b {
            ExpressionTerm::BooleanLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::FloatLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Some(a == b),
            ExpressionTerm::DoubleLiteral(b) => Some(Double::from(*a) == *b),
            ExpressionTerm::IntegerLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::DecimalLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::DoubleLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::DoubleLiteral(b) => Some(a == b),
            ExpressionTerm::IntegerLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::DecimalLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::IntegerLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Some(Float::from(*a) == *b),
            ExpressionTerm::DoubleLiteral(b) => Some(Double::from(*a) == *b),
            ExpressionTerm::IntegerLiteral(b) => Some(a == b),
            ExpressionTerm::DecimalLiteral(b) => Some(Decimal::from(*a) == *b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::DecimalLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Some(Float::from(*a) == *b),
            ExpressionTerm::DoubleLiteral(b) => Some(Double::from(*a) == *b),
            ExpressionTerm::IntegerLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::DecimalLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::DateTimeLiteral(a) => match b {
            ExpressionTerm::DateTimeLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::TimeLiteral(a) => match b {
            ExpressionTerm::TimeLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DateLiteral(a) => match b {
            ExpressionTerm::DateLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearMonthLiteral(a) => match b {
            ExpressionTerm::GYearMonthLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearLiteral(a) => match b {
            ExpressionTerm::GYearLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthDayLiteral(a) => match b {
            ExpressionTerm::GMonthDayLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GDayLiteral(a) => match b {
            ExpressionTerm::GDayLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthLiteral(a) => match b {
            ExpressionTerm::GMonthLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => Some(a == b),
            ExpressionTerm::YearMonthDurationLiteral(b) => Some(a == b),
            ExpressionTerm::DayTimeDurationLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::YearMonthDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => Some(a == b),
            ExpressionTerm::YearMonthDurationLiteral(b) => Some(a == b),
            ExpressionTerm::DayTimeDurationLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DayTimeDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => Some(a == b),
            ExpressionTerm::YearMonthDurationLiteral(b) => Some(a == b),
            ExpressionTerm::DayTimeDurationLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sparql-12")]
        ExpressionTerm::Triple(a) => {
            if let ExpressionTerm::Triple(b) = b {
                triple_equals(a, b)
            } else {
                Some(false)
            }
        }
    }
}

#[cfg(feature = "sparql-12")]
fn triple_equals(a: &ExpressionTriple, b: &ExpressionTriple) -> Option<bool> {
    Some(a.subject == b.subject && a.predicate == b.predicate && equals(&a.object, &b.object)?)
}

/// Comparison for <, >, <= and >= operators
pub(crate) fn partial_cmp(a: &ExpressionTerm, b: &ExpressionTerm) -> Option<Ordering> {
    if a == b {
        return Some(Ordering::Equal);
    }
    match a {
        ExpressionTerm::StringLiteral(a) => {
            if let ExpressionTerm::StringLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        ExpressionTerm::LangStringLiteral {
            value: va,
            language: la,
        } => {
            if let ExpressionTerm::LangStringLiteral {
                value: vb,
                language: lb,
            } = b
            {
                if la == lb { va.partial_cmp(vb) } else { None }
            } else {
                None
            }
        }
        #[cfg(feature = "sparql-12")]
        ExpressionTerm::DirLangStringLiteral {
            value: va,
            language: la,
            direction: da,
        } => {
            if let ExpressionTerm::DirLangStringLiteral {
                value: vb,
                language: lb,
                direction: db,
            } = b
            {
                if la == lb && da == db {
                    va.partial_cmp(vb)
                } else {
                    None
                }
            } else {
                None
            }
        }
        ExpressionTerm::FloatLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DoubleLiteral(b) => Double::from(*a).partial_cmp(b),
            ExpressionTerm::IntegerLiteral(b) => a.partial_cmp(&Float::from(*b)),
            ExpressionTerm::DecimalLiteral(b) => a.partial_cmp(&(*b).into()),
            _ => None,
        },
        ExpressionTerm::DoubleLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => a.partial_cmp(&(*b).into()),
            ExpressionTerm::DoubleLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::IntegerLiteral(b) => a.partial_cmp(&Double::from(*b)),
            ExpressionTerm::DecimalLiteral(b) => a.partial_cmp(&(*b).into()),
            _ => None,
        },
        ExpressionTerm::IntegerLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Float::from(*a).partial_cmp(b),
            ExpressionTerm::DoubleLiteral(b) => Double::from(*a).partial_cmp(b),
            ExpressionTerm::IntegerLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DecimalLiteral(b) => Decimal::from(*a).partial_cmp(b),
            _ => None,
        },
        ExpressionTerm::DecimalLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Float::from(*a).partial_cmp(b),
            ExpressionTerm::DoubleLiteral(b) => Double::from(*a).partial_cmp(b),
            ExpressionTerm::IntegerLiteral(b) => a.partial_cmp(&Decimal::from(*b)),
            ExpressionTerm::DecimalLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        ExpressionTerm::DateTimeLiteral(a) => {
            if let ExpressionTerm::DateTimeLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::TimeLiteral(a) => {
            if let ExpressionTerm::TimeLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DateLiteral(a) => {
            if let ExpressionTerm::DateLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearMonthLiteral(a) => {
            if let ExpressionTerm::GYearMonthLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearLiteral(a) => {
            if let ExpressionTerm::GYearLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthDayLiteral(a) => {
            if let ExpressionTerm::GMonthDayLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GDayLiteral(a) => {
            if let ExpressionTerm::GDayLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthLiteral(a) => {
            if let ExpressionTerm::GMonthLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::YearMonthDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DayTimeDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        _ => None,
    }
}

pub enum NumericBinaryOperands {
    Float(Float, Float),
    Double(Double, Double),
    Integer(Integer, Integer),
    Decimal(Decimal, Decimal),
    #[cfg(feature = "sep-0002")]
    Duration(Duration, Duration),
    #[cfg(feature = "sep-0002")]
    YearMonthDuration(YearMonthDuration, YearMonthDuration),
    #[cfg(feature = "sep-0002")]
    DayTimeDuration(DayTimeDuration, DayTimeDuration),
    #[cfg(feature = "sep-0002")]
    DateTime(DateTime, DateTime),
    #[cfg(feature = "sep-0002")]
    Time(Time, Time),
    #[cfg(feature = "sep-0002")]
    Date(Date, Date),
    #[cfg(feature = "sep-0002")]
    DateTimeDuration(DateTime, Duration),
    #[cfg(feature = "sep-0002")]
    DateTimeYearMonthDuration(DateTime, YearMonthDuration),
    #[cfg(feature = "sep-0002")]
    DateTimeDayTimeDuration(DateTime, DayTimeDuration),
    #[cfg(feature = "sep-0002")]
    DateDuration(Date, Duration),
    #[cfg(feature = "sep-0002")]
    DateYearMonthDuration(Date, YearMonthDuration),
    #[cfg(feature = "sep-0002")]
    DateDayTimeDuration(Date, DayTimeDuration),
    #[cfg(feature = "sep-0002")]
    TimeDuration(Time, Duration),
    #[cfg(feature = "sep-0002")]
    TimeDayTimeDuration(Time, DayTimeDuration),
}

impl NumericBinaryOperands {
    pub fn new(a: ExpressionTerm, b: ExpressionTerm) -> Option<Self> {
        match (a, b) {
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1, v2))
            }
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Float(v1, v2.into()))
            }
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Float(v1, v2.into()))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1, v2))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1.into(), v2))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Integer(v1, v2))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal(v1.into(), v2))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1.into(), v2))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Decimal(v1, v2.into()))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DurationLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DurationLiteral(v1), ExpressionTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2.into()))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DurationLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2.into()))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::YearMonthDurationLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1.into(), v2))
            }
            #[cfg(feature = "sep-0002")]
            (
                ExpressionTerm::YearMonthDurationLiteral(v1),
                ExpressionTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::YearMonthDuration(v1, v2)),
            #[cfg(feature = "sep-0002")]
            (
                ExpressionTerm::YearMonthDurationLiteral(v1),
                ExpressionTerm::DayTimeDurationLiteral(v2),
            ) => Some(Self::Duration(v1.into(), v2.into())),
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DayTimeDurationLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1.into(), v2))
            }
            #[cfg(feature = "sep-0002")]
            (
                ExpressionTerm::DayTimeDurationLiteral(v1),
                ExpressionTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::Duration(v1.into(), v2.into())),
            #[cfg(feature = "sep-0002")]
            (
                ExpressionTerm::DayTimeDurationLiteral(v1),
                ExpressionTerm::DayTimeDurationLiteral(v2),
            ) => Some(Self::DayTimeDuration(v1, v2)),
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::DateTimeLiteral(v2)) => {
                Some(Self::DateTime(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::DateLiteral(v2)) => {
                Some(Self::Date(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::TimeLiteral(v1), ExpressionTerm::TimeLiteral(v2)) => {
                Some(Self::Time(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::DateTimeDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::DateTimeYearMonthDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::DateTimeDayTimeDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::DateDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::DateYearMonthDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::DateDayTimeDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::TimeLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::TimeDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::TimeLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::TimeDayTimeDuration(v1, v2))
            }
            _ => None,
        }
    }
}

struct ZipLongest<T1, T2, I1: Iterator<Item = T1>, I2: Iterator<Item = T2>> {
    a: I1,
    b: I2,
}

impl<T1, T2, I1: Iterator<Item = T1>, I2: Iterator<Item = T2>> ZipLongest<T1, T2, I1, I2> {
    fn new(a: I1, b: I2) -> Self {
        Self { a, b }
    }
}

impl<T1, T2, I1: Iterator<Item = T1>, I2: Iterator<Item = T2>> Iterator
    for ZipLongest<T1, T2, I1, I2>
{
    type Item = (Option<T1>, Option<T2>);

    fn next(&mut self) -> Option<Self::Item> {
        match (self.a.next(), self.b.next()) {
            (None, None) => None,
            r => Some(r),
        }
    }
}

fn generate_uuid(buffer: &mut String) {
    let mut uuid = random::<u128>().to_le_bytes();
    uuid[6] = (uuid[6] & 0x0F) | 0x40;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;

    write_hexa_bytes(&uuid[0..4], buffer);
    buffer.push('-');
    write_hexa_bytes(&uuid[4..6], buffer);
    buffer.push('-');
    write_hexa_bytes(&uuid[6..8], buffer);
    buffer.push('-');
    write_hexa_bytes(&uuid[8..10], buffer);
    buffer.push('-');
    write_hexa_bytes(&uuid[10..16], buffer);
}

fn write_hexa_bytes(bytes: &[u8], buffer: &mut String) {
    for b in bytes {
        let high = b / 16;
        buffer.push(char::from(if high < 10 {
            b'0' + high
        } else {
            b'a' + (high - 10)
        }));
        let low = b % 16;
        buffer.push(char::from(if low < 10 {
            b'0' + low
        } else {
            b'a' + (low - 10)
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid() {
        let mut buffer = String::default();
        generate_uuid(&mut buffer);
        assert!(
            Regex::new("^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
                .unwrap()
                .is_match(&buffer),
            "{buffer} is not a valid UUID"
        );
    }
}
