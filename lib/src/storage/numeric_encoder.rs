#![allow(clippy::unreadable_literal)]

use crate::model::*;
use crate::storage::small_string::SmallString;
use crate::store::{CorruptionError, StorageError};
use oxsdatatypes::*;
use siphasher::sip128::{Hasher128, SipHasher24};
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::rc::Rc;
use std::str;

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
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

#[derive(Debug, Clone)]
pub enum EncodedTerm {
    DefaultGraph,
    NamedNode {
        iri_id: StrHash,
    },
    NumericalBlankNode {
        id: u128,
    },
    SmallBlankNode(SmallString),
    BigBlankNode {
        id_id: StrHash,
    },
    SmallStringLiteral(SmallString),
    BigStringLiteral {
        value_id: StrHash,
    },
    SmallSmallLangStringLiteral {
        value: SmallString,
        language: SmallString,
    },
    SmallBigLangStringLiteral {
        value: SmallString,
        language_id: StrHash,
    },
    BigSmallLangStringLiteral {
        value_id: StrHash,
        language: SmallString,
    },
    BigBigLangStringLiteral {
        value_id: StrHash,
        language_id: StrHash,
    },
    SmallTypedLiteral {
        value: SmallString,
        datatype_id: StrHash,
    },
    BigTypedLiteral {
        value_id: StrHash,
        datatype_id: StrHash,
    },
    BooleanLiteral(Boolean),
    FloatLiteral(Float),
    DoubleLiteral(Double),
    IntegerLiteral(Integer),
    DecimalLiteral(Decimal),
    DateTimeLiteral(DateTime),
    TimeLiteral(Time),
    DateLiteral(Date),
    GYearMonthLiteral(GYearMonth),
    GYearLiteral(GYear),
    GMonthDayLiteral(GMonthDay),
    GDayLiteral(GDay),
    GMonthLiteral(GMonth),
    DurationLiteral(Duration),
    YearMonthDurationLiteral(YearMonthDuration),
    DayTimeDurationLiteral(DayTimeDuration),
    Triple(Rc<EncodedTriple>),
}

