mod ebv;
mod order;
mod to_rdf_literal;
mod utils;

#[cfg(feature = "rdf-12")]
use crate::model::NamedOrBlankNode;
use crate::model::Term;
use crate::sparql::datafusion::function::utils::{
    TermAccumulator, boolean_function, term_aggregate_function, term_function,
};
use crate::sparql::dataset::DatasetView;
use datafusion::common::{Result, internal_err};
use datafusion::logical_expr::{Expr, Volatility};
pub use ebv::effective_boolean_value;
pub use order::order_by_collation;
use oxsdatatypes::{
    Date, DateTime, DayTimeDuration, Decimal, Double, Duration, Float, Integer, Time,
    YearMonthDuration,
};
use regex::{Regex, RegexBuilder};
use spareval::ExpressionTerm;
#[cfg(feature = "rdf-12")]
use spareval::ExpressionTriple;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::sync::Arc;
pub use to_rdf_literal::to_rdf_literal;

const REGEX_SIZE_LIMIT: usize = 1_000_000;

pub fn term_equals(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    boolean_function(
        dataset,
        [left, right],
        "sparql:equals",
        |[left, right]| equals(&left, &right),
        Volatility::Immutable,
    )
}

fn equals(a: &ExpressionTerm, b: &ExpressionTerm) -> Option<bool> {
    match a {
        ExpressionTerm::NamedNode(_)
        | ExpressionTerm::BlankNode(_)
        | ExpressionTerm::LangStringLiteral { .. } => Some(a == b),
        #[cfg(feature = "rdf-12")]
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
            #[cfg(feature = "rdf-12")]
            ExpressionTerm::DirLangStringLiteral { .. } => Some(false),
            #[cfg(feature = "rdf-12")]
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
        ExpressionTerm::TimeLiteral(a) => match b {
            ExpressionTerm::TimeLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::DateLiteral(a) => match b {
            ExpressionTerm::DateLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::GYearMonthLiteral(a) => match b {
            ExpressionTerm::GYearMonthLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::GYearLiteral(a) => match b {
            ExpressionTerm::GYearLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::GMonthDayLiteral(a) => match b {
            ExpressionTerm::GMonthDayLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::GDayLiteral(a) => match b {
            ExpressionTerm::GDayLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::GMonthLiteral(a) => match b {
            ExpressionTerm::GMonthLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::DurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => Some(a == b),
            ExpressionTerm::YearMonthDurationLiteral(b) => Some(a == b),
            ExpressionTerm::DayTimeDurationLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::YearMonthDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => Some(a == b),
            ExpressionTerm::YearMonthDurationLiteral(b) => Some(a == b),
            ExpressionTerm::DayTimeDurationLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::DayTimeDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => Some(a == b),
            ExpressionTerm::YearMonthDurationLiteral(b) => Some(a == b),
            ExpressionTerm::DayTimeDurationLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "rdf-12")]
        ExpressionTerm::Triple(a) => {
            if let ExpressionTerm::Triple(b) = b {
                triple_equals(a, b)
            } else {
                Some(false)
            }
        }
    }
}

#[cfg(feature = "rdf-12")]
fn triple_equals(a: &ExpressionTriple, b: &ExpressionTriple) -> Option<bool> {
    Some(a.subject == b.subject && a.predicate == b.predicate && equals(&a.object, &b.object)?)
}

pub fn greater_than(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    boolean_function(
        dataset,
        [left, right],
        "sparql:greater-than",
        |[left, right]| Some(partial_cmp(&left, &right)?.is_gt()),
        Volatility::Immutable,
    )
}

pub fn less_than(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    boolean_function(
        dataset,
        [left, right],
        "sparql:less-than",
        |[left, right]| Some(partial_cmp(&left, &right)?.is_lt()),
        Volatility::Immutable,
    )
}

pub fn greater_than_or_equal(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    boolean_function(
        dataset,
        [left, right],
        "sparql:greater-than-or-equal",
        |[left, right]| Some(partial_cmp(&left, &right)?.is_ge()),
        Volatility::Immutable,
    )
}

pub fn less_than_or_equal(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    boolean_function(
        dataset,
        [left, right],
        "sparql:greater-than-or-equal",
        |[left, right]| Some(partial_cmp(&left, &right)?.is_le()),
        Volatility::Immutable,
    )
}

fn partial_cmp(a: &ExpressionTerm, b: &ExpressionTerm) -> Option<Ordering> {
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
        #[cfg(feature = "rdf-12")]
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
        ExpressionTerm::TimeLiteral(a) => {
            if let ExpressionTerm::TimeLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        ExpressionTerm::DateLiteral(a) => {
            if let ExpressionTerm::DateLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        ExpressionTerm::GYearMonthLiteral(a) => {
            if let ExpressionTerm::GYearMonthLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        ExpressionTerm::GYearLiteral(a) => {
            if let ExpressionTerm::GYearLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        ExpressionTerm::GMonthDayLiteral(a) => {
            if let ExpressionTerm::GMonthDayLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        ExpressionTerm::GDayLiteral(a) => {
            if let ExpressionTerm::GDayLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        ExpressionTerm::GMonthLiteral(a) => {
            if let ExpressionTerm::GMonthLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        ExpressionTerm::DurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        ExpressionTerm::YearMonthDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        ExpressionTerm::DayTimeDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        _ => None,
    }
}

/// Comparison for ordering
fn cmp_terms(a: &ExpressionTerm, b: &ExpressionTerm) -> Ordering {
    match a {
        ExpressionTerm::BlankNode(a) => match b {
            ExpressionTerm::BlankNode(b) => a.as_str().cmp(b.as_str()),
            _ => Ordering::Less,
        },
        ExpressionTerm::NamedNode(a) => match b {
            ExpressionTerm::BlankNode(_) => Ordering::Greater,
            ExpressionTerm::NamedNode(b) => a.as_str().cmp(b.as_str()),
            _ => Ordering::Less,
        },
        #[cfg(feature = "rdf-12")]
        ExpressionTerm::Triple(a) => match b {
            ExpressionTerm::Triple(b) => match match &a.subject {
                NamedOrBlankNode::BlankNode(a) => match &b.subject {
                    NamedOrBlankNode::BlankNode(b) => a.as_str().cmp(b.as_str()),
                    NamedOrBlankNode::NamedNode(_) => Ordering::Less,
                },
                NamedOrBlankNode::NamedNode(a) => match &b.subject {
                    NamedOrBlankNode::BlankNode(_) => Ordering::Greater,
                    NamedOrBlankNode::NamedNode(b) => a.as_str().cmp(b.as_str()),
                },
            } {
                Ordering::Equal => match a.predicate.as_str().cmp(b.predicate.as_str()) {
                    Ordering::Equal => cmp_terms(&a.object, &b.object),
                    o => o,
                },
                o => o,
            },
            _ => Ordering::Greater,
        },
        _ => match b {
            ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => Ordering::Greater,
            #[cfg(feature = "rdf-12")]
            ExpressionTerm::Triple(_) => Ordering::Less,
            _ => {
                if let Some(ord) = partial_cmp(a, b) {
                    ord
                } else if let (Term::Literal(a), Term::Literal(b)) =
                    (a.clone().into(), b.clone().into())
                {
                    (a.value(), a.datatype(), a.language()).cmp(&(
                        b.value(),
                        b.datatype(),
                        b.language(),
                    ))
                } else {
                    Ordering::Equal // Should never happen
                }
            }
        },
    }
}

pub fn plus(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    term_function(
        dataset,
        [left, right],
        "sparql:plus",
        |[left, right]| do_add(left, right),
        Volatility::Immutable,
    )
}

fn do_add(left: ExpressionTerm, right: ExpressionTerm) -> Option<ExpressionTerm> {
    Some(match NumericBinaryOperands::new(left, right)? {
        NumericBinaryOperands::Float(v1, v2) => ExpressionTerm::FloatLiteral(v1 + v2),
        NumericBinaryOperands::Double(v1, v2) => ExpressionTerm::DoubleLiteral(v1 + v2),
        NumericBinaryOperands::Integer(v1, v2) => {
            ExpressionTerm::IntegerLiteral(v1.checked_add(v2)?)
        }
        NumericBinaryOperands::Decimal(v1, v2) => {
            ExpressionTerm::DecimalLiteral(v1.checked_add(v2)?)
        }
        NumericBinaryOperands::Duration(v1, v2) => {
            ExpressionTerm::DurationLiteral(v1.checked_add(v2)?)
        }
        NumericBinaryOperands::YearMonthDuration(v1, v2) => {
            ExpressionTerm::YearMonthDurationLiteral(v1.checked_add(v2)?)
        }
        NumericBinaryOperands::DayTimeDuration(v1, v2) => {
            ExpressionTerm::DayTimeDurationLiteral(v1.checked_add(v2)?)
        }
        NumericBinaryOperands::DateTimeDuration(v1, v2) => {
            ExpressionTerm::DateTimeLiteral(v1.checked_add_duration(v2)?)
        }
        NumericBinaryOperands::DateTimeYearMonthDuration(v1, v2) => {
            ExpressionTerm::DateTimeLiteral(v1.checked_add_year_month_duration(v2)?)
        }
        NumericBinaryOperands::DateTimeDayTimeDuration(v1, v2) => {
            ExpressionTerm::DateTimeLiteral(v1.checked_add_day_time_duration(v2)?)
        }
        NumericBinaryOperands::DateDuration(v1, v2) => {
            ExpressionTerm::DateLiteral(v1.checked_add_duration(v2)?)
        }
        NumericBinaryOperands::DateYearMonthDuration(v1, v2) => {
            ExpressionTerm::DateLiteral(v1.checked_add_year_month_duration(v2)?)
        }
        NumericBinaryOperands::DateDayTimeDuration(v1, v2) => {
            ExpressionTerm::DateLiteral(v1.checked_add_day_time_duration(v2)?)
        }
        NumericBinaryOperands::TimeDuration(v1, v2) => {
            ExpressionTerm::TimeLiteral(v1.checked_add_duration(v2)?)
        }
        NumericBinaryOperands::TimeDayTimeDuration(v1, v2) => {
            ExpressionTerm::TimeLiteral(v1.checked_add_day_time_duration(v2)?)
        }
        NumericBinaryOperands::DateTime(_, _)
        | NumericBinaryOperands::Time(_, _)
        | NumericBinaryOperands::Date(_, _) => return None,
    })
}

pub fn subtract(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    term_function(
        dataset,
        [left, right],
        "sparql:subtract",
        |[left, right]| do_subtract(left, right),
        Volatility::Immutable,
    )
}

fn do_subtract(left: ExpressionTerm, right: ExpressionTerm) -> Option<ExpressionTerm> {
    Some(match NumericBinaryOperands::new(left, right)? {
        NumericBinaryOperands::Float(v1, v2) => ExpressionTerm::FloatLiteral(v1 - v2),
        NumericBinaryOperands::Double(v1, v2) => ExpressionTerm::DoubleLiteral(v1 - v2),
        NumericBinaryOperands::Integer(v1, v2) => {
            ExpressionTerm::IntegerLiteral(v1.checked_sub(v2)?)
        }
        NumericBinaryOperands::Decimal(v1, v2) => {
            ExpressionTerm::DecimalLiteral(v1.checked_sub(v2)?)
        }
        NumericBinaryOperands::DateTime(v1, v2) => {
            ExpressionTerm::DayTimeDurationLiteral(v1.checked_sub(v2)?)
        }
        NumericBinaryOperands::Date(v1, v2) => {
            ExpressionTerm::DayTimeDurationLiteral(v1.checked_sub(v2)?)
        }
        NumericBinaryOperands::Time(v1, v2) => {
            ExpressionTerm::DayTimeDurationLiteral(v1.checked_sub(v2)?)
        }
        NumericBinaryOperands::Duration(v1, v2) => {
            ExpressionTerm::DurationLiteral(v1.checked_sub(v2)?)
        }
        NumericBinaryOperands::YearMonthDuration(v1, v2) => {
            ExpressionTerm::YearMonthDurationLiteral(v1.checked_sub(v2)?)
        }
        NumericBinaryOperands::DayTimeDuration(v1, v2) => {
            ExpressionTerm::DayTimeDurationLiteral(v1.checked_sub(v2)?)
        }
        NumericBinaryOperands::DateTimeDuration(v1, v2) => {
            ExpressionTerm::DateTimeLiteral(v1.checked_sub_duration(v2)?)
        }
        NumericBinaryOperands::DateTimeYearMonthDuration(v1, v2) => {
            ExpressionTerm::DateTimeLiteral(v1.checked_sub_year_month_duration(v2)?)
        }
        NumericBinaryOperands::DateTimeDayTimeDuration(v1, v2) => {
            ExpressionTerm::DateTimeLiteral(v1.checked_sub_day_time_duration(v2)?)
        }
        NumericBinaryOperands::DateDuration(v1, v2) => {
            ExpressionTerm::DateLiteral(v1.checked_sub_duration(v2)?)
        }
        NumericBinaryOperands::DateYearMonthDuration(v1, v2) => {
            ExpressionTerm::DateLiteral(v1.checked_sub_year_month_duration(v2)?)
        }
        NumericBinaryOperands::DateDayTimeDuration(v1, v2) => {
            ExpressionTerm::DateLiteral(v1.checked_sub_day_time_duration(v2)?)
        }
        NumericBinaryOperands::TimeDuration(v1, v2) => {
            ExpressionTerm::TimeLiteral(v1.checked_sub_duration(v2)?)
        }
        NumericBinaryOperands::TimeDayTimeDuration(v1, v2) => {
            ExpressionTerm::TimeLiteral(v1.checked_sub_day_time_duration(v2)?)
        }
    })
}

pub fn multiply(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    term_function(
        dataset,
        [left, right],
        "sparql:multiply",
        |[left, right]| do_multiply(left, right),
        Volatility::Immutable,
    )
}

fn do_multiply(left: ExpressionTerm, right: ExpressionTerm) -> Option<ExpressionTerm> {
    Some(match NumericBinaryOperands::new(left, right)? {
        NumericBinaryOperands::Float(v1, v2) => ExpressionTerm::FloatLiteral(v1 * v2),
        NumericBinaryOperands::Double(v1, v2) => ExpressionTerm::DoubleLiteral(v1 * v2),
        NumericBinaryOperands::Integer(v1, v2) => {
            ExpressionTerm::IntegerLiteral(v1.checked_mul(v2)?)
        }
        NumericBinaryOperands::Decimal(v1, v2) => {
            ExpressionTerm::DecimalLiteral(v1.checked_mul(v2)?)
        }
        _ => return None,
    })
}

pub fn divide(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    term_function(
        dataset,
        [left, right],
        "sparql:divide",
        |[left, right]| do_divide(left, right),
        Volatility::Immutable,
    )
}

fn do_divide(left: ExpressionTerm, right: ExpressionTerm) -> Option<ExpressionTerm> {
    Some(match NumericBinaryOperands::new(left, right)? {
        NumericBinaryOperands::Float(v1, v2) => ExpressionTerm::FloatLiteral(v1 / v2),
        NumericBinaryOperands::Double(v1, v2) => ExpressionTerm::DoubleLiteral(v1 / v2),
        NumericBinaryOperands::Integer(v1, v2) => {
            ExpressionTerm::DecimalLiteral(Decimal::from(v1).checked_div(v2)?)
        }
        NumericBinaryOperands::Decimal(v1, v2) => {
            ExpressionTerm::DecimalLiteral(v1.checked_div(v2)?)
        }
        _ => return None,
    })
}

enum NumericBinaryOperands {
    Float(Float, Float),
    Double(Double, Double),
    Integer(Integer, Integer),
    Decimal(Decimal, Decimal),
    Duration(Duration, Duration),
    YearMonthDuration(YearMonthDuration, YearMonthDuration),
    DayTimeDuration(DayTimeDuration, DayTimeDuration),
    DateTime(DateTime, DateTime),
    Time(Time, Time),
    Date(Date, Date),
    DateTimeDuration(DateTime, Duration),
    DateTimeYearMonthDuration(DateTime, YearMonthDuration),
    DateTimeDayTimeDuration(DateTime, DayTimeDuration),
    DateDuration(Date, Duration),
    DateYearMonthDuration(Date, YearMonthDuration),
    DateDayTimeDuration(Date, DayTimeDuration),
    TimeDuration(Time, Duration),
    TimeDayTimeDuration(Time, DayTimeDuration),
}

impl NumericBinaryOperands {
    fn new(a: ExpressionTerm, b: ExpressionTerm) -> Option<Self> {
        match (a, b) {
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1, v2))
            }
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double((v1).into(), v2))
            }
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Float(v1, (v2).into()))
            }
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Float(v1, (v2).into()))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Double(v1, (v2).into()))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1, v2))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Double(v1, (v2).into()))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Double(v1, (v2).into()))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Float((v1).into(), v2))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double((v1).into(), v2))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Integer(v1, v2))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal((v1).into(), v2))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Float((v1).into(), v2))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double((v1).into(), v2))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Decimal(v1, (v2).into()))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal(v1, v2))
            }
            (ExpressionTerm::DurationLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2))
            }
            (ExpressionTerm::DurationLiteral(v1), ExpressionTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::Duration(v1, (v2).into()))
            }
            (ExpressionTerm::DurationLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::Duration(v1, (v2).into()))
            }
            (ExpressionTerm::YearMonthDurationLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::Duration((v1).into(), v2))
            }
            (
                ExpressionTerm::YearMonthDurationLiteral(v1),
                ExpressionTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::YearMonthDuration(v1, v2)),
            (
                ExpressionTerm::YearMonthDurationLiteral(v1),
                ExpressionTerm::DayTimeDurationLiteral(v2),
            ) => Some(Self::Duration((v1).into(), (v2).into())),
            (ExpressionTerm::DayTimeDurationLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::Duration((v1).into(), v2))
            }
            (
                ExpressionTerm::DayTimeDurationLiteral(v1),
                ExpressionTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::Duration((v1).into(), (v2).into())),
            (
                ExpressionTerm::DayTimeDurationLiteral(v1),
                ExpressionTerm::DayTimeDurationLiteral(v2),
            ) => Some(Self::DayTimeDuration(v1, v2)),
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::DateTimeLiteral(v2)) => {
                Some(Self::DateTime(v1, v2))
            }
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::DateLiteral(v2)) => {
                Some(Self::Date(v1, v2))
            }
            (ExpressionTerm::TimeLiteral(v1), ExpressionTerm::TimeLiteral(v2)) => {
                Some(Self::Time(v1, v2))
            }
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::DateTimeDuration(v1, v2))
            }
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::DateTimeYearMonthDuration(v1, v2))
            }
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::DateTimeDayTimeDuration(v1, v2))
            }
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::DateDuration(v1, v2))
            }
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::DateYearMonthDuration(v1, v2))
            }
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::DateDayTimeDuration(v1, v2))
            }
            (ExpressionTerm::TimeLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::TimeDuration(v1, v2))
            }
            (ExpressionTerm::TimeLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::TimeDayTimeDuration(v1, v2))
            }
            _ => None,
        }
    }
}

