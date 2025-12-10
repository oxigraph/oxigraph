use crate::dataset::ExpressionTermEncoder;
use datafusion::arrow::array::Array;
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{Result, ScalarValue, internal_err, not_impl_err};
use datafusion::logical_expr::{
    ColumnarValue, Expr, ScalarFunctionArgs, ScalarUDF, ScalarUDFImpl, Signature, Volatility,
};
use spareval::ExpressionTerm;
use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};

pub fn to_rdf_literal(encoder: impl ExpressionTermEncoder, expr: Expr) -> Expr {
    ScalarUDF::new_from_impl(ToRdfLiteral {
        encoder,
        signature: Signature::uniform(
            1,
            vec![DataType::Int64, DataType::Boolean, DataType::Null],
            Volatility::Immutable,
        ),
    })
    .call(vec![expr])
}

struct ToRdfLiteral<E> {
    encoder: E,
    signature: Signature,
}

impl<E> fmt::Debug for ToRdfLiteral<E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToRdfLiteral").finish()
    }
}

impl<E> PartialEq for ToRdfLiteral<E> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<E> Eq for ToRdfLiteral<E> {}

impl<E> Hash for ToRdfLiteral<E> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<E: ExpressionTermEncoder> ScalarUDFImpl for ToRdfLiteral<E> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        "toRdfLiteral"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _args: &[DataType]) -> Result<DataType> {
        Ok(self.encoder.internal_type().clone())
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let Some(arg) = args.args.into_iter().next() else {
            return internal_err!("toRdfLiteral requires an argument");
        };
        Ok(match arg {
            ColumnarValue::Scalar(scalar) => if let Some(term) = scalar_value_to_term(scalar) {
                self.encoder.internalize_expression_term(term?)?
            } else {
                ScalarValue::try_new_null(self.encoder.internal_type())?
            }
            .into(),
            ColumnarValue::Array(arg) => {
                let terms = (0..arg.len())
                    .map(|i| {
                        scalar_value_to_term(ScalarValue::try_from_array(&arg, i)?).transpose()
                    })
                    .collect::<Result<Vec<_>>>()?;
                self.encoder
                    .internalize_expression_terms(terms.into_iter())?
                    .into()
            }
        })
    }
}

fn scalar_value_to_term(scalar: ScalarValue) -> Option<Result<ExpressionTerm>> {
    Some(Ok(match scalar {
        ScalarValue::Null => return None,
        ScalarValue::Boolean(v) => ExpressionTerm::BooleanLiteral(v?.into()),
        ScalarValue::Float32(v) => ExpressionTerm::FloatLiteral(v?.into()),
        ScalarValue::Float64(v) => ExpressionTerm::DoubleLiteral(v?.into()),
        ScalarValue::Int8(v) => ExpressionTerm::IntegerLiteral(v?.into()),
        ScalarValue::Int16(v) => ExpressionTerm::IntegerLiteral(v?.into()),
        ScalarValue::Int32(v) => ExpressionTerm::IntegerLiteral(v?.into()),
        ScalarValue::Int64(v) => ExpressionTerm::IntegerLiteral(v?.into()),
        ScalarValue::UInt8(v) => ExpressionTerm::IntegerLiteral(v?.into()),
        ScalarValue::UInt16(v) => ExpressionTerm::IntegerLiteral(v?.into()),
        ScalarValue::UInt32(v) => ExpressionTerm::IntegerLiteral(v?.into()),
        ScalarValue::Utf8(v) | ScalarValue::Utf8View(v) | ScalarValue::LargeUtf8(v) => {
            ExpressionTerm::StringLiteral(v?)
        }
        _ => {
            return Some(not_impl_err!(
                "toRdfLiteral does not work on {}",
                scalar.data_type()
            ));
        }
    }))
}
