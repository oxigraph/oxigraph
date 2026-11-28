use crate::named_node::NamedNode;
use crate::vocab::{rdf, xsd};
use oxilangtag::{LanguageTag, LanguageTagParseError};
#[cfg(feature = "oxsdatatypes")]
use oxsdatatypes::*;
use oxstr::{OxString, oxformat};
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;
use std::fmt::Write;

static XSD_STRING: NamedNode = xsd::STRING;
static RDF_LANG_STRING: NamedNode = rdf::LANG_STRING;
#[cfg(feature = "rdf-12")]
static RDF_DIR_LANG_STRING: NamedNode = rdf::DIR_LANG_STRING;

/// An owned RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// # use std::error::Error;
/// use oxrdf::Literal;
/// use oxrdf::vocab::xsd;
///
/// assert_eq!("\"foo\\nbar\"", Literal::from("foo\nbar").to_string());
///
/// assert_eq!(
///     r#""1999-01-01"^^<http://www.w3.org/2001/XMLSchema#date>"#,
///     Literal::new_typed("1999-01-01", xsd::DATE)?.to_string()
/// );
///
/// assert_eq!(
///     r#""foo"@en"#,
///     Literal::new_language_tagged("foo", "en")?.to_string()
/// );
/// # Result::<_, Box<dyn Error>>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Literal(LiteralContent);

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
enum LiteralContent {
    String(OxString),
    LanguageTaggedString {
        value: OxString,
        language: OxString,
    },
    #[cfg(feature = "rdf-12")]
    DirectionalLanguageTaggedString {
        value: OxString,
        language: OxString,
        direction: BaseDirection,
    },
    TypedLiteral {
        value: OxString,
        datatype: NamedNode,
    },
}

impl Literal {
    /// Builds an RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) with a [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    ///
    /// Returns an error for `rdf:langString` and, with the `rdf-12` feature, `rdf:dirLangString`.
    #[inline]
    pub fn new_typed(
        value: impl Into<OxString>,
        datatype: impl Into<NamedNode>,
    ) -> Result<Self, MalformedLangStringLiteral> {
        let datatype = datatype.into();
        if datatype == rdf::LANG_STRING {
            return Err(MalformedLangStringLiteral);
        }
        #[cfg(feature = "rdf-12")]
        if datatype == rdf::DIR_LANG_STRING {
            return Err(MalformedLangStringLiteral);
        }
        Ok(Self::new_typed_unchecked(value, datatype))
    }

