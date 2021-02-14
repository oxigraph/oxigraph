#![allow(clippy::unreadable_literal)]

use crate::error::invalid_data_error;
use crate::model::xsd::*;
use crate::model::*;
use crate::sparql::EvaluationError;
use crate::store::small_string::SmallString;
use rand::random;
use rio_api::model as rio;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::error::Error;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::{fmt, io, str};

pub trait StrId: Eq + Debug + Copy + Hash {}

#[derive(Debug, Clone, Copy)]
pub enum EncodedTerm<I: StrId> {
    DefaultGraph,
    NamedNode {
        iri_id: I,
    },
    NumericalBlankNode {
        id: u128,
    },
    SmallBlankNode(SmallString),
    BigBlankNode {
        id_id: I,
    },
    SmallStringLiteral(SmallString),
    BigStringLiteral {
        value_id: I,
    },
    SmallSmallLangStringLiteral {
        value: SmallString,
        language: SmallString,
    },
    SmallBigLangStringLiteral {
        value: SmallString,
        language_id: I,
    },
    BigSmallLangStringLiteral {
        value_id: I,
        language: SmallString,
    },
    BigBigLangStringLiteral {
        value_id: I,
        language_id: I,
    },
    SmallTypedLiteral {
        value: SmallString,
        datatype_id: I,
    },
    BigTypedLiteral {
        value_id: I,
        datatype_id: I,
    },
    BooleanLiteral(bool),
    FloatLiteral(f32),
    DoubleLiteral(f64),
    IntegerLiteral(i64),
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
}

impl<I: StrId> PartialEq for EncodedTerm<I> {
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
            (Self::DateTimeLiteral(a), Self::DateTimeLiteral(b)) => a.is_identical_with(b),
            (Self::TimeLiteral(a), Self::TimeLiteral(b)) => a.is_identical_with(b),
            (Self::DateLiteral(a), Self::DateLiteral(b)) => a.is_identical_with(b),
            (Self::GYearMonthLiteral(a), Self::GYearMonthLiteral(b)) => a.is_identical_with(b),
            (Self::GYearLiteral(a), Self::GYearLiteral(b)) => a.is_identical_with(b),
            (Self::GMonthDayLiteral(a), Self::GMonthDayLiteral(b)) => a.is_identical_with(b),
            (Self::GMonthLiteral(a), Self::GMonthLiteral(b)) => a.is_identical_with(b),
            (Self::GDayLiteral(a), Self::GDayLiteral(b)) => a.is_identical_with(b),
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
            Self::FloatLiteral(value) => state.write(&value.to_ne_bytes()),
            Self::DoubleLiteral(value) => state.write(&value.to_ne_bytes()),
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
        }
    }
}

impl<I: StrId> EncodedTerm<I> {
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

