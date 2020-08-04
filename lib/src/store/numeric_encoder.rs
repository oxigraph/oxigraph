#![allow(clippy::unreadable_literal)]

use crate::error::invalid_data_error;
use crate::model::xsd::*;
use crate::model::*;
use crate::sparql::EvaluationError;
use rand::random;
use rio_api::model as rio;
use siphasher::sip128::{Hasher128, SipHasher24};
use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Read;
use std::mem::size_of;
use std::{fmt, io, str};

pub trait StrId: Eq + Debug + Copy + Hash {}

pub trait SerializableStrId: StrId {
    fn len() -> usize;

    fn from_be_bytes(bytes: &[u8]) -> Self;

    fn push_be_bytes(&self, buffer: &mut Vec<u8>);
}

#[derive(Eq, PartialEq, Debug, Copy, Clone, Hash)]
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

impl StrId for StrHash {}

impl SerializableStrId for StrHash {
    fn len() -> usize {
        16
    }

    fn from_be_bytes(bytes: &[u8]) -> Self {
        let mut hash = [0; 16];
        hash.copy_from_slice(bytes);
        Self {
            hash: u128::from_be_bytes(hash),
        }
    }

    fn push_be_bytes(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.to_be_bytes())
    }
}

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

#[derive(Debug, Clone, Copy)]
pub enum EncodedTerm<I: StrId> {
    DefaultGraph,
    NamedNode { iri_id: I },
    InlineBlankNode { id: u128 },
    NamedBlankNode { id_id: I },
    StringLiteral { value_id: I },
    LangStringLiteral { value_id: I, language_id: I },
    TypedLiteral { value_id: I, datatype_id: I },
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

impl<I: StrId> PartialEq for EncodedTerm<I> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::DefaultGraph, Self::DefaultGraph) => true,
            (Self::NamedNode { iri_id: iri_id_a }, Self::NamedNode { iri_id: iri_id_b }) => {
                iri_id_a == iri_id_b
            }
            (Self::InlineBlankNode { id: id_a }, Self::InlineBlankNode { id: id_b }) => {
                id_a == id_b
            }
            (Self::NamedBlankNode { id_id: id_a }, Self::NamedBlankNode { id_id: id_b }) => {
                id_a == id_b
            }
            (
                Self::StringLiteral {
                    value_id: value_id_a,
                },
                Self::StringLiteral {
                    value_id: value_id_b,
                },
            ) => value_id_a == value_id_b,
            (
                Self::LangStringLiteral {
                    value_id: value_id_a,
                    language_id: language_id_a,
                },
                Self::LangStringLiteral {
                    value_id: value_id_b,
                    language_id: language_id_b,
                },
            ) => value_id_a == value_id_b && language_id_a == language_id_b,
            (
                Self::TypedLiteral {
                    value_id: value_id_a,
                    datatype_id: datatype_id_a,
                },
                Self::TypedLiteral {
                    value_id: value_id_b,
                    datatype_id: datatype_id_b,
                },
            ) => value_id_a == value_id_b && datatype_id_a == datatype_id_b,
            (Self::BooleanLiteral(a), Self::BooleanLiteral(b)) => a == b,
            (Self::FloatLiteral(a), Self::FloatLiteral(b)) => {
                if a.is_nan() {
                    b.is_nan()
                } else {
                    a == b
                }
            }
            (Self::DoubleLiteral(a), Self::DoubleLiteral(b)) => {
                if a.is_nan() {
                    b.is_nan()
                } else {
                    a == b
                }
            }
            (Self::IntegerLiteral(a), Self::IntegerLiteral(b)) => a == b,
            (Self::DecimalLiteral(a), Self::DecimalLiteral(b)) => a == b,
            (Self::DateLiteral(a), Self::DateLiteral(b)) => a == b,
            (Self::TimeLiteral(a), Self::TimeLiteral(b)) => a == b,
            (Self::DateTimeLiteral(a), Self::DateTimeLiteral(b)) => a == b,
            (Self::DurationLiteral(a), Self::DurationLiteral(b)) => a == b,
            (Self::YearMonthDurationLiteral(a), Self::YearMonthDurationLiteral(b)) => a == b,
            (Self::DayTimeDurationLiteral(a), Self::DayTimeDurationLiteral(b)) => a == b,
            (_, _) => false,
        }
    }
}

impl<I: StrId> Eq for EncodedTerm<I> {}