pub fn str(dataset: Arc<DatasetView<'static>>, arg: Expr) -> Expr {
    term_function(
        dataset,
        [arg],
        "sparql:str",
        |[arg]| {
            Some(ExpressionTerm::StringLiteral(match arg.into() {
                Term::NamedNode(term) => term.into_string(),
                Term::BlankNode(_) => return None,
                Term::Literal(term) => term.destruct().0,
                #[cfg(feature = "rdf-12")]
                Term::Triple(_) => return None,
            }))
        },
        Volatility::Immutable,
    )
}

pub fn lang(dataset: Arc<DatasetView<'static>>, literal: Expr) -> Expr {
    term_function(
        dataset,
        [literal],
        "sparql:lang",
        |[literal]| {
            Some(ExpressionTerm::StringLiteral(match literal {
                ExpressionTerm::LangStringLiteral { language, .. } => language,
                #[cfg(feature = "rdf-12")]
                ExpressionTerm::DirLangStringLiteral { language, .. } => language,
                ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                    return None;
                }
                #[cfg(feature = "rdf-12")]
                ExpressionTerm::Triple(_) => return None,
                _ => String::new(),
            }))
        },
        Volatility::Immutable,
    )
}

pub fn is_blank(dataset: Arc<DatasetView<'static>>, term: Expr) -> Expr {
    boolean_function(
        dataset,
        [term],
        "sparql:isBlank",
        |[term]| Some(matches!(term, ExpressionTerm::BlankNode(_))),
        Volatility::Immutable,
    )
}