impl PartialEq for EncodedTerm {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::DefaultGraph, Self::DefaultGraph) => true,
            (Self::NamedNode { iri_id: iri_id_a }, Self::NamedNode { iri_id: iri_id_b }) => {
                iri_id_a == iri_id_b
            }
            (Self::NumericalBlankNode { id: id_a }, Self::NumericalBlankNode { id: id_b }) => {
                id_a == id_b
            }
            (Self::SmallBlankNode(id_a), Self::SmallBlankNode(id_b)) => id_a == id_b,
            (Self::BigBlankNode { id_id: id_a }, Self::BigBlankNode { id_id: id_b }) => {
                id_a == id_b
            }
            (Self::SmallStringLiteral(a), Self::SmallStringLiteral(b)) => a == b,
            (
                Self::BigStringLiteral {
                    value_id: value_id_a,
                },
                Self::BigStringLiteral {
                    value_id: value_id_b,
                },
            ) => value_id_a == value_id_b,
            (
                Self::SmallSmallLangStringLiteral {
                    value: value_a,
                    language: language_a,
                },
                Self::SmallSmallLangStringLiteral {
                    value: value_b,
                    language: language_b,
                },
            ) => value_a == value_b && language_a == language_b,
            (
                Self::SmallBigLangStringLiteral {
                    value: value_a,
                    language_id: language_id_a,
                },
                Self::SmallBigLangStringLiteral {
                    value: value_b,
                    language_id: language_id_b,
                },
            ) => value_a == value_b && language_id_a == language_id_b,
            (
                Self::BigSmallLangStringLiteral {
                    value_id: value_id_a,
                    language: language_a,
                },
                Self::BigSmallLangStringLiteral {
                    value_id: value_id_b,
                    language: language_b,
                },
            ) => value_id_a == value_id_b && language_a == language_b,
            (
                Self::BigBigLangStringLiteral {
                    value_id: value_id_a,
                    language_id: language_id_a,
                },
                Self::BigBigLangStringLiteral {
                    value_id: value_id_b,
                    language_id: language_id_b,
                },
            ) => value_id_a == value_id_b && language_id_a == language_id_b,
            (
                Self::SmallTypedLiteral {
                    value: value_a,
                    datatype_id: datatype_id_a,
                },
                Self::SmallTypedLiteral {
                    value: value_b,
                    datatype_id: datatype_id_b,
                },
            ) => value_a == value_b && datatype_id_a == datatype_id_b,
            (
                Self::BigTypedLiteral {
                    value_id: value_id_a,
                    datatype_id: datatype_id_a,
                },
                Self::BigTypedLiteral {
                    value_id: value_id_b,
                    datatype_id: datatype_id_b,
                },
            ) => value_id_a == value_id_b && datatype_id_a == datatype_id_b,
            (Self::BooleanLiteral(a), Self::BooleanLiteral(b)) => a == b,
            (Self::FloatLiteral(a), Self::FloatLiteral(b)) => a.is_identical_with(b),
            (Self::DoubleLiteral(a), Self::DoubleLiteral(b)) => a.is_identical_with(b),
            (Self::IntegerLiteral(a), Self::IntegerLiteral(b)) => a.is_identical_with(b),
            (Self::DecimalLiteral(a), Self::DecimalLiteral(b)) => a.is_identical_with(b),
            (Self::DateTimeLiteral(a), Self::DateTimeLiteral(b)) => a.is_identical_with(b),
            (Self::TimeLiteral(a), Self::TimeLiteral(b)) => a.is_identical_with(b),
            (Self::DateLiteral(a), Self::DateLiteral(b)) => a.is_identical_with(b),
            (Self::GYearMonthLiteral(a), Self::GYearMonthLiteral(b)) => a.is_identical_with(b),
            (Self::GYearLiteral(a), Self::GYearLiteral(b)) => a.is_identical_with(b),
            (Self::GMonthDayLiteral(a), Self::GMonthDayLiteral(b)) => a.is_identical_with(b),
            (Self::GMonthLiteral(a), Self::GMonthLiteral(b)) => a.is_identical_with(b),
            (Self::GDayLiteral(a), Self::GDayLiteral(b)) => a.is_identical_with(b),
            (Self::DurationLiteral(a), Self::DurationLiteral(b)) => a.is_identical_with(b),
            (Self::YearMonthDurationLiteral(a), Self::YearMonthDurationLiteral(b)) => {
                a.is_identical_with(b)
            }
            (Self::DayTimeDurationLiteral(a), Self::DayTimeDurationLiteral(b)) => {
                a.is_identical_with(b)
            }
            (Self::Triple(a), Self::Triple(b)) => a == b,
            (_, _) => false,
        }
    }
}

impl Eq for EncodedTerm {}

