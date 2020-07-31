#![allow(clippy::unreadable_literal)]

use crate::error::{invalid_data_error, Infallible, UnwrapInfallible};
use crate::model::vocab::rdf;
use crate::model::vocab::xsd;
use crate::model::xsd::*;
use crate::model::*;
use rand::random;
use rio_api::model as rio;
use siphasher::sip128::{Hasher128, SipHasher24};
use std::collections::HashMap;
use std::error::Error;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Read;
use std::mem::size_of;
use std::{io, str};

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, Hash)]
#[repr(transparent)]
pub struct StrHash {
    hash: u128,
}

impl StrHash {
    pub fn new(value: &str) -> Self {
        let mut hasher = SipHasher24::new();
        hasher.write(value.as_bytes());
        Self {
            hash: hasher.finish128().into(),
        }
    }

    const fn constant(hash: u128) -> Self {
        Self { hash }
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 16]) -> Self {
        Self {
            hash: u128::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn to_be_bytes(&self) -> [u8; 16] {
        self.hash.to_be_bytes()
    }
}

const EMPTY_STRING_ID: StrHash = StrHash::constant(0xf4f2ced447ab02427de0a38047d74950);
const RDF_LANG_STRING_ID: StrHash = StrHash::constant(0x8fab6bc1501d6d114e5d4e0116f67a49);
const XSD_STRING_ID: StrHash = StrHash::constant(0xe72300970ee9bf77f2df7bdb300e3d84);
const XSD_BOOLEAN_ID: StrHash = StrHash::constant(0xfafac8b356be81954f64e70756e59e32);
const XSD_FLOAT_ID: StrHash = StrHash::constant(0x34bd4a8ede4564c36445b76e84fa7502);
const XSD_DOUBLE_ID: StrHash = StrHash::constant(0x3614a889da2f0c7616d96d01b2ff1a97);
const XSD_INTEGER_ID: StrHash = StrHash::constant(0xe2b19c79f5f04dbcdc7f52f4f7869da0);
const XSD_DECIMAL_ID: StrHash = StrHash::constant(0xb50bffedfd084528ff892173dc0d1fad);
const XSD_DATE_TIME_ID: StrHash = StrHash::constant(0xd7496e779a321ade51e92da1a5aa6cb);
const XSD_DATE_ID: StrHash = StrHash::constant(0x87c4351dea4b98f59a22f7b636d4031);
const XSD_TIME_ID: StrHash = StrHash::constant(0xc7487be3f3d27d1926b27abf005a9cd2);
const XSD_DURATION_ID: StrHash = StrHash::constant(0x226af08ea5b7e6b08ceed6030c721228);
const XSD_YEAR_MONTH_DURATION_ID: StrHash = StrHash::constant(0xc6dacde7afc0bd2f6e178d7229948191);
const XSD_DAY_TIME_DURATION_ID: StrHash = StrHash::constant(0xc8d6cfdf45e12c10bd711a76aae43bc6);

const TYPE_DEFAULT_GRAPH_ID: u8 = 0;
const TYPE_NAMED_NODE_ID: u8 = 1;
const TYPE_INLINE_BLANK_NODE_ID: u8 = 2;
const TYPE_NAMED_BLANK_NODE_ID: u8 = 3;
const TYPE_LANG_STRING_LITERAL_ID: u8 = 4;
const TYPE_TYPED_LITERAL_ID: u8 = 5;
const TYPE_STRING_LITERAL: u8 = 6;
const TYPE_BOOLEAN_LITERAL_TRUE: u8 = 7;
const TYPE_BOOLEAN_LITERAL_FALSE: u8 = 8;
const TYPE_FLOAT_LITERAL: u8 = 9;
const TYPE_DOUBLE_LITERAL: u8 = 10;
const TYPE_INTEGER_LITERAL: u8 = 11;
const TYPE_DECIMAL_LITERAL: u8 = 12;
const TYPE_DATE_TIME_LITERAL: u8 = 13;
const TYPE_DATE_LITERAL: u8 = 14;
const TYPE_TIME_LITERAL: u8 = 15;
const TYPE_DURATION_LITERAL: u8 = 16;
const TYPE_YEAR_MONTH_DURATION_LITERAL: u8 = 17;
const TYPE_DAY_TIME_DURATION_LITERAL: u8 = 18;

pub const ENCODED_DEFAULT_GRAPH: EncodedTerm = EncodedTerm::DefaultGraph;
pub const ENCODED_EMPTY_STRING_LITERAL: EncodedTerm = EncodedTerm::StringLiteral {
    value_id: EMPTY_STRING_ID,
};
pub const ENCODED_RDF_LANG_STRING_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: RDF_LANG_STRING_ID,
};
pub const ENCODED_XSD_STRING_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_STRING_ID,
};
pub const ENCODED_XSD_BOOLEAN_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_BOOLEAN_ID,
};
pub const ENCODED_XSD_FLOAT_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_FLOAT_ID,
};
pub const ENCODED_XSD_DOUBLE_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DOUBLE_ID,
};
pub const ENCODED_XSD_INTEGER_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_INTEGER_ID,
};
pub const ENCODED_XSD_DECIMAL_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DECIMAL_ID,
};
pub const ENCODED_XSD_DATE_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DATE_ID,
};
pub const ENCODED_XSD_TIME_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_TIME_ID,
};
pub const ENCODED_XSD_DATE_TIME_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DATE_TIME_ID,
};
pub const ENCODED_XSD_DURATION_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DURATION_ID,
};
pub const ENCODED_XSD_YEAR_MONTH_DURATION_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_YEAR_MONTH_DURATION_ID,
};
pub const ENCODED_XSD_DAY_TIME_DURATION_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DAY_TIME_DURATION_ID,
};

#[derive(Debug, Clone, Copy)]
pub enum EncodedTerm {
    DefaultGraph,
    NamedNode {
        iri_id: StrHash,
    },
    InlineBlankNode {
        id: u128,
    },
    NamedBlankNode {
        id_id: StrHash,
    },
    StringLiteral {
        value_id: StrHash,
    },
    LangStringLiteral {
        value_id: StrHash,
        language_id: StrHash,
    },
    TypedLiteral {
        value_id: StrHash,
        datatype_id: StrHash,
    },
    BooleanLiteral(bool),
    FloatLiteral(f32),
    DoubleLiteral(f64),
    IntegerLiteral(i64),
    DecimalLiteral(Decimal),
    DateLiteral(Date),
    TimeLiteral(Time),
    DateTimeLiteral(DateTime),
    DurationLiteral(Duration),
    YearMonthDurationLiteral(YearMonthDuration),
    DayTimeDurationLiteral(DayTimeDuration),
}

