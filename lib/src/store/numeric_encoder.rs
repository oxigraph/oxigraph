use crate::model::vocab::rdf;
use crate::model::vocab::xsd;
use crate::model::*;
use crate::Result;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use chrono::format::{parse, Parsed, StrftimeItems};
use chrono::prelude::*;
use failure::format_err;
use failure::Backtrace;
use failure::Fail;
use ordered_float::OrderedFloat;
use rio_api::model as rio;
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::io::Write;
use std::ops::Deref;
use std::str;
use std::sync::PoisonError;
use std::sync::RwLock;
use uuid::Uuid;

const EMPTY_STRING_ID: u64 = 0;
const RDF_LANG_STRING_ID: u64 = 1;
const XSD_STRING_ID: u64 = 2;
const XSD_BOOLEAN_ID: u64 = 3;
const XSD_FLOAT_ID: u64 = 4;
const XSD_DOUBLE_ID: u64 = 5;
const XSD_INTEGER_ID: u64 = 6;
const XSD_DECIMAL_ID: u64 = 7;
const XSD_DATE_TIME_ID: u64 = 8;
const XSD_DATE_ID: u64 = 9;
const XSD_TIME_ID: u64 = 10;

pub trait StringStore {
    type StringType: Deref<Target = str> + ToString + Into<String>;

    fn get_str(&self, id: u64) -> Result<Option<Self::StringType>>;
    fn get_str_id(&self, value: &str) -> Result<Option<u64>>;
    fn insert_str(&self, value: &str) -> Result<u64>;

    /// Should be called when the bytes store is created
    fn set_first_strings(&self) -> Result<()> {
        if EMPTY_STRING_ID == self.insert_str("")?
            && RDF_LANG_STRING_ID == self.insert_str(rdf::LANG_STRING.as_str())?
            && XSD_STRING_ID == self.insert_str(xsd::STRING.as_str())?
            && XSD_BOOLEAN_ID == self.insert_str(xsd::BOOLEAN.as_str())?
            && XSD_FLOAT_ID == self.insert_str(xsd::FLOAT.as_str())?
            && XSD_DOUBLE_ID == self.insert_str(xsd::DOUBLE.as_str())?
            && XSD_INTEGER_ID == self.insert_str(xsd::INTEGER.as_str())?
            && XSD_DECIMAL_ID == self.insert_str(xsd::DECIMAL.as_str())?
            && XSD_DATE_TIME_ID == self.insert_str(xsd::DATE_TIME.as_str())?
            && XSD_DATE_ID == self.insert_str(xsd::DATE.as_str())?
            && XSD_TIME_ID == self.insert_str(xsd::TIME.as_str())?
        {
            Ok(())
        } else {
            Err(format_err!(
                "Failed to properly setup the basic string ids in the dictionnary"
            ))
        }
    }
}

impl<'a, S: StringStore> StringStore for &'a S {
    type StringType = S::StringType;

    fn insert_str(&self, value: &str) -> Result<u64> {
        (*self).insert_str(value)
    }

    fn get_str(&self, id: u64) -> Result<Option<S::StringType>> {
        (*self).get_str(id)
    }

    fn get_str_id(&self, value: &str) -> Result<Option<u64>> {
        (*self).get_str_id(value)
    }
}

pub struct MemoryStringStore {
    id2str: RwLock<Vec<String>>,
    str2id: RwLock<BTreeMap<String, u64>>,
}

impl Default for MemoryStringStore {
    fn default() -> Self {
        let new = Self {
            id2str: RwLock::default(),
            str2id: RwLock::default(),
        };
        new.set_first_strings().unwrap();
        new
    }
}

impl StringStore for MemoryStringStore {
    type StringType = String;

    fn insert_str(&self, value: &str) -> Result<u64> {
        let mut id2str = self.id2str.write().map_err(MutexPoisonError::from)?;
        let mut str2id = self.str2id.write().map_err(MutexPoisonError::from)?;
        Ok(if let Some(id) = str2id.get(value) {
            *id
        } else {
            let id = id2str.len() as u64;
            id2str.push(value.to_string());
            str2id.insert(value.to_string(), id);
            id
        })
    }

