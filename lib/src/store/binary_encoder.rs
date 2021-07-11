use crate::error::invalid_data_error;
use crate::model::xsd::*;
use crate::store::numeric_encoder::StrId;
use crate::store::small_string::SmallString;
use siphasher::sip128::{Hasher128, SipHasher24};
use std::hash::Hasher;
use std::io;
use std::io::{Cursor, Read};
use std::mem::size_of;

type EncodedTerm = crate::store::numeric_encoder::EncodedTerm<StrHash>;
type EncodedQuad = crate::store::numeric_encoder::EncodedQuad<StrHash>;

pub const LATEST_STORAGE_VERSION: u64 = 1;
pub const WRITTEN_TERM_MAX_SIZE: usize = size_of::<u8>() + 2 * size_of::<StrHash>();

// Encoded term type blocks
// 1-7: usual named nodes (except prefixes c.f. later)
// 8-15: blank nodes
// 16-47: literals
// 48-64: future use
// 64-127: default named node prefixes
// 128-255: custom named node prefixes
const TYPE_NAMED_NODE_ID: u8 = 1;
const TYPE_NUMERICAL_BLANK_NODE_ID: u8 = 8;
const TYPE_SMALL_BLANK_NODE_ID: u8 = 9;
const TYPE_BIG_BLANK_NODE_ID: u8 = 10;
const TYPE_SMALL_STRING_LITERAL: u8 = 16;
const TYPE_BIG_STRING_LITERAL: u8 = 17;
const TYPE_SMALL_SMALL_LANG_STRING_LITERAL: u8 = 20;
const TYPE_SMALL_BIG_LANG_STRING_LITERAL: u8 = 21;
const TYPE_BIG_SMALL_LANG_STRING_LITERAL: u8 = 22;
const TYPE_BIG_BIG_LANG_STRING_LITERAL: u8 = 23;
const TYPE_SMALL_TYPED_LITERAL: u8 = 24;
const TYPE_BIG_TYPED_LITERAL: u8 = 25;
const TYPE_BOOLEAN_LITERAL_TRUE: u8 = 28;
const TYPE_BOOLEAN_LITERAL_FALSE: u8 = 29;
const TYPE_FLOAT_LITERAL: u8 = 30;
const TYPE_DOUBLE_LITERAL: u8 = 31;
const TYPE_INTEGER_LITERAL: u8 = 32;
const TYPE_DECIMAL_LITERAL: u8 = 33;
const TYPE_DATE_TIME_LITERAL: u8 = 34;
const TYPE_TIME_LITERAL: u8 = 35;
const TYPE_DATE_LITERAL: u8 = 36;
const TYPE_G_YEAR_MONTH_LITERAL: u8 = 37;
const TYPE_G_YEAR_LITERAL: u8 = 38;
const TYPE_G_MONTH_DAY_LITERAL: u8 = 39;
const TYPE_G_DAY_LITERAL: u8 = 40;
const TYPE_G_MONTH_LITERAL: u8 = 41;
const TYPE_DURATION_LITERAL: u8 = 42;
const TYPE_YEAR_MONTH_DURATION_LITERAL: u8 = 43;
const TYPE_DAY_TIME_DURATION_LITERAL: u8 = 44;

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
    pub fn to_be_bytes(self) -> [u8; 16] {
        self.hash.to_be_bytes()
    }
}

impl StrId for StrHash {}

#[derive(Clone, Copy)]
pub enum QuadEncoding {
    Spog,
    Posg,
    Ospg,
    Gspo,
    Gpos,
    Gosp,
    Dspo,
    Dpos,
    Dosp,
}

impl QuadEncoding {
    pub fn decode(self, buffer: &[u8]) -> Result<EncodedQuad, io::Error> {
        let mut cursor = Cursor::new(&buffer);
        match self {
            QuadEncoding::Spog => cursor.read_spog_quad(),
            QuadEncoding::Posg => cursor.read_posg_quad(),
            QuadEncoding::Ospg => cursor.read_ospg_quad(),
            QuadEncoding::Gspo => cursor.read_gspo_quad(),
            QuadEncoding::Gpos => cursor.read_gpos_quad(),
            QuadEncoding::Gosp => cursor.read_gosp_quad(),
            QuadEncoding::Dspo => cursor.read_dspo_quad(),
            QuadEncoding::Dpos => cursor.read_dpos_quad(),
            QuadEncoding::Dosp => cursor.read_dosp_quad(),
        }
    }
}

pub fn decode_term(buffer: &[u8]) -> Result<EncodedTerm, io::Error> {
    Cursor::new(&buffer).read_term()
}

pub trait TermReader {
    fn read_term(&mut self) -> Result<EncodedTerm, io::Error>;

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

