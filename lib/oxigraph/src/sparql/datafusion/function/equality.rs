use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::decode_term;
use datafusion::arrow::array::{ArrayRef, BinaryArray, BooleanArray};
use datafusion::arrow::datatypes::DataType;
use datafusion::common::downcast_value;
use datafusion::logical_expr::{
    ColumnarValue, Expr, ScalarFunctionArgs, ScalarUDF, ScalarUDFImpl, Signature, TypeSignature,
    Volatility,
};
use oxsdatatypes::{Decimal, Double, Float};
use spareval::{ExpressionTerm, ExpressionTriple, QueryableDataset};
use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub fn term_equals(dataset: Arc<DatasetView<'static>>, left: Expr, right: Expr) -> Expr {
    ScalarUDF::new_from_impl(TermEquals {
        dataset,
        signature: Signature::one_of(
            vec![
                TypeSignature::Exact(vec![DataType::Binary, DataType::Binary]),
                TypeSignature::Exact(vec![DataType::Binary, DataType::Null]),
                TypeSignature::Exact(vec![DataType::Null, DataType::Binary]),
                TypeSignature::Exact(vec![DataType::Null, DataType::Null]),
            ],
            Volatility::Immutable,
        ),
    })
    .call(vec![left, right])
}

struct TermEquals {
    dataset: Arc<DatasetView<'static>>,
    signature: Signature,
}

impl fmt::Debug for TermEquals {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompareTerms").finish()
    }
}

impl PartialEq for TermEquals {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for TermEquals {}

impl Hash for TermEquals {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl ScalarUDFImpl for TermEquals {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        "sparql:equals"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _args: &[DataType]) -> datafusion::common::Result<DataType> {
        Ok(DataType::Boolean)
    }

    fn invoke_with_args(
        &self,
        args: ScalarFunctionArgs,
    ) -> datafusion::common::Result<ColumnarValue> {
        let args = ColumnarValue::values_to_arrays(&args.args)?;
        for arg in &args {
            // We return nulls if any of the arguments is null
            if arg.data_type().is_null() {
                let mut builder = BooleanArray::builder(arg.len());
                builder.append_nulls(arg.len());
                let result: ArrayRef = Arc::new(builder.finish());
                return Ok(result.into());
            }
        }
        let result: ArrayRef = Arc::new(
            downcast_value!(args[0], BinaryArray)
                .iter()
                .zip(downcast_value!(args[1], BinaryArray))
                .map(|(left, right)| {
                    let Some(left) = left else { return Ok(None) };
                    let left = self
                        .dataset
                        .externalize_expression_term(decode_term(left)?)?;
                    let Some(right) = right else { return Ok(None) };
                    let right = self
                        .dataset
                        .externalize_expression_term(decode_term(right)?)?;
                    Ok(equals(&left, &right))
                })
                .collect::<datafusion::common::Result<BooleanArray>>()?,
        );
        Ok(result.into())
    }
}

/// Equality operator (=)
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