impl<I: StrId> Hash for EncodedTerm<I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::NamedNode { iri_id } => iri_id.hash(state),
            Self::InlineBlankNode { id } => id.hash(state),
            Self::NamedBlankNode { id_id } => id_id.hash(state),
            Self::DefaultGraph => (),
            Self::StringLiteral { value_id } => value_id.hash(state),
            Self::LangStringLiteral {
                value_id,
                language_id,
            } => {
                value_id.hash(state);
                language_id.hash(state);
            }
            Self::TypedLiteral {
                value_id,
                datatype_id,
            } => {
                value_id.hash(state);
                datatype_id.hash(state);
            }
            Self::BooleanLiteral(value) => value.hash(state),
            Self::FloatLiteral(value) => state.write(&value.to_ne_bytes()),
            Self::DoubleLiteral(value) => state.write(&value.to_ne_bytes()),
            Self::IntegerLiteral(value) => value.hash(state),
            Self::DecimalLiteral(value) => value.hash(state),
            Self::DateLiteral(value) => value.hash(state),
            Self::TimeLiteral(value) => value.hash(state),
            Self::DateTimeLiteral(value) => value.hash(state),
            Self::DurationLiteral(value) => value.hash(state),
            Self::YearMonthDurationLiteral(value) => value.hash(state),
            Self::DayTimeDurationLiteral(value) => value.hash(state),
        }
    }
}

impl<I: StrId> EncodedTerm<I> {
    pub fn is_named_node(&self) -> bool {
        match self {
            Self::NamedNode { .. } => true,
            _ => false,
        }
    }

    pub fn is_blank_node(&self) -> bool {
        match self {
            Self::InlineBlankNode { .. } | Self::NamedBlankNode { .. } => true,
            _ => false,
        }
    }

    pub fn is_literal(&self) -> bool {
        match self {
            Self::StringLiteral { .. }
            | Self::LangStringLiteral { .. }
            | Self::TypedLiteral { .. }
            | Self::BooleanLiteral(_)
            | Self::FloatLiteral(_)
            | Self::DoubleLiteral(_)
            | Self::IntegerLiteral(_)
            | Self::DecimalLiteral(_)
            | Self::DateLiteral(_)
            | Self::TimeLiteral(_)
            | Self::DateTimeLiteral(_)
            | Self::DurationLiteral(_)
            | Self::YearMonthDurationLiteral(_)
            | Self::DayTimeDurationLiteral(_) => true,
            _ => false,
        }
    }

    fn type_id(&self) -> u8 {
        match self {
            Self::DefaultGraph { .. } => TYPE_DEFAULT_GRAPH_ID,
            Self::NamedNode { .. } => TYPE_NAMED_NODE_ID,
            Self::InlineBlankNode { .. } => TYPE_INLINE_BLANK_NODE_ID,
            Self::NamedBlankNode { .. } => TYPE_NAMED_BLANK_NODE_ID,
            Self::StringLiteral { .. } => TYPE_STRING_LITERAL,
            Self::LangStringLiteral { .. } => TYPE_LANG_STRING_LITERAL_ID,
            Self::TypedLiteral { .. } => TYPE_TYPED_LITERAL_ID,
            Self::BooleanLiteral(true) => TYPE_BOOLEAN_LITERAL_TRUE,
            Self::BooleanLiteral(false) => TYPE_BOOLEAN_LITERAL_FALSE,
            Self::FloatLiteral(_) => TYPE_FLOAT_LITERAL,
            Self::DoubleLiteral(_) => TYPE_DOUBLE_LITERAL,
            Self::IntegerLiteral(_) => TYPE_INTEGER_LITERAL,
            Self::DecimalLiteral(_) => TYPE_DECIMAL_LITERAL,
            Self::DateLiteral(_) => TYPE_DATE_LITERAL,
            Self::TimeLiteral(_) => TYPE_TIME_LITERAL,
            Self::DateTimeLiteral(_) => TYPE_DATE_TIME_LITERAL,
            Self::DurationLiteral(_) => TYPE_DURATION_LITERAL,
            Self::YearMonthDurationLiteral(_) => TYPE_YEAR_MONTH_DURATION_LITERAL,
            Self::DayTimeDurationLiteral(_) => TYPE_DAY_TIME_DURATION_LITERAL,
        }
    }