impl PartialEq for EncodedTerm {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (EncodedTerm::DefaultGraph, EncodedTerm::DefaultGraph) => true,
            (
                EncodedTerm::NamedNode { iri_id: iri_id_a },
                EncodedTerm::NamedNode { iri_id: iri_id_b },
            ) => iri_id_a == iri_id_b,
            (
                EncodedTerm::InlineBlankNode { id: id_a },
                EncodedTerm::InlineBlankNode { id: id_b },
            ) => id_a == id_b,
            (
                EncodedTerm::NamedBlankNode { id_id: id_a },
                EncodedTerm::NamedBlankNode { id_id: id_b },
            ) => id_a == id_b,
            (
                EncodedTerm::StringLiteral {
                    value_id: value_id_a,
                },
                EncodedTerm::StringLiteral {
                    value_id: value_id_b,
                },
            ) => value_id_a == value_id_b,
            (
                EncodedTerm::LangStringLiteral {
                    value_id: value_id_a,
                    language_id: language_id_a,
                },
                EncodedTerm::LangStringLiteral {
                    value_id: value_id_b,
                    language_id: language_id_b,
                },
            ) => value_id_a == value_id_b && language_id_a == language_id_b,
            (
                EncodedTerm::TypedLiteral {
                    value_id: value_id_a,
                    datatype_id: datatype_id_a,
                },
                EncodedTerm::TypedLiteral {
                    value_id: value_id_b,
                    datatype_id: datatype_id_b,
                },
            ) => value_id_a == value_id_b && datatype_id_a == datatype_id_b,
            (EncodedTerm::BooleanLiteral(a), EncodedTerm::BooleanLiteral(b)) => a == b,
            (EncodedTerm::FloatLiteral(a), EncodedTerm::FloatLiteral(b)) => {
                if a.is_nan() {
                    b.is_nan()
                } else {
                    a == b
                }
            }
            (EncodedTerm::DoubleLiteral(a), EncodedTerm::DoubleLiteral(b)) => {
                if a.is_nan() {
                    b.is_nan()
                } else {
                    a == b
                }
            }
            (EncodedTerm::IntegerLiteral(a), EncodedTerm::IntegerLiteral(b)) => a == b,
            (EncodedTerm::DecimalLiteral(a), EncodedTerm::DecimalLiteral(b)) => a == b,
            (EncodedTerm::DateLiteral(a), EncodedTerm::DateLiteral(b)) => a == b,
            (EncodedTerm::TimeLiteral(a), EncodedTerm::TimeLiteral(b)) => a == b,
            (EncodedTerm::DateTimeLiteral(a), EncodedTerm::DateTimeLiteral(b)) => a == b,
            (EncodedTerm::DurationLiteral(a), EncodedTerm::DurationLiteral(b)) => a == b,
            (
                EncodedTerm::YearMonthDurationLiteral(a),
                EncodedTerm::YearMonthDurationLiteral(b),
            ) => a == b,
            (EncodedTerm::DayTimeDurationLiteral(a), EncodedTerm::DayTimeDurationLiteral(b)) => {
                a == b
            }
            (_, _) => false,
        }
    }
}

impl Eq for EncodedTerm {}

impl Hash for EncodedTerm {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            EncodedTerm::NamedNode { iri_id } => iri_id.hash(state),
            EncodedTerm::InlineBlankNode { id } => id.hash(state),
            EncodedTerm::NamedBlankNode { id_id } => id_id.hash(state),
            EncodedTerm::DefaultGraph => (),
            EncodedTerm::StringLiteral { value_id } => value_id.hash(state),
            EncodedTerm::LangStringLiteral {
                value_id,
                language_id,
            } => {
                value_id.hash(state);
                language_id.hash(state);
            }
            EncodedTerm::TypedLiteral {
                value_id,
                datatype_id,
            } => {
                value_id.hash(state);
                datatype_id.hash(state);
            }
            EncodedTerm::BooleanLiteral(value) => value.hash(state),
            EncodedTerm::FloatLiteral(value) => state.write(&value.to_ne_bytes()),
            EncodedTerm::DoubleLiteral(value) => state.write(&value.to_ne_bytes()),
            EncodedTerm::IntegerLiteral(value) => value.hash(state),
            EncodedTerm::DecimalLiteral(value) => value.hash(state),
            EncodedTerm::DateLiteral(value) => value.hash(state),
            EncodedTerm::TimeLiteral(value) => value.hash(state),
            EncodedTerm::DateTimeLiteral(value) => value.hash(state),
            EncodedTerm::DurationLiteral(value) => value.hash(state),
            EncodedTerm::YearMonthDurationLiteral(value) => value.hash(state),
            EncodedTerm::DayTimeDurationLiteral(value) => value.hash(state),
        }
    }
}

impl EncodedTerm {
    pub fn is_named_node(&self) -> bool {
        match self {
            EncodedTerm::NamedNode { .. } => true,
            _ => false,
        }
    }

    pub fn is_blank_node(&self) -> bool {
        match self {
            EncodedTerm::InlineBlankNode { .. } | EncodedTerm::NamedBlankNode { .. } => true,
            _ => false,
        }
    }

    pub fn is_literal(&self) -> bool {
        match self {
            EncodedTerm::StringLiteral { .. }
            | EncodedTerm::LangStringLiteral { .. }
            | EncodedTerm::TypedLiteral { .. }
            | EncodedTerm::BooleanLiteral(_)
            | EncodedTerm::FloatLiteral(_)
            | EncodedTerm::DoubleLiteral(_)
            | EncodedTerm::IntegerLiteral(_)
            | EncodedTerm::DecimalLiteral(_)
            | EncodedTerm::DateLiteral(_)
            | EncodedTerm::TimeLiteral(_)
            | EncodedTerm::DateTimeLiteral(_)
            | EncodedTerm::DurationLiteral(_)
            | EncodedTerm::YearMonthDurationLiteral(_)
            | EncodedTerm::DayTimeDurationLiteral(_) => true,
            _ => false,
        }
    }