    /// Builds an RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) with a [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    ///
    /// It is the responsibility of the caller to check that `datatype` is not `rdf:langString` or `rdf:dirLangString`.
    ///
    /// [`Literal::new_typed`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub fn new_typed_unchecked(value: impl Into<OxString>, datatype: impl Into<NamedNode>) -> Self {
        let value = value.into();
        let datatype = datatype.into();
        Self(if datatype == xsd::STRING {
            LiteralContent::String(value)
        } else {
            LiteralContent::TypedLiteral { value, datatype }
        })
    }

    /// Builds an RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    #[inline]
    pub fn new_language_tagged(
        value: impl Into<OxString>,
        language: impl Into<OxString>,
    ) -> Result<Self, LanguageTagParseError> {
        let mut language = language.into();
        if !language.bytes().all(|b| b.is_ascii_lowercase()) {
            language.make_mut().make_ascii_lowercase();
        }
        Ok(Self::new_language_tagged_unchecked(
            value,
            LanguageTag::parse(language)?.into_inner(),
        ))
    }

    /// Builds an RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    ///
    /// It is the responsibility of the caller to check that `language`
    /// is valid [BCP47](https://tools.ietf.org/html/bcp47) language tag,
    /// and is lowercase.
    ///
    /// [`Literal::new_language_tagged`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub fn new_language_tagged_unchecked(
        value: impl Into<OxString>,
        language: impl Into<OxString>,
    ) -> Self {
        Self(LiteralContent::LanguageTaggedString {
            value: value.into(),
            language: language.into(),
        })
    }

    /// Builds an RDF [directional language-tagged string](https://www.w3.org/TR/rdf12-concepts/#dfn-dir-lang-string).
    #[cfg(feature = "rdf-12")]
    #[inline]
    pub fn new_directional_language_tagged(
        value: impl Into<OxString>,
        language: impl Into<OxString>,
        direction: impl Into<BaseDirection>,
    ) -> Result<Self, LanguageTagParseError> {
        let mut language = language.into();
        if !language.bytes().all(|b| b.is_ascii_lowercase()) {
            language.make_mut().make_ascii_lowercase();
        }
        Ok(Self::new_directional_language_tagged_unchecked(
            value,
            LanguageTag::parse(language)?.into_inner(),
            direction,
        ))
    }

    /// Builds an RDF [directional language-tagged string](https://www.w3.org/TR/rdf12-concepts/#dfn-dir-lang-string).
    ///
    /// It is the responsibility of the caller to check that `language`
    /// is valid [BCP47](https://tools.ietf.org/html/bcp47) language tag,
    /// and is lowercase.
    ///
    /// [`Literal::new_directional_language_tagged`] is a safe version of this constructor and should be used for untrusted data.
    #[cfg(feature = "rdf-12")]
    #[inline]
    pub fn new_directional_language_tagged_unchecked(
        value: impl Into<OxString>,
        language: impl Into<OxString>,
        direction: impl Into<BaseDirection>,
    ) -> Self {
        Self(LiteralContent::DirectionalLanguageTaggedString {
            value: value.into(),
            language: language.into(),
            direction: direction.into(),
        })
    }

    #[deprecated(note = "Use Literal::from with a string instead")]
    #[inline]
    pub fn new_simple_literal(value: impl Into<OxString>) -> Self {
        Self::from(value.into())
    }

    #[deprecated(note = "Use Literal::new_typed instead")]
    #[inline]
    pub fn new_typed_literal(value: impl Into<OxString>, datatype: impl Into<NamedNode>) -> Self {
        Self::new_typed_unchecked(value, datatype)
    }

    #[deprecated(note = "Use Literal::new_language_tagged instead")]
    #[inline]
    pub fn new_language_tagged_literal(
        value: impl Into<OxString>,
        language: impl Into<OxString>,
    ) -> Result<Self, LanguageTagParseError> {
        Self::new_language_tagged(value, language)
    }

    #[deprecated(note = "Use Literal::new_language_tagged_unchecked instead")]
    #[inline]
    pub fn new_language_tagged_literal_unchecked(
        value: impl Into<OxString>,
        language: impl Into<OxString>,
    ) -> Self {
        Self::new_language_tagged_unchecked(value, language)
    }

    #[cfg(feature = "rdf-12")]
    #[deprecated(note = "Use Literal::new_directional_language_tagged instead")]
    #[inline]
    pub fn new_directional_language_tagged_literal(
        value: impl Into<OxString>,
        language: impl Into<OxString>,
        direction: impl Into<BaseDirection>,
    ) -> Result<Self, LanguageTagParseError> {
        Self::new_directional_language_tagged(value, language, direction)
    }

    #[cfg(feature = "rdf-12")]
    #[deprecated(note = "Use Literal::new_directional_language_tagged_unchecked instead")]
    #[inline]
    pub fn new_directional_language_tagged_literal_unchecked(
        value: impl Into<OxString>,
        language: impl Into<OxString>,
        direction: impl Into<BaseDirection>,
    ) -> Self {
        Self::new_directional_language_tagged_unchecked(value, language, direction)
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form).
    #[inline]
    pub fn value(&self) -> &str {
        match &self.0 {
            LiteralContent::String(value)
            | LiteralContent::TypedLiteral { value, .. }
            | LiteralContent::LanguageTaggedString { value, .. } => value,
            #[cfg(feature = "rdf-12")]
            LiteralContent::DirectionalLanguageTaggedString { value, .. } => value,
        }
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form).
    #[inline]
    pub fn into_value(self) -> OxString {
        match self.0 {
            LiteralContent::String(value)
            | LiteralContent::TypedLiteral { value, .. }
            | LiteralContent::LanguageTaggedString { value, .. } => value,
            #[cfg(feature = "rdf-12")]
            LiteralContent::DirectionalLanguageTaggedString { value, .. } => value,
        }
    }

    /// The literal [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag) if it is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    ///
    /// Language tags are defined by the [BCP47](https://tools.ietf.org/html/bcp47).
    /// They are normalized to lowercase by this implementation.
    #[inline]
    pub fn language(&self) -> Option<&str> {
        match &self.0 {
            LiteralContent::LanguageTaggedString { language, .. } => Some(language),
            #[cfg(feature = "rdf-12")]
            LiteralContent::DirectionalLanguageTaggedString { language, .. } => Some(language),
            _ => None,
        }
    }

    /// The literal [base direction](https://www.w3.org/TR/rdf12-concepts/#dfn-base-direction) if it is a [directional language-tagged string](https://www.w3.org/TR/rdf12-concepts/#dfn-base-direction).
    ///
    /// The two possible base directions are left-to-right (`ltr`) and right-to-left (`rtl`).
    #[cfg(feature = "rdf-12")]
    #[inline]
    pub fn direction(&self) -> Option<BaseDirection> {
        if let LiteralContent::DirectionalLanguageTaggedString { direction, .. } = &self.0 {
            Some(*direction)
        } else {
            None
        }
    }

    /// The literal [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    ///
    /// The datatype of [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string) is always [rdf:langString](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    /// The datatype of [simple literals](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal) is [xsd:string](https://www.w3.org/TR/xmlschema11-2/#string).
    #[inline]
    pub fn datatype(&self) -> &NamedNode {
        match &self.0 {
            LiteralContent::String(_) => &XSD_STRING,
            LiteralContent::LanguageTaggedString { .. } => &RDF_LANG_STRING,
            #[cfg(feature = "rdf-12")]
            LiteralContent::DirectionalLanguageTaggedString { .. } => &RDF_DIR_LANG_STRING,
            LiteralContent::TypedLiteral { datatype, .. } => datatype,
        }
    }

    /// Extract components from this literal (value, datatype, language tag and base direction).
    #[cfg(feature = "rdf-12")]
    #[inline]
    pub fn destruct(
        self,
    ) -> (
        OxString,
        Option<NamedNode>,
        Option<OxString>,
        Option<BaseDirection>,
    ) {
        match self.0 {
            LiteralContent::String(s) => (s, None, None, None),
            LiteralContent::LanguageTaggedString { value, language } => {
                (value, None, Some(language), None)
            }
            LiteralContent::DirectionalLanguageTaggedString {
                value,
                language,
                direction,
            } => (value, None, Some(language), Some(direction)),
            LiteralContent::TypedLiteral { value, datatype } => (value, Some(datatype), None, None),
        }
    }

    /// Extract components from this literal (value, datatype and language tag).
    #[cfg(not(feature = "rdf-12"))]
    #[inline]
    pub fn destruct(self) -> (OxString, Option<NamedNode>, Option<OxString>) {
        match self.0 {
            LiteralContent::String(s) => (s, None, None),
            LiteralContent::LanguageTaggedString { value, language } => {
                (value, None, Some(language))
            }
            LiteralContent::TypedLiteral { value, datatype } => (value, Some(datatype), None),
        }
    }
}