    pub fn map_id<J: StrId>(self, mapping: impl Fn(I) -> J) -> EncodedTerm<J> {
        match self {
            Self::DefaultGraph { .. } => EncodedTerm::DefaultGraph,
            Self::NamedNode { iri_id } => EncodedTerm::NamedNode {
                iri_id: mapping(iri_id),
            },
            Self::InlineBlankNode { id } => EncodedTerm::InlineBlankNode { id },
            Self::NamedBlankNode { id_id } => EncodedTerm::NamedBlankNode {
                id_id: mapping(id_id),
            },
            Self::StringLiteral { value_id } => EncodedTerm::StringLiteral {
                value_id: mapping(value_id),
            },
            Self::LangStringLiteral {
                value_id,
                language_id,
            } => EncodedTerm::LangStringLiteral {
                value_id: mapping(value_id),
                language_id: mapping(language_id),
            },
            Self::TypedLiteral {
                value_id,
                datatype_id,
            } => EncodedTerm::TypedLiteral {
                value_id: mapping(value_id),
                datatype_id: mapping(datatype_id),
            },
            Self::BooleanLiteral(value) => EncodedTerm::BooleanLiteral(value),
            Self::FloatLiteral(value) => EncodedTerm::FloatLiteral(value),
            Self::DoubleLiteral(value) => EncodedTerm::DoubleLiteral(value),
            Self::IntegerLiteral(value) => EncodedTerm::IntegerLiteral(value),
            Self::DecimalLiteral(value) => EncodedTerm::DecimalLiteral(value),
            Self::DateLiteral(value) => EncodedTerm::DateLiteral(value),
            Self::TimeLiteral(value) => EncodedTerm::TimeLiteral(value),
            Self::DateTimeLiteral(value) => EncodedTerm::DateTimeLiteral(value),
            Self::DurationLiteral(value) => EncodedTerm::DurationLiteral(value),
            Self::YearMonthDurationLiteral(value) => EncodedTerm::YearMonthDurationLiteral(value),
            Self::DayTimeDurationLiteral(value) => EncodedTerm::DayTimeDurationLiteral(value),
        }
    }

    pub fn try_map_id<J: StrId>(self, mapping: impl Fn(I) -> Option<J>) -> Option<EncodedTerm<J>> {
        Some(match self {
            Self::DefaultGraph { .. } => EncodedTerm::DefaultGraph,
            Self::NamedNode { iri_id } => EncodedTerm::NamedNode {
                iri_id: mapping(iri_id)?,
            },
            Self::InlineBlankNode { id } => EncodedTerm::InlineBlankNode { id },
            Self::NamedBlankNode { id_id } => EncodedTerm::NamedBlankNode {
                id_id: mapping(id_id)?,
            },
            Self::StringLiteral { value_id } => EncodedTerm::StringLiteral {
                value_id: mapping(value_id)?,
            },
            Self::LangStringLiteral {
                value_id,
                language_id,
            } => EncodedTerm::LangStringLiteral {
                value_id: mapping(value_id)?,
                language_id: mapping(language_id)?,
            },
            Self::TypedLiteral {
                value_id,
                datatype_id,
            } => EncodedTerm::TypedLiteral {
                value_id: mapping(value_id)?,
                datatype_id: mapping(datatype_id)?,
            },
            Self::BooleanLiteral(value) => EncodedTerm::BooleanLiteral(value),
            Self::FloatLiteral(value) => EncodedTerm::FloatLiteral(value),
            Self::DoubleLiteral(value) => EncodedTerm::DoubleLiteral(value),
            Self::IntegerLiteral(value) => EncodedTerm::IntegerLiteral(value),
            Self::DecimalLiteral(value) => EncodedTerm::DecimalLiteral(value),
            Self::DateLiteral(value) => EncodedTerm::DateLiteral(value),
            Self::TimeLiteral(value) => EncodedTerm::TimeLiteral(value),
            Self::DateTimeLiteral(value) => EncodedTerm::DateTimeLiteral(value),
            Self::DurationLiteral(value) => EncodedTerm::DurationLiteral(value),
            Self::YearMonthDurationLiteral(value) => EncodedTerm::YearMonthDurationLiteral(value),
            Self::DayTimeDurationLiteral(value) => EncodedTerm::DayTimeDurationLiteral(value),
        })
    }
}

impl<I: StrId> From<bool> for EncodedTerm<I> {
    fn from(value: bool) -> Self {
        Self::BooleanLiteral(value)
    }
}

impl<I: StrId> From<i64> for EncodedTerm<I> {
    fn from(value: i64) -> Self {
        Self::IntegerLiteral(value)
    }
}

impl<I: StrId> From<i32> for EncodedTerm<I> {
    fn from(value: i32) -> Self {
        Self::IntegerLiteral(value.into())
    }
}

impl<I: StrId> From<u32> for EncodedTerm<I> {
    fn from(value: u32) -> Self {
        Self::IntegerLiteral(value.into())
    }
}