impl Hash for EncodedTerm {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::NamedNode { iri_id } => iri_id.hash(state),
            Self::NumericalBlankNode { id } => id.hash(state),
            Self::SmallBlankNode(id) => id.hash(state),
            Self::BigBlankNode { id_id } => id_id.hash(state),
            Self::DefaultGraph => (),
            Self::SmallStringLiteral(value) => value.hash(state),
            Self::BigStringLiteral { value_id } => value_id.hash(state),
            Self::SmallSmallLangStringLiteral { value, language } => {
                value.hash(state);
                language.hash(state);
            }
            Self::SmallBigLangStringLiteral { value, language_id } => {
                value.hash(state);
                language_id.hash(state);
            }
            Self::BigSmallLangStringLiteral { value_id, language } => {
                value_id.hash(state);
                language.hash(state);
            }
            Self::BigBigLangStringLiteral {
                value_id,
                language_id,
            } => {
                value_id.hash(state);
                language_id.hash(state);
            }
            Self::SmallTypedLiteral { value, datatype_id } => {
                value.hash(state);
                datatype_id.hash(state);
            }
            Self::BigTypedLiteral {
                value_id,
                datatype_id,
            } => {
                value_id.hash(state);
                datatype_id.hash(state);
            }
            Self::BooleanLiteral(value) => value.hash(state),
            Self::FloatLiteral(value) => value.to_be_bytes().hash(state),
            Self::DoubleLiteral(value) => value.to_be_bytes().hash(state),
            Self::IntegerLiteral(value) => value.hash(state),
            Self::DecimalLiteral(value) => value.hash(state),
            Self::DateTimeLiteral(value) => value.hash(state),
            Self::TimeLiteral(value) => value.hash(state),
            Self::DateLiteral(value) => value.hash(state),
            Self::GYearMonthLiteral(value) => value.hash(state),
            Self::GYearLiteral(value) => value.hash(state),
            Self::GMonthDayLiteral(value) => value.hash(state),
            Self::GDayLiteral(value) => value.hash(state),
            Self::GMonthLiteral(value) => value.hash(state),
            Self::DurationLiteral(value) => value.hash(state),
            Self::YearMonthDurationLiteral(value) => value.hash(state),
            Self::DayTimeDurationLiteral(value) => value.hash(state),
            Self::Triple(value) => value.hash(state),
        }
    }
}

impl EncodedTerm {
    pub fn is_named_node(&self) -> bool {
        matches!(self, Self::NamedNode { .. })
    }

    pub fn is_blank_node(&self) -> bool {
        matches!(
            self,
            Self::NumericalBlankNode { .. }
                | Self::SmallBlankNode { .. }
                | Self::BigBlankNode { .. }
        )
    }

    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Self::SmallStringLiteral { .. }
                | Self::BigStringLiteral { .. }
                | Self::SmallSmallLangStringLiteral { .. }
                | Self::SmallBigLangStringLiteral { .. }
                | Self::BigSmallLangStringLiteral { .. }
                | Self::BigBigLangStringLiteral { .. }
                | Self::SmallTypedLiteral { .. }
                | Self::BigTypedLiteral { .. }
                | Self::BooleanLiteral(_)
                | Self::FloatLiteral(_)
                | Self::DoubleLiteral(_)
                | Self::IntegerLiteral(_)
                | Self::DecimalLiteral(_)
                | Self::DateTimeLiteral(_)
                | Self::TimeLiteral(_)
                | Self::DateLiteral(_)
                | Self::GYearMonthLiteral(_)
                | Self::GYearLiteral(_)
                | Self::GMonthDayLiteral(_)
                | Self::GDayLiteral(_)
                | Self::GMonthLiteral(_)
                | Self::DurationLiteral(_)
                | Self::YearMonthDurationLiteral(_)
                | Self::DayTimeDurationLiteral(_)
        )
    }

    pub fn is_unknown_typed_literal(&self) -> bool {
        matches!(
            self,
            Self::SmallTypedLiteral { .. } | Self::BigTypedLiteral { .. }
        )
    }

    pub fn is_default_graph(&self) -> bool {
        matches!(self, Self::DefaultGraph)
    }

    pub fn is_triple(&self) -> bool {
        matches!(self, Self::Triple { .. })
    }
}

impl From<bool> for EncodedTerm {
    fn from(value: bool) -> Self {
        Self::BooleanLiteral(value.into())
    }
}

impl From<i64> for EncodedTerm {
    fn from(value: i64) -> Self {
        Self::IntegerLiteral(value.into())
    }
}

impl From<i32> for EncodedTerm {
    fn from(value: i32) -> Self {
        Self::IntegerLiteral(value.into())
    }
}

impl From<u32> for EncodedTerm {
    fn from(value: u32) -> Self {
        Self::IntegerLiteral(value.into())
    }
}

impl From<u8> for EncodedTerm {
    fn from(value: u8) -> Self {
        Self::IntegerLiteral(value.into())
    }
}

impl From<f32> for EncodedTerm {
    fn from(value: f32) -> Self {
        Self::FloatLiteral(value.into())
    }
}