pub fn lang_matches(
    dataset: Arc<DatasetView<'static>>,
    language_tag: Expr,
    language_range: Expr,
) -> Expr {
    boolean_function(
        dataset,
        [language_tag, language_range],
        "sparql:langMatches",
        |[language_tag, language_range]| {
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

            let ExpressionTerm::StringLiteral(mut language_tag) = language_tag else {
                return None;
            };
            language_tag.make_ascii_lowercase();
            let ExpressionTerm::StringLiteral(mut language_range) = language_range else {
                return None;
            };
            language_range.make_ascii_lowercase();
            Some(if &*language_range == "*" {
                !language_tag.is_empty()
            } else {
                !ZipLongest::new(language_range.split('-'), language_tag.split('-')).any(|parts| {
                    match parts {
                        (Some(range_subtag), Some(language_subtag)) => {
                            range_subtag != language_subtag
                        }
                        (Some(_), None) => true,
                        (None, _) => false,
                    }
                })
            })
        },
        Volatility::Immutable,
    )
}

pub fn regex(
    dataset: Arc<DatasetView<'static>>,
    text: Expr,
    pattern: Expr,
    flags: Option<Expr>,
) -> Expr {
    if let Some(flags) = flags {
        boolean_function(
            dataset,
            [text, pattern, flags],
            "sparql:regex",
            |[text, pattern, flags]| {
                let text = match text {
                    ExpressionTerm::StringLiteral(value)
                    | ExpressionTerm::LangStringLiteral { value, .. } => value,
                    #[cfg(feature = "rdf-12")]
                    ExpressionTerm::DirLangStringLiteral { value, .. } => value,
                    _ => return None,
                };
                let ExpressionTerm::StringLiteral(pattern) = pattern else {
                    return None;
                };
                let ExpressionTerm::StringLiteral(options) = flags else {
                    return None;
                };
                let regex = compile_pattern(&pattern, Some(&options))?;
                Some(regex.is_match(&text))
            },
            Volatility::Immutable,
        )
    } else {
        boolean_function(
            dataset,
            [text, pattern],
            "sparql:regex",
            |[text, pattern]| {
                let text = match text {
                    ExpressionTerm::StringLiteral(value)
                    | ExpressionTerm::LangStringLiteral { value, .. } => value,
                    #[cfg(feature = "rdf-12")]
                    ExpressionTerm::DirLangStringLiteral { value, .. } => value,
                    _ => return None,
                };
                let ExpressionTerm::StringLiteral(pattern) = pattern else {
                    return None;
                };
                let regex = compile_pattern(&pattern, None)?;
                Some(regex.is_match(&text))
            },
            Volatility::Immutable,
        )
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

pub fn xsd_integer(dataset: Arc<DatasetView<'static>>, literal: Expr) -> Expr {
    term_function(
        dataset,
        [literal],
        "xsd:integer",
        |[literal]| {
            Some(ExpressionTerm::IntegerLiteral(match literal {
                ExpressionTerm::FloatLiteral(value) => value.try_into().ok()?,
                ExpressionTerm::DoubleLiteral(value) => value.try_into().ok()?,
                ExpressionTerm::IntegerLiteral(value) => value,
                ExpressionTerm::DecimalLiteral(value) => value.try_into().ok()?,
                ExpressionTerm::BooleanLiteral(value) => value.into(),
                ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                _ => return None,
            }))
        },
        Volatility::Immutable,
    )
}

pub fn xsd_decimal(dataset: Arc<DatasetView<'static>>, literal: Expr) -> Expr {
    term_function(
        dataset,
        [literal],
        "xsd:decimal",
        |[literal]| {
            Some(ExpressionTerm::DecimalLiteral(match literal {
                ExpressionTerm::FloatLiteral(value) => value.try_into().ok()?,
                ExpressionTerm::DoubleLiteral(value) => value.try_into().ok()?,
                ExpressionTerm::IntegerLiteral(value) => value.into(),
                ExpressionTerm::DecimalLiteral(value) => value,
                ExpressionTerm::BooleanLiteral(value) => value.into(),
                ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                _ => return None,
            }))
        },
        Volatility::Immutable,
    )
}

pub fn xsd_float(dataset: Arc<DatasetView<'static>>, literal: Expr) -> Expr {
    term_function(
        dataset,
        [literal],
        "xsd:float",
        |[literal]| {
            Some(ExpressionTerm::FloatLiteral(match literal {
                ExpressionTerm::FloatLiteral(value) => value,
                ExpressionTerm::DoubleLiteral(value) => value.into(),
                ExpressionTerm::IntegerLiteral(value) => value.into(),
                ExpressionTerm::DecimalLiteral(value) => value.into(),
                ExpressionTerm::BooleanLiteral(value) => value.into(),
                ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                _ => return None,
            }))
        },
        Volatility::Immutable,
    )
}

pub fn xsd_double(dataset: Arc<DatasetView<'static>>, literal: Expr) -> Expr {
    term_function(
        dataset,
        [literal],
        "xsd:double",
        |[literal]| {
            Some(ExpressionTerm::DoubleLiteral(match literal {
                ExpressionTerm::FloatLiteral(value) => value.into(),
                ExpressionTerm::DoubleLiteral(value) => value,
                ExpressionTerm::IntegerLiteral(value) => value.into(),
                ExpressionTerm::DecimalLiteral(value) => value.into(),
                ExpressionTerm::BooleanLiteral(value) => value.into(),
                ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                _ => return None,
            }))
        },
        Volatility::Immutable,
    )
}

pub fn agg_sum(dataset: Arc<DatasetView<'static>>, input: Expr, distinct: bool) -> Expr {
    struct SumAccumulator {
        sum: Option<ExpressionTerm>,
    }

    impl TermAccumulator for SumAccumulator {
        const STATE_COLUMNS: &[&str] = &["sum"];

        fn update(&mut self, term: ExpressionTerm) {
            self.sum = self.sum.take().and_then(|sum| do_add(sum, term));
        }

        fn evaluate(self) -> Result<Option<ExpressionTerm>> {
            Ok(self.sum)
        }

        fn state(self) -> Vec<Option<ExpressionTerm>> {
            vec![self.sum]
        }

        fn merge(&mut self, state: Vec<Option<ExpressionTerm>>) -> Result<()> {
            let Some(state_sum) = state.into_iter().next() else {
                return internal_err!("missing sum in sum accumulator state");
            };
            let Some(state_sum) = state_sum else {
                self.sum = None;
                return Ok(());
            };
            self.update(state_sum);
            Ok(())
        }
    }

    term_aggregate_function(
        dataset,
        input,
        distinct,
        "sparql:agg-sum",
        || SumAccumulator {
            sum: Some(ExpressionTerm::IntegerLiteral(0.into())),
        },
        Volatility::Immutable,
    )
}

pub fn agg_avg(dataset: Arc<DatasetView<'static>>, input: Expr, distinct: bool) -> Expr {
    struct AvgAccumulator {
        sum: Option<ExpressionTerm>,
        count: Option<Integer>,
    }

    impl TermAccumulator for AvgAccumulator {
        const STATE_COLUMNS: &[&str] = &["sum", "count"];
        fn update(&mut self, term: ExpressionTerm) {
            self.sum = self.sum.take().and_then(|sum| do_add(sum, term));
            self.count = self.count.and_then(|count| count.checked_add(1));
        }

        fn evaluate(self) -> Result<Option<ExpressionTerm>> {
            let (Some(sum), Some(count)) = (self.sum, self.count) else {
                return Ok(None);
            };
            Ok(if count == 0.into() {
                Some(ExpressionTerm::IntegerLiteral(0.into()))
            } else {
                do_divide(sum, ExpressionTerm::IntegerLiteral(count))
            })
        }

        fn state(self) -> Vec<Option<ExpressionTerm>> {
            vec![self.sum, self.count.map(ExpressionTerm::IntegerLiteral)]
        }

        fn merge(&mut self, state: Vec<Option<ExpressionTerm>>) -> Result<()> {
            let mut state = state.into_iter();
            let Some(state_sum) = state.next() else {
                return internal_err!("missing sum in avg accumulator state");
            };
            let Some(state_count) = state.next() else {
                return internal_err!("missing count in avg accumulator state");
            };
            self.sum = self.sum.take().and_then(|sum| do_add(sum, state_sum?));
            self.count = self
                .count
                .take()
                .and_then(|count| {
                    Some(
                        if let ExpressionTerm::IntegerLiteral(state_count) = state_count? {
                            Ok(count.checked_add(state_count)?)
                        } else {
                            internal_err!("count in avg accumulator state is not an integer")
                        },
                    )
                })
                .transpose()?;
            Ok(())
        }
    }

    term_aggregate_function(
        dataset,
        input,
        distinct,
        "sparql:agg-avg",
        || AvgAccumulator {
            sum: Some(ExpressionTerm::IntegerLiteral(0.into())),
            count: Some(0.into()),
        },
        Volatility::Immutable,
    )
}

pub fn agg_min(dataset: Arc<DatasetView<'static>>, input: Expr, distinct: bool) -> Expr {
    struct MinAccumulator {
        min: Option<ExpressionTerm>,
    }

    impl TermAccumulator for MinAccumulator {
        const STATE_COLUMNS: &[&str] = &["min"];
        fn update(&mut self, term: ExpressionTerm) {
            let Some(min) = &self.min else {
                self.min = Some(term);
                return;
            };
            if cmp_terms(&term, min) == Ordering::Less {
                self.min = Some(term);
            }
        }

        fn evaluate(self) -> Result<Option<ExpressionTerm>> {
            Ok(self.min)
        }

        fn state(self) -> Vec<Option<ExpressionTerm>> {
            vec![self.min]
        }

        fn merge(&mut self, state: Vec<Option<ExpressionTerm>>) -> Result<()> {
            if let Some(Some(min)) = state.into_iter().next() {
                self.update(min)
            }
            Ok(())
        }
    }

    term_aggregate_function(
        dataset,
        input,
        distinct,
        "sparql:agg-min",
        || MinAccumulator { min: None },
        Volatility::Immutable,
    )
}

pub fn agg_max(dataset: Arc<DatasetView<'static>>, input: Expr, distinct: bool) -> Expr {
    struct MaxAccumulator {
        max: Option<ExpressionTerm>,
    }

    impl TermAccumulator for MaxAccumulator {
        const STATE_COLUMNS: &[&str] = &["max"];
        fn update(&mut self, term: ExpressionTerm) {
            let Some(max) = &self.max else {
                self.max = Some(term);
                return;
            };
            if cmp_terms(&term, max) == Ordering::Greater {
                self.max = Some(term);
            }
        }

        fn evaluate(self) -> Result<Option<ExpressionTerm>> {
            Ok(self.max)
        }

        fn state(self) -> Vec<Option<ExpressionTerm>> {
            vec![self.max]
        }

        fn merge(&mut self, state: Vec<Option<ExpressionTerm>>) -> Result<()> {
            if let Some(Some(max)) = state.into_iter().next() {
                self.update(max)
            }
            Ok(())
        }
    }

    term_aggregate_function(
        dataset,
        input,
        distinct,
        "sparql:agg-max",
        || MaxAccumulator { max: None },
        Volatility::Immutable,
    )
}