impl<I: StrId> From<u8> for EncodedTerm<I> {
    fn from(value: u8) -> Self {
        Self::IntegerLiteral(value.into())
    }
}
impl<I: StrId> From<f32> for EncodedTerm<I> {
    fn from(value: f32) -> Self {
        Self::FloatLiteral(value)
    }
}

impl<I: StrId> From<f64> for EncodedTerm<I> {
    fn from(value: f64) -> Self {
        Self::DoubleLiteral(value)
    }
}

impl<I: StrId> From<Decimal> for EncodedTerm<I> {
    fn from(value: Decimal) -> Self {
        Self::DecimalLiteral(value)
    }
}

impl<I: StrId> From<Date> for EncodedTerm<I> {
    fn from(value: Date) -> Self {
        Self::DateLiteral(value)
    }
}

impl<I: StrId> From<Time> for EncodedTerm<I> {
    fn from(value: Time) -> Self {
        Self::TimeLiteral(value)
    }
}

impl<I: StrId> From<DateTime> for EncodedTerm<I> {
    fn from(value: DateTime) -> Self {
        Self::DateTimeLiteral(value)
    }
}

impl<I: StrId> From<Duration> for EncodedTerm<I> {
    fn from(value: Duration) -> Self {
        Self::DurationLiteral(value)
    }
}

impl<I: StrId> From<YearMonthDuration> for EncodedTerm<I> {
    fn from(value: YearMonthDuration) -> Self {
        Self::YearMonthDurationLiteral(value)
    }
}