impl fmt::Display for Literal {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            LiteralContent::String(value) => print_quoted_str(value, f),
            LiteralContent::LanguageTaggedString { value, language } => {
                print_quoted_str(value, f)?;
                f.write_char('@')?;
                f.write_str(language)
            }
            #[cfg(feature = "rdf-12")]
            LiteralContent::DirectionalLanguageTaggedString {
                value,
                language,
                direction,
            } => {
                print_quoted_str(value, f)?;
                write!(f, "@{language}--{direction}")
            }
            LiteralContent::TypedLiteral { value, datatype } => {
                print_quoted_str(value, f)?;
                write!(f, "^^{datatype}")
            }
        }
    }
}

impl From<&'static str> for Literal {
    #[inline]
    fn from(value: &'static str) -> Self {
        OxString::from(value).into()
    }
}

impl From<String> for Literal {
    #[inline]
    fn from(value: String) -> Self {
        OxString::from(value).into()
    }
}

impl From<OxString> for Literal {
    #[inline]
    fn from(value: OxString) -> Self {
        Self(LiteralContent::String(value))
    }
}

impl From<bool> for Literal {
    #[inline]
    fn from(value: bool) -> Self {
        Literal::new_typed_unchecked(if value { "true" } else { "false" }, xsd::BOOLEAN)
    }
}

