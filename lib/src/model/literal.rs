use crate::model::named_node::NamedNode;
use crate::model::vocab::rdf;
use crate::model::vocab::xsd;
use chrono::format::{parse, Parsed, StrftimeItems};
use chrono::prelude::*;
use num_traits::identities::Zero;
use num_traits::FromPrimitive;
use num_traits::One;
use num_traits::ToPrimitive;
use ordered_float::OrderedFloat;
use rio_api::model as rio;
use rust_decimal::Decimal;
use std::borrow::Cow;
use std::fmt;
use std::option::Option;

/// A RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal)
///
/// The default string formatter is returning a N-Triples, Turtle and SPARQL compatible representation:
/// ```
/// use rudf::model::Literal;
/// use rudf::model::vocab::xsd;
///
/// assert_eq!(
///     "\"foo\\tbar\"",
///     Literal::new_simple_literal("foo\tbar").to_string()
/// );
///
/// assert_eq!(
///     "\"1999-01-01\"^^<http://www.w3.org/2001/XMLSchema#date>",
///     Literal::new_typed_literal("1999-01-01", xsd::DATE.clone()).to_string()
/// );
///
/// assert_eq!(
///     "\"foo\"@en",
///     Literal::new_language_tagged_literal("foo", "en").to_string()
/// );
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Literal(LiteralContent);

#[derive(PartialEq, Eq, Ord, PartialOrd, Debug, Clone, Hash)]
enum LiteralContent {
    String(String),
    LanguageTaggedString { value: String, language: String },
    Boolean(bool),
    Float(OrderedFloat<f32>),
    Double(OrderedFloat<f64>),
    Integer(i128),
    Decimal(Decimal),
    Date(Date<FixedOffset>),
    NaiveDate(NaiveDate),
    NaiveTime(NaiveTime),
    DateTime(DateTime<FixedOffset>),
    NaiveDateTime(NaiveDateTime),
    TypedLiteral { value: String, datatype: NamedNode },
}

impl Literal {
    /// Builds a RDF [simple literal](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal)
    pub fn new_simple_literal(value: impl Into<String>) -> Self {
        Literal(LiteralContent::String(value.into()))
    }