impl<I: StrId> From<DayTimeDuration> for EncodedTerm<I> {
    fn from(value: DayTimeDuration) -> Self {
        Self::DayTimeDurationLiteral(value)
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct EncodedQuad<I: StrId> {
    pub subject: EncodedTerm<I>,
    pub predicate: EncodedTerm<I>,
    pub object: EncodedTerm<I>,
    pub graph_name: EncodedTerm<I>,
}

impl<I: StrId> EncodedQuad<I> {
    pub fn new(
        subject: EncodedTerm<I>,
        predicate: EncodedTerm<I>,
        object: EncodedTerm<I>,
        graph_name: EncodedTerm<I>,
    ) -> Self {
        Self {
            subject,
            predicate,
            object,
            graph_name,
        }
    }
}

pub trait TermReader {
    fn read_term(&mut self) -> Result<EncodedTerm<StrHash>, io::Error>;
    fn read_spog_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error>;
    fn read_posg_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error>;
    fn read_ospg_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error>;
    fn read_gspo_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error>;
    fn read_gpos_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error>;
    fn read_gosp_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error>;
}

impl<R: Read> TermReader for R {
    fn read_term(&mut self) -> Result<EncodedTerm<StrHash>, io::Error> {
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

    fn read_spog_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error> {
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

    fn read_posg_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error> {
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

    fn read_ospg_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error> {
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

    fn read_gspo_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error> {
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

    fn read_gpos_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error> {
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

    fn read_gosp_quad(&mut self) -> Result<EncodedQuad<StrHash>, io::Error> {
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

pub fn write_term<I: SerializableStrId>(sink: &mut Vec<u8>, term: EncodedTerm<I>) {
    sink.push(term.type_id());
    match term {
        EncodedTerm::DefaultGraph => {}
        EncodedTerm::NamedNode { iri_id } => iri_id.push_be_bytes(sink),
        EncodedTerm::InlineBlankNode { id } => sink.extend_from_slice(&id.to_be_bytes()),
        EncodedTerm::NamedBlankNode { id_id } => id_id.push_be_bytes(sink),
        EncodedTerm::StringLiteral { value_id } => value_id.push_be_bytes(sink),
        EncodedTerm::LangStringLiteral {
            value_id,
            language_id,
        } => {
            value_id.push_be_bytes(sink);
            language_id.push_be_bytes(sink);
        }
        EncodedTerm::TypedLiteral {
            value_id,
            datatype_id,
        } => {
            value_id.push_be_bytes(sink);
            datatype_id.push_be_bytes(sink);
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
    //TODO: rename
    type Error: Error + Into<EvaluationError> + 'static;
    type StrId: StrId + 'static;
}

impl<'a, T: WithStoreError> WithStoreError for &'a T {
    type Error = T::Error;
    type StrId = T::StrId;
}

pub(crate) trait StrLookup: WithStoreError {
    fn get_str(&self, id: Self::StrId) -> Result<Option<String>, Self::Error>;

    fn get_str_id(&self, value: &str) -> Result<Option<Self::StrId>, Self::Error>;
}

pub(crate) trait StrContainer: WithStoreError {
    fn insert_str(&mut self, value: &str) -> Result<Self::StrId, Self::Error>;
}

pub struct MemoryStrStore {
    id2str: HashMap<StrHash, String>,
}

impl Default for MemoryStrStore {
    fn default() -> Self {
        Self {
            id2str: HashMap::default(),
        }
    }
}

impl WithStoreError for MemoryStrStore {
    type Error = Infallible;
    type StrId = StrHash;
}

impl StrLookup for MemoryStrStore {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, Infallible> {
        //TODO: avoid copy by adding a lifetime limit to get_str
        Ok(self.id2str.get(&id).cloned())
    }

    fn get_str_id(&self, value: &str) -> Result<Option<StrHash>, Infallible> {
        let id = StrHash::new(value);
        Ok(if self.id2str.contains_key(&id) {
            Some(id)
        } else {
            None
        })
    }
}

impl StrContainer for MemoryStrStore {
    fn insert_str(&mut self, value: &str) -> Result<StrHash, Infallible> {
        let key = StrHash::new(value);
        self.id2str.entry(key).or_insert_with(|| value.to_owned());
        Ok(key)
    }
}

/// Tries to encode a term based on the existing strings (does not insert anything)
pub(crate) trait ReadEncoder: WithStoreError {
    fn get_encoded_named_node(
        &self,
        named_node: NamedNodeRef<'_>,
    ) -> Result<Option<EncodedTerm<Self::StrId>>, Self::Error> {
        Ok(Some(EncodedTerm::NamedNode {
            iri_id: if let Some(iri_id) = self.get_encoded_str(named_node.as_str())? {
                iri_id
            } else {
                return Ok(None);
            },
        }))
    }

    fn get_encoded_blank_node(
        &self,
        blank_node: BlankNodeRef<'_>,
    ) -> Result<Option<EncodedTerm<Self::StrId>>, Self::Error> {
        Ok(Some(if let Some(id) = blank_node.id() {
            EncodedTerm::InlineBlankNode { id }
        } else {
            EncodedTerm::NamedBlankNode {
                id_id: if let Some(id_id) = self.get_encoded_str(blank_node.as_str())? {
                    id_id
                } else {
                    return Ok(None);
                },
            }
        }))
    }

    fn get_encoded_literal(
        &self,
        literal: LiteralRef<'_>,
    ) -> Result<Option<EncodedTerm<Self::StrId>>, Self::Error> {
        Ok(Some(
            match match literal.datatype().as_str() {
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString" => {
                    if let Some(language) = literal.language() {
                        Some(EncodedTerm::LangStringLiteral {
                            value_id: if let Some(value_id) =
                                self.get_encoded_str(literal.value())?
                            {
                                value_id
                            } else {
                                return Ok(None);
                            },
                            language_id: if let Some(language_id) =
                                self.get_encoded_str(language)?
                            {
                                language_id
                            } else {
                                return Ok(None);
                            },
                        })
                    } else {
                        None
                    }
                }
                "http://www.w3.org/2001/XMLSchema#boolean" => parse_boolean_str(literal.value()),
                "http://www.w3.org/2001/XMLSchema#string" => Some(EncodedTerm::StringLiteral {
                    value_id: if let Some(value_id) = self.get_encoded_str(literal.value())? {
                        value_id
                    } else {
                        return Ok(None);
                    },
                }),
                "http://www.w3.org/2001/XMLSchema#float" => parse_float_str(literal.value()),
                "http://www.w3.org/2001/XMLSchema#double" => parse_double_str(literal.value()),
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
                    parse_integer_str(literal.value())
                }
                "http://www.w3.org/2001/XMLSchema#decimal" => parse_decimal_str(literal.value()),
                "http://www.w3.org/2001/XMLSchema#date" => parse_date_str(literal.value()),
                "http://www.w3.org/2001/XMLSchema#time" => parse_time_str(literal.value()),
                "http://www.w3.org/2001/XMLSchema#dateTime"
                | "http://www.w3.org/2001/XMLSchema#dateTimeStamp" => {
                    parse_date_time_str(literal.value())
                }
                "http://www.w3.org/2001/XMLSchema#duration" => parse_duration_str(literal.value()),
                "http://www.w3.org/2001/XMLSchema#yearMonthDuration" => {
                    parse_year_month_duration_str(literal.value())
                }
                "http://www.w3.org/2001/XMLSchema#dayTimeDuration" => {
                    parse_day_time_duration_str(literal.value())
                }
                _ => None,
            } {
                Some(term) => term,
                None => EncodedTerm::TypedLiteral {
                    value_id: if let Some(value_id) = self.get_encoded_str(literal.value())? {
                        value_id
                    } else {
                        return Ok(None);
                    },
                    datatype_id: if let Some(datatype_id) =
                        self.get_encoded_str(literal.datatype().as_str())?
                    {
                        datatype_id
                    } else {
                        return Ok(None);
                    },
                },
            },
        ))
    }

    fn get_encoded_named_or_blank_node(
        &self,
        term: NamedOrBlankNodeRef<'_>,
    ) -> Result<Option<EncodedTerm<Self::StrId>>, Self::Error> {
        match term {
            NamedOrBlankNodeRef::NamedNode(named_node) => self.get_encoded_named_node(named_node),
            NamedOrBlankNodeRef::BlankNode(blank_node) => self.get_encoded_blank_node(blank_node),
        }
    }

    fn get_encoded_term(
        &self,
        term: TermRef<'_>,
    ) -> Result<Option<EncodedTerm<Self::StrId>>, Self::Error> {
        match term {
            TermRef::NamedNode(named_node) => self.get_encoded_named_node(named_node),
            TermRef::BlankNode(blank_node) => self.get_encoded_blank_node(blank_node),
            TermRef::Literal(literal) => self.get_encoded_literal(literal),
        }
    }

    fn get_encoded_graph_name(
        &self,
        name: GraphNameRef<'_>,
    ) -> Result<Option<EncodedTerm<Self::StrId>>, Self::Error> {
        match name {
            GraphNameRef::NamedNode(named_node) => self.get_encoded_named_node(named_node),
            GraphNameRef::BlankNode(blank_node) => self.get_encoded_blank_node(blank_node),
            GraphNameRef::DefaultGraph => Ok(Some(EncodedTerm::DefaultGraph)),
        }
    }

    fn get_encoded_quad(
        &self,
        quad: QuadRef<'_>,
    ) -> Result<Option<EncodedQuad<Self::StrId>>, Self::Error> {
        Ok(Some(EncodedQuad {
            subject: if let Some(subject) = self.get_encoded_named_or_blank_node(quad.subject)? {
                subject
            } else {
                return Ok(None);
            },
            predicate: if let Some(predicate) = self.get_encoded_named_node(quad.predicate)? {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) = self.get_encoded_term(quad.object)? {
                object
            } else {
                return Ok(None);
            },
            graph_name: if let Some(graph_name) = self.get_encoded_graph_name(quad.graph_name)? {
                graph_name
            } else {
                return Ok(None);
            },
        }))
    }

    fn get_encoded_str(&self, value: &str) -> Result<Option<Self::StrId>, Self::Error>;
}

impl<S: StrLookup> ReadEncoder for S {
    fn get_encoded_str(&self, value: &str) -> Result<Option<Self::StrId>, Self::Error> {
        self.get_str_id(value)
    }
}

/// Encodes a term and insert strings if needed
pub(crate) trait WriteEncoder: WithStoreError {
    fn encode_named_node(
        &mut self,
        named_node: NamedNodeRef<'_>,
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
        self.encode_rio_named_node(named_node.into())
    }

    fn encode_blank_node(
        &mut self,
        blank_node: BlankNodeRef<'_>,
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
        if let Some(id) = blank_node.id() {
            Ok(EncodedTerm::InlineBlankNode { id })
        } else {
            Ok(EncodedTerm::NamedBlankNode {
                id_id: self.encode_str(blank_node.as_str())?,
            })
        }
    }

    fn encode_literal(
        &mut self,
        literal: LiteralRef<'_>,
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
        self.encode_rio_literal(literal.into())
    }

    fn encode_named_or_blank_node(
        &mut self,
        term: NamedOrBlankNodeRef<'_>,
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
        match term {
            NamedOrBlankNodeRef::NamedNode(named_node) => self.encode_named_node(named_node),
            NamedOrBlankNodeRef::BlankNode(blank_node) => self.encode_blank_node(blank_node),
        }
    }

    fn encode_term(&mut self, term: TermRef<'_>) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
        match term {
            TermRef::NamedNode(named_node) => self.encode_named_node(named_node),
            TermRef::BlankNode(blank_node) => self.encode_blank_node(blank_node),
            TermRef::Literal(literal) => self.encode_literal(literal),
        }
    }

    fn encode_graph_name(
        &mut self,
        name: GraphNameRef<'_>,
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
        match name {
            GraphNameRef::NamedNode(named_node) => self.encode_named_node(named_node),
            GraphNameRef::BlankNode(blank_node) => self.encode_blank_node(blank_node),
            GraphNameRef::DefaultGraph => Ok(EncodedTerm::DefaultGraph),
        }
    }

    fn encode_quad(&mut self, quad: QuadRef<'_>) -> Result<EncodedQuad<Self::StrId>, Self::Error> {
        Ok(EncodedQuad {
            subject: self.encode_named_or_blank_node(quad.subject)?,
            predicate: self.encode_named_node(quad.predicate)?,
            object: self.encode_term(quad.object)?,
            graph_name: self.encode_graph_name(quad.graph_name)?,
        })
    }

    fn encode_triple_in_graph(
        &mut self,
        triple: TripleRef<'_>,
        graph_name: EncodedTerm<Self::StrId>,
    ) -> Result<EncodedQuad<Self::StrId>, Self::Error> {
        Ok(EncodedQuad {
            subject: self.encode_named_or_blank_node(triple.subject)?,
            predicate: self.encode_named_node(triple.predicate)?,
            object: self.encode_term(triple.object)?,
            graph_name,
        })
    }

    fn encode_rio_named_node(
        &mut self,
        named_node: rio::NamedNode<'_>,
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
        Ok(EncodedTerm::NamedNode {
            iri_id: self.encode_str(named_node.iri)?,
        })
    }

    fn encode_rio_blank_node(
        &mut self,
        blank_node: rio::BlankNode<'_>,
        bnodes_map: &mut HashMap<String, u128>,
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
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
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
        Ok(match literal {
            rio::Literal::Simple { value } => EncodedTerm::StringLiteral {
                value_id: self.encode_str(value)?,
            },
            rio::Literal::LanguageTaggedString { value, language } => {
                EncodedTerm::LangStringLiteral {
                    value_id: self.encode_str(value)?,
                    language_id: self.encode_str(language)?,
                }
            }
            rio::Literal::Typed { value, datatype } => {
                match match datatype.iri {
                    "http://www.w3.org/2001/XMLSchema#boolean" => parse_boolean_str(value),
                    "http://www.w3.org/2001/XMLSchema#string" => Some(EncodedTerm::StringLiteral {
                        value_id: self.encode_str(value)?,
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
                        value_id: self.encode_str(value)?,
                        datatype_id: self.encode_str(datatype.iri)?,
                    },
                }
            }
        })
    }

    fn encode_rio_named_or_blank_node(
        &mut self,
        term: rio::NamedOrBlankNode<'_>,
        bnodes_map: &mut HashMap<String, u128>,
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
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
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
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
    ) -> Result<EncodedQuad<Self::StrId>, Self::Error> {
        Ok(EncodedQuad {
            subject: self.encode_rio_named_or_blank_node(quad.subject, bnodes_map)?,
            predicate: self.encode_rio_named_node(quad.predicate)?,
            object: self.encode_rio_term(quad.object, bnodes_map)?,
            graph_name: match quad.graph_name {
                Some(graph_name) => self.encode_rio_named_or_blank_node(graph_name, bnodes_map)?,
                None => EncodedTerm::DefaultGraph,
            },
        })
    }

    fn encode_rio_triple_in_graph(
        &mut self,
        triple: rio::Triple<'_>,
        graph_name: EncodedTerm<Self::StrId>,
        bnodes_map: &mut HashMap<String, u128>,
    ) -> Result<EncodedQuad<Self::StrId>, Self::Error> {
        Ok(EncodedQuad {
            subject: self.encode_rio_named_or_blank_node(triple.subject, bnodes_map)?,
            predicate: self.encode_rio_named_node(triple.predicate)?,
            object: self.encode_rio_term(triple.object, bnodes_map)?,
            graph_name,
        })
    }

    fn encode_str(&mut self, value: &str) -> Result<Self::StrId, Self::Error>;
}

impl<S: StrContainer> WriteEncoder for S {
    fn encode_str(&mut self, value: &str) -> Result<Self::StrId, Self::Error> {
        self.insert_str(value)
    }
}

pub fn parse_boolean_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    match value {
        "true" | "1" => Some(EncodedTerm::BooleanLiteral(true)),
        "false" | "0" => Some(EncodedTerm::BooleanLiteral(false)),
        _ => None,
    }
}

pub fn parse_float_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::FloatLiteral).ok()
}

pub fn parse_double_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::DoubleLiteral).ok()
}

pub fn parse_integer_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::IntegerLiteral).ok()
}

pub fn parse_decimal_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::DecimalLiteral).ok()
}

pub fn parse_date_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::DateLiteral).ok()
}

