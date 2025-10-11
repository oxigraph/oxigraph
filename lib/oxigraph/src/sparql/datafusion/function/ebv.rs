use crate::storage::binary_encoder::decode_term;
use crate::storage::numeric_encoder::EncodedTerm;
use datafusion::arrow::array::{ArrayRef, BinaryArray, BooleanArray};
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{Result, downcast_value};
use datafusion::logical_expr::{
    ColumnarValue, Expr, ScalarFunctionArgs, ScalarUDF, ScalarUDFImpl, Signature, Volatility,
};
use oxsdatatypes::Boolean;
use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub fn effective_boolean_value(arg: Expr) -> Expr {
    ScalarUDF::new_from_impl(EffectiveBooleanValue {
        signature: Signature::uniform(1, vec![DataType::Binary], Volatility::Immutable),
    })
    .call(vec![arg])
}

struct EffectiveBooleanValue {
    signature: Signature,
}

impl fmt::Debug for EffectiveBooleanValue {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectiveBooleanValue").finish()
    }
}

impl PartialEq for EffectiveBooleanValue {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for EffectiveBooleanValue {}

impl Hash for EffectiveBooleanValue {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl ScalarUDFImpl for EffectiveBooleanValue {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        "sparql:ebv"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _args: &[DataType]) -> Result<DataType> {
        Ok(DataType::Boolean)
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let args = ColumnarValue::values_to_arrays(&args.args)?;
        let result: ArrayRef = Arc::new(
            downcast_value!(args[0], BinaryArray)
                .iter()
                .map(|value| {
                    let Some(value) = value else {
                        return Ok(None);
                    };
                    Ok(match decode_term(value)? {
                        EncodedTerm::BooleanLiteral(value) => Some(value.into()),
                        EncodedTerm::SmallStringLiteral(value) => Some(!value.is_empty()),
                        EncodedTerm::FloatLiteral(value) => Some(Boolean::from(value).into()),
                        EncodedTerm::DoubleLiteral(value) => Some(Boolean::from(value).into()),
                        EncodedTerm::IntegerLiteral(value) => Some(Boolean::from(value).into()),
                        EncodedTerm::DecimalLiteral(value) => Some(Boolean::from(value).into()),
                        _ => None,
                    })
                })
                .collect::<Result<BooleanArray>>()?,
        );
        Ok(result.into())
    }
}