    pub fn datatype(&self) -> Option<Self> {
        match self {
            EncodedTerm::StringLiteral { .. } => Some(ENCODED_XSD_STRING_NAMED_NODE),
            EncodedTerm::LangStringLiteral { .. } => Some(ENCODED_RDF_LANG_STRING_NAMED_NODE),
            EncodedTerm::TypedLiteral { datatype_id, .. } => Some(EncodedTerm::NamedNode {
                iri_id: *datatype_id,
            }),
            EncodedTerm::BooleanLiteral(..) => Some(ENCODED_XSD_BOOLEAN_NAMED_NODE),
            EncodedTerm::FloatLiteral(..) => Some(ENCODED_XSD_FLOAT_NAMED_NODE),
            EncodedTerm::DoubleLiteral(..) => Some(ENCODED_XSD_DOUBLE_NAMED_NODE),
            EncodedTerm::IntegerLiteral(..) => Some(ENCODED_XSD_INTEGER_NAMED_NODE),
            EncodedTerm::DecimalLiteral(..) => Some(ENCODED_XSD_DECIMAL_NAMED_NODE),
            EncodedTerm::DateLiteral(..) => Some(ENCODED_XSD_DATE_NAMED_NODE),
            EncodedTerm::TimeLiteral(..) => Some(ENCODED_XSD_TIME_NAMED_NODE),
            EncodedTerm::DateTimeLiteral(..) => Some(ENCODED_XSD_DATE_TIME_NAMED_NODE),
            EncodedTerm::DurationLiteral(..) => Some(ENCODED_XSD_DURATION_NAMED_NODE),
            EncodedTerm::YearMonthDurationLiteral(..) => {
                Some(ENCODED_XSD_YEAR_MONTH_DURATION_NAMED_NODE)
            }
            EncodedTerm::DayTimeDurationLiteral(..) => {
                Some(ENCODED_XSD_DAY_TIME_DURATION_NAMED_NODE)
            }
            _ => None,
        }
    }

    fn type_id(&self) -> u8 {
        match self {
            EncodedTerm::DefaultGraph { .. } => TYPE_DEFAULT_GRAPH_ID,
            EncodedTerm::NamedNode { .. } => TYPE_NAMED_NODE_ID,
            EncodedTerm::InlineBlankNode { .. } => TYPE_INLINE_BLANK_NODE_ID,
            EncodedTerm::NamedBlankNode { .. } => TYPE_NAMED_BLANK_NODE_ID,
            EncodedTerm::StringLiteral { .. } => TYPE_STRING_LITERAL,
            EncodedTerm::LangStringLiteral { .. } => TYPE_LANG_STRING_LITERAL_ID,
            EncodedTerm::TypedLiteral { .. } => TYPE_TYPED_LITERAL_ID,
            EncodedTerm::BooleanLiteral(true) => TYPE_BOOLEAN_LITERAL_TRUE,
            EncodedTerm::BooleanLiteral(false) => TYPE_BOOLEAN_LITERAL_FALSE,
            EncodedTerm::FloatLiteral(_) => TYPE_FLOAT_LITERAL,
            EncodedTerm::DoubleLiteral(_) => TYPE_DOUBLE_LITERAL,
            EncodedTerm::IntegerLiteral(_) => TYPE_INTEGER_LITERAL,
            EncodedTerm::DecimalLiteral(_) => TYPE_DECIMAL_LITERAL,
            EncodedTerm::DateLiteral(_) => TYPE_DATE_LITERAL,
            EncodedTerm::TimeLiteral(_) => TYPE_TIME_LITERAL,
            EncodedTerm::DateTimeLiteral(_) => TYPE_DATE_TIME_LITERAL,
            EncodedTerm::DurationLiteral(_) => TYPE_DURATION_LITERAL,
            EncodedTerm::YearMonthDurationLiteral(_) => TYPE_YEAR_MONTH_DURATION_LITERAL,
            EncodedTerm::DayTimeDurationLiteral(_) => TYPE_DAY_TIME_DURATION_LITERAL,
        }
    }
}

impl From<bool> for EncodedTerm {
    fn from(value: bool) -> Self {
        EncodedTerm::BooleanLiteral(value)
    }
}

impl From<i64> for EncodedTerm {
    fn from(value: i64) -> Self {
        EncodedTerm::IntegerLiteral(value)
    }
}

impl From<i32> for EncodedTerm {
    fn from(value: i32) -> Self {
        EncodedTerm::IntegerLiteral(value.into())
    }
}

impl From<u32> for EncodedTerm {
    fn from(value: u32) -> Self {
        EncodedTerm::IntegerLiteral(value.into())
    }
}

impl From<u8> for EncodedTerm {
    fn from(value: u8) -> Self {
        EncodedTerm::IntegerLiteral(value.into())
    }
}
impl From<f32> for EncodedTerm {
    fn from(value: f32) -> Self {
        EncodedTerm::FloatLiteral(value)
    }
}

impl From<f64> for EncodedTerm {
    fn from(value: f64) -> Self {
        EncodedTerm::DoubleLiteral(value)
    }
}

impl From<Decimal> for EncodedTerm {
    fn from(value: Decimal) -> Self {
        EncodedTerm::DecimalLiteral(value)
    }
}

impl From<Date> for EncodedTerm {
    fn from(value: Date) -> Self {
        EncodedTerm::DateLiteral(value)
    }
}

impl From<Time> for EncodedTerm {
    fn from(value: Time) -> Self {
        EncodedTerm::TimeLiteral(value)
    }
}

impl From<DateTime> for EncodedTerm {
    fn from(value: DateTime) -> Self {
        EncodedTerm::DateTimeLiteral(value)
    }
}

impl From<Duration> for EncodedTerm {
    fn from(value: Duration) -> Self {
        EncodedTerm::DurationLiteral(value)
    }
}

impl From<YearMonthDuration> for EncodedTerm {
    fn from(value: YearMonthDuration) -> Self {
        EncodedTerm::YearMonthDurationLiteral(value)
    }
}

impl From<DayTimeDuration> for EncodedTerm {
    fn from(value: DayTimeDuration) -> Self {
        EncodedTerm::DayTimeDurationLiteral(value)
    }
}

impl From<&NamedNode> for EncodedTerm {
    fn from(node: &NamedNode) -> Self {
        rio::NamedNode::from(node).into()
    }
}

impl<'a> From<rio::NamedNode<'a>> for EncodedTerm {
    fn from(node: rio::NamedNode<'a>) -> Self {
        EncodedTerm::NamedNode {
            iri_id: StrHash::new(node.iri),
        }
    }
}