pub fn parse_time_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::TimeLiteral).ok()
}

pub fn parse_date_time_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::DateTimeLiteral).ok()
}

pub fn parse_duration_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::DurationLiteral).ok()
}

pub fn parse_year_month_duration_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value
        .parse()
        .map(EncodedTerm::YearMonthDurationLiteral)
        .ok()
}

pub fn parse_day_time_duration_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::DayTimeDurationLiteral).ok()
}

pub(crate) trait Decoder: StrLookup {
    fn decode_term(
        &self,
        encoded: EncodedTerm<Self::StrId>,
    ) -> Result<Term, DecoderError<Self::Error>>;

    fn decode_named_or_blank_node(
        &self,
        encoded: EncodedTerm<Self::StrId>,
    ) -> Result<NamedOrBlankNode, DecoderError<Self::Error>> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node.into()),
            Term::BlankNode(blank_node) => Ok(blank_node.into()),
            Term::Literal(_) => Err(DecoderError::Decoder {
                msg: "A literal has ben found instead of a named node".to_owned(),
            }),
        }
    }

    fn decode_named_node(
        &self,
        encoded: EncodedTerm<Self::StrId>,
    ) -> Result<NamedNode, DecoderError<Self::Error>> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node),
            Term::BlankNode(_) => Err(DecoderError::Decoder {
                msg: "A blank node has been found instead of a named node".to_owned(),
            }),
            Term::Literal(_) => Err(DecoderError::Decoder {
                msg: "A literal has ben found instead of a named node".to_owned(),
            }),
        }
    }

    fn decode_triple(
        &self,
        encoded: &EncodedQuad<Self::StrId>,
    ) -> Result<Triple, DecoderError<Self::Error>> {
        Ok(Triple::new(
            self.decode_named_or_blank_node(encoded.subject)?,
            self.decode_named_node(encoded.predicate)?,
            self.decode_term(encoded.object)?,
        ))
    }

    fn decode_quad(
        &self,
        encoded: &EncodedQuad<Self::StrId>,
    ) -> Result<Quad, DecoderError<Self::Error>> {
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
    fn decode_term(
        &self,
        encoded: EncodedTerm<Self::StrId>,
    ) -> Result<Term, DecoderError<Self::Error>> {
        match encoded {
            EncodedTerm::DefaultGraph => Err(DecoderError::Decoder {
                msg: "The default graph tag is not a valid term".to_owned(),
            }),
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

fn get_required_str<L: StrLookup>(
    lookup: &L,
    id: L::StrId,
) -> Result<String, DecoderError<L::Error>> {
    lookup
        .get_str(id)
        .map_err(DecoderError::Store)?
        .ok_or_else(|| DecoderError::Decoder {
            msg: format!(
                "Not able to find the string with id {:?} in the string store",
                id
            ),
        })
}

#[derive(Debug)]
pub(crate) enum DecoderError<E> {
    Store(E),
    Decoder { msg: String },
}

impl<E: fmt::Display> fmt::Display for DecoderError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => e.fmt(f),
            Self::Decoder { msg } => write!(f, "{}", msg),
        }
    }
}