    fn get_str(&self, id: u64) -> Result<Option<String>> {
        //TODO: avoid copy by adding a lifetime limit to get_str
        let id2str = self.id2str.read().map_err(MutexPoisonError::from)?;
        Ok(if id2str.len() as u64 <= id {
            None
        } else {
            Some(id2str[id as usize].to_owned())
        })
    }

    fn get_str_id(&self, value: &str) -> Result<Option<u64>> {
        let str2id = self.str2id.read().map_err(MutexPoisonError::from)?;
        Ok(str2id.get(value).cloned())
    }
}

const TYPE_DEFAULT_GRAPH_ID: u8 = 0;
const TYPE_NAMED_NODE_ID: u8 = 1;
const TYPE_BLANK_NODE_ID: u8 = 2;
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
const TYPE_NAIVE_DATE_TIME_LITERAL: u8 = 14;
const TYPE_DATE_LITERAL: u8 = 15;
const TYPE_NAIVE_DATE_LITERAL: u8 = 16;
const TYPE_NAIVE_TIME_LITERAL: u8 = 17;

pub static ENCODED_DEFAULT_GRAPH: EncodedTerm = EncodedTerm::DefaultGraph;
pub static ENCODED_EMPTY_STRING_LITERAL: EncodedTerm = EncodedTerm::StringLiteral {
    value_id: EMPTY_STRING_ID,
};
pub static ENCODED_RDF_LANG_STRING_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: RDF_LANG_STRING_ID,
};
pub static ENCODED_XSD_STRING_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_STRING_ID,
};
pub static ENCODED_XSD_BOOLEAN_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_BOOLEAN_ID,
};
pub static ENCODED_XSD_FLOAT_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_FLOAT_ID,
};
pub static ENCODED_XSD_DOUBLE_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DOUBLE_ID,
};
pub static ENCODED_XSD_INTEGER_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_INTEGER_ID,
};
pub static ENCODED_XSD_DECIMAL_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DECIMAL_ID,
};
pub static ENCODED_XSD_DATE_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DATE_ID,
};
pub static ENCODED_XSD_TIME_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_TIME_ID,
};
pub static ENCODED_XSD_DATE_TIME_NAMED_NODE: EncodedTerm = EncodedTerm::NamedNode {
    iri_id: XSD_DATE_TIME_ID,
};

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub enum EncodedTerm {
    DefaultGraph,
    NamedNode { iri_id: u64 },
    BlankNode(Uuid),
    StringLiteral { value_id: u64 },
    LangStringLiteral { value_id: u64, language_id: u64 },
    TypedLiteral { value_id: u64, datatype_id: u64 },
    BooleanLiteral(bool),
    FloatLiteral(OrderedFloat<f32>),
    DoubleLiteral(OrderedFloat<f64>),
    IntegerLiteral(i128),
    DecimalLiteral(Decimal),
    DateLiteral(Date<FixedOffset>),
    NaiveDateLiteral(NaiveDate),
    NaiveTimeLiteral(NaiveTime),
    DateTimeLiteral(DateTime<FixedOffset>),
    NaiveDateTimeLiteral(NaiveDateTime),
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
            EncodedTerm::BlankNode(_) => true,
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
            | EncodedTerm::NaiveDateLiteral(_)
            | EncodedTerm::NaiveTimeLiteral(_)
            | EncodedTerm::DateTimeLiteral(_)
            | EncodedTerm::NaiveDateTimeLiteral(_) => true,
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
            EncodedTerm::NaiveDateLiteral(..) => Some(ENCODED_XSD_DATE_NAMED_NODE),
            EncodedTerm::NaiveTimeLiteral(..) => Some(ENCODED_XSD_TIME_NAMED_NODE),
            EncodedTerm::DateTimeLiteral(..) | EncodedTerm::NaiveDateTimeLiteral(..) => {
                Some(ENCODED_XSD_DATE_TIME_NAMED_NODE)
            }
            _ => None,
        }
    }

    fn type_id(&self) -> u8 {
        match self {
            EncodedTerm::DefaultGraph { .. } => TYPE_DEFAULT_GRAPH_ID,
            EncodedTerm::NamedNode { .. } => TYPE_NAMED_NODE_ID,
            EncodedTerm::BlankNode(_) => TYPE_BLANK_NODE_ID,
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
            EncodedTerm::NaiveDateLiteral(_) => TYPE_NAIVE_DATE_LITERAL,
            EncodedTerm::NaiveTimeLiteral(_) => TYPE_NAIVE_TIME_LITERAL,
            EncodedTerm::DateTimeLiteral(_) => TYPE_DATE_TIME_LITERAL,
            EncodedTerm::NaiveDateTimeLiteral(_) => TYPE_NAIVE_DATE_TIME_LITERAL,
        }
    }
}