impl From<&BlankNode> for EncodedTerm {
    fn from(node: &BlankNode) -> Self {
        if let Some(id) = node.id() {
            EncodedTerm::InlineBlankNode { id }
        } else {
            EncodedTerm::NamedBlankNode {
                id_id: StrHash::new(node.as_str()),
            }
        }
    }
}

impl From<&Literal> for EncodedTerm {
    fn from(literal: &Literal) -> Self {
        rio::Literal::from(literal).into()
    }
}

impl<'a> From<rio::Literal<'a>> for EncodedTerm {
    fn from(literal: rio::Literal<'a>) -> Self {
        match literal {
            rio::Literal::Simple { value } => EncodedTerm::StringLiteral {
                value_id: StrHash::new(value),
            },
            rio::Literal::LanguageTaggedString { value, language } => {
                EncodedTerm::LangStringLiteral {
                    value_id: StrHash::new(value),
                    language_id: StrHash::new(language),
                }
            }
            rio::Literal::Typed { value, datatype } => {
                match match datatype.iri {
                    "http://www.w3.org/2001/XMLSchema#boolean" => parse_boolean_str(value),
                    "http://www.w3.org/2001/XMLSchema#string" => Some(EncodedTerm::StringLiteral {
                        value_id: StrHash::new(value),
                    }),
                    "http://www.w3.org/2001/XMLSchema#float" => parse_float_str(value),
                    "http://www.w3.org/2001/XMLSchema#double" => parse_double_str(value),
                    "http://www.w3.org/2001/XMLSchema#integer"
                    | "http://www.w3.org/2001/XMLSchema#byte"
                    | "http://www.w3.org/2001/XMLSchema#short"
                    | "http://www.w3.org/2001/XMLSchema#int"
                    | "http://www.w3.org/2001/XMLSchema#long"
                    | "http://www.w3.org/2001/XMLSchema#unsignedByte"
                    | "http://www.w3.org/2001/XMLSchema#unsignedShort"
                    | "http://www.w3.org/2001/XMLSchema#unsignedInt"
                    | "http://www.w3.org/2001/XMLSchema#unsignedLong"
                    | "http://www.w3.org/2001/XMLSchema#positiveInteger"
                    | "http://www.w3.org/2001/XMLSchema#negativeInteger"
                    | "http://www.w3.org/2001/XMLSchema#nonPositiveInteger"
                    | "http://www.w3.org/2001/XMLSchema#nonNegativeInteger" => {
                        parse_integer_str(value)
                    }
                    "http://www.w3.org/2001/XMLSchema#decimal" => parse_decimal_str(value),
                    "http://www.w3.org/2001/XMLSchema#date" => parse_date_str(value),
                    "http://www.w3.org/2001/XMLSchema#time" => parse_time_str(value),
                    "http://www.w3.org/2001/XMLSchema#dateTime"
                    | "http://www.w3.org/2001/XMLSchema#dateTimeStamp" => {
                        parse_date_time_str(value)
                    }
                    "http://www.w3.org/2001/XMLSchema#duration" => parse_duration_str(value),
                    "http://www.w3.org/2001/XMLSchema#yearMonthDuration" => {
                        parse_year_month_duration_str(value)
                    }
                    "http://www.w3.org/2001/XMLSchema#dayTimeDuration" => {
                        parse_day_time_duration_str(value)
                    }
                    _ => None,
                } {
                    Some(v) => v,
                    None => EncodedTerm::TypedLiteral {
                        value_id: StrHash::new(value),
                        datatype_id: StrHash::new(datatype.iri),
                    },
                }
            }
        }
    }
}

impl From<&NamedOrBlankNode> for EncodedTerm {
    fn from(node: &NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(node) => node.into(),
            NamedOrBlankNode::BlankNode(node) => node.into(),
        }
    }
}

impl From<&Term> for EncodedTerm {
    fn from(node: &Term) -> Self {
        match node {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            Term::Literal(literal) => literal.into(),
        }
    }
}