impl From<Float> for EncodedTerm {
    fn from(value: Float) -> Self {
        Self::FloatLiteral(value)
    }
}

impl From<f64> for EncodedTerm {
    fn from(value: f64) -> Self {
        Self::DoubleLiteral(value.into())
    }
}

impl From<Boolean> for EncodedTerm {
    fn from(value: Boolean) -> Self {
        Self::BooleanLiteral(value)
    }
}

impl From<Double> for EncodedTerm {
    fn from(value: Double) -> Self {
        Self::DoubleLiteral(value)
    }
}

impl From<Integer> for EncodedTerm {
    fn from(value: Integer) -> Self {
        Self::IntegerLiteral(value)
    }
}

impl From<Decimal> for EncodedTerm {
    fn from(value: Decimal) -> Self {
        Self::DecimalLiteral(value)
    }
}

impl From<DateTime> for EncodedTerm {
    fn from(value: DateTime) -> Self {
        Self::DateTimeLiteral(value)
    }
}

impl From<Time> for EncodedTerm {
    fn from(value: Time) -> Self {
        Self::TimeLiteral(value)
    }
}

impl From<Date> for EncodedTerm {
    fn from(value: Date) -> Self {
        Self::DateLiteral(value)
    }
}

impl From<GMonthDay> for EncodedTerm {
    fn from(value: GMonthDay) -> Self {
        Self::GMonthDayLiteral(value)
    }
}

impl From<GDay> for EncodedTerm {
    fn from(value: GDay) -> Self {
        Self::GDayLiteral(value)
    }
}

impl From<GMonth> for EncodedTerm {
    fn from(value: GMonth) -> Self {
        Self::GMonthLiteral(value)
    }
}

impl From<GYearMonth> for EncodedTerm {
    fn from(value: GYearMonth) -> Self {
        Self::GYearMonthLiteral(value)
    }
}

impl From<GYear> for EncodedTerm {
    fn from(value: GYear) -> Self {
        Self::GYearLiteral(value)
    }
}

impl From<Duration> for EncodedTerm {
    fn from(value: Duration) -> Self {
        Self::DurationLiteral(value)
    }
}

impl From<YearMonthDuration> for EncodedTerm {
    fn from(value: YearMonthDuration) -> Self {
        Self::YearMonthDurationLiteral(value)
    }
}

impl From<DayTimeDuration> for EncodedTerm {
    fn from(value: DayTimeDuration) -> Self {
        Self::DayTimeDurationLiteral(value)
    }
}

impl From<EncodedTriple> for EncodedTerm {
    fn from(value: EncodedTriple) -> Self {
        Self::Triple(Rc::new(value))
    }
}

impl From<NamedNodeRef<'_>> for EncodedTerm {
    fn from(named_node: NamedNodeRef<'_>) -> Self {
        Self::NamedNode {
            iri_id: StrHash::new(named_node.as_str()),
        }
    }
}

impl From<BlankNodeRef<'_>> for EncodedTerm {
    fn from(blank_node: BlankNodeRef<'_>) -> Self {
        if let Some(id) = blank_node.unique_id() {
            Self::NumericalBlankNode { id }
        } else {
            let id = blank_node.as_str();
            if let Ok(id) = id.try_into() {
                Self::SmallBlankNode(id)
            } else {
                Self::BigBlankNode {
                    id_id: StrHash::new(id),
                }
            }
        }
    }
}