impl<E: Error + 'static> Error for DecoderError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(e) => Some(e),
            Self::Decoder { .. } => None,
        }
    }
}

impl<E: Into<io::Error>> From<DecoderError<E>> for io::Error {
    fn from(e: DecoderError<E>) -> Self {
        match e {
            DecoderError::Store(e) => e.into(),
            DecoderError::Decoder { msg } => invalid_data_error(msg),
        }
    }
}

#[test]
fn test_encoding() {
    use crate::model::vocab::xsd;

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
        Literal::new_typed_literal("-1.32", xsd::DECIMAL).into(),
        Literal::new_typed_literal("2020-01-01T01:01:01Z", xsd::DATE_TIME).into(),
        Literal::new_typed_literal("2020-01-01", xsd::DATE).into(),
        Literal::new_typed_literal("01:01:01Z", xsd::TIME).into(),
        Literal::new_typed_literal("PT1S", xsd::DURATION).into(),
        Literal::new_typed_literal("-foo", NamedNode::new_unchecked("http://foo.com")).into(),
    ];
    for term in terms {
        let encoded = store.encode_term(term.as_ref()).unwrap();
        assert_eq!(
            Some(encoded),
            store.get_encoded_term(term.as_ref()).unwrap()
        );
        assert_eq!(term, store.decode_term(encoded).unwrap());
    }
}