impl From<&GraphName> for EncodedTerm {
    fn from(node: &GraphName) -> Self {
        match node {
            GraphName::NamedNode(node) => node.into(),
            GraphName::BlankNode(node) => node.into(),
            GraphName::DefaultGraph => ENCODED_DEFAULT_GRAPH,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct EncodedQuad {
    pub subject: EncodedTerm,
    pub predicate: EncodedTerm,
    pub object: EncodedTerm,
    pub graph_name: EncodedTerm,
}

impl EncodedQuad {
    pub const fn new(
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Self {
        Self {
            subject,
            predicate,
            object,
            graph_name,
        }
    }
}

impl From<&Quad> for EncodedQuad {
    fn from(quad: &Quad) -> Self {
        Self {
            subject: (&quad.subject).into(),
            predicate: (&quad.predicate).into(),
            object: (&quad.object).into(),
            graph_name: (&quad.graph_name).into(),
        }
    }
}

pub trait TermReader {
    fn read_term(&mut self) -> Result<EncodedTerm, io::Error>;
    fn read_spog_quad(&mut self) -> Result<EncodedQuad, io::Error>;
    fn read_posg_quad(&mut self) -> Result<EncodedQuad, io::Error>;
    fn read_ospg_quad(&mut self) -> Result<EncodedQuad, io::Error>;
    fn read_gspo_quad(&mut self) -> Result<EncodedQuad, io::Error>;
    fn read_gpos_quad(&mut self) -> Result<EncodedQuad, io::Error>;
    fn read_gosp_quad(&mut self) -> Result<EncodedQuad, io::Error>;
}

impl<R: Read> TermReader for R {
    fn read_term(&mut self) -> Result<EncodedTerm, io::Error> {
        let mut type_buffer = [0];
        self.read_exact(&mut type_buffer)?;
        match type_buffer[0] {
            TYPE_DEFAULT_GRAPH_ID => Ok(EncodedTerm::DefaultGraph),
            TYPE_NAMED_NODE_ID => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::NamedNode {
                    iri_id: StrHash::from_be_bytes(buffer),
                })
            }
            TYPE_INLINE_BLANK_NODE_ID => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::InlineBlankNode {
                    id: u128::from_be_bytes(buffer),
                })
            }
            TYPE_NAMED_BLANK_NODE_ID => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::NamedBlankNode {
                    id_id: StrHash::from_be_bytes(buffer),
                })
            }
            TYPE_LANG_STRING_LITERAL_ID => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::LangStringLiteral {
                    language_id: StrHash::from_be_bytes(language_buffer),
                    value_id: StrHash::from_be_bytes(value_buffer),
                })
            }
            TYPE_TYPED_LITERAL_ID => {
                let mut datatype_buffer = [0; 16];
                self.read_exact(&mut datatype_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::TypedLiteral {
                    datatype_id: StrHash::from_be_bytes(datatype_buffer),
                    value_id: StrHash::from_be_bytes(value_buffer),
                })
            }
            TYPE_STRING_LITERAL => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::StringLiteral {
                    value_id: StrHash::from_be_bytes(buffer),
                })
            }
            TYPE_BOOLEAN_LITERAL_TRUE => Ok(EncodedTerm::BooleanLiteral(true)),
            TYPE_BOOLEAN_LITERAL_FALSE => Ok(EncodedTerm::BooleanLiteral(false)),
            TYPE_FLOAT_LITERAL => {
                let mut buffer = [0; 4];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::FloatLiteral(f32::from_be_bytes(buffer)))
            }
            TYPE_DOUBLE_LITERAL => {
                let mut buffer = [0; 8];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DoubleLiteral(f64::from_be_bytes(buffer)))
            }
            TYPE_INTEGER_LITERAL => {
                let mut buffer = [0; 8];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::IntegerLiteral(i64::from_be_bytes(buffer)))
            }
            TYPE_DECIMAL_LITERAL => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DecimalLiteral(Decimal::from_be_bytes(buffer)))
            }
            TYPE_DATE_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DateLiteral(Date::from_be_bytes(buffer)))
            }
            TYPE_TIME_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::TimeLiteral(Time::from_be_bytes(buffer)))
            }
            TYPE_DATE_TIME_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DateTimeLiteral(DateTime::from_be_bytes(
                    buffer,
                )))
            }
            TYPE_DURATION_LITERAL => {
                let mut buffer = [0; 24];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DurationLiteral(Duration::from_be_bytes(
                    buffer,
                )))
            }
            TYPE_YEAR_MONTH_DURATION_LITERAL => {
                let mut buffer = [0; 8];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::YearMonthDurationLiteral(
                    YearMonthDuration::from_be_bytes(buffer),
                ))
            }
            TYPE_DAY_TIME_DURATION_LITERAL => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DayTimeDurationLiteral(
                    DayTimeDuration::from_be_bytes(buffer),
                ))
            }
            _ => Err(invalid_data_error("the term buffer has an invalid type id")),
        }
    }

    fn read_spog_quad(&mut self) -> Result<EncodedQuad, io::Error> {
        let subject = self.read_term()?;
        let predicate = self.read_term()?;
        let object = self.read_term()?;
        let graph_name = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    fn read_posg_quad(&mut self) -> Result<EncodedQuad, io::Error> {
        let predicate = self.read_term()?;
        let object = self.read_term()?;
        let subject = self.read_term()?;
        let graph_name = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    fn read_ospg_quad(&mut self) -> Result<EncodedQuad, io::Error> {
        let object = self.read_term()?;
        let subject = self.read_term()?;
        let predicate = self.read_term()?;
        let graph_name = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    fn read_gspo_quad(&mut self) -> Result<EncodedQuad, io::Error> {
        let graph_name = self.read_term()?;
        let subject = self.read_term()?;
        let predicate = self.read_term()?;
        let object = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    fn read_gpos_quad(&mut self) -> Result<EncodedQuad, io::Error> {
        let graph_name = self.read_term()?;
        let predicate = self.read_term()?;
        let object = self.read_term()?;
        let subject = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    fn read_gosp_quad(&mut self) -> Result<EncodedQuad, io::Error> {
        let graph_name = self.read_term()?;
        let object = self.read_term()?;
        let subject = self.read_term()?;
        let predicate = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name,
        })
    }
}

pub const WRITTEN_TERM_MAX_SIZE: usize = size_of::<u8>() + 2 * size_of::<StrHash>();

pub fn write_term(sink: &mut Vec<u8>, term: EncodedTerm) {
    sink.push(term.type_id());
    match term {
        EncodedTerm::DefaultGraph => {}
        EncodedTerm::NamedNode { iri_id } => sink.extend_from_slice(&iri_id.to_be_bytes()),
        EncodedTerm::InlineBlankNode { id } => sink.extend_from_slice(&id.to_be_bytes()),
        EncodedTerm::NamedBlankNode { id_id } => sink.extend_from_slice(&id_id.to_be_bytes()),
        EncodedTerm::StringLiteral { value_id } => sink.extend_from_slice(&value_id.to_be_bytes()),
        EncodedTerm::LangStringLiteral {
            value_id,
            language_id,
        } => {
            sink.extend_from_slice(&language_id.to_be_bytes());
            sink.extend_from_slice(&value_id.to_be_bytes());
        }
        EncodedTerm::TypedLiteral {
            value_id,
            datatype_id,
        } => {
            sink.extend_from_slice(&datatype_id.to_be_bytes());
            sink.extend_from_slice(&value_id.to_be_bytes());
        }
        EncodedTerm::BooleanLiteral(_) => {}
        EncodedTerm::FloatLiteral(value) => sink.extend_from_slice(&value.to_be_bytes()),
        EncodedTerm::DoubleLiteral(value) => sink.extend_from_slice(&value.to_be_bytes()),
        EncodedTerm::IntegerLiteral(value) => sink.extend_from_slice(&value.to_be_bytes()),
        EncodedTerm::DecimalLiteral(value) => sink.extend_from_slice(&value.to_be_bytes()),
        EncodedTerm::DateLiteral(value) => sink.extend_from_slice(&value.to_be_bytes()),
        EncodedTerm::TimeLiteral(value) => sink.extend_from_slice(&value.to_be_bytes()),
        EncodedTerm::DateTimeLiteral(value) => sink.extend_from_slice(&value.to_be_bytes()),
        EncodedTerm::DurationLiteral(value) => sink.extend_from_slice(&value.to_be_bytes()),
        EncodedTerm::YearMonthDurationLiteral(value) => {
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::DayTimeDurationLiteral(value) => sink.extend_from_slice(&value.to_be_bytes()),
    }
}

pub(crate) trait WithStoreError {
    type Error: Error + Into<io::Error> + 'static;
}

pub(crate) trait StrLookup: WithStoreError {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, Self::Error>;
}

pub(crate) trait StrContainer: WithStoreError {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<(), Self::Error>;

    /// Should be called when the bytes store is created
    fn set_first_strings(&mut self) -> Result<(), Self::Error> {
        self.insert_str(EMPTY_STRING_ID, "")?;
        self.insert_str(RDF_LANG_STRING_ID, rdf::LANG_STRING.as_str())?;
        self.insert_str(XSD_STRING_ID, xsd::STRING.as_str())?;
        self.insert_str(XSD_BOOLEAN_ID, xsd::BOOLEAN.as_str())?;
        self.insert_str(XSD_FLOAT_ID, xsd::FLOAT.as_str())?;
        self.insert_str(XSD_DOUBLE_ID, xsd::DOUBLE.as_str())?;
        self.insert_str(XSD_INTEGER_ID, xsd::INTEGER.as_str())?;
        self.insert_str(XSD_DECIMAL_ID, xsd::DECIMAL.as_str())?;
        self.insert_str(XSD_DATE_TIME_ID, xsd::DATE_TIME.as_str())?;
        self.insert_str(XSD_DATE_ID, xsd::DATE.as_str())?;
        self.insert_str(XSD_TIME_ID, xsd::TIME.as_str())?;
        self.insert_str(XSD_DURATION_ID, xsd::DURATION.as_str())?;
        self.insert_str(
            XSD_YEAR_MONTH_DURATION_ID,
            xsd::YEAR_MONTH_DURATION.as_str(),
        )?;
        self.insert_str(XSD_DAY_TIME_DURATION_ID, xsd::DAY_TIME_DURATION.as_str())?;
        Ok(())
    }
}

pub struct MemoryStrStore {
    id2str: HashMap<StrHash, String>,
}

impl Default for MemoryStrStore {
    fn default() -> Self {
        let mut new = Self {
            id2str: HashMap::default(),
        };
        new.set_first_strings().unwrap_infallible();
        new
    }
}

impl WithStoreError for MemoryStrStore {
    type Error = Infallible;
}

impl StrLookup for MemoryStrStore {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, Infallible> {
        //TODO: avoid copy by adding a lifetime limit to get_str
        Ok(self.id2str.get(&id).cloned())
    }
}

impl StrContainer for MemoryStrStore {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<(), Infallible> {
        self.id2str.entry(key).or_insert_with(|| value.to_owned());
        Ok(())
    }
}

pub(crate) trait Encoder: WithStoreError {
    fn encode_named_node(&mut self, named_node: &NamedNode) -> Result<EncodedTerm, Self::Error> {
        self.encode_rio_named_node(named_node.into())
    }

    fn encode_blank_node(&mut self, blank_node: &BlankNode) -> Result<EncodedTerm, Self::Error>;

    fn encode_literal(&mut self, literal: &Literal) -> Result<EncodedTerm, Self::Error> {
        self.encode_rio_literal(literal.into())
    }

    fn encode_named_or_blank_node(
        &mut self,
        term: &NamedOrBlankNode,
    ) -> Result<EncodedTerm, Self::Error> {
        match term {
            NamedOrBlankNode::NamedNode(named_node) => self.encode_named_node(named_node),
            NamedOrBlankNode::BlankNode(blank_node) => self.encode_blank_node(blank_node),
        }
    }

    fn encode_term(&mut self, term: &Term) -> Result<EncodedTerm, Self::Error> {
        match term {
            Term::NamedNode(named_node) => self.encode_named_node(named_node),
            Term::BlankNode(blank_node) => self.encode_blank_node(blank_node),
            Term::Literal(literal) => self.encode_literal(literal),
        }
    }

    fn encode_graph_name(&mut self, name: &GraphName) -> Result<EncodedTerm, Self::Error> {
        match name {
            GraphName::NamedNode(named_node) => self.encode_named_node(named_node),
            GraphName::BlankNode(blank_node) => self.encode_blank_node(blank_node),
            GraphName::DefaultGraph => Ok(ENCODED_DEFAULT_GRAPH),
        }
    }

    fn encode_quad(&mut self, quad: &Quad) -> Result<EncodedQuad, Self::Error> {
        Ok(EncodedQuad {
            subject: self.encode_named_or_blank_node(&quad.subject)?,
            predicate: self.encode_named_node(&quad.predicate)?,
            object: self.encode_term(&quad.object)?,
            graph_name: self.encode_graph_name(&quad.graph_name)?,
        })
    }

    fn encode_triple_in_graph(
        &mut self,
        triple: &Triple,
        graph_name: EncodedTerm,
    ) -> Result<EncodedQuad, Self::Error> {
        Ok(EncodedQuad {
            subject: self.encode_named_or_blank_node(&triple.subject)?,
            predicate: self.encode_named_node(&triple.predicate)?,
            object: self.encode_term(&triple.object)?,
            graph_name,
        })
    }

    fn encode_rio_named_node(
        &mut self,
        named_node: rio::NamedNode<'_>,
    ) -> Result<EncodedTerm, Self::Error>;

    fn encode_rio_blank_node(
        &mut self,
        blank_node: rio::BlankNode<'_>,
        bnodes_map: &mut HashMap<String, u128>,
    ) -> Result<EncodedTerm, Self::Error>;

    fn encode_rio_literal(&mut self, literal: rio::Literal<'_>)
        -> Result<EncodedTerm, Self::Error>;

    fn encode_rio_named_or_blank_node(
        &mut self,
        term: rio::NamedOrBlankNode<'_>,
        bnodes_map: &mut HashMap<String, u128>,
    ) -> Result<EncodedTerm, Self::Error> {
        match term {
            rio::NamedOrBlankNode::NamedNode(named_node) => self.encode_rio_named_node(named_node),
            rio::NamedOrBlankNode::BlankNode(blank_node) => {
                self.encode_rio_blank_node(blank_node, bnodes_map)
            }
        }
    }

    fn encode_rio_term(
        &mut self,
        term: rio::Term<'_>,
        bnodes_map: &mut HashMap<String, u128>,
    ) -> Result<EncodedTerm, Self::Error> {
        match term {
            rio::Term::NamedNode(named_node) => self.encode_rio_named_node(named_node),
            rio::Term::BlankNode(blank_node) => self.encode_rio_blank_node(blank_node, bnodes_map),
            rio::Term::Literal(literal) => self.encode_rio_literal(literal),
        }
    }

    fn encode_rio_quad(
        &mut self,
        quad: rio::Quad<'_>,
        bnodes_map: &mut HashMap<String, u128>,
    ) -> Result<EncodedQuad, Self::Error> {
        Ok(EncodedQuad {
            subject: self.encode_rio_named_or_blank_node(quad.subject, bnodes_map)?,
            predicate: self.encode_rio_named_node(quad.predicate)?,
            object: self.encode_rio_term(quad.object, bnodes_map)?,
            graph_name: match quad.graph_name {
                Some(graph_name) => self.encode_rio_named_or_blank_node(graph_name, bnodes_map)?,
                None => ENCODED_DEFAULT_GRAPH,
            },
        })
    }

    fn encode_rio_triple_in_graph(
        &mut self,
        triple: rio::Triple<'_>,
        graph_name: EncodedTerm,
        bnodes_map: &mut HashMap<String, u128>,
    ) -> Result<EncodedQuad, Self::Error> {
        Ok(EncodedQuad {
            subject: self.encode_rio_named_or_blank_node(triple.subject, bnodes_map)?,
            predicate: self.encode_rio_named_node(triple.predicate)?,
            object: self.encode_rio_term(triple.object, bnodes_map)?,
            graph_name,
        })
    }
}

impl<S: StrContainer> Encoder for S {
    fn encode_rio_named_node(
        &mut self,
        named_node: rio::NamedNode<'_>,
    ) -> Result<EncodedTerm, Self::Error> {
        let iri_id = StrHash::new(named_node.iri);
        self.insert_str(iri_id, named_node.iri)?;
        Ok(EncodedTerm::NamedNode { iri_id })
    }

    fn encode_blank_node(&mut self, blank_node: &BlankNode) -> Result<EncodedTerm, Self::Error> {
        if let Some(id) = blank_node.id() {
            Ok(EncodedTerm::InlineBlankNode { id })
        } else {
            let id = blank_node.as_str();
            let id_id = StrHash::new(id);
            self.insert_str(id_id, id)?;
            Ok(EncodedTerm::NamedBlankNode { id_id })
        }
    }

    fn encode_rio_blank_node(
        &mut self,
        blank_node: rio::BlankNode<'_>,
        bnodes_map: &mut HashMap<String, u128>,
    ) -> Result<EncodedTerm, Self::Error> {
        Ok(if let Some(id) = bnodes_map.get(blank_node.id) {
            EncodedTerm::InlineBlankNode { id: *id }
        } else {
            let id = random::<u128>();
            bnodes_map.insert(blank_node.id.to_owned(), id);
            EncodedTerm::InlineBlankNode { id }
        })
    }

    fn encode_rio_literal(
        &mut self,
        literal: rio::Literal<'_>,
    ) -> Result<EncodedTerm, Self::Error> {
        Ok(match literal {
            rio::Literal::Simple { value } => {
                let value_id = StrHash::new(value);
                self.insert_str(value_id, value)?;
                EncodedTerm::StringLiteral { value_id }
            }
            rio::Literal::LanguageTaggedString { value, language } => {
                let value_id = StrHash::new(value);
                self.insert_str(value_id, value)?;
                let language_id = StrHash::new(language);
                self.insert_str(language_id, language)?;
                EncodedTerm::LangStringLiteral {
                    value_id,
                    language_id,
                }
            }
            rio::Literal::Typed { value, datatype } => {
                match match datatype.iri {
                    "http://www.w3.org/2001/XMLSchema#boolean" => parse_boolean_str(value),
                    "http://www.w3.org/2001/XMLSchema#string" => {
                        let value_id = StrHash::new(value);
                        self.insert_str(value_id, value)?;
                        Some(EncodedTerm::StringLiteral { value_id })
                    }
                    "http://www.w3.org/2001/XMLSchema#float" => parse_float_str(value),
                    "http://www.w3.org/2001/XMLSchema#double" => parse_double_str(value),
                    "http://www.w3.org/2001/XMLSchema#integer"
                    | "http://www.w3.org/2001/XMLSchema#byte"
                    | "http://www.w3.org/2001/XMLSchema#short"
                    | "http://www.w3.org/2001/XMLSchema#int"
                    | "http://www.w3.org/2001/XMLSchema#long"
                    | "http://www.w3.org/2001/XMLSchema#unsignedByte"
                    | "http://www.w3.org/2001/XMLSchema#unsignedShort"
                    | "http://www.w3.org/2001/XMLSchema#unsignedInt"
                    | "http://www.w3.org/2001/XMLSchema#unsignedLong"
                    | "http://www.w3.org/2001/XMLSchema#positiveInteger"
                    | "http://www.w3.org/2001/XMLSchema#negativeInteger"
                    | "http://www.w3.org/2001/XMLSchema#nonPositiveInteger"
                    | "http://www.w3.org/2001/XMLSchema#nonNegativeInteger" => {
                        parse_integer_str(value)
                    }
                    "http://www.w3.org/2001/XMLSchema#decimal" => parse_decimal_str(value),
                    "http://www.w3.org/2001/XMLSchema#date" => parse_date_str(value),
                    "http://www.w3.org/2001/XMLSchema#time" => parse_time_str(value),
                    "http://www.w3.org/2001/XMLSchema#dateTime"
                    | "http://www.w3.org/2001/XMLSchema#dateTimeStamp" => {
                        parse_date_time_str(value)
                    }
                    "http://www.w3.org/2001/XMLSchema#duration" => parse_duration_str(value),
                    "http://www.w3.org/2001/XMLSchema#yearMonthDuration" => {
                        parse_year_month_duration_str(value)
                    }
                    "http://www.w3.org/2001/XMLSchema#dayTimeDuration" => {
                        parse_day_time_duration_str(value)
                    }
                    _ => None,
                } {
                    Some(v) => v,
                    None => {
                        let value_id = StrHash::new(value);
                        self.insert_str(value_id, value)?;
                        let datatype_id = StrHash::new(datatype.iri);
                        self.insert_str(datatype_id, datatype.iri)?;
                        EncodedTerm::TypedLiteral {
                            value_id,
                            datatype_id,
                        }
                    }
                }
            }
        })
    }
}

pub fn parse_boolean_str(value: &str) -> Option<EncodedTerm> {
    match value {
        "true" | "1" => Some(EncodedTerm::BooleanLiteral(true)),
        "false" | "0" => Some(EncodedTerm::BooleanLiteral(false)),
        _ => None,
    }
}

pub fn parse_float_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::FloatLiteral).ok()
}

pub fn parse_double_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::DoubleLiteral).ok()
}