impl From<LiteralRef<'_>> for EncodedTerm {
    fn from(literal: LiteralRef<'_>) -> Self {
        let value = literal.value();
        let datatype = literal.datatype().as_str();
        let native_encoding = match datatype {
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString" => {
                literal.language().map(|language| {
                    if let Ok(value) = SmallString::try_from(value) {
                        if let Ok(language) = SmallString::try_from(language) {
                            Self::SmallSmallLangStringLiteral { value, language }
                        } else {
                            Self::SmallBigLangStringLiteral {
                                value,
                                language_id: StrHash::new(language),
                            }
                        }
                    } else if let Ok(language) = SmallString::try_from(language) {
                        Self::BigSmallLangStringLiteral {
                            value_id: StrHash::new(value),
                            language,
                        }
                    } else {
                        Self::BigBigLangStringLiteral {
                            value_id: StrHash::new(value),
                            language_id: StrHash::new(language),
                        }
                    }
                })
            }
            "http://www.w3.org/2001/XMLSchema#boolean" => parse_boolean_str(value),
            "http://www.w3.org/2001/XMLSchema#string" => {
                let value = value;
                Some(if let Ok(value) = SmallString::try_from(value) {
                    Self::SmallStringLiteral(value)
                } else {
                    Self::BigStringLiteral {
                        value_id: StrHash::new(value),
                    }
                })
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
            | "http://www.w3.org/2001/XMLSchema#nonNegativeInteger" => parse_integer_str(value),
            "http://www.w3.org/2001/XMLSchema#decimal" => parse_decimal_str(value),
            "http://www.w3.org/2001/XMLSchema#dateTime"
            | "http://www.w3.org/2001/XMLSchema#dateTimeStamp" => parse_date_time_str(value),
            "http://www.w3.org/2001/XMLSchema#time" => parse_time_str(value),
            "http://www.w3.org/2001/XMLSchema#date" => parse_date_str(value),
            "http://www.w3.org/2001/XMLSchema#gYearMonth" => parse_g_year_month_str(value),
            "http://www.w3.org/2001/XMLSchema#gYear" => parse_g_year_str(value),
            "http://www.w3.org/2001/XMLSchema#gMonthDay" => parse_g_month_day_str(value),
            "http://www.w3.org/2001/XMLSchema#gDay" => parse_g_day_str(value),
            "http://www.w3.org/2001/XMLSchema#gMonth" => parse_g_month_str(value),
            "http://www.w3.org/2001/XMLSchema#duration" => parse_duration_str(value),
            "http://www.w3.org/2001/XMLSchema#yearMonthDuration" => {
                parse_year_month_duration_str(value)
            }
            "http://www.w3.org/2001/XMLSchema#dayTimeDuration" => {
                parse_day_time_duration_str(value)
            }
            _ => None,
        };
        match native_encoding {
            Some(term) => term,
            None => {
                if let Ok(value) = SmallString::try_from(value) {
                    Self::SmallTypedLiteral {
                        value,
                        datatype_id: StrHash::new(datatype),
                    }
                } else {
                    Self::BigTypedLiteral {
                        value_id: StrHash::new(value),
                        datatype_id: StrHash::new(datatype),
                    }
                }
            }
        }
    }
}

impl From<NamedOrBlankNodeRef<'_>> for EncodedTerm {
    fn from(term: NamedOrBlankNodeRef<'_>) -> Self {
        match term {
            NamedOrBlankNodeRef::NamedNode(named_node) => named_node.into(),
            NamedOrBlankNodeRef::BlankNode(blank_node) => blank_node.into(),
        }
    }
}

impl From<SubjectRef<'_>> for EncodedTerm {
    fn from(term: SubjectRef<'_>) -> Self {
        match term {
            SubjectRef::NamedNode(named_node) => named_node.into(),
            SubjectRef::BlankNode(blank_node) => blank_node.into(),
            SubjectRef::Triple(triple) => triple.as_ref().into(),
        }
    }
}

impl From<TermRef<'_>> for EncodedTerm {
    fn from(term: TermRef<'_>) -> Self {
        match term {
            TermRef::NamedNode(named_node) => named_node.into(),
            TermRef::BlankNode(blank_node) => blank_node.into(),
            TermRef::Literal(literal) => literal.into(),
            TermRef::Triple(triple) => triple.as_ref().into(),
        }
    }
}

