use crate::storage::binary_encoder::{decode_term, encode_term};
use crate::storage::numeric_encoder::EncodedTerm;
use datafusion::arrow::array::{ArrayRef, BinaryArray, BooleanArray, Int64Array};
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{Result, downcast_value};
use datafusion::logical_expr::{
    ColumnarValue, ScalarFunctionArgs, ScalarUDFImpl, Signature, Volatility,
};
use oxsdatatypes::Boolean;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Eq, PartialEq, Hash)]
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

    fn return_type(&self, _args: &[DataType]) -> Result<DataType> {
        Ok(DataType::Boolean)
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let args = ColumnarValue::values_to_arrays(&args.args)?;
        Ok(ColumnarValue::from(Arc::new(
            downcast_value!(args[0], BinaryArray)
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

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct ToRdfLiteral {
    signature: Signature,
}

impl ToRdfLiteral {
    pub fn new() -> Self {
        Self {
            signature: Signature::uniform(1, vec![DataType::Int64], Volatility::Immutable),
        }
    }
}

impl ScalarUDFImpl for ToRdfLiteral {
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
        Ok(DataType::Binary)
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let args = ColumnarValue::values_to_arrays(&args.args)?;
        Ok(ColumnarValue::from(Arc::new(
            downcast_value!(args[0], Int64Array)
                .iter()
                .map(|value| Some(encode_term(&EncodedTerm::IntegerLiteral(value?.into()))))
                .collect::<BinaryArray>(),
        ) as ArrayRef))
    }
}
