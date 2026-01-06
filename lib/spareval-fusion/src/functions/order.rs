use crate::dataset::ExpressionTermEncoder;
use crate::functions::utils::signature;
use datafusion::arrow::array::{ArrayRef, BinaryArray};
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{Result, ScalarValue, internal_err};
use datafusion::logical_expr::{
    ColumnarValue, Expr, ScalarFunctionArgs, ScalarUDF, ScalarUDFImpl, Signature, Volatility,
};
#[cfg(feature = "sparql-12")]
use oxrdf::{BaseDirection, NamedOrBlankNode};
use oxsdatatypes::Double;
use spareval::ExpressionTerm;
use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Return a byte string which lexicographic ordering is the order expected by SPARQL ORDER BY and that is injective.
///
/// We take binary values and prefix them with a byte to sort bnode < iri < literal
pub fn order_by_collation(encoder: impl ExpressionTermEncoder, expr: Expr) -> Expr {
    let signature = signature(&encoder, 1, Volatility::Immutable);
    ScalarUDF::new_from_impl(OrderByCollation { encoder, signature }).call(vec![expr])
}

struct OrderByCollation<E> {
    encoder: E,
    signature: Signature,
}

impl<E> fmt::Debug for OrderByCollation<E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToOrderByValue").finish()
    }
}

impl<E> PartialEq for OrderByCollation<E> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<E> Eq for OrderByCollation<E> {}

impl<E> Hash for OrderByCollation<E> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<E: ExpressionTermEncoder> ScalarUDFImpl for OrderByCollation<E> {
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
        let Some(arg) = args.args.into_iter().next() else {
            return internal_err!("toRdfLiteral requires an argument");
        };
        Ok(match arg {
            ColumnarValue::Scalar(scalar) => {
                let Some(term) = self.encoder.externalize_expression_term(scalar)? else {
                    return Ok(ScalarValue::Binary(None).into());
                };
                let mut buffer = Vec::new();
                write_term_collation(&term, &mut buffer);
                ScalarValue::Binary(Some(buffer)).into()
            }
            ColumnarValue::Array(arg) => {
                let result: ArrayRef = Arc::new(
                    self.encoder
                        .externalize_expression_terms(arg)?
                        .into_iter()
                        .map(|term| {
                            let Some(term) = term? else {
                                return Ok(None);
                            };
                            let mut buffer = Vec::new();
                            write_term_collation(&term, &mut buffer);
                            Ok(Some(buffer))
                        })
                        .collect::<Result<BinaryArray>>()?,
                );
                result.into()
            }
        })
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
        #[cfg(feature = "sparql-12")]
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
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::TimeLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(1); // Hack to keep datatype
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DateLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(2); // Hack to keep datatype
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GDayLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(3); // Hack to keep datatype
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthDayLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(4); // Hack to keep datatype
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(5); // Hack to keep datatype
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearMonthLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(6); // Hack to keep datatype
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearLiteral(v) => {
            buffer.push(5);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(7); // Hack to keep datatype
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DurationLiteral(v) => {
            buffer.push(6);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(0); // Hack to keep datatype
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::YearMonthDurationLiteral(v) => {
            buffer.push(6);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(1); // Hack to keep datatype
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DayTimeDurationLiteral(v) => {
            buffer.push(6);
            buffer.extend_from_slice(&v.bytes_collation());
            buffer.push(2); // Hack to keep datatype
        }
        #[cfg(feature = "sparql-12")]
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
