use crate::storage::error::{CorruptionError, StorageError};
#[cfg(feature = "rdf-12")]
use crate::storage::numeric_encoder::EncodedTriple;
use crate::storage::numeric_encoder::{EncodedQuad, EncodedTerm, StrHash};
use crate::storage::small_string::SmallString;
use oxsdatatypes::*;
use std::io::Read;
use std::mem::size_of;
#[cfg(feature = "rdf-12")]
use std::sync::Arc;

pub const WRITTEN_TERM_MAX_SIZE: usize = size_of::<u8>() + 2 * size_of::<StrHash>();

// Encoded term type blocks
// 1-7: usual named nodes (except prefixes c.f. later)
// 8-15: blank nodes
// 16-47: literals
// 48-55: triples
// 56-64: literal again
// 64-127: default named node prefixes
// 128-254: custom named node prefixes
// 255: reserved for easy "all" ranges
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
pub const TYPE_STAR_TRIPLE: u8 = 48;
#[cfg(feature = "rdf-12")]
const TYPE_TRIPLE: u8 = 49;
#[cfg(feature = "rdf-12")]
const TYPE_LTR_SMALL_SMALL_DIR_LANG_STRING_LITERAL: u8 = 56;
#[cfg(feature = "rdf-12")]
const TYPE_LTR_SMALL_BIG_DIR_LANG_STRING_LITERAL: u8 = 57;
#[cfg(feature = "rdf-12")]
const TYPE_LTR_BIG_SMALL_DIR_LANG_STRING_LITERAL: u8 = 58;
#[cfg(feature = "rdf-12")]
const TYPE_LTR_BIG_BIG_DIR_LANG_STRING_LITERAL: u8 = 59;
#[cfg(feature = "rdf-12")]
const TYPE_RTL_SMALL_SMALL_DIR_LANG_STRING_LITERAL: u8 = 60;
#[cfg(feature = "rdf-12")]
const TYPE_RTL_SMALL_BIG_DIR_LANG_STRING_LITERAL: u8 = 61;
#[cfg(feature = "rdf-12")]
const TYPE_RTL_BIG_SMALL_DIR_LANG_STRING_LITERAL: u8 = 62;
#[cfg(feature = "rdf-12")]
const TYPE_RTL_BIG_BIG_DIR_LANG_STRING_LITERAL: u8 = 63;

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
    pub fn decode(self, mut buffer: &[u8]) -> Result<EncodedQuad, StorageError> {
        match self {
            Self::Spog => buffer.read_spog_quad(),
            Self::Posg => buffer.read_posg_quad(),
            Self::Ospg => buffer.read_ospg_quad(),
            Self::Gspo => buffer.read_gspo_quad(),
            Self::Gpos => buffer.read_gpos_quad(),
            Self::Gosp => buffer.read_gosp_quad(),
            Self::Dspo => buffer.read_dspo_quad(),
            Self::Dpos => buffer.read_dpos_quad(),
            Self::Dosp => buffer.read_dosp_quad(),
        }
    }
}

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
pub fn decode_term(mut buffer: &[u8]) -> Result<EncodedTerm, StorageError> {
    buffer.read_term()
}

pub trait TermReader {
    fn read_term(&mut self) -> Result<EncodedTerm, StorageError>;