    fn read_dspo_quad(&mut self) -> Result<EncodedQuad, io::Error> {
        let subject = self.read_term()?;
        let predicate = self.read_term()?;
        let object = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name: EncodedTerm::DefaultGraph,
        })
    }

    fn read_dpos_quad(&mut self) -> Result<EncodedQuad, io::Error> {
        let predicate = self.read_term()?;
        let object = self.read_term()?;
        let subject = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name: EncodedTerm::DefaultGraph,
        })
    }

    fn read_dosp_quad(&mut self) -> Result<EncodedQuad, io::Error> {
        let object = self.read_term()?;
        let subject = self.read_term()?;
        let predicate = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name: EncodedTerm::DefaultGraph,
        })
    }
}

impl<R: Read> TermReader for R {
    fn read_term(&mut self) -> Result<EncodedTerm, io::Error> {
        let mut type_buffer = [0];
        self.read_exact(&mut type_buffer)?;
        match type_buffer[0] {
            TYPE_NAMED_NODE_ID => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::NamedNode {
                    iri_id: StrHash::from_be_bytes(buffer),
                })
            }
            TYPE_NUMERICAL_BLANK_NODE_ID => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::NumericalBlankNode {
                    id: u128::from_be_bytes(buffer),
                })
            }
            TYPE_SMALL_BLANK_NODE_ID => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::SmallBlankNode(
                    SmallString::from_be_bytes(buffer).map_err(invalid_data_error)?,
                ))
            }
            TYPE_BIG_BLANK_NODE_ID => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::BigBlankNode {
                    id_id: StrHash::from_be_bytes(buffer),
                })
            }
            TYPE_SMALL_SMALL_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::SmallSmallLangStringLiteral {
                    value: SmallString::from_be_bytes(value_buffer).map_err(invalid_data_error)?,
                    language: SmallString::from_be_bytes(language_buffer)
                        .map_err(invalid_data_error)?,
                })
            }
            TYPE_SMALL_BIG_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::SmallBigLangStringLiteral {
                    value: SmallString::from_be_bytes(value_buffer).map_err(invalid_data_error)?,
                    language_id: StrHash::from_be_bytes(language_buffer),
                })
            }
            TYPE_BIG_SMALL_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::BigSmallLangStringLiteral {
                    value_id: StrHash::from_be_bytes(value_buffer),
                    language: SmallString::from_be_bytes(language_buffer)
                        .map_err(invalid_data_error)?,
                })
            }
            TYPE_BIG_BIG_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::BigBigLangStringLiteral {
                    value_id: StrHash::from_be_bytes(value_buffer),
                    language_id: StrHash::from_be_bytes(language_buffer),
                })
            }
            TYPE_SMALL_TYPED_LITERAL => {
                let mut datatype_buffer = [0; 16];
                self.read_exact(&mut datatype_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::SmallTypedLiteral {
                    datatype_id: StrHash::from_be_bytes(datatype_buffer),
                    value: SmallString::from_be_bytes(value_buffer).map_err(invalid_data_error)?,
                })
            }
            TYPE_BIG_TYPED_LITERAL => {
                let mut datatype_buffer = [0; 16];
                self.read_exact(&mut datatype_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::BigTypedLiteral {
                    datatype_id: StrHash::from_be_bytes(datatype_buffer),
                    value_id: StrHash::from_be_bytes(value_buffer),
                })
            }
            TYPE_SMALL_STRING_LITERAL => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::SmallStringLiteral(
                    SmallString::from_be_bytes(buffer).map_err(invalid_data_error)?,
                ))
            }
            TYPE_BIG_STRING_LITERAL => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::BigStringLiteral {
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
            TYPE_DATE_TIME_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DateTimeLiteral(DateTime::from_be_bytes(
                    buffer,
                )))
            }
            TYPE_TIME_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::TimeLiteral(Time::from_be_bytes(buffer)))
            }
            TYPE_DATE_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DateLiteral(Date::from_be_bytes(buffer)))
            }
            TYPE_G_YEAR_MONTH_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::GYearMonthLiteral(GYearMonth::from_be_bytes(
                    buffer,
                )))
            }
            TYPE_G_YEAR_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::GYearLiteral(GYear::from_be_bytes(buffer)))
            }
            TYPE_G_MONTH_DAY_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::GMonthDayLiteral(GMonthDay::from_be_bytes(
                    buffer,
                )))
            }
            TYPE_G_DAY_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::GDayLiteral(GDay::from_be_bytes(buffer)))
            }
            TYPE_G_MONTH_LITERAL => {
                let mut buffer = [0; 18];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::GMonthLiteral(GMonth::from_be_bytes(buffer)))
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
}