impl From<GraphNameRef<'_>> for EncodedTerm {
    fn from(name: GraphNameRef<'_>) -> Self {
        match name {
            GraphNameRef::NamedNode(named_node) => named_node.into(),
            GraphNameRef::BlankNode(blank_node) => blank_node.into(),
            GraphNameRef::DefaultGraph => Self::DefaultGraph,
        }
    }
}

impl From<TripleRef<'_>> for EncodedTerm {
    fn from(triple: TripleRef<'_>) -> Self {
        Self::Triple(Rc::new(triple.into()))
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct EncodedTriple {
    pub subject: EncodedTerm,
    pub predicate: EncodedTerm,
    pub object: EncodedTerm,
}

impl EncodedTriple {
    pub fn new(subject: EncodedTerm, predicate: EncodedTerm, object: EncodedTerm) -> Self {
        Self {
            subject,
            predicate,
            object,
        }
    }
}

impl From<TripleRef<'_>> for EncodedTriple {
    fn from(triple: TripleRef<'_>) -> Self {
        Self {
            subject: triple.subject.into(),
            predicate: triple.predicate.into(),
            object: triple.object.into(),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
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

impl From<QuadRef<'_>> for EncodedQuad {
    fn from(quad: QuadRef<'_>) -> Self {
        Self {
            subject: quad.subject.into(),
            predicate: quad.predicate.into(),
            object: quad.object.into(),
            graph_name: quad.graph_name.into(),
        }
    }
}

pub trait StrLookup {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError>;

    fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError>;
}

pub fn insert_term<F: FnMut(&StrHash, &str) -> Result<(), StorageError>>(
    term: TermRef<'_>,
    encoded: &EncodedTerm,
    insert_str: &mut F,
) -> Result<(), StorageError> {
    match term {
        TermRef::NamedNode(node) => {
            if let EncodedTerm::NamedNode { iri_id } = encoded {
                insert_str(iri_id, node.as_str())
            } else {
                unreachable!("Invalid term encoding {:?} for {}", encoded, term)
            }
        }
        TermRef::BlankNode(node) => match encoded {
            EncodedTerm::BigBlankNode { id_id } => insert_str(id_id, node.as_str()),
            EncodedTerm::SmallBlankNode(..) | EncodedTerm::NumericalBlankNode { .. } => Ok(()),
            _ => unreachable!("Invalid term encoding {:?} for {}", encoded, term),
        },
        TermRef::Literal(literal) => match encoded {
            EncodedTerm::BigStringLiteral { value_id }
            | EncodedTerm::BigSmallLangStringLiteral { value_id, .. } => {
                insert_str(value_id, literal.value())
            }
            EncodedTerm::SmallBigLangStringLiteral { language_id, .. } => {
                if let Some(language) = literal.language() {
                    insert_str(language_id, language)
                } else {
                    unreachable!("Invalid term encoding {:?} for {}", encoded, term)
                }
            }
            EncodedTerm::BigBigLangStringLiteral {
                value_id,
                language_id,
            } => {
                insert_str(value_id, literal.value())?;
                if let Some(language) = literal.language() {
                    insert_str(language_id, language)
                } else {
                    unreachable!("Invalid term encoding {:?} for {}", encoded, term)
                }
            }
            EncodedTerm::SmallTypedLiteral { datatype_id, .. } => {
                insert_str(datatype_id, literal.datatype().as_str())
            }
            EncodedTerm::BigTypedLiteral {
                value_id,
                datatype_id,
            } => {
                insert_str(value_id, literal.value())?;
                insert_str(datatype_id, literal.datatype().as_str())
            }
            EncodedTerm::SmallStringLiteral(..)
            | EncodedTerm::SmallSmallLangStringLiteral { .. }
            | EncodedTerm::BooleanLiteral(..)
            | EncodedTerm::FloatLiteral(..)
            | EncodedTerm::DoubleLiteral(..)
            | EncodedTerm::IntegerLiteral(..)
            | EncodedTerm::DecimalLiteral(..)
            | EncodedTerm::DateTimeLiteral(..)
            | EncodedTerm::TimeLiteral(..)
            | EncodedTerm::DateLiteral(..)
            | EncodedTerm::GYearMonthLiteral(..)
            | EncodedTerm::GYearLiteral(..)
            | EncodedTerm::GMonthDayLiteral(..)
            | EncodedTerm::GDayLiteral(..)
            | EncodedTerm::GMonthLiteral(..)
            | EncodedTerm::DurationLiteral(..)
            | EncodedTerm::YearMonthDurationLiteral(..)
            | EncodedTerm::DayTimeDurationLiteral(..) => Ok(()),
            _ => unreachable!("Invalid term encoding {:?} for {}", encoded, term),
        },
        TermRef::Triple(triple) => {
            if let EncodedTerm::Triple(encoded) = encoded {
                insert_term(triple.subject.as_ref().into(), &encoded.subject, insert_str)?;
                insert_term(
                    triple.predicate.as_ref().into(),
                    &encoded.predicate,
                    insert_str,
                )?;
                insert_term(triple.object.as_ref(), &encoded.object, insert_str)
            } else {
                unreachable!("Invalid term encoding {:?} for {}", encoded, term)
            }
        }
    }
}

pub fn parse_boolean_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::BooleanLiteral).ok()
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

pub fn parse_date_time_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::DateTimeLiteral).ok()
}