impl From<i128> for Literal {
    #[inline]
    fn from(value: i128) -> Self {
        Literal::new_typed_unchecked(oxformat!("{value}"), xsd::INTEGER)
    }
}

impl From<i64> for Literal {
    #[inline]
    fn from(value: i64) -> Self {
        Literal::new_typed_unchecked(oxformat!("{value}"), xsd::INTEGER)
    }
}

impl From<i32> for Literal {
    #[inline]
    fn from(value: i32) -> Self {
        Literal::new_typed_unchecked(oxformat!("{value}"), xsd::INTEGER)
    }
}

impl From<i16> for Literal {
    #[inline]
    fn from(value: i16) -> Self {
        Literal::new_typed_unchecked(oxformat!("{value}"), xsd::INTEGER)
    }
}

impl From<u64> for Literal {
    #[inline]
    fn from(value: u64) -> Self {
        Literal::new_typed_unchecked(oxformat!("{value}"), xsd::INTEGER)
    }
}

impl From<u32> for Literal {
    #[inline]
    fn from(value: u32) -> Self {
        Literal::new_typed_unchecked(oxformat!("{value}"), xsd::INTEGER)
    }
}

impl From<u16> for Literal {
    #[inline]
    fn from(value: u16) -> Self {
        Literal::new_typed_unchecked(oxformat!("{value}"), xsd::INTEGER)
    }
}

impl From<f32> for Literal {
    #[inline]
    fn from(value: f32) -> Self {
        Literal::new_typed_unchecked(
            {
                #[cfg(feature = "oxsdatatypes")]
                {
                    oxformat!("{}", Float::from(value))
                }
                #[cfg(not(feature = "oxsdatatypes"))]
                if value == f32::INFINITY {
                    OxString::new("INF")
                } else if value == f32::NEG_INFINITY {
                    OxString::new("-INF")
                } else {
                    oxformat!("{value}")
                }
            },
            xsd::FLOAT,
        )
    }
}

