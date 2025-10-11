#[cfg(feature = "rdf-12")]
use crate::model::{BaseDirection, NamedOrBlankNode};
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::{decode_term, encode_term};
use crate::storage::numeric_encoder::EncodedTerm;
use datafusion::arrow::array::{ArrayRef, BinaryArray, BooleanArray, Int64Array};
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{Result, downcast_value};
use datafusion::logical_expr::{
    ColumnarValue, ScalarFunctionArgs, ScalarUDFImpl, Signature, Volatility,
};
use oxsdatatypes::{Boolean, Double};
use spareval::{ExpressionTerm, QueryableDataset};
use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub struct EffectiveBooleanValue {
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

pub struct ToRdfLiteral {
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
        let result: ArrayRef = Arc::new(
            downcast_value!(args[0], Int64Array)
                .iter()
                .map(|value| Some(encode_term(&EncodedTerm::IntegerLiteral(value?.into()))))
                .collect::<BinaryArray>(),
        );
        Ok(result.into())
    }
}

/// Return a byte string which lexicographic ordering is the order expected by SPARQL ORDER BY and that is injective.
///
/// We take binary values and prefix them with a byte to sort bnode < iri < literal
pub struct OrderByCollation {
    dataset: Arc<DatasetView<'static>>,
    signature: Signature,
}

impl fmt::Debug for OrderByCollation {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToOrderByValue").finish()
    }
}

impl PartialEq for OrderByCollation {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for OrderByCollation {}

impl Hash for OrderByCollation {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl OrderByCollation {
    pub fn new(dataset: Arc<DatasetView<'static>>) -> Self {
        Self {
            dataset,
            signature: Signature::uniform(1, vec![DataType::Binary], Volatility::Immutable),
        }
    }
}

impl ScalarUDFImpl for OrderByCollation {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        "toOrderByValue"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _args: &[DataType]) -> Result<DataType> {
        Ok(DataType::Binary)
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let args = ColumnarValue::values_to_arrays(&args.args)?;
        let result: ArrayRef = Arc::new(
            downcast_value!(args[0], BinaryArray)
                .iter()
                .map(|value| {
                    let Some(value) = value else { return Ok(None) };
                    let mut buffer = Vec::new();
                    write_term_collation(
                        &self
                            .dataset
                            .externalize_expression_term(decode_term(value)?)?,
                        &mut buffer,
                    );
                    Ok(Some(buffer))
                })
                .collect::<Result<BinaryArray>>()?,
        );
        Ok(result.into())
    }
}

fn write_term_collation(term: &ExpressionTerm, buffer: &mut Vec<u8>) {
    // TODO: this is wrong: we use \0 as separator, but it is a valid UTF-8 character.
    // We should maybe just do +1 to each string byte and call it a day
    match term {
        ExpressionTerm::BlankNode(bnode) => {
            buffer.push(1);
            buffer.extend_from_slice(bnode.as_str().as_bytes());
        }
        ExpressionTerm::NamedNode(iri) => {
            buffer.push(2);
            buffer.extend_from_slice(iri.as_str().as_bytes());
        }
        ExpressionTerm::StringLiteral(value) => {
            buffer.push(3);
            buffer.extend_from_slice(value.as_str().as_bytes());
        }
        ExpressionTerm::LangStringLiteral { value, language } => {
            buffer.push(3);
            buffer.extend_from_slice(value.as_str().as_bytes());
            buffer.push(0);
            buffer.extend_from_slice(language.as_str().as_bytes());
        }
        #[cfg(feature = "rdf-12")]
        ExpressionTerm::DirLangStringLiteral {
            value,
            language,
            direction,
        } => {
            buffer.push(3);
            buffer.extend_from_slice(value.as_str().as_bytes());
            buffer.push(0);
            buffer.extend_from_slice(language.as_str().as_bytes());
            buffer.push(0);
            buffer.push(match direction {
                BaseDirection::Ltr => 0,
                BaseDirection::Rtl => 1,
            })
        }
        ExpressionTerm::OtherTypedLiteral { value, datatype } => {
            buffer.push(3);
            buffer.extend_from_slice(value.as_str().as_bytes());
            buffer.push(0);
            buffer.extend_from_slice(datatype.as_str().as_bytes());
        }
        ExpressionTerm::BooleanLiteral(v) => {
            buffer.push(4);
            buffer.extend_from_slice(&Double::from(*v).bytes_collation());
            buffer.push(0); // Hack to keep datatype
        }
        ExpressionTerm::IntegerLiteral(v) => {
            buffer.push(4);
            buffer.extend_from_slice(&Double::from(*v).bytes_collation());
            buffer.push(1); // Hack to keep datatype
        }
        ExpressionTerm::DecimalLiteral(v) => {
            buffer.push(4);
            buffer.extend_from_slice(&Double::from(*v).bytes_collation());
            buffer.push(2); // Hack to keep datatype
        }
        ExpressionTerm::FloatLiteral(v) => {
            buffer.push(4);
            buffer.extend_from_slice(&Double::from(*v).bytes_collation());
            buffer.push(3); // Hack to keep datatype
        }
        ExpressionTerm::DoubleLiteral(v) => {
            buffer.push(4);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(4); // Hack to keep datatype
        }
        ExpressionTerm::DateTimeLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(0); // Hack to keep datatype
        }
        ExpressionTerm::TimeLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(1); // Hack to keep datatype
        }
        ExpressionTerm::DateLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(2); // Hack to keep datatype
        }
        ExpressionTerm::GDayLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(3); // Hack to keep datatype
        }
        ExpressionTerm::GMonthDayLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(4); // Hack to keep datatype
        }
        ExpressionTerm::GMonthLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(5); // Hack to keep datatype
        }
        ExpressionTerm::GYearMonthLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(6); // Hack to keep datatype
        }
        ExpressionTerm::GYearLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(7); // Hack to keep datatype
        }
        ExpressionTerm::DurationLiteral(v) => {
            buffer.push(6);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(0); // Hack to keep datatype
        }
        ExpressionTerm::YearMonthDurationLiteral(v) => {
            buffer.push(6);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(1); // Hack to keep datatype
        }
        ExpressionTerm::DayTimeDurationLiteral(v) => {
            buffer.push(6);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(2); // Hack to keep datatype
        }
        #[cfg(feature = "rdf-12")]
        ExpressionTerm::Triple(t) => {
            buffer.push(u8::MAX);
            match &t.subject {
                NamedOrBlankNode::BlankNode(bnode) => {
                    buffer.push(1);
                    buffer.extend_from_slice(bnode.as_str().as_bytes());
                }
                NamedOrBlankNode::NamedNode(iri) => {
                    buffer.push(2);
                    buffer.extend_from_slice(iri.as_str().as_bytes());
                }
            }
            buffer.push(0);
            buffer.extend_from_slice(t.predicate.as_str().as_bytes());
            buffer.push(0);
            write_term_collation(&t.object, buffer);
        }
    }
}