    /// Builds a RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) with a [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri)
    pub fn new_typed_literal(value: impl Into<String>, datatype: impl Into<NamedNode>) -> Self {
        let value = value.into();
        let datatype = datatype.into();
        Literal(if datatype == *xsd::BOOLEAN {
            match value.as_str() {
                "true" | "1" => LiteralContent::Boolean(true),
                "false" | "0" => LiteralContent::Boolean(false),
                _ => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::STRING {
            LiteralContent::String(value)
        } else if datatype == *xsd::FLOAT {
            match value.parse() {
                Ok(value) => LiteralContent::Float(OrderedFloat(value)),
                Err(_) => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::DOUBLE {
            match value.parse() {
                Ok(value) => LiteralContent::Double(OrderedFloat(value)),
                Err(_) => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::INTEGER
            || datatype == *xsd::BYTE
            || datatype == *xsd::SHORT
            || datatype == *xsd::INT
            || datatype == *xsd::LONG
            || datatype == *xsd::UNSIGNED_BYTE
            || datatype == *xsd::UNSIGNED_SHORT
            || datatype == *xsd::UNSIGNED_INT
            || datatype == *xsd::UNSIGNED_LONG
            || datatype == *xsd::POSITIVE_INTEGER
            || datatype == *xsd::NEGATIVE_INTEGER
            || datatype == *xsd::NON_POSITIVE_INTEGER
            || datatype == *xsd::NON_NEGATIVE_INTEGER
        {
            match value.parse() {
                Ok(value) => LiteralContent::Integer(value),
                Err(_) => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::DECIMAL {
            match value.parse() {
                Ok(value) => LiteralContent::Decimal(value),
                Err(_) => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::DATE {
            let mut parsed = Parsed::new();
            match parse(&mut parsed, &value, StrftimeItems::new("%Y-%m-%d%:z")).and_then(|_| {
                Ok(Date::from_utc(
                    parsed.to_naive_date()?,
                    parsed.to_fixed_offset()?,
                ))
            }) {
                Ok(value) => LiteralContent::Date(value),
                Err(_) => match NaiveDate::parse_from_str(&value, "%Y-%m-%dZ") {
                    Ok(value) => LiteralContent::Date(Date::from_utc(value, FixedOffset::east(0))),
                    Err(_) => match NaiveDate::parse_from_str(&value, "%Y-%m-%d") {
                        Ok(value) => LiteralContent::NaiveDate(value),
                        Err(_) => LiteralContent::TypedLiteral { value, datatype },
                    },
                },
            }
        } else if datatype == *xsd::TIME {
            match NaiveTime::parse_from_str(&value, "%H:%M:%S") {
                Ok(value) => LiteralContent::NaiveTime(value),
                Err(_) => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::DATE_TIME || datatype == *xsd::DATE_TIME_STAMP {
            match DateTime::parse_from_rfc3339(&value) {
                Ok(value) => LiteralContent::DateTime(value),
                Err(_) => match NaiveDateTime::parse_from_str(&value, "%Y-%m-%dT%H:%M:%S") {
                    Ok(value) => LiteralContent::NaiveDateTime(value),
                    Err(_) => LiteralContent::TypedLiteral { value, datatype },
                },
            }
        } else {
            LiteralContent::TypedLiteral { value, datatype }
        })
    }

    /// Builds a RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    pub fn new_language_tagged_literal(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        let mut language = language.into();
        language.make_ascii_lowercase();
        Literal(LiteralContent::LanguageTaggedString {
            value: value.into(),
            language,
        })
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form)
    pub fn value(&self) -> Cow<'_, str> {
        match self.0 {
            LiteralContent::String(ref value)
            | LiteralContent::LanguageTaggedString { ref value, .. }
            | LiteralContent::TypedLiteral { ref value, .. } => Cow::Borrowed(value),
            LiteralContent::Boolean(value) => Cow::Owned(value.to_string()),
            LiteralContent::Float(value) => Cow::Owned(value.to_string()),
            LiteralContent::Double(value) => Cow::Owned(value.to_string()),
            LiteralContent::Integer(value) => Cow::Owned(value.to_string()),
            LiteralContent::Decimal(value) => Cow::Owned(value.to_string()),
            LiteralContent::Date(value) => Cow::Owned(value.to_string()),
            LiteralContent::NaiveDate(value) => Cow::Owned(value.to_string()),
            LiteralContent::NaiveTime(value) => Cow::Owned(value.to_string()),
            LiteralContent::DateTime(value) => Cow::Owned(value.to_string()),
            LiteralContent::NaiveDateTime(value) => Cow::Owned(value.to_string()),
        }
    }

    /// The literal [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag) if it is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    ///
    /// Language tags are defined by the [BCP47](https://tools.ietf.org/html/bcp47).
    /// They are normalized to lowercase by this implementation.
    pub fn language(&self) -> Option<&String> {
        match self.0 {
            LiteralContent::LanguageTaggedString { ref language, .. } => Some(language),
            _ => None,
        }
    }

    /// The literal [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    ///
    /// The datatype of [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string) is always [rdf:langString](http://www.w3.org/1999/02/22-rdf-syntax-ns#langString).
    /// The datatype of [simple literals](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal) is [xsd:string](http://www.w3.org/2001/XMLSchema#string).
    pub fn datatype(&self) -> &NamedNode {
        match self.0 {
            LiteralContent::String(_) => &xsd::STRING,
            LiteralContent::LanguageTaggedString { .. } => &rdf::LANG_STRING,
            LiteralContent::Boolean(_) => &xsd::BOOLEAN,
            LiteralContent::Float(_) => &xsd::FLOAT,
            LiteralContent::Double(_) => &xsd::DOUBLE,
            LiteralContent::Integer(_) => &xsd::INTEGER,
            LiteralContent::Decimal(_) => &xsd::DECIMAL,
            LiteralContent::Date(_) | LiteralContent::NaiveDate(_) => &xsd::DATE,
            LiteralContent::NaiveTime(_) => &xsd::TIME,
            LiteralContent::DateTime(_) | LiteralContent::NaiveDateTime(_) => &xsd::DATE_TIME,
            LiteralContent::TypedLiteral { ref datatype, .. } => datatype,
        }
    }

    /// Checks if it could be considered as an RDF 1.0 [plain literal](https://www.w3.org/TR/rdf-concepts/#dfn-plain-literal).
    ///
    /// It returns true if the literal is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    /// or has the datatype [xsd:string](http://www.w3.org/2001/XMLSchema#string).
    pub fn is_plain(&self) -> bool {
        match self.0 {
            LiteralContent::String(_) | LiteralContent::LanguageTaggedString { .. } => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:string](http://www.w3.org/2001/XMLSchema#string) and is valid
    pub fn is_string(&self) -> bool {
        match self.0 {
            LiteralContent::String(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:boolean](http://www.w3.org/2001/XMLSchema#boolean) and is valid
    pub fn is_boolean(&self) -> bool {
        match self.0 {
            LiteralContent::Boolean(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:float](http://www.w3.org/2001/XMLSchema#float) and is valid
    pub fn is_float(&self) -> bool {
        match self.0 {
            LiteralContent::Float(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:double](http://www.w3.org/2001/XMLSchema#double) and is valid
    pub fn is_double(&self) -> bool {
        match self.0 {
            LiteralContent::Double(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:integer](http://www.w3.org/2001/XMLSchema#integer) and is valid
    pub fn is_integer(&self) -> bool {
        match self.0 {
            LiteralContent::Integer(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:decimal](http://www.w3.org/2001/XMLSchema#decimal) or one of its sub datatype and is valid
    pub fn is_decimal(&self) -> bool {
        match self.0 {
            LiteralContent::Integer(_) | LiteralContent::Decimal(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:date](http://www.w3.org/2001/XMLSchema#date) and is valid
    pub fn is_date(&self) -> bool {
        match self.0 {
            LiteralContent::Date(_) | LiteralContent::NaiveDate(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:date](http://www.w3.org/2001/XMLSchema#time) and is valid
    pub fn is_time(&self) -> bool {
        match self.0 {
            LiteralContent::NaiveTime(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:dateTime](http://www.w3.org/2001/XMLSchema#dateTime) or one of its sub datatype and is valid
    pub fn is_date_time(&self) -> bool {
        match self.0 {
            LiteralContent::DateTime(_) | LiteralContent::NaiveDateTime(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:dateTimeStamp](http://www.w3.org/2001/XMLSchema#dateTimeStamp) or [xsd:dateTime](http://www.w3.org/2001/XMLSchema#dateTime) with a fixed timezone and is valid
    pub fn is_date_time_stamp(&self) -> bool {
        match self.0 {
            LiteralContent::DateTime(_) => true,
            _ => false,
        }
    }

    /// Returns the [effective boolean value](https://www.w3.org/TR/sparql11-query/#ebv) of the literal if it exists
    pub fn to_bool(&self) -> Option<bool> {
        match self.0 {
            LiteralContent::String(ref value) => Some(!value.is_empty()),
            LiteralContent::Boolean(value) => Some(value),
            LiteralContent::Float(value) => Some(!value.is_zero()),
            LiteralContent::Double(value) => Some(!value.is_zero()),
            LiteralContent::Integer(value) => Some(!value.is_zero()),
            LiteralContent::Decimal(value) => Some(!value.is_zero()),
            _ => None,
        }
    }

    /// Returns the value of this literal as an f32 if it exists following the rules of [XPath xsd:float casting](https://www.w3.org/TR/xpath-functions/#casting-to-float)
    pub fn to_float(&self) -> Option<f32> {
        match self.0 {
            LiteralContent::Float(value) => value.to_f32(),
            LiteralContent::Double(value) => value.to_f32(),
            LiteralContent::Integer(value) => value.to_f32(),
            LiteralContent::Decimal(value) => value.to_f32(),
            LiteralContent::Boolean(value) => Some(if value { 1. } else { 0. }),
            LiteralContent::String(ref value) => value.parse().ok(),
            _ => None,
        }
    }

    /// Returns the value of this literal as an f64 if it exists following the rules of [XPath xsd:double casting](https://www.w3.org/TR/xpath-functions/#casting-to-double)
    pub fn to_double(&self) -> Option<f64> {
        match self.0 {
            LiteralContent::Float(value) => value.to_f64(),
            LiteralContent::Double(value) => value.to_f64(),
            LiteralContent::Integer(value) => value.to_f64(),
            LiteralContent::Decimal(value) => value.to_f64(),
            LiteralContent::Boolean(value) => Some(if value { 1. } else { 0. }),
            LiteralContent::String(ref value) => value.parse().ok(),
            _ => None,
        }
    }

    /// Returns the value of this literal as an i128 if it exists following the rules of [XPath xsd:integer casting](https://www.w3.org/TR/xpath-functions/#casting-to-integer)
    pub fn to_integer(&self) -> Option<i128> {
        match self.0 {
            LiteralContent::Float(value) => value.to_i128(),
            LiteralContent::Double(value) => value.to_i128(),
            LiteralContent::Integer(value) => value.to_i128(),
            LiteralContent::Decimal(value) => value.to_i128(),
            LiteralContent::Boolean(value) => Some(if value { 1 } else { 0 }),
            LiteralContent::String(ref value) => value.parse().ok(),
            _ => None,
        }
    }

    /// Returns the value of this literal as Decimal if it exists following the rules of [XPath xsd:decimal casting](https://www.w3.org/TR/xpath-functions/#casting-to-decimal)
    pub(crate) fn to_decimal(&self) -> Option<Decimal> {
        match self.0 {
            LiteralContent::Float(value) => Decimal::from_f32(*value),
            LiteralContent::Double(value) => Decimal::from_f64(*value),
            LiteralContent::Integer(value) => Decimal::from_i128(value),
            LiteralContent::Decimal(value) => Some(value),
            LiteralContent::Boolean(value) => Some(if value {
                Decimal::one()
            } else {
                Decimal::zero()
            }),
            LiteralContent::String(ref value) => value.parse().ok(),
            _ => None,
        }
    }

    /// Returns the value of this literal as NaiveDate if possible
    pub(crate) fn to_naive_date(&self) -> Option<NaiveDate> {
        match self.0 {
            LiteralContent::Date(value) => Some(value.naive_utc()),
            LiteralContent::NaiveDate(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the value of this literal as Date if possible
    pub(crate) fn to_date(&self) -> Option<Date<FixedOffset>> {
        match self.0 {
            LiteralContent::Date(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the value of this literal as NaiveTime if possible
    pub(crate) fn to_time(&self) -> Option<NaiveTime> {
        match self.0 {
            LiteralContent::NaiveTime(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the value of this literal as NaiveDateTime if possible
    pub(crate) fn to_date_time(&self) -> Option<NaiveDateTime> {
        match self.0 {
            LiteralContent::DateTime(value) => Some(value.naive_utc()),
            LiteralContent::NaiveDateTime(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the value of this literal as DateTime<FixedOffset> if possible
    pub(crate) fn to_date_time_stamp(&self) -> Option<DateTime<FixedOffset>> {
        if let LiteralContent::DateTime(value) = self.0 {
            Some(value)
        } else {
            None
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_plain() {
            self.language()
                .map(|lang| {
                    rio::Literal::LanguageTaggedString {
                        value: &self.value(),
                        language: lang.as_str(),
                    }
                    .fmt(f)
                })
                .unwrap_or_else(|| {
                    rio::Literal::Simple {
                        value: &self.value(),
                    }
                    .fmt(f)
                })
        } else {
            rio::Literal::Typed {
                value: &self.value(),
                datatype: rio::NamedNode {
                    iri: self.datatype().as_str(),
                },
            }
            .fmt(f)
        }
    }
}

impl<'a> From<&'a str> for Literal {
    fn from(value: &'a str) -> Self {
        Literal(LiteralContent::String(value.into()))
    }
}

impl From<String> for Literal {
    fn from(value: String) -> Self {
        Literal(LiteralContent::String(value))
    }
}

impl<'a> From<Cow<'a, str>> for Literal {
    fn from(value: Cow<'a, str>) -> Self {
        Literal(LiteralContent::String(value.into()))
    }
}

impl From<bool> for Literal {
    fn from(value: bool) -> Self {
        Literal(LiteralContent::Boolean(value))
    }
}

impl From<i128> for Literal {
    fn from(value: i128) -> Self {
        Literal(LiteralContent::Integer(value))
    }
}

impl From<i64> for Literal {
    fn from(value: i64) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<i32> for Literal {
    fn from(value: i32) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<i16> for Literal {
    fn from(value: i16) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<u64> for Literal {
    fn from(value: u64) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<u32> for Literal {
    fn from(value: u32) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<u16> for Literal {
    fn from(value: u16) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<f32> for Literal {
    fn from(value: f32) -> Self {
        Literal(LiteralContent::Float(value.into()))
    }
}

impl From<f64> for Literal {
    fn from(value: f64) -> Self {
        Literal(LiteralContent::Double(value.into()))
    }
}

impl From<Decimal> for Literal {
    fn from(value: Decimal) -> Self {
        Literal(LiteralContent::Decimal(value))
    }
}

impl From<Date<FixedOffset>> for Literal {
    fn from(value: Date<FixedOffset>) -> Self {
        Literal(LiteralContent::Date(value))
    }
}

impl From<NaiveDate> for Literal {
    fn from(value: NaiveDate) -> Self {
        Literal(LiteralContent::NaiveDate(value))
    }
}

impl From<NaiveTime> for Literal {
    fn from(value: NaiveTime) -> Self {
        Literal(LiteralContent::NaiveTime(value))
    }
}

impl From<DateTime<FixedOffset>> for Literal {
    fn from(value: DateTime<FixedOffset>) -> Self {
        Literal(LiteralContent::DateTime(value))
    }
}

impl From<NaiveDateTime> for Literal {
    fn from(value: NaiveDateTime) -> Self {
        Literal(LiteralContent::NaiveDateTime(value))
    }
}