impl From<bool> for EncodedTerm {
    fn from(value: bool) -> Self {
        EncodedTerm::BooleanLiteral(value)
    }
}

impl From<i128> for EncodedTerm {
    fn from(value: i128) -> Self {
        EncodedTerm::IntegerLiteral(value)
    }
}

impl From<i64> for EncodedTerm {
    fn from(value: i64) -> Self {
        EncodedTerm::IntegerLiteral(value.into())
    }
}

impl From<u64> for EncodedTerm {
    fn from(value: u64) -> Self {
        EncodedTerm::IntegerLiteral(value.into())
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

impl From<f32> for EncodedTerm {
    fn from(value: f32) -> Self {
        EncodedTerm::FloatLiteral(value.into())
    }
}

impl From<f64> for EncodedTerm {
    fn from(value: f64) -> Self {
        EncodedTerm::DoubleLiteral(value.into())
    }
}

impl From<Decimal> for EncodedTerm {
    fn from(value: Decimal) -> Self {
        EncodedTerm::DecimalLiteral(value)
    }
}

impl From<Date<FixedOffset>> for EncodedTerm {
    fn from(value: Date<FixedOffset>) -> Self {
        EncodedTerm::DateLiteral(value)
    }
}

impl From<NaiveDate> for EncodedTerm {
    fn from(value: NaiveDate) -> Self {
        EncodedTerm::NaiveDateLiteral(value)
    }
}

impl From<NaiveTime> for EncodedTerm {
    fn from(value: NaiveTime) -> Self {
        EncodedTerm::NaiveTimeLiteral(value)
    }
}

impl From<DateTime<FixedOffset>> for EncodedTerm {
    fn from(value: DateTime<FixedOffset>) -> Self {
        EncodedTerm::DateTimeLiteral(value)
    }
}

impl From<NaiveDateTime> for EncodedTerm {
    fn from(value: NaiveDateTime) -> Self {
        EncodedTerm::NaiveDateTimeLiteral(value)
    }
}

impl From<BlankNode> for EncodedTerm {
    fn from(node: BlankNode) -> Self {
        EncodedTerm::BlankNode(node.uuid())
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct EncodedQuad {
    pub subject: EncodedTerm,
    pub predicate: EncodedTerm,
    pub object: EncodedTerm,
    pub graph_name: EncodedTerm,
}

impl EncodedQuad {
    pub fn new(
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

pub trait TermReader {
    fn read_term(&mut self) -> Result<EncodedTerm>;
    fn read_spog_quad(&mut self) -> Result<EncodedQuad>;
    fn read_posg_quad(&mut self) -> Result<EncodedQuad>;
    fn read_ospg_quad(&mut self) -> Result<EncodedQuad>;
}

impl<R: Read> TermReader for R {
    fn read_term(&mut self) -> Result<EncodedTerm> {
        match self.read_u8()? {
            TYPE_DEFAULT_GRAPH_ID => Ok(EncodedTerm::DefaultGraph),
            TYPE_NAMED_NODE_ID => Ok(EncodedTerm::NamedNode {
                iri_id: self.read_u64::<LittleEndian>()?,
            }),
            TYPE_BLANK_NODE_ID => {
                let mut uuid_buffer = [0 as u8; 16];
                self.read_exact(&mut uuid_buffer)?;
                Ok(EncodedTerm::BlankNode(Uuid::from_bytes(uuid_buffer)))
            }
            TYPE_LANG_STRING_LITERAL_ID => Ok(EncodedTerm::LangStringLiteral {
                language_id: self.read_u64::<LittleEndian>()?,
                value_id: self.read_u64::<LittleEndian>()?,
            }),
            TYPE_TYPED_LITERAL_ID => Ok(EncodedTerm::TypedLiteral {
                datatype_id: self.read_u64::<LittleEndian>()?,
                value_id: self.read_u64::<LittleEndian>()?,
            }),
            TYPE_STRING_LITERAL => Ok(EncodedTerm::StringLiteral {
                value_id: self.read_u64::<LittleEndian>()?,
            }),
            TYPE_BOOLEAN_LITERAL_TRUE => Ok(EncodedTerm::BooleanLiteral(true)),
            TYPE_BOOLEAN_LITERAL_FALSE => Ok(EncodedTerm::BooleanLiteral(false)),
            TYPE_FLOAT_LITERAL => Ok(EncodedTerm::FloatLiteral(OrderedFloat(
                self.read_f32::<LittleEndian>()?,
            ))),
            TYPE_DOUBLE_LITERAL => Ok(EncodedTerm::DoubleLiteral(OrderedFloat(
                self.read_f64::<LittleEndian>()?,
            ))),
            TYPE_INTEGER_LITERAL => Ok(EncodedTerm::IntegerLiteral(
                self.read_i128::<LittleEndian>()?,
            )),
            TYPE_DECIMAL_LITERAL => {
                let mut buffer = [0 as u8; 16];
                self.read_exact(&mut buffer)?;
                Ok(EncodedTerm::DecimalLiteral(Decimal::deserialize(buffer)))
            }
            TYPE_DATE_LITERAL => Ok(EncodedTerm::DateLiteral(Date::from_utc(
                NaiveDate::from_num_days_from_ce_opt(self.read_i32::<LittleEndian>()?)
                    .ok_or_else(|| format_err!("Invalid date serialization"))?,
                FixedOffset::east_opt(self.read_i32::<LittleEndian>()?)
                    .ok_or_else(|| format_err!("Invalid timezone offset"))?,
            ))),
            TYPE_NAIVE_DATE_LITERAL => Ok(EncodedTerm::NaiveDateLiteral(
                NaiveDate::from_num_days_from_ce_opt(self.read_i32::<LittleEndian>()?)
                    .ok_or_else(|| format_err!("Invalid date serialization"))?,
            )),
            TYPE_NAIVE_TIME_LITERAL => Ok(EncodedTerm::NaiveTimeLiteral(
                NaiveTime::from_num_seconds_from_midnight_opt(
                    self.read_u32::<LittleEndian>()?,
                    self.read_u32::<LittleEndian>()?,
                )
                .ok_or_else(|| format_err!("Invalid time serialization"))?,
            )),
            TYPE_DATE_TIME_LITERAL => Ok(EncodedTerm::DateTimeLiteral(DateTime::from_utc(
                NaiveDateTime::from_timestamp_opt(
                    self.read_i64::<LittleEndian>()?,
                    self.read_u32::<LittleEndian>()?,
                )
                .ok_or_else(|| format_err!("Invalid date time serialization"))?,
                FixedOffset::east_opt(self.read_i32::<LittleEndian>()?)
                    .ok_or_else(|| format_err!("Invalid timezone offset"))?,
            ))),
            TYPE_NAIVE_DATE_TIME_LITERAL => Ok(EncodedTerm::NaiveDateTimeLiteral(
                NaiveDateTime::from_timestamp_opt(
                    self.read_i64::<LittleEndian>()?,
                    self.read_u32::<LittleEndian>()?,
                )
                .ok_or_else(|| format_err!("Invalid date time serialization"))?,
            )),
            _ => Err(format_err!("the term buffer has an invalid type id")),
        }
    }

    fn read_spog_quad(&mut self) -> Result<EncodedQuad> {
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

    fn read_posg_quad(&mut self) -> Result<EncodedQuad> {
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

    fn read_ospg_quad(&mut self) -> Result<EncodedQuad> {
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
}

pub trait TermWriter {
    fn write_term(&mut self, term: EncodedTerm) -> Result<()>;
    fn write_spog_quad(&mut self, quad: &EncodedQuad) -> Result<()>;
    fn write_posg_quad(&mut self, quad: &EncodedQuad) -> Result<()>;
    fn write_ospg_quad(&mut self, quad: &EncodedQuad) -> Result<()>;
}

impl<R: Write> TermWriter for R {
    fn write_term(&mut self, term: EncodedTerm) -> Result<()> {
        self.write_u8(term.type_id())?;
        match term {
            EncodedTerm::DefaultGraph => {}
            EncodedTerm::NamedNode { iri_id } => self.write_u64::<LittleEndian>(iri_id)?,
            EncodedTerm::BlankNode(id) => self.write_all(id.as_bytes())?,
            EncodedTerm::StringLiteral { value_id } => {
                self.write_u64::<LittleEndian>(value_id)?;
            }
            EncodedTerm::LangStringLiteral {
                value_id,
                language_id,
            } => {
                self.write_u64::<LittleEndian>(language_id)?;
                self.write_u64::<LittleEndian>(value_id)?;
            }
            EncodedTerm::TypedLiteral {
                value_id,
                datatype_id,
            } => {
                self.write_u64::<LittleEndian>(datatype_id)?;
                self.write_u64::<LittleEndian>(value_id)?;
            }
            EncodedTerm::BooleanLiteral(_) => {}
            EncodedTerm::FloatLiteral(value) => self.write_f32::<LittleEndian>(*value)?,
            EncodedTerm::DoubleLiteral(value) => self.write_f64::<LittleEndian>(*value)?,
            EncodedTerm::IntegerLiteral(value) => self.write_i128::<LittleEndian>(value)?,
            EncodedTerm::DecimalLiteral(value) => self.write_all(&value.serialize())?,
            EncodedTerm::DateLiteral(value) => {
                self.write_i32::<LittleEndian>(value.num_days_from_ce())?;
                self.write_i32::<LittleEndian>(value.timezone().local_minus_utc())?;
            }
            EncodedTerm::NaiveDateLiteral(value) => {
                self.write_i32::<LittleEndian>(value.num_days_from_ce())?;
            }
            EncodedTerm::NaiveTimeLiteral(value) => {
                self.write_u32::<LittleEndian>(value.num_seconds_from_midnight())?;
                self.write_u32::<LittleEndian>(value.nanosecond())?;
            }
            EncodedTerm::DateTimeLiteral(value) => {
                self.write_i64::<LittleEndian>(value.timestamp())?;
                self.write_u32::<LittleEndian>(value.timestamp_subsec_nanos())?;
                self.write_i32::<LittleEndian>(value.timezone().local_minus_utc())?;
            }
            EncodedTerm::NaiveDateTimeLiteral(value) => {
                self.write_i64::<LittleEndian>(value.timestamp())?;
                self.write_u32::<LittleEndian>(value.timestamp_subsec_nanos())?;
            }
        }
        Ok(())
    }

    fn write_spog_quad(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.write_term(quad.subject)?;
        self.write_term(quad.predicate)?;
        self.write_term(quad.object)?;
        self.write_term(quad.graph_name)?;
        Ok(())
    }

    fn write_posg_quad(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.write_term(quad.predicate)?;
        self.write_term(quad.object)?;
        self.write_term(quad.subject)?;
        self.write_term(quad.graph_name)?;
        Ok(())
    }

    fn write_ospg_quad(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.write_term(quad.object)?;
        self.write_term(quad.subject)?;
        self.write_term(quad.predicate)?;
        self.write_term(quad.graph_name)?;
        Ok(())
    }
}

pub struct Encoder<S: StringStore> {
    string_store: S,
}

impl<S: StringStore> Encoder<S> {
    pub fn new(string_store: S) -> Self {
        Self { string_store }
    }

    pub fn encode_named_node(&self, named_node: &NamedNode) -> Result<EncodedTerm> {
        self.encode_rio_named_node(named_node.into())
    }

    pub fn encode_blank_node(&self, blank_node: &BlankNode) -> Result<EncodedTerm> {
        Ok(EncodedTerm::BlankNode(blank_node.uuid()))
    }

    pub fn encode_literal(&self, literal: &Literal) -> Result<EncodedTerm> {
        self.encode_rio_literal(literal.into())
    }

    pub fn encode_named_or_blank_node(&self, term: &NamedOrBlankNode) -> Result<EncodedTerm> {
        match term {
            NamedOrBlankNode::NamedNode(named_node) => self.encode_named_node(named_node),
            NamedOrBlankNode::BlankNode(blank_node) => self.encode_blank_node(blank_node),
        }
    }

    pub fn encode_term(&self, term: &Term) -> Result<EncodedTerm> {
        match term {
            Term::NamedNode(named_node) => self.encode_named_node(named_node),
            Term::BlankNode(blank_node) => self.encode_blank_node(blank_node),
            Term::Literal(literal) => self.encode_literal(literal),
        }
    }

    pub fn encode_quad(&self, quad: &Quad) -> Result<EncodedQuad> {
        Ok(EncodedQuad {
            subject: self.encode_named_or_blank_node(quad.subject())?,
            predicate: self.encode_named_node(quad.predicate())?,
            object: self.encode_term(quad.object())?,
            graph_name: match quad.graph_name() {
                Some(graph_name) => self.encode_named_or_blank_node(&graph_name)?,
                None => ENCODED_DEFAULT_GRAPH,
            },
        })
    }

    pub fn encode_triple_in_graph(
        &self,
        triple: &Triple,
        graph_name: EncodedTerm,
    ) -> Result<EncodedQuad> {
        Ok(EncodedQuad {
            subject: self.encode_named_or_blank_node(triple.subject())?,
            predicate: self.encode_named_node(triple.predicate())?,
            object: self.encode_term(triple.object())?,
            graph_name,
        })
    }

    pub fn encode_rio_named_node(&self, named_node: rio::NamedNode) -> Result<EncodedTerm> {
        Ok(EncodedTerm::NamedNode {
            iri_id: self.string_store.insert_str(named_node.iri)?,
        })
    }

    pub fn encode_rio_blank_node(
        &self,
        blank_node: rio::BlankNode,
        bnodes_map: &mut HashMap<String, Uuid>,
    ) -> Result<EncodedTerm> {
        Ok(if let Some(uuid) = bnodes_map.get(blank_node.id) {
            EncodedTerm::BlankNode(*uuid)
        } else {
            let uuid = Uuid::new_v4();
            bnodes_map.insert(blank_node.id.to_owned(), uuid);
            EncodedTerm::BlankNode(uuid)
        })
    }

    pub fn encode_rio_literal(&self, literal: rio::Literal) -> Result<EncodedTerm> {
        Ok(match literal {
            rio::Literal::Simple { value } => EncodedTerm::StringLiteral {
                value_id: self.string_store.insert_str(value)?,
            },
            rio::Literal::LanguageTaggedString { value, language } => {
                EncodedTerm::LangStringLiteral {
                    value_id: self.string_store.insert_str(value)?,
                    language_id: if language.bytes().all(|b| b.is_ascii_lowercase()) {
                        self.string_store.insert_str(language)
                    } else {
                        self.string_store.insert_str(&language.to_ascii_lowercase())
                    }?,
                }
            }
            rio::Literal::Typed { value, datatype } => {
                match match datatype.iri {
                    "http://www.w3.org/2001/XMLSchema#boolean" => self.encode_boolean_str(value),
                    "http://www.w3.org/2001/XMLSchema#string" => Some(EncodedTerm::StringLiteral {
                        value_id: self.string_store.insert_str(value)?,
                    }),
                    "http://www.w3.org/2001/XMLSchema#float" => self.encode_float_str(value),
                    "http://www.w3.org/2001/XMLSchema#double" => self.encode_double_str(value),
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
                        self.encode_integer_str(value)
                    }
                    "http://www.w3.org/2001/XMLSchema#decimal" => self.encode_decimal_str(value),
                    "http://www.w3.org/2001/XMLSchema#date" => self.encode_date_str(value),
                    "http://www.w3.org/2001/XMLSchema#time" => self.encode_time_str(value),
                    "http://www.w3.org/2001/XMLSchema#dateTime"
                    | "http://www.w3.org/2001/XMLSchema#dateTimeStamp" => {
                        self.encode_date_time_str(value)
                    }
                    _ => None,
                } {
                    Some(v) => v,
                    None => EncodedTerm::TypedLiteral {
                        value_id: self.string_store.insert_str(value)?,
                        datatype_id: self.string_store.insert_str(datatype.iri)?,
                    },
                }
            }
        })
    }

    pub fn encode_rio_named_or_blank_node(
        &self,
        term: rio::NamedOrBlankNode,
        bnodes_map: &mut HashMap<String, Uuid>,
    ) -> Result<EncodedTerm> {
        match term {
            rio::NamedOrBlankNode::NamedNode(named_node) => self.encode_rio_named_node(named_node),
            rio::NamedOrBlankNode::BlankNode(blank_node) => {
                self.encode_rio_blank_node(blank_node, bnodes_map)
            }
        }
    }

    pub fn encode_rio_term(
        &self,
        term: rio::Term,
        bnodes_map: &mut HashMap<String, Uuid>,
    ) -> Result<EncodedTerm> {
        match term {
            rio::Term::NamedNode(named_node) => self.encode_rio_named_node(named_node),
            rio::Term::BlankNode(blank_node) => self.encode_rio_blank_node(blank_node, bnodes_map),
            rio::Term::Literal(literal) => self.encode_rio_literal(literal),
        }
    }

    pub fn encode_rio_quad(
        &self,
        quad: rio::Quad,
        bnodes_map: &mut HashMap<String, Uuid>,
    ) -> Result<EncodedQuad> {
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

    pub fn encode_rio_triple_in_graph(
        &self,
        triple: rio::Triple,
        graph_name: EncodedTerm,
        bnodes_map: &mut HashMap<String, Uuid>,
    ) -> Result<EncodedQuad> {
        Ok(EncodedQuad {
            subject: self.encode_rio_named_or_blank_node(triple.subject, bnodes_map)?,
            predicate: self.encode_rio_named_node(triple.predicate)?,
            object: self.encode_rio_term(triple.object, bnodes_map)?,
            graph_name,
        })
    }

    pub fn encode_boolean_str(&self, value: &str) -> Option<EncodedTerm> {
        match value {
            "true" | "1" => Some(EncodedTerm::BooleanLiteral(true)),
            "false" | "0" => Some(EncodedTerm::BooleanLiteral(false)),
            _ => None,
        }
    }

    pub fn encode_float_str(&self, value: &str) -> Option<EncodedTerm> {
        value
            .parse()
            .map(|value| EncodedTerm::FloatLiteral(OrderedFloat(value)))
            .ok()
    }

    pub fn encode_double_str(&self, value: &str) -> Option<EncodedTerm> {
        value
            .parse()
            .map(|value| EncodedTerm::DoubleLiteral(OrderedFloat(value)))
            .ok()
    }

    pub fn encode_integer_str(&self, value: &str) -> Option<EncodedTerm> {
        value.parse().map(EncodedTerm::IntegerLiteral).ok()
    }

    pub fn encode_decimal_str(&self, value: &str) -> Option<EncodedTerm> {
        value.parse().map(EncodedTerm::DecimalLiteral).ok()
    }

    pub fn encode_date_str(&self, value: &str) -> Option<EncodedTerm> {
        let mut parsed = Parsed::new();
        match parse(&mut parsed, &value, StrftimeItems::new("%Y-%m-%d%:z")).and_then(|_| {
            Ok(Date::from_utc(
                parsed.to_naive_date()?,
                parsed.to_fixed_offset()?,
            ))
        }) {
            Ok(value) => Some(EncodedTerm::DateLiteral(value)),
            Err(_) => match NaiveDate::parse_from_str(&value, "%Y-%m-%dZ") {
                Ok(value) => Some(EncodedTerm::DateLiteral(Date::from_utc(
                    value,
                    FixedOffset::east(0),
                ))),
                Err(_) => NaiveDate::parse_from_str(&value, "%Y-%m-%d")
                    .map(EncodedTerm::NaiveDateLiteral)
                    .ok(),
            },
        }
    }

    pub fn encode_time_str(&self, value: &str) -> Option<EncodedTerm> {
        NaiveTime::parse_from_str(&value, "%H:%M:%S")
            .map(EncodedTerm::NaiveTimeLiteral)
            .ok()
    }

    pub fn encode_date_time_str(&self, value: &str) -> Option<EncodedTerm> {
        match DateTime::parse_from_rfc3339(&value) {
            Ok(value) => Some(EncodedTerm::DateTimeLiteral(value)),
            Err(_) => NaiveDateTime::parse_from_str(&value, "%Y-%m-%dT%H:%M:%S")
                .map(EncodedTerm::NaiveDateTimeLiteral)
                .ok(),
        }
    }

    pub fn decode_term(&self, encoded: EncodedTerm) -> Result<Term> {
        match encoded {
            EncodedTerm::DefaultGraph => {
                Err(format_err!("The default graph tag is not a valid term"))
            }
            EncodedTerm::NamedNode { iri_id } => {
                Ok(NamedNode::new_from_string(self.get_str(iri_id)?).into())
            }
            EncodedTerm::BlankNode(id) => Ok(BlankNode::from(id).into()),
            EncodedTerm::StringLiteral { value_id } => {
                Ok(Literal::new_simple_literal(self.get_str(value_id)?).into())
            }
            EncodedTerm::LangStringLiteral {
                value_id,
                language_id,
            } => Ok(Literal::new_language_tagged_literal(
                self.get_str(value_id)?,
                self.get_str(language_id)?,
            )
            .into()),
            EncodedTerm::TypedLiteral {
                value_id,
                datatype_id,
            } => Ok(Literal::new_typed_literal(
                self.get_str(value_id)?,
                NamedNode::new_from_string(self.get_str(datatype_id)?),
            )
            .into()),
            EncodedTerm::BooleanLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::FloatLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::DoubleLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::IntegerLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DecimalLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DateLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::NaiveDateLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::NaiveTimeLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DateTimeLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::NaiveDateTimeLiteral(value) => Ok(Literal::from(value).into()),
        }
    }

    fn get_str(&self, id: u64) -> Result<S::StringType> {
        self.string_store.get_str(id)?.ok_or_else(|| {
            format_err!(
                "Not able to find the string with id {} in the string store",
                id
            )
        })
    }

    pub fn decode_named_or_blank_node(&self, encoded: EncodedTerm) -> Result<NamedOrBlankNode> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node.into()),
            Term::BlankNode(blank_node) => Ok(blank_node.into()),
            Term::Literal(_) => Err(format_err!(
                "A literal has ben found instead of a named node"
            )),
        }
    }

    pub fn decode_named_node(&self, encoded: EncodedTerm) -> Result<NamedNode> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node),
            Term::BlankNode(_) => Err(format_err!(
                "A blank node has been found instead of a named node"
            )),
            Term::Literal(_) => Err(format_err!(
                "A literal has ben found instead of a named node"
            )),
        }
    }

    pub fn decode_triple(&self, encoded: &EncodedQuad) -> Result<Triple> {
        Ok(Triple::new(
            self.decode_named_or_blank_node(encoded.subject)?,
            self.decode_named_node(encoded.predicate)?,
            self.decode_term(encoded.object)?,
        ))
    }

    pub fn decode_quad(&self, encoded: &EncodedQuad) -> Result<Quad> {
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

impl<S: StringStore + Default> Default for Encoder<S> {
    fn default() -> Self {
        Self {
            string_store: S::default(),
        }
    }
}

#[derive(Debug, Fail)]
#[fail(display = "Mutex Mutex was poisoned")]
pub struct MutexPoisonError {
    backtrace: Backtrace,
}

impl<T> From<PoisonError<T>> for MutexPoisonError {
    fn from(_: PoisonError<T>) -> Self {
        Self {
            backtrace: Backtrace::new(),
        }
    }
}

#[test]
fn test_encoding() {
    let encoder: Encoder<MemoryStringStore> = Encoder::default();
    let terms: Vec<Term> = vec![
        NamedNode::new_from_string("http://foo.com").into(),
        NamedNode::new_from_string("http://bar.com").into(),
        NamedNode::new_from_string("http://foo.com").into(),
        BlankNode::default().into(),
        Literal::new_simple_literal("foo").into(),
        Literal::from(true).into(),
        Literal::from(1.2).into(),
        Literal::from(1).into(),
        Literal::from("foo").into(),
        Literal::new_language_tagged_literal("foo", "fr").into(),
        Literal::new_language_tagged_literal("foo", "FR").into(),
    ];
    for term in terms {
        let encoded = encoder.encode_term(&term).unwrap();
        assert_eq!(term, encoder.decode_term(encoded).unwrap())
    }
}

#[test]
fn test_encoded_term_size() {
    use std::mem::size_of;

    assert_eq!(size_of::<EncodedTerm>(), 24);
}