pub fn parse_integer_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::IntegerLiteral).ok()
}

pub fn parse_decimal_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::DecimalLiteral).ok()
}

pub fn parse_date_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::DateLiteral).ok()
}

pub fn parse_time_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::TimeLiteral).ok()
}

pub fn parse_date_time_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::DateTimeLiteral).ok()
}

pub fn parse_duration_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::DurationLiteral).ok()
}

pub fn parse_year_month_duration_str(value: &str) -> Option<EncodedTerm> {
    value
        .parse()
        .map(EncodedTerm::YearMonthDurationLiteral)
        .ok()
}

pub fn parse_day_time_duration_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::DayTimeDurationLiteral).ok()
}

pub(crate) trait Decoder {
    fn decode_term(&self, encoded: EncodedTerm) -> Result<Term, io::Error>;

    fn decode_named_or_blank_node(
        &self,
        encoded: EncodedTerm,
    ) -> Result<NamedOrBlankNode, io::Error> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node.into()),
            Term::BlankNode(blank_node) => Ok(blank_node.into()),
            Term::Literal(_) => Err(invalid_data_error(
                "A literal has ben found instead of a named node",
            )),
        }
    }

    fn decode_named_node(&self, encoded: EncodedTerm) -> Result<NamedNode, io::Error> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node),
            Term::BlankNode(_) => Err(invalid_data_error(
                "A blank node has been found instead of a named node",
            )),
            Term::Literal(_) => Err(invalid_data_error(
                "A literal has ben found instead of a named node",
            )),
        }
    }

    fn decode_triple(&self, encoded: &EncodedQuad) -> Result<Triple, io::Error> {
        Ok(Triple::new(
            self.decode_named_or_blank_node(encoded.subject)?,
            self.decode_named_node(encoded.predicate)?,
            self.decode_term(encoded.object)?,
        ))
    }

    fn decode_quad(&self, encoded: &EncodedQuad) -> Result<Quad, io::Error> {
        Ok(Quad::new(
            self.decode_named_or_blank_node(encoded.subject)?,
            self.decode_named_node(encoded.predicate)?,
            self.decode_term(encoded.object)?,
            match encoded.graph_name {
                EncodedTerm::DefaultGraph => None,
                graph_name => Some(self.decode_named_or_blank_node(graph_name)?),
            },
        ))
    }
}