    pub fn map_id<J: StrId>(self, mapping: impl Fn(I) -> J) -> EncodedTerm<J> {
        match self {
            Self::DefaultGraph { .. } => EncodedTerm::DefaultGraph,
            Self::NamedNode { iri_id } => EncodedTerm::NamedNode {
                iri_id: mapping(iri_id),
            },
            Self::NumericalBlankNode { id } => EncodedTerm::NumericalBlankNode { id },
            Self::SmallBlankNode(id) => EncodedTerm::SmallBlankNode(id),
            Self::BigBlankNode { id_id } => EncodedTerm::BigBlankNode {
                id_id: mapping(id_id),
            },
            Self::SmallStringLiteral(value) => EncodedTerm::SmallStringLiteral(value),
            Self::BigStringLiteral { value_id } => EncodedTerm::BigStringLiteral {
                value_id: mapping(value_id),
            },
            Self::SmallSmallLangStringLiteral { value, language } => {
                EncodedTerm::SmallSmallLangStringLiteral { value, language }
            }
            Self::SmallBigLangStringLiteral { value, language_id } => {
                EncodedTerm::SmallBigLangStringLiteral {
                    value,
                    language_id: mapping(language_id),
                }
            }
            Self::BigSmallLangStringLiteral { value_id, language } => {
                EncodedTerm::BigSmallLangStringLiteral {
                    value_id: mapping(value_id),
                    language,
                }
            }
            Self::BigBigLangStringLiteral {
                value_id,
                language_id,
            } => EncodedTerm::BigBigLangStringLiteral {
                value_id: mapping(value_id),
                language_id: mapping(language_id),
            },
            Self::SmallTypedLiteral { value, datatype_id } => EncodedTerm::SmallTypedLiteral {
                value,
                datatype_id: mapping(datatype_id),
            },
            Self::BigTypedLiteral {
                value_id,
                datatype_id,
            } => EncodedTerm::BigTypedLiteral {
                value_id: mapping(value_id),
                datatype_id: mapping(datatype_id),
            },
            Self::BooleanLiteral(value) => EncodedTerm::BooleanLiteral(value),
            Self::FloatLiteral(value) => EncodedTerm::FloatLiteral(value),
            Self::DoubleLiteral(value) => EncodedTerm::DoubleLiteral(value),
            Self::IntegerLiteral(value) => EncodedTerm::IntegerLiteral(value),
            Self::DecimalLiteral(value) => EncodedTerm::DecimalLiteral(value),
            Self::DateTimeLiteral(value) => EncodedTerm::DateTimeLiteral(value),
            Self::DateLiteral(value) => EncodedTerm::DateLiteral(value),
            Self::TimeLiteral(value) => EncodedTerm::TimeLiteral(value),
            Self::GYearMonthLiteral(value) => EncodedTerm::GYearMonthLiteral(value),
            Self::GYearLiteral(value) => EncodedTerm::GYearLiteral(value),
            Self::GMonthDayLiteral(value) => EncodedTerm::GMonthDayLiteral(value),
            Self::GDayLiteral(value) => EncodedTerm::GDayLiteral(value),
            Self::GMonthLiteral(value) => EncodedTerm::GMonthLiteral(value),
            Self::DurationLiteral(value) => EncodedTerm::DurationLiteral(value),
            Self::YearMonthDurationLiteral(value) => EncodedTerm::YearMonthDurationLiteral(value),
            Self::DayTimeDurationLiteral(value) => EncodedTerm::DayTimeDurationLiteral(value),
        }
    }