pub fn write_spog_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.graph_name);
}

pub fn write_posg_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.graph_name);
}

pub fn write_ospg_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.graph_name);
}

pub fn write_gspo_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.graph_name);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
}

pub fn write_gpos_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.graph_name);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
}

pub fn write_gosp_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.graph_name);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
}

pub fn write_spo_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
}

pub fn write_pos_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
}

pub fn write_osp_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
}

pub fn encode_term(t: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t);
    vec
}

pub fn encode_term_pair(t1: EncodedTerm, t2: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(2 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    vec
}

pub fn encode_term_triple(t1: EncodedTerm, t2: EncodedTerm, t3: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(3 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    vec
}

pub fn encode_term_quad(
    t1: EncodedTerm,
    t2: EncodedTerm,
    t3: EncodedTerm,
    t4: EncodedTerm,
) -> Vec<u8> {
    let mut vec = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    write_term(&mut vec, t4);
    vec
}

pub fn write_term(sink: &mut Vec<u8>, term: EncodedTerm) {
    match term {
        EncodedTerm::DefaultGraph => (),
        EncodedTerm::NamedNode { iri_id } => {
            sink.push(TYPE_NAMED_NODE_ID);
            sink.extend_from_slice(&iri_id.to_be_bytes());
        }
        EncodedTerm::NumericalBlankNode { id } => {
            sink.push(TYPE_NUMERICAL_BLANK_NODE_ID);
            sink.extend_from_slice(&id.to_be_bytes())
        }
        EncodedTerm::SmallBlankNode(id) => {
            sink.push(TYPE_SMALL_BLANK_NODE_ID);
            sink.extend_from_slice(&id.to_be_bytes())
        }
        EncodedTerm::BigBlankNode { id_id } => {
            sink.push(TYPE_BIG_BLANK_NODE_ID);
            sink.extend_from_slice(&id_id.to_be_bytes());
        }
        EncodedTerm::SmallStringLiteral(value) => {
            sink.push(TYPE_SMALL_STRING_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::BigStringLiteral { value_id } => {
            sink.push(TYPE_BIG_STRING_LITERAL);
            sink.extend_from_slice(&value_id.to_be_bytes());
        }
        EncodedTerm::SmallSmallLangStringLiteral { value, language } => {
            sink.push(TYPE_SMALL_SMALL_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language.to_be_bytes());
            sink.extend_from_slice(&value.to_be_bytes());
        }
        EncodedTerm::SmallBigLangStringLiteral { value, language_id } => {
            sink.push(TYPE_SMALL_BIG_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language_id.to_be_bytes());
            sink.extend_from_slice(&value.to_be_bytes());
        }
        EncodedTerm::BigSmallLangStringLiteral { value_id, language } => {
            sink.push(TYPE_BIG_SMALL_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language.to_be_bytes());
            sink.extend_from_slice(&value_id.to_be_bytes());
        }
        EncodedTerm::BigBigLangStringLiteral {
            value_id,
            language_id,
        } => {
            sink.push(TYPE_BIG_BIG_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language_id.to_be_bytes());
            sink.extend_from_slice(&value_id.to_be_bytes());
        }
        EncodedTerm::SmallTypedLiteral { value, datatype_id } => {
            sink.push(TYPE_SMALL_TYPED_LITERAL);
            sink.extend_from_slice(&datatype_id.to_be_bytes());
            sink.extend_from_slice(&value.to_be_bytes());
        }
        EncodedTerm::BigTypedLiteral {
            value_id,
            datatype_id,
        } => {
            sink.push(TYPE_BIG_TYPED_LITERAL);
            sink.extend_from_slice(&datatype_id.to_be_bytes());
            sink.extend_from_slice(&value_id.to_be_bytes());
        }
        EncodedTerm::BooleanLiteral(true) => sink.push(TYPE_BOOLEAN_LITERAL_TRUE),
        EncodedTerm::BooleanLiteral(false) => sink.push(TYPE_BOOLEAN_LITERAL_FALSE),
        EncodedTerm::FloatLiteral(value) => {
            sink.push(TYPE_FLOAT_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::DoubleLiteral(value) => {
            sink.push(TYPE_DOUBLE_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::IntegerLiteral(value) => {
            sink.push(TYPE_INTEGER_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::DecimalLiteral(value) => {
            sink.push(TYPE_DECIMAL_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::DateTimeLiteral(value) => {
            sink.push(TYPE_DATE_TIME_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::TimeLiteral(value) => {
            sink.push(TYPE_TIME_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::DurationLiteral(value) => {
            sink.push(TYPE_DURATION_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::DateLiteral(value) => {
            sink.push(TYPE_DATE_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::GYearMonthLiteral(value) => {
            sink.push(TYPE_G_YEAR_MONTH_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::GYearLiteral(value) => {
            sink.push(TYPE_G_YEAR_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::GMonthDayLiteral(value) => {
            sink.push(TYPE_G_MONTH_DAY_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::GDayLiteral(value) => {
            sink.push(TYPE_G_DAY_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::GMonthLiteral(value) => {
            sink.push(TYPE_G_MONTH_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::YearMonthDurationLiteral(value) => {
            sink.push(TYPE_YEAR_MONTH_DURATION_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::DayTimeDurationLiteral(value) => {
            sink.push(TYPE_DAY_TIME_DURATION_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::numeric_encoder::*;
    use std::collections::HashMap;
    use std::convert::Infallible;

    struct MemoryStrStore {
        id2str: HashMap<StrHash, String>,
    }

    impl Default for MemoryStrStore {
        fn default() -> Self {
            Self {
                id2str: HashMap::default(),
            }
        }
    }

    impl StrEncodingAware for MemoryStrStore {
        type Error = Infallible;
        type StrId = StrHash;
    }

    impl StrLookup for MemoryStrStore {
        fn get_str(&self, id: StrHash) -> Result<Option<String>, Infallible> {
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

    #[test]
    fn test_encoding() {
        use crate::model::vocab::xsd;
        use crate::model::*;

        let mut store = MemoryStrStore::default();
        let terms: Vec<Term> = vec![
            NamedNode::new_unchecked("http://foo.com").into(),
            NamedNode::new_unchecked("http://bar.com").into(),
            NamedNode::new_unchecked("http://foo.com").into(),
            BlankNode::default().into(),
            BlankNode::new_unchecked("bnode").into(),
            BlankNode::new_unchecked("foo-bnode-thisisaverylargeblanknode").into(),
            Literal::new_simple_literal("literal").into(),
            BlankNode::new_unchecked("foo-literal-thisisaverylargestringliteral").into(),
            Literal::from(true).into(),
            Literal::from(1.2).into(),
            Literal::from(1).into(),
            Literal::from("foo-string").into(),
            Literal::new_language_tagged_literal_unchecked("foo-fr", "fr").into(),
            Literal::new_language_tagged_literal_unchecked(
                "foo-fr-literal-thisisaverylargelanguagetaggedstringliteral",
                "fr",
            )
            .into(),
            Literal::new_language_tagged_literal_unchecked(
                "foo-big",
                "fr-FR-Latn-x-foo-bar-baz-bat-aaaa-bbbb-cccc",
            )
            .into(),
            Literal::new_language_tagged_literal_unchecked(
                "foo-big-literal-thisisaverylargelanguagetaggedstringliteral",
                "fr-FR-Latn-x-foo-bar-baz-bat-aaaa-bbbb-cccc",
            )
            .into(),
            Literal::new_typed_literal("-1.32", xsd::DECIMAL).into(),
            Literal::new_typed_literal("2020-01-01T01:01:01Z", xsd::DATE_TIME).into(),
            Literal::new_typed_literal("2020-01-01", xsd::DATE).into(),
            Literal::new_typed_literal("01:01:01Z", xsd::TIME).into(),
            Literal::new_typed_literal("2020-01", xsd::G_YEAR_MONTH).into(),
            Literal::new_typed_literal("2020", xsd::G_YEAR).into(),
            Literal::new_typed_literal("--01-01", xsd::G_MONTH_DAY).into(),
            Literal::new_typed_literal("--01", xsd::G_MONTH).into(),
            Literal::new_typed_literal("---01", xsd::G_DAY).into(),
            Literal::new_typed_literal("PT1S", xsd::DURATION).into(),
            Literal::new_typed_literal("PT1S", xsd::DAY_TIME_DURATION).into(),
            Literal::new_typed_literal("P1Y", xsd::YEAR_MONTH_DURATION).into(),
            Literal::new_typed_literal("-foo", NamedNode::new_unchecked("http://foo.com")).into(),
            Literal::new_typed_literal(
                "-foo-thisisaverybigtypedliteralwiththefoodatatype",
                NamedNode::new_unchecked("http://foo.com"),
            )
            .into(),
        ];
        for term in terms {
            let encoded = store.encode_term(term.as_ref()).unwrap();
            assert_eq!(
                Some(encoded),
                store.get_encoded_term(term.as_ref()).unwrap()
            );
            assert_eq!(term, store.decode_term(encoded).unwrap());

            let mut buffer = Vec::new();
            write_term(&mut buffer, encoded);
            assert_eq!(encoded, Cursor::new(&buffer).read_term().unwrap());
        }
    }
}