pub fn parse_time_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::TimeLiteral).ok()
}

pub fn parse_date_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::DateLiteral).ok()
}

pub fn parse_g_year_month_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::GYearMonthLiteral).ok()
}

pub fn parse_g_year_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::GYearLiteral).ok()
}

pub fn parse_g_month_day_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::GMonthDayLiteral).ok()
}

pub fn parse_g_day_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::GDayLiteral).ok()
}

pub fn parse_g_month_str(value: &str) -> Option<EncodedTerm> {
    value.parse().map(EncodedTerm::GMonthLiteral).ok()
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

pub trait Decoder: StrLookup {
    fn decode_term(&self, encoded: &EncodedTerm) -> Result<Term, StorageError>;

    fn decode_subject(&self, encoded: &EncodedTerm) -> Result<Subject, StorageError> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node.into()),
            Term::BlankNode(blank_node) => Ok(blank_node.into()),
            Term::Literal(_) => Err(CorruptionError::msg(
                "A literal has been found instead of a subject node",
            )
            .into()),
            Term::Triple(triple) => Ok(Subject::Triple(triple)),
        }
    }

    fn decode_named_or_blank_node(
        &self,
        encoded: &EncodedTerm,
    ) -> Result<NamedOrBlankNode, StorageError> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node.into()),
            Term::BlankNode(blank_node) => Ok(blank_node.into()),
            Term::Literal(_) => Err(CorruptionError::msg(
                "A literal has been found instead of a named or blank node",
            )
            .into()),
            Term::Triple(_) => Err(CorruptionError::msg(
                "A triple has been found instead of a named or blank node",
            )
            .into()),
        }
    }

    fn decode_named_node(&self, encoded: &EncodedTerm) -> Result<NamedNode, StorageError> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node),
            Term::BlankNode(_) => Err(CorruptionError::msg(
                "A blank node has been found instead of a named node",
            )
            .into()),
            Term::Literal(_) => {
                Err(CorruptionError::msg("A literal has been found instead of a named node").into())
            }
            Term::Triple(_) => {
                Err(CorruptionError::msg("A triple has been found instead of a named node").into())
            }
        }
    }

    fn decode_triple(&self, encoded: &EncodedTriple) -> Result<Triple, StorageError> {
        Ok(Triple::new(
            self.decode_subject(&encoded.subject)?,
            self.decode_named_node(&encoded.predicate)?,
            self.decode_term(&encoded.object)?,
        ))
    }

    fn decode_quad(&self, encoded: &EncodedQuad) -> Result<Quad, StorageError> {
        Ok(Quad::new(
            self.decode_subject(&encoded.subject)?,
            self.decode_named_node(&encoded.predicate)?,
            self.decode_term(&encoded.object)?,
            if encoded.graph_name == EncodedTerm::DefaultGraph {
                GraphName::DefaultGraph
            } else {
                match self.decode_term(&encoded.graph_name)? {
                    Term::NamedNode(named_node) => named_node.into(),
                    Term::BlankNode(blank_node) => blank_node.into(),
                    Term::Literal(_) => {
                        return Err(
                            CorruptionError::msg("A literal is not a valid graph name").into()
                        )
                    }
                    Term::Triple(_) => {
                        return Err(
                            CorruptionError::msg("A triple is not a valid graph name").into()
                        )
                    }
                }
            },
        ))
    }
}

