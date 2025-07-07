use crate::storage::binary_encoder::decode_term;
use crate::storage::numeric_encoder::EncodedTerm;
use datafusion::arrow::array::{ArrayRef, AsArray, BooleanArray};
use datafusion::arrow::datatypes::{DataType, GenericBinaryType};
use datafusion::logical_expr::{
    ColumnarValue, ScalarFunctionArgs, ScalarUDFImpl, Signature, Volatility,
};
use oxsdatatypes::Boolean;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug)]
pub struct EffectiveBooleanValue {
    signature: Signature,
}

impl EffectiveBooleanValue {
    pub fn new() -> Self {
        Self {
            signature: Signature::uniform(1, vec![DataType::Binary], Volatility::Immutable),
        }
    }
}

impl ScalarUDFImpl for EffectiveBooleanValue {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn name(&self) -> &str {
        "ebv"
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
        Ok(ColumnarValue::from(Arc::new(
            args[0]
                .as_bytes::<GenericBinaryType<i32>>()
                .iter()
                .map(|value| match decode_term(value?).ok()? {
                    EncodedTerm::BooleanLiteral(value) => Some(value.into()),
                    EncodedTerm::SmallStringLiteral(value) => Some(!value.is_empty()),
                    EncodedTerm::FloatLiteral(value) => Some(Boolean::from(value).into()),
                    EncodedTerm::DoubleLiteral(value) => Some(Boolean::from(value).into()),
                    EncodedTerm::IntegerLiteral(value) => Some(Boolean::from(value).into()),
                    EncodedTerm::DecimalLiteral(value) => Some(Boolean::from(value).into()),
                    _ => None,
                })
                .collect::<BooleanArray>(),
        ) as ArrayRef))
    }
}