impl<S: StrLookup> Decoder for S {
    fn decode_term(&self, encoded: EncodedTerm) -> Result<Term, io::Error> {
        match encoded {
            EncodedTerm::DefaultGraph => Err(invalid_data_error(
                "The default graph tag is not a valid term",
            )),
            EncodedTerm::NamedNode { iri_id } => {
                Ok(NamedNode::new_unchecked(get_required_str(self, iri_id)?).into())
            }
            EncodedTerm::InlineBlankNode { id } => Ok(BlankNode::new_from_unique_id(id).into()),
            EncodedTerm::NamedBlankNode { id_id } => {
                Ok(BlankNode::new_unchecked(get_required_str(self, id_id)?).into())
            }
            EncodedTerm::StringLiteral { value_id } => {
                Ok(Literal::new_simple_literal(get_required_str(self, value_id)?).into())
            }
            EncodedTerm::LangStringLiteral {
                value_id,
                language_id,
            } => Ok(Literal::new_language_tagged_literal_unchecked(
                get_required_str(self, value_id)?,
                get_required_str(self, language_id)?,
            )
            .into()),
            EncodedTerm::TypedLiteral {
                value_id,
                datatype_id,
            } => Ok(Literal::new_typed_literal(
                get_required_str(self, value_id)?,
                NamedNode::new_unchecked(get_required_str(self, datatype_id)?),
            )
            .into()),
            EncodedTerm::BooleanLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::FloatLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DoubleLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::IntegerLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DecimalLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DateLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::TimeLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DateTimeLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DurationLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::YearMonthDurationLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DayTimeDurationLiteral(value) => Ok(Literal::from(value).into()),
        }
    }
}