impl<S: StrLookup> Decoder for S {
    fn decode_term(&self, encoded: &EncodedTerm) -> Result<Term, StorageError> {
        match encoded {
            EncodedTerm::DefaultGraph => {
                Err(CorruptionError::msg("The default graph tag is not a valid term").into())
            }
            EncodedTerm::NamedNode { iri_id } => {
                Ok(NamedNode::new_unchecked(get_required_str(self, iri_id)?).into())
            }
            EncodedTerm::NumericalBlankNode { id } => Ok(BlankNode::new_from_unique_id(*id).into()),
            EncodedTerm::SmallBlankNode(id) => Ok(BlankNode::new_unchecked(id.as_str()).into()),
            EncodedTerm::BigBlankNode { id_id } => {
                Ok(BlankNode::new_unchecked(get_required_str(self, id_id)?).into())
            }
            EncodedTerm::SmallStringLiteral(value) => {
                Ok(Literal::new_simple_literal(*value).into())
            }
            EncodedTerm::BigStringLiteral { value_id } => {
                Ok(Literal::new_simple_literal(get_required_str(self, value_id)?).into())
            }
            EncodedTerm::SmallSmallLangStringLiteral { value, language } => {
                Ok(Literal::new_language_tagged_literal_unchecked(*value, *language).into())
            }
            EncodedTerm::SmallBigLangStringLiteral { value, language_id } => {
                Ok(Literal::new_language_tagged_literal_unchecked(
                    *value,
                    get_required_str(self, language_id)?,
                )
                .into())
            }
            EncodedTerm::BigSmallLangStringLiteral { value_id, language } => {
                Ok(Literal::new_language_tagged_literal_unchecked(
                    get_required_str(self, value_id)?,
                    *language,
                )
                .into())
            }
            EncodedTerm::BigBigLangStringLiteral {
                value_id,
                language_id,
            } => Ok(Literal::new_language_tagged_literal_unchecked(
                get_required_str(self, value_id)?,
                get_required_str(self, language_id)?,
            )
            .into()),
            EncodedTerm::SmallTypedLiteral { value, datatype_id } => {
                Ok(Literal::new_typed_literal(
                    *value,
                    NamedNode::new_unchecked(get_required_str(self, datatype_id)?),
                )
                .into())
            }
            EncodedTerm::BigTypedLiteral {
                value_id,
                datatype_id,
            } => Ok(Literal::new_typed_literal(
                get_required_str(self, value_id)?,
                NamedNode::new_unchecked(get_required_str(self, datatype_id)?),
            )
            .into()),
            EncodedTerm::BooleanLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::FloatLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::DoubleLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::IntegerLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::DecimalLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::DateTimeLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::DateLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::TimeLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::GYearMonthLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::GYearLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::GMonthDayLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::GDayLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::GMonthLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::DurationLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::YearMonthDurationLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::DayTimeDurationLiteral(value) => Ok(Literal::from(*value).into()),
            EncodedTerm::Triple(triple) => Ok(self.decode_triple(triple)?.into()),
        }
    }
}

fn get_required_str<L: StrLookup>(lookup: &L, id: &StrHash) -> Result<String, StorageError> {
    Ok(lookup.get_str(id)?.ok_or_else(|| {
        CorruptionError::new(format!(
            "Not able to find the string with id {id:?} in the string store"
        ))
    })?)
}