impl From<f64> for Literal {
    #[inline]
    fn from(value: f64) -> Self {
        Literal::new_typed_unchecked(
            {
                #[cfg(feature = "oxsdatatypes")]
                {
                    oxformat!("{}", Double::from(value))
                }
                #[cfg(not(feature = "oxsdatatypes"))]
                if value == f64::INFINITY {
                    OxString::new("INF")
                } else if value == f64::NEG_INFINITY {
                    OxString::new("-INF")
                } else {
                    oxformat!("{value}")
                }
            },
            xsd::DOUBLE,
        )
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Boolean> for Literal {
    #[inline]
    fn from(value: Boolean) -> Self {
        bool::from(value).into()
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Float> for Literal {
    #[inline]
    fn from(value: Float) -> Self {
        Self::new_typed_unchecked(value.to_string(), xsd::FLOAT)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Double> for Literal {
    #[inline]
    fn from(value: Double) -> Self {
        Self::new_typed_unchecked(value.to_string(), xsd::DOUBLE)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Integer> for Literal {
    #[inline]
    fn from(value: Integer) -> Self {
        Self::new_typed_unchecked(value.to_string(), xsd::INTEGER)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Decimal> for Literal {
    #[inline]
    fn from(value: Decimal) -> Self {
        Self::new_typed_unchecked(value.to_string(), xsd::DECIMAL)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<DateTime> for Literal {
    #[inline]
    fn from(value: DateTime) -> Self {
        Self::new_typed_unchecked(value.to_string(), xsd::DATE_TIME)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Time> for Literal {
    #[inline]
    fn from(value: Time) -> Self {
        Self::new_typed_unchecked(value.to_string(), xsd::TIME)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Date> for Literal {
    #[inline]
    fn from(value: Date) -> Self {
        Self::new_typed_unchecked(value.to_string(), xsd::DATE)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GYearMonth> for Literal {
    #[inline]
    fn from(value: GYearMonth) -> Self {
        Self::new_typed_unchecked(value.to_string(), xsd::G_YEAR_MONTH)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GYear> for Literal {
    #[inline]
    fn from(value: GYear) -> Self {
        Self::new_typed_unchecked(oxformat!("{value}"), xsd::G_YEAR)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GMonthDay> for Literal {
    #[inline]
    fn from(value: GMonthDay) -> Self {
        Self::new_typed_unchecked(oxformat!("{value}"), xsd::G_MONTH_DAY)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GMonth> for Literal {
    #[inline]
    fn from(value: GMonth) -> Self {
        Self::new_typed_unchecked(oxformat!("{value}"), xsd::G_MONTH)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GDay> for Literal {
    #[inline]
    fn from(value: GDay) -> Self {
        Self::new_typed_unchecked(oxformat!("{value}"), xsd::G_DAY)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Duration> for Literal {
    #[inline]
    fn from(value: Duration) -> Self {
        Self::new_typed_unchecked(oxformat!("{value}"), xsd::DURATION)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<YearMonthDuration> for Literal {
    #[inline]
    fn from(value: YearMonthDuration) -> Self {
        Self::new_typed_unchecked(oxformat!("{value}"), xsd::YEAR_MONTH_DURATION)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<DayTimeDuration> for Literal {
    #[inline]
    fn from(value: DayTimeDuration) -> Self {
        Self::new_typed_unchecked(oxformat!("{value}"), xsd::DAY_TIME_DURATION)
    }
}

#[inline]
pub fn print_quoted_str(string: &str, f: &mut impl Write) -> fmt::Result {
    f.write_char('"')?;
    for c in string.chars() {
        match c {
            '\u{8}' => f.write_str("\\b"),
            '\t' => f.write_str("\\t"),
            '\n' => f.write_str("\\n"),
            '\u{C}' => f.write_str("\\f"),
            '\r' => f.write_str("\\r"),
            '"' => f.write_str("\\\""),
            '\\' => f.write_str("\\\\"),
            '\0'..='\u{1F}' | '\u{7F}' | '\u{FFFE}' | '\u{FFFF}' => {
                write!(f, "\\u{:04X}", u32::from(c))
            }
            _ => f.write_char(c),
        }?;
    }
    f.write_char('"')
}

/// A [directional language-tagged string](https://www.w3.org/TR/rdf12-concepts/#dfn-dir-lang-string) [base-direction](https://www.w3.org/TR/rdf12-concepts/#dfn-base-direction)
#[cfg(feature = "rdf-12")]
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BaseDirection {
    /// the initial text direction is set to left-to-right
    #[cfg_attr(feature = "serde", serde(rename = "ltr"))]
    Ltr,
    /// the initial text direction is set to right-to-left
    #[cfg_attr(feature = "serde", serde(rename = "rtl"))]
    Rtl,
}

#[cfg(feature = "rdf-12")]
impl fmt::Display for BaseDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        })
    }
}

#[cfg(feature = "serde")]
impl Serialize for Literal {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[expect(clippy::struct_field_names)]
        #[derive(Serialize)]
        #[serde(rename = "Literal")]
        struct Value<'a> {
            value: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            language: Option<&'a str>,
            #[cfg(feature = "rdf-12")]
            #[serde(skip_serializing_if = "Option::is_none")]
            direction: Option<BaseDirection>,
            #[serde(skip_serializing_if = "Option::is_none")]
            datatype: Option<&'a str>,
        }
        Value {
            value: self.value(),
            language: self.language(),
            #[cfg(feature = "rdf-12")]
            direction: self.direction(),
            datatype: if let LiteralContent::TypedLiteral { datatype, .. } = &self.0 {
                Some(datatype.as_str())
            } else {
                None
            },
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Literal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[expect(clippy::struct_field_names)]
        #[derive(Deserialize)]
        #[serde(rename = "Literal")]
        struct Value {
            value: OxString,
            language: Option<OxString>,
            #[cfg(feature = "rdf-12")]
            direction: Option<BaseDirection>,
            datatype: Option<OxString>,
        }
        let Value {
            value,
            language,
            #[cfg(feature = "rdf-12")]
            direction,
            datatype,
        } = Value::deserialize(deserializer)?;
        if let Some(language) = language {
            #[cfg(feature = "rdf-12")]
            if let Some(direction) = direction {
                return Literal::new_directional_language_tagged(value, language, direction)
                    .map_err(de::Error::custom);
            }
            Literal::new_language_tagged(value, language).map_err(de::Error::custom)
        } else if let Some(datatype) = datatype {
            Literal::new_typed(value, NamedNode::new(datatype).map_err(de::Error::custom)?)
                .map_err(de::Error::custom)
        } else {
            Ok(value.into())
        }
    }
}

/// Error returned by [`Literal::new_typed`] when used with `rdf:langString` or `rdf:dirLangString`.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("rdf:langString and rdf:dirLangString literals must have a language tag")]
pub struct MalformedLangStringLiteral;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_literal_equality() {
        assert_eq!(
            Literal::from("foo"),
            Literal::new_typed_unchecked("foo", xsd::STRING)
        );
    }

    #[test]
    fn typed_literal_requires_language_tag() {
        Literal::new_typed("foo", rdf::LANG_STRING).unwrap_err();
        #[cfg(feature = "rdf-12")]
        Literal::new_typed("foo", rdf::DIR_LANG_STRING).unwrap_err();
    }

    #[test]
    fn test_float_format() {
        assert_eq!("INF", Literal::from(f32::INFINITY).value());
        assert_eq!("INF", Literal::from(f64::INFINITY).value());
        assert_eq!("-INF", Literal::from(f32::NEG_INFINITY).value());
        assert_eq!("-INF", Literal::from(f64::NEG_INFINITY).value());
        assert_eq!("NaN", Literal::from(f32::NAN).value());
        assert_eq!("NaN", Literal::from(f64::NAN).value());
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde() {
        // Simple literal
        let simple = Literal::from("foo");
        let j = serde_json::to_string(&simple).unwrap();
        assert_eq!("{\"value\":\"foo\"}", j);
        let simple2: Literal = serde_json::from_str(&j).unwrap();
        assert_eq!(simple, simple2);

        // Typed literal
        let typed = Literal::new_typed_unchecked("foo", xsd::BOOLEAN);
        let j = serde_json::to_string(&typed).unwrap();
        assert_eq!(
            "{\"value\":\"foo\",\"datatype\":\"http://www.w3.org/2001/XMLSchema#boolean\"}",
            j
        );
        let typed2: Literal = serde_json::from_str(&j).unwrap();
        assert_eq!(typed, typed2);

        // Language-tagged string
        let lt = Literal::new_language_tagged("foo", "en").unwrap();
        let j = serde_json::to_string(&lt).unwrap();
        assert_eq!("{\"value\":\"foo\",\"language\":\"en\"}", j);
        let lt2: Literal = serde_json::from_str(&j).unwrap();
        assert_eq!(lt, lt2);
    }
}
