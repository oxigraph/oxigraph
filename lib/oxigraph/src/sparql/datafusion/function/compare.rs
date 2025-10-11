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
use spareval::{ExpressionTerm, QueryableDataset};
use std::any::Any;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub fn compare_terms(
    dataset: Arc<DatasetView<'static>>,
    left: Expr,
    operator: ComparisonOperator,
    right: Expr,
) -> Expr {
    ScalarUDF::new_from_impl(CompareTerms {
        operator,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComparisonOperator {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl ComparisonOperator {
    #[inline]
    fn result(self, ordering: Ordering) -> bool {
        match self {
            ComparisonOperator::Less => ordering.is_lt(),
            ComparisonOperator::LessOrEqual => ordering.is_le(),
            ComparisonOperator::Greater => ordering.is_gt(),
            ComparisonOperator::GreaterOrEqual => ordering.is_ge(),
        }
    }
}

struct CompareTerms {
    operator: ComparisonOperator,
    dataset: Arc<DatasetView<'static>>,
    signature: Signature,
}

impl fmt::Debug for CompareTerms {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompareTerms")
            .field("operator", &self.operator)
            .finish()
    }
}

impl PartialEq for CompareTerms {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.operator == other.operator
    }
}

impl Eq for CompareTerms {}

impl Hash for CompareTerms {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.operator.hash(state);
    }
}

impl ScalarUDFImpl for CompareTerms {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        match self.operator {
            ComparisonOperator::Less => "sparql:less-than",
            ComparisonOperator::LessOrEqual => "sparql:less-than-or-equal",
            ComparisonOperator::Greater => "sparql:greater-than",
            ComparisonOperator::GreaterOrEqual => "sparql:greater-than-or-equal",
        }
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
                    Ok(partial_cmp(&left, &right).map(|ordering| self.operator.result(ordering)))
                })
                .collect::<datafusion::common::Result<BooleanArray>>()?,
        );
        Ok(result.into())
    }
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
