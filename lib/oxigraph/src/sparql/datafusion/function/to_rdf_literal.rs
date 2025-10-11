use crate::storage::binary_encoder::encode_term;
use crate::storage::numeric_encoder::EncodedTerm;
use datafusion::arrow::array::{Array, ArrayRef, BinaryArray, BooleanArray, Int64Array};
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{Result, downcast_value, not_impl_err};
use datafusion::logical_expr::{
    ColumnarValue, Expr, ScalarFunctionArgs, ScalarUDF, ScalarUDFImpl, Signature, Volatility,
};
use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub fn to_rdf_literal(expr: Expr) -> Expr {
    ScalarUDF::new_from_impl(ToRdfLiteral {
        signature: Signature::uniform(
            1,
            vec![DataType::Int64, DataType::Boolean, DataType::Null],
            Volatility::Immutable,
        ),
    })
    .call(vec![expr])
}

struct ToRdfLiteral {
    signature: Signature,
}

impl fmt::Debug for ToRdfLiteral {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToRdfLiteral").finish()
    }
}

impl PartialEq for ToRdfLiteral {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for ToRdfLiteral {}

impl Hash for ToRdfLiteral {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
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
        let result: ArrayRef = Arc::new(match args[0].data_type() {
            DataType::Null => BinaryArray::new_null(args[0].len()),
            DataType::Boolean => downcast_value!(args[0], BooleanArray)
                .iter()
                .map(|value| Some(encode_term(&EncodedTerm::BooleanLiteral(value?.into()))))
                .collect::<BinaryArray>(),
            DataType::Int64 => downcast_value!(args[0], Int64Array)
                .iter()
                .map(|value| Some(encode_term(&EncodedTerm::IntegerLiteral(value?.into()))))
                .collect::<BinaryArray>(),
            _ => {
                return not_impl_err!("toRdfLiteral does not work on {}", args[0].data_type());
            }
        });
        Ok(result.into())
    }
}
