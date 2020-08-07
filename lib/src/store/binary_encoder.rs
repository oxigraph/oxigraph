use crate::error::invalid_data_error;
use crate::model::xsd::*;
use crate::store::numeric_encoder::StrId;
use siphasher::sip128::{Hasher128, SipHasher24};
use std::hash::Hasher;
use std::io;
use std::io::{Cursor, Read};
use std::mem::size_of;

type EncodedTerm = crate::store::numeric_encoder::EncodedTerm<StrHash>;
type EncodedQuad = crate::store::numeric_encoder::EncodedQuad<StrHash>;

pub const WRITTEN_TERM_MAX_SIZE: usize = size_of::<u8>() + 2 * size_of::<StrHash>();
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

#[derive(Clone, Copy)]
pub enum QuadEncoding {
    SPOG,
    POSG,
    OSPG,
    GSPO,
    GPOS,
    GOSP,
    DSPO,
    DPOS,
    DOSP,
}

impl QuadEncoding {
    pub fn decode(self, buffer: &[u8]) -> Result<EncodedQuad, io::Error> {
        let mut cursor = Cursor::new(&buffer);
        match self {
            QuadEncoding::SPOG => cursor.read_spog_quad(),
            QuadEncoding::POSG => cursor.read_posg_quad(),
            QuadEncoding::OSPG => cursor.read_ospg_quad(),
            QuadEncoding::GSPO => cursor.read_gspo_quad(),
            QuadEncoding::GPOS => cursor.read_gpos_quad(),
            QuadEncoding::GOSP => cursor.read_gosp_quad(),
            QuadEncoding::DSPO => cursor.read_dspo_quad(),
            QuadEncoding::DPOS => cursor.read_dpos_quad(),
            QuadEncoding::DOSP => cursor.read_dosp_quad(),
        }
    }
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
        EncodedTerm::DefaultGraph => sink.push(TYPE_DEFAULT_GRAPH_ID),
        EncodedTerm::NamedNode { iri_id } => {
            sink.push(TYPE_NAMED_NODE_ID);
            iri_id.push_be_bytes(sink)
        }
        EncodedTerm::InlineBlankNode { id } => {
            sink.push(TYPE_INLINE_BLANK_NODE_ID);
            sink.extend_from_slice(&id.to_be_bytes())
        }
        EncodedTerm::NamedBlankNode { id_id } => {
            sink.push(TYPE_NAMED_BLANK_NODE_ID);
            id_id.push_be_bytes(sink)
        }
        EncodedTerm::StringLiteral { value_id } => {
            sink.push(TYPE_STRING_LITERAL);
            value_id.push_be_bytes(sink)
        }
        EncodedTerm::LangStringLiteral {
            value_id,
            language_id,
        } => {
            sink.push(TYPE_LANG_STRING_LITERAL_ID);
            value_id.push_be_bytes(sink);
            language_id.push_be_bytes(sink);
        }
        EncodedTerm::TypedLiteral {
            value_id,
            datatype_id,
        } => {
            sink.push(TYPE_TYPED_LITERAL_ID);
            value_id.push_be_bytes(sink);
            datatype_id.push_be_bytes(sink);
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
        EncodedTerm::DateLiteral(value) => {
            sink.push(TYPE_DATE_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::TimeLiteral(value) => {
            sink.push(TYPE_TIME_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::DateTimeLiteral(value) => {
            sink.push(TYPE_DATE_TIME_LITERAL);
            sink.extend_from_slice(&value.to_be_bytes())
        }
        EncodedTerm::DurationLiteral(value) => {
            sink.push(TYPE_DURATION_LITERAL);
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