    pub fn try_map_id<J: StrId, E>(
        self,
        mut mapping: impl FnMut(I) -> Result<J, E>,
    ) -> Result<EncodedTerm<J>, E> {
        Ok(match self {
            Self::DefaultGraph { .. } => EncodedTerm::DefaultGraph,
            Self::NamedNode { iri_id } => EncodedTerm::NamedNode {
                iri_id: mapping(iri_id)?,
            },
            Self::NumericalBlankNode { id } => EncodedTerm::NumericalBlankNode { id },
            Self::SmallBlankNode(id) => EncodedTerm::SmallBlankNode(id),
            Self::BigBlankNode { id_id } => EncodedTerm::BigBlankNode {
                id_id: mapping(id_id)?,
            },
            Self::SmallStringLiteral(value) => EncodedTerm::SmallStringLiteral(value),
            Self::BigStringLiteral { value_id } => EncodedTerm::BigStringLiteral {
                value_id: mapping(value_id)?,
            },
            Self::SmallSmallLangStringLiteral { value, language } => {
                EncodedTerm::SmallSmallLangStringLiteral { value, language }
            }
            Self::SmallBigLangStringLiteral { value, language_id } => {
                EncodedTerm::SmallBigLangStringLiteral {
                    value,
                    language_id: mapping(language_id)?,
                }
            }
            Self::BigSmallLangStringLiteral { value_id, language } => {
                EncodedTerm::BigSmallLangStringLiteral {
                    value_id: mapping(value_id)?,
                    language,
                }
            }
            Self::BigBigLangStringLiteral {
                value_id,
                language_id,
            } => EncodedTerm::BigBigLangStringLiteral {
                value_id: mapping(value_id)?,
                language_id: mapping(language_id)?,
            },
            Self::SmallTypedLiteral { value, datatype_id } => EncodedTerm::SmallTypedLiteral {
                value,
                datatype_id: mapping(datatype_id)?,
            },
            Self::BigTypedLiteral {
                value_id,
                datatype_id,
            } => EncodedTerm::BigTypedLiteral {
                value_id: mapping(value_id)?,
                datatype_id: mapping(datatype_id)?,
            },
            Self::BooleanLiteral(value) => EncodedTerm::BooleanLiteral(value),
            Self::FloatLiteral(value) => EncodedTerm::FloatLiteral(value),
            Self::DoubleLiteral(value) => EncodedTerm::DoubleLiteral(value),
            Self::IntegerLiteral(value) => EncodedTerm::IntegerLiteral(value),
            Self::DecimalLiteral(value) => EncodedTerm::DecimalLiteral(value),
            Self::DateTimeLiteral(value) => EncodedTerm::DateTimeLiteral(value),
            Self::DateLiteral(value) => EncodedTerm::DateLiteral(value),
            Self::TimeLiteral(value) => EncodedTerm::TimeLiteral(value),
            Self::GYearMonthLiteral(value) => EncodedTerm::GYearMonthLiteral(value),
            Self::GYearLiteral(value) => EncodedTerm::GYearLiteral(value),
            Self::GMonthDayLiteral(value) => EncodedTerm::GMonthDayLiteral(value),
            Self::GDayLiteral(value) => EncodedTerm::GDayLiteral(value),
            Self::GMonthLiteral(value) => EncodedTerm::GMonthLiteral(value),
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

impl<I: StrId> From<DateTime> for EncodedTerm<I> {
    fn from(value: DateTime) -> Self {
        Self::DateTimeLiteral(value)
    }
}

impl<I: StrId> From<Time> for EncodedTerm<I> {
    fn from(value: Time) -> Self {
        Self::TimeLiteral(value)
    }
}

impl<I: StrId> From<Date> for EncodedTerm<I> {
    fn from(value: Date) -> Self {
        Self::DateLiteral(value)
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

pub(crate) trait StrEncodingAware {
    //TODO: rename
    type Error: Error + Into<EvaluationError> + 'static;
    type StrId: StrId + 'static;
}

impl<'a, T: StrEncodingAware> StrEncodingAware for &'a T {
    type Error = T::Error;
    type StrId = T::StrId;
}

pub(crate) trait StrLookup: StrEncodingAware {
    fn get_str(&self, id: Self::StrId) -> Result<Option<String>, Self::Error>;

    fn get_str_id(&self, value: &str) -> Result<Option<Self::StrId>, Self::Error>;
}

pub(crate) trait StrContainer: StrEncodingAware {
    fn insert_str(&mut self, value: &str) -> Result<Self::StrId, Self::Error>;
}

/// Tries to encode a term based on the existing strings (does not insert anything)
pub(crate) trait ReadEncoder: StrEncodingAware {
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
            EncodedTerm::NumericalBlankNode { id }
        } else {
            let id = blank_node.as_str();
            if let Ok(id) = id.try_into() {
                EncodedTerm::SmallBlankNode(id)
            } else {
                EncodedTerm::BigBlankNode {
                    id_id: if let Some(id_id) = self.get_encoded_str(id)? {
                        id_id
                    } else {
                        return Ok(None);
                    },
                }
            }
        }))
    }

    fn get_encoded_literal(
        &self,
        literal: LiteralRef<'_>,
    ) -> Result<Option<EncodedTerm<Self::StrId>>, Self::Error> {
        let value = literal.value();
        let datatype = literal.datatype().as_str();
        Ok(Some(
            match match datatype {
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString" => {
                    if let Some(language) = literal.language() {
                        if let Ok(value) = SmallString::try_from(value) {
                            if let Ok(language) = SmallString::try_from(language) {
                                Some(EncodedTerm::SmallSmallLangStringLiteral { value, language })
                            } else {
                                Some(EncodedTerm::SmallBigLangStringLiteral {
                                    value,
                                    language_id: if let Some(language_id) =
                                        self.get_encoded_str(language)?
                                    {
                                        language_id
                                    } else {
                                        return Ok(None);
                                    },
                                })
                            }
                        } else if let Ok(language) = SmallString::try_from(language) {
                            Some(EncodedTerm::BigSmallLangStringLiteral {
                                value_id: if let Some(value_id) = self.get_encoded_str(value)? {
                                    value_id
                                } else {
                                    return Ok(None);
                                },
                                language,
                            })
                        } else {
                            Some(EncodedTerm::BigBigLangStringLiteral {
                                value_id: if let Some(value_id) = self.get_encoded_str(value)? {
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
                        }
                    } else {
                        None
                    }
                }
                "http://www.w3.org/2001/XMLSchema#boolean" => parse_boolean_str(value),
                "http://www.w3.org/2001/XMLSchema#string" => {
                    let value = value;
                    Some(if let Ok(value) = SmallString::try_from(value) {
                        EncodedTerm::SmallStringLiteral(value)
                    } else {
                        EncodedTerm::BigStringLiteral {
                            value_id: if let Some(value_id) = self.get_encoded_str(value)? {
                                value_id
                            } else {
                                return Ok(None);
                            },
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
            } {
                Some(term) => term,
                None => {
                    if let Ok(value) = SmallString::try_from(value) {
                        EncodedTerm::SmallTypedLiteral {
                            value,
                            datatype_id: if let Some(datatype_id) =
                                self.get_encoded_str(datatype)?
                            {
                                datatype_id
                            } else {
                                return Ok(None);
                            },
                        }
                    } else {
                        EncodedTerm::BigTypedLiteral {
                            value_id: if let Some(value_id) = self.get_encoded_str(value)? {
                                value_id
                            } else {
                                return Ok(None);
                            },
                            datatype_id: if let Some(datatype_id) =
                                self.get_encoded_str(datatype)?
                            {
                                datatype_id
                            } else {
                                return Ok(None);
                            },
                        }
                    }
                }
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
pub(crate) trait WriteEncoder: StrEncodingAware {
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
        Ok(if let Some(id) = blank_node.id() {
            EncodedTerm::NumericalBlankNode { id }
        } else {
            let id = blank_node.as_str();
            if let Ok(id) = id.try_into() {
                EncodedTerm::SmallBlankNode(id)
            } else {
                EncodedTerm::BigBlankNode {
                    id_id: self.encode_str(id)?,
                }
            }
        })
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
            EncodedTerm::NumericalBlankNode { id: *id }
        } else {
            let id = random::<u128>();
            bnodes_map.insert(blank_node.id.to_owned(), id);
            EncodedTerm::NumericalBlankNode { id }
        })
    }
    fn encode_rio_literal(
        &mut self,
        literal: rio::Literal<'_>,
    ) -> Result<EncodedTerm<Self::StrId>, Self::Error> {
        Ok(match literal {
            rio::Literal::Simple { value } => {
                if let Ok(value) = SmallString::try_from(value) {
                    EncodedTerm::SmallStringLiteral(value)
                } else {
                    EncodedTerm::BigStringLiteral {
                        value_id: self.encode_str(value)?,
                    }
                }
            }
            rio::Literal::LanguageTaggedString { value, language } => {
                if let Ok(value) = SmallString::try_from(value) {
                    if let Ok(language) = SmallString::try_from(language) {
                        EncodedTerm::SmallSmallLangStringLiteral { value, language }
                    } else {
                        EncodedTerm::SmallBigLangStringLiteral {
                            value,
                            language_id: self.encode_str(language)?,
                        }
                    }
                } else if let Ok(language) = SmallString::try_from(language) {
                    EncodedTerm::BigSmallLangStringLiteral {
                        value_id: self.encode_str(value)?,
                        language,
                    }
                } else {
                    EncodedTerm::BigBigLangStringLiteral {
                        value_id: self.encode_str(value)?,
                        language_id: self.encode_str(language)?,
                    }
                }
            }
            rio::Literal::Typed { value, datatype } => {
                match match datatype.iri {
                    "http://www.w3.org/2001/XMLSchema#boolean" => parse_boolean_str(value),
                    "http://www.w3.org/2001/XMLSchema#string" => {
                        Some(if let Ok(value) = SmallString::try_from(value) {
                            EncodedTerm::SmallStringLiteral(value)
                        } else {
                            EncodedTerm::BigStringLiteral {
                                value_id: self.encode_str(value)?,
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
                    | "http://www.w3.org/2001/XMLSchema#nonNegativeInteger" => {
                        parse_integer_str(value)
                    }
                    "http://www.w3.org/2001/XMLSchema#decimal" => parse_decimal_str(value),
                    "http://www.w3.org/2001/XMLSchema#dateTime"
                    | "http://www.w3.org/2001/XMLSchema#dateTimeStamp" => {
                        parse_date_time_str(value)
                    }
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
                } {
                    Some(v) => v,
                    None => {
                        if let Ok(value) = SmallString::try_from(value) {
                            EncodedTerm::SmallTypedLiteral {
                                value,
                                datatype_id: self.encode_str(datatype.iri)?,
                            }
                        } else {
                            EncodedTerm::BigTypedLiteral {
                                value_id: self.encode_str(value)?,
                                datatype_id: self.encode_str(datatype.iri)?,
                            }
                        }
                    }
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

pub fn parse_date_time_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::DateTimeLiteral).ok()
}

pub fn parse_time_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::TimeLiteral).ok()
}

pub fn parse_date_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::DateLiteral).ok()
}

pub fn parse_g_year_month_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::GYearMonthLiteral).ok()
}

pub fn parse_g_year_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::GYearLiteral).ok()
}

pub fn parse_g_month_day_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::GMonthDayLiteral).ok()
}

pub fn parse_g_day_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::GDayLiteral).ok()
}

pub fn parse_g_month_str<I: StrId>(value: &str) -> Option<EncodedTerm<I>> {
    value.parse().map(EncodedTerm::GMonthLiteral).ok()
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
            EncodedTerm::NumericalBlankNode { id } => Ok(BlankNode::new_from_unique_id(id).into()),
            EncodedTerm::SmallBlankNode(id) => Ok(BlankNode::new_unchecked(id.as_str()).into()),
            EncodedTerm::BigBlankNode { id_id } => {
                Ok(BlankNode::new_unchecked(get_required_str(self, id_id)?).into())
            }
            EncodedTerm::SmallStringLiteral(value) => Ok(Literal::new_simple_literal(value).into()),
            EncodedTerm::BigStringLiteral { value_id } => {
                Ok(Literal::new_simple_literal(get_required_str(self, value_id)?).into())
            }
            EncodedTerm::SmallSmallLangStringLiteral { value, language } => {
                Ok(Literal::new_language_tagged_literal_unchecked(value, language).into())
            }
            EncodedTerm::SmallBigLangStringLiteral { value, language_id } => {
                Ok(Literal::new_language_tagged_literal_unchecked(
                    value,
                    get_required_str(self, language_id)?,
                )
                .into())
            }
            EncodedTerm::BigSmallLangStringLiteral { value_id, language } => {
                Ok(Literal::new_language_tagged_literal_unchecked(
                    get_required_str(self, value_id)?,
                    language,
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
                    value,
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
            EncodedTerm::BooleanLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::FloatLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DoubleLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::IntegerLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DecimalLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DateTimeLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::DateLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::TimeLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::GYearMonthLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::GYearLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::GMonthDayLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::GDayLiteral(value) => Ok(Literal::from(value).into()),
            EncodedTerm::GMonthLiteral(value) => Ok(Literal::from(value).into()),
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