fn get_required_str(lookup: &impl StrLookup, id: StrHash) -> Result<String, io::Error> {
    lookup.get_str(id).map_err(|e| e.into())?.ok_or_else(|| {
        invalid_data_error(format!(
            "Not able to find the string with id {:?} in the string store",
            id
        ))
    })
}

#[test]
fn test_encoding() {
    let mut store = MemoryStrStore::default();
    let terms: Vec<Term> = vec![
        NamedNode::new_unchecked("http://foo.com").into(),
        NamedNode::new_unchecked("http://bar.com").into(),
        NamedNode::new_unchecked("http://foo.com").into(),
        BlankNode::default().into(),
        BlankNode::new_unchecked("foo-bnode").into(),
        Literal::new_simple_literal("foo-literal").into(),
        Literal::from(true).into(),
        Literal::from(1.2).into(),
        Literal::from(1).into(),
        Literal::from("foo-string").into(),
        Literal::new_language_tagged_literal("foo-fr", "fr")
            .unwrap()
            .into(),
        Literal::new_language_tagged_literal("foo-FR", "FR")
            .unwrap()
            .into(),
        Literal::new_typed_literal("-1.32", xsd::DECIMAL.clone()).into(),
        Literal::new_typed_literal("2020-01-01T01:01:01Z", xsd::DATE_TIME.clone()).into(),
        Literal::new_typed_literal("2020-01-01", xsd::DATE.clone()).into(),
        Literal::new_typed_literal("01:01:01Z", xsd::TIME.clone()).into(),
        Literal::new_typed_literal("PT1S", xsd::DURATION.clone()).into(),
        Literal::new_typed_literal("-foo", NamedNode::new_unchecked("http://foo.com")).into(),
    ];
    for term in terms {
        let encoded = store.encode_term(&term).unwrap();
        assert_eq!(term, store.decode_term(encoded).unwrap());
        assert_eq!(encoded, EncodedTerm::from(&term));
    }
}

#[test]
fn test_str_hash() {
    assert_eq!(StrHash::new(""), EMPTY_STRING_ID);
    assert_eq!(
        StrHash::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString"),
        RDF_LANG_STRING_ID
    );
}