    fn read_spog_quad(&mut self) -> Result<EncodedQuad, StorageError> {
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

    fn read_posg_quad(&mut self) -> Result<EncodedQuad, StorageError> {
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

    fn read_ospg_quad(&mut self) -> Result<EncodedQuad, StorageError> {
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

    fn read_gspo_quad(&mut self) -> Result<EncodedQuad, StorageError> {
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

    fn read_gpos_quad(&mut self) -> Result<EncodedQuad, StorageError> {
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

    fn read_gosp_quad(&mut self) -> Result<EncodedQuad, StorageError> {
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

    fn read_dspo_quad(&mut self) -> Result<EncodedQuad, StorageError> {
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

    fn read_dpos_quad(&mut self) -> Result<EncodedQuad, StorageError> {
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

    fn read_dosp_quad(&mut self) -> Result<EncodedQuad, StorageError> {
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
    fn read_term(&mut self) -> Result<EncodedTerm, StorageError> {
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
                let mut id = [0; 16];
                self.read_exact(&mut id)?;
                Ok(EncodedTerm::NumericalBlankNode { id })
            }
            TYPE_SMALL_BLANK_NODE_ID => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::SmallBlankNode(
                    SmallString::from_be_bytes(buffer).map_err(CorruptionError::new)?,
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
                    value: SmallString::from_be_bytes(value_buffer)
                        .map_err(CorruptionError::new)?,
                    language: SmallString::from_be_bytes(language_buffer)
                        .map_err(CorruptionError::new)?,
                })
            }
            TYPE_SMALL_BIG_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::SmallBigLangStringLiteral {
                    value: SmallString::from_be_bytes(value_buffer)
                        .map_err(CorruptionError::new)?,
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
                        .map_err(CorruptionError::new)?,
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
            #[cfg(feature = "rdf-12")]
            TYPE_RTL_SMALL_SMALL_DIR_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::RtlSmallSmallDirLangStringLiteral {
                    value: SmallString::from_be_bytes(value_buffer)
                        .map_err(CorruptionError::new)?,
                    language: SmallString::from_be_bytes(language_buffer)
                        .map_err(CorruptionError::new)?,
                })
            }
            #[cfg(feature = "rdf-12")]
            TYPE_RTL_SMALL_BIG_DIR_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::RtlSmallBigDirLangStringLiteral {
                    value: SmallString::from_be_bytes(value_buffer)
                        .map_err(CorruptionError::new)?,
                    language_id: StrHash::from_be_bytes(language_buffer),
                })
            }
            #[cfg(feature = "rdf-12")]
            TYPE_RTL_BIG_SMALL_DIR_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::RtlBigSmallDirLangStringLiteral {
                    value_id: StrHash::from_be_bytes(value_buffer),
                    language: SmallString::from_be_bytes(language_buffer)
                        .map_err(CorruptionError::new)?,
                })
            }
            #[cfg(feature = "rdf-12")]
            TYPE_RTL_BIG_BIG_DIR_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::RtlBigBigDirLangStringLiteral {
                    value_id: StrHash::from_be_bytes(value_buffer),
                    language_id: StrHash::from_be_bytes(language_buffer),
                })
            }
            #[cfg(feature = "rdf-12")]
            TYPE_LTR_SMALL_SMALL_DIR_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::LtrSmallSmallDirLangStringLiteral {
                    value: SmallString::from_be_bytes(value_buffer)
                        .map_err(CorruptionError::new)?,
                    language: SmallString::from_be_bytes(language_buffer)
                        .map_err(CorruptionError::new)?,
                })
            }
            #[cfg(feature = "rdf-12")]
            TYPE_LTR_SMALL_BIG_DIR_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::LtrSmallBigDirLangStringLiteral {
                    value: SmallString::from_be_bytes(value_buffer)
                        .map_err(CorruptionError::new)?,
                    language_id: StrHash::from_be_bytes(language_buffer),
                })
            }
            #[cfg(feature = "rdf-12")]
            TYPE_LTR_BIG_SMALL_DIR_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::LtrBigSmallDirLangStringLiteral {
                    value_id: StrHash::from_be_bytes(value_buffer),
                    language: SmallString::from_be_bytes(language_buffer)
                        .map_err(CorruptionError::new)?,
                })
            }
            #[cfg(feature = "rdf-12")]
            TYPE_LTR_BIG_BIG_DIR_LANG_STRING_LITERAL => {
                let mut language_buffer = [0; 16];
                self.read_exact(&mut language_buffer)?;
                let mut value_buffer = [0; 16];
                self.read_exact(&mut value_buffer)?;
                Ok(EncodedTerm::LtrBigBigDirLangStringLiteral {
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
                    value: SmallString::from_be_bytes(value_buffer)
                        .map_err(CorruptionError::new)?,
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
                    SmallString::from_be_bytes(buffer).map_err(CorruptionError::new)?,
                ))
            }
            TYPE_BIG_STRING_LITERAL => {
                let mut buffer = [0; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::BigStringLiteral {
                    value_id: StrHash::from_be_bytes(buffer),
                })
            }
            TYPE_BOOLEAN_LITERAL_TRUE => Ok(EncodedTerm::BooleanLiteral(true.into())),
            TYPE_BOOLEAN_LITERAL_FALSE => Ok(EncodedTerm::BooleanLiteral(false.into())),
            TYPE_FLOAT_LITERAL => {
                let mut buffer = [0; 4];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::FloatLiteral(Float::from_be_bytes(buffer)))
            }
            TYPE_DOUBLE_LITERAL => {
                let mut buffer = [0; 8];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DoubleLiteral(Double::from_be_bytes(buffer)))
            }
            TYPE_INTEGER_LITERAL => {
                let mut buffer = [0; 8];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::IntegerLiteral(Integer::from_be_bytes(buffer)))
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
            #[cfg(feature = "rdf-12")]
            TYPE_TRIPLE | TYPE_STAR_TRIPLE => Ok(EncodedTerm::Triple(Arc::new(EncodedTriple {
                subject: self.read_term()?,
                predicate: self.read_term()?,
                object: self.read_term()?,
            }))),
            _ => Err(CorruptionError::msg("the term buffer has an invalid type id").into()),
        }
    }
}

pub fn write_spog_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, &quad.subject);
    write_term(sink, &quad.predicate);
    write_term(sink, &quad.object);
    write_term(sink, &quad.graph_name);
}

pub fn write_posg_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, &quad.predicate);
    write_term(sink, &quad.object);
    write_term(sink, &quad.subject);
    write_term(sink, &quad.graph_name);
}

pub fn write_ospg_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, &quad.object);
    write_term(sink, &quad.subject);
    write_term(sink, &quad.predicate);
    write_term(sink, &quad.graph_name);
}

pub fn write_gspo_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, &quad.graph_name);
    write_term(sink, &quad.subject);
    write_term(sink, &quad.predicate);
    write_term(sink, &quad.object);
}

pub fn write_gpos_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, &quad.graph_name);
    write_term(sink, &quad.predicate);
    write_term(sink, &quad.object);
    write_term(sink, &quad.subject);
}

pub fn write_gosp_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, &quad.graph_name);
    write_term(sink, &quad.object);
    write_term(sink, &quad.subject);
    write_term(sink, &quad.predicate);
}

pub fn write_spo_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, &quad.subject);
    write_term(sink, &quad.predicate);
    write_term(sink, &quad.object);
}

pub fn write_pos_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, &quad.predicate);
    write_term(sink, &quad.object);
    write_term(sink, &quad.subject);
}

pub fn write_osp_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, &quad.object);
    write_term(sink, &quad.subject);
    write_term(sink, &quad.predicate);
}

pub fn encode_term(t: &EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t);
    vec
}

pub fn encode_term_pair(t1: &EncodedTerm, t2: &EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(2 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    vec
}

pub fn encode_term_triple(t1: &EncodedTerm, t2: &EncodedTerm, t3: &EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(3 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    vec
}

pub fn encode_term_quad(
    t1: &EncodedTerm,
    t2: &EncodedTerm,
    t3: &EncodedTerm,
    t4: &EncodedTerm,
) -> Vec<u8> {
    let mut vec = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    write_term(&mut vec, t4);
    vec
}

pub fn write_term(sink: &mut Vec<u8>, term: &EncodedTerm) {
    match term {
        EncodedTerm::DefaultGraph => (),
        EncodedTerm::NamedNode { iri_id } => {
            sink.push(TYPE_NAMED_NODE_ID);
            sink.extend_from_slice(&iri_id.to_be_bytes());
        }
        EncodedTerm::NumericalBlankNode { id } => {
            sink.push(TYPE_NUMERICAL_BLANK_NODE_ID);
            sink.extend_from_slice(id)
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
        #[cfg(feature = "rdf-12")]
        EncodedTerm::LtrSmallSmallDirLangStringLiteral { value, language } => {
            sink.push(TYPE_LTR_SMALL_SMALL_DIR_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language.to_be_bytes());
            sink.extend_from_slice(&value.to_be_bytes());
        }
        #[cfg(feature = "rdf-12")]
        EncodedTerm::LtrSmallBigDirLangStringLiteral { value, language_id } => {
            sink.push(TYPE_LTR_SMALL_BIG_DIR_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language_id.to_be_bytes());
            sink.extend_from_slice(&value.to_be_bytes());
        }
        #[cfg(feature = "rdf-12")]
        EncodedTerm::LtrBigSmallDirLangStringLiteral { value_id, language } => {
            sink.push(TYPE_LTR_BIG_SMALL_DIR_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language.to_be_bytes());
            sink.extend_from_slice(&value_id.to_be_bytes());
        }
        #[cfg(feature = "rdf-12")]
        EncodedTerm::LtrBigBigDirLangStringLiteral {
            value_id,
            language_id,
        } => {
            sink.push(TYPE_LTR_BIG_BIG_DIR_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language_id.to_be_bytes());
            sink.extend_from_slice(&value_id.to_be_bytes());
        }
        #[cfg(feature = "rdf-12")]
        EncodedTerm::RtlSmallSmallDirLangStringLiteral { value, language } => {
            sink.push(TYPE_RTL_SMALL_SMALL_DIR_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language.to_be_bytes());
            sink.extend_from_slice(&value.to_be_bytes());
        }
        #[cfg(feature = "rdf-12")]
        EncodedTerm::RtlSmallBigDirLangStringLiteral { value, language_id } => {
            sink.push(TYPE_RTL_SMALL_BIG_DIR_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language_id.to_be_bytes());
            sink.extend_from_slice(&value.to_be_bytes());
        }
        #[cfg(feature = "rdf-12")]
        EncodedTerm::RtlBigSmallDirLangStringLiteral { value_id, language } => {
            sink.push(TYPE_RTL_BIG_SMALL_DIR_LANG_STRING_LITERAL);
            sink.extend_from_slice(&language.to_be_bytes());
            sink.extend_from_slice(&value_id.to_be_bytes());
        }
        #[cfg(feature = "rdf-12")]
        EncodedTerm::RtlBigBigDirLangStringLiteral {
            value_id,
            language_id,
        } => {
            sink.push(TYPE_RTL_BIG_BIG_DIR_LANG_STRING_LITERAL);
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
        EncodedTerm::BooleanLiteral(value) => sink.push(if bool::from(*value) {
            TYPE_BOOLEAN_LITERAL_TRUE
        } else {
            TYPE_BOOLEAN_LITERAL_FALSE
        }),
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
        #[cfg(feature = "rdf-12")]
        EncodedTerm::Triple(value) => {
            sink.push(TYPE_TRIPLE);
            write_term(sink, &value.subject);
            write_term(sink, &value.predicate);
            write_term(sink, &value.object);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TermRef;
    use crate::storage::numeric_encoder::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemoryStrStore {
        id2str: RefCell<HashMap<StrHash, String>>,
    }

    impl StrLookup for MemoryStrStore {
        fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
            Ok(self.id2str.borrow().get(key).cloned())
        }
    }

    impl MemoryStrStore {
        fn insert_term(&self, term: TermRef<'_>, encoded: &EncodedTerm) {
            insert_term(term, encoded, &mut |h, v| self.insert_str(h, v));
        }

        fn insert_str(&self, key: &StrHash, value: &str) {
            self.id2str
                .borrow_mut()
                .entry(*key)
                .or_insert_with(|| value.to_owned());
        }
    }

    #[test]
    fn test_encoding() {
        use crate::model::vocab::xsd;
        use crate::model::*;

        let store = MemoryStrStore::default();
        let terms: Vec<Term> = vec![
            NamedNode::new_unchecked("http://foo.com").into(),
            NamedNode::new_unchecked("http://bar.com").into(),
            NamedNode::new_unchecked("http://foo.com").into(),
            BlankNode::default().into(),
            BlankNode::new_unchecked("1234567890").into(),
            BlankNode::new_unchecked("bnode").into(),
            BlankNode::new_unchecked("foo-bnode-thisisaverylargeblanknode").into(),
            Literal::new_simple_literal("literal").into(),
            BlankNode::new_unchecked("foo-literal-thisisaverylargestringliteral").into(),
            Literal::from(true).into(),
            Literal::from(false).into(),
            Literal::from(1).into(),
            Literal::from(Decimal::from(1)).into(),
            Literal::from(1.2_f32).into(),
            Literal::from(1.2_f64).into(),
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
            #[cfg(feature = "rdf-12")]
            Literal::new_directional_language_tagged_literal_unchecked(
                "foo-fr",
                "fr",
                BaseDirection::Ltr,
            )
            .into(),
            #[cfg(feature = "rdf-12")]
            Literal::new_directional_language_tagged_literal_unchecked(
                "foo-fr-literal-thisisaverylargelanguagetaggedstringliteral",
                "fr",
                BaseDirection::Ltr,
            )
            .into(),
            #[cfg(feature = "rdf-12")]
            Literal::new_directional_language_tagged_literal_unchecked(
                "foo-big",
                "fr-FR-Latn-x-foo-bar-baz-bat-aaaa-bbbb-cccc",
                BaseDirection::Ltr,
            )
            .into(),
            #[cfg(feature = "rdf-12")]
            Literal::new_directional_language_tagged_literal_unchecked(
                "foo-big-literal-thisisaverylargelanguagetaggedstringliteral",
                "fr-FR-Latn-x-foo-bar-baz-bat-aaaa-bbbb-cccc",
                BaseDirection::Ltr,
            )
            .into(),
            #[cfg(feature = "rdf-12")]
            Literal::new_directional_language_tagged_literal_unchecked(
                "foo-fr",
                "fr",
                BaseDirection::Rtl,
            )
            .into(),
            #[cfg(feature = "rdf-12")]
            Literal::new_directional_language_tagged_literal_unchecked(
                "foo-fr-literal-thisisaverylargelanguagetaggedstringliteral",
                "fr",
                BaseDirection::Rtl,
            )
            .into(),
            #[cfg(feature = "rdf-12")]
            Literal::new_directional_language_tagged_literal_unchecked(
                "foo-big",
                "fr-FR-Latn-x-foo-bar-baz-bat-aaaa-bbbb-cccc",
                BaseDirection::Rtl,
            )
            .into(),
            #[cfg(feature = "rdf-12")]
            Literal::new_directional_language_tagged_literal_unchecked(
                "foo-big-literal-thisisaverylargelanguagetaggedstringliteral",
                "fr-FR-Latn-x-foo-bar-baz-bat-aaaa-bbbb-cccc",
                BaseDirection::Rtl,
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
            #[cfg(feature = "rdf-12")]
            Triple::new(
                NamedNode::new_unchecked("http://foo.com"),
                NamedNode::new_unchecked("http://bar.com"),
                Literal::from(true),
            )
            .into(),
        ];
        for term in terms {
            let encoded = term.as_ref().into();
            store.insert_term(term.as_ref(), &encoded);
            assert_eq!(encoded, term.as_ref().into());
            assert_eq!(term, store.decode_term(&encoded).unwrap());

            let mut buffer = Vec::new();
            write_term(&mut buffer, &encoded);
            assert_eq!(encoded, buffer.as_slice().read_term().unwrap());
        }
    }
}
