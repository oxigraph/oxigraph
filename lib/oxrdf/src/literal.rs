use crate::named_node::{NamedNode, NamedNodeRef};
use crate::vocab::{rdf, xsd};
use oxilangtag::{LanguageTag, LanguageTagParseError};
#[cfg(feature = "oxsdatatypes")]
use oxsdatatypes::*;
#[cfg(feature = "serde")]
use serde::{de, ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::fmt;
use std::fmt::Write;

/// An owned RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// # use oxilangtag::LanguageTagParseError;
/// use oxrdf::vocab::xsd;
/// use oxrdf::Literal;
///
/// assert_eq!(
///     "\"foo\\nbar\"",
///     Literal::new_simple_literal("foo\nbar").to_string()
/// );
///
/// assert_eq!(
///     r#""1999-01-01"^^<http://www.w3.org/2001/XMLSchema#date>"#,
///     Literal::new_typed_literal("1999-01-01", xsd::DATE).to_string()
/// );
///
/// assert_eq!(
///     r#""foo"@en"#,
///     Literal::new_language_tagged_literal("foo", "en")?.to_string()
/// );
/// # Result::<_, LanguageTagParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Literal(LiteralContent);

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
enum LiteralContent {
    String(String),
    LanguageTaggedString { value: String, language: String },
    TypedLiteral { value: String, datatype: NamedNode },
}

impl Literal {
    /// Builds an RDF [simple literal](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal).
    #[inline]
    pub fn new_simple_literal(value: impl Into<String>) -> Self {
        Self(LiteralContent::String(value.into()))
    }

    /// Builds an RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) with a [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    #[inline]
    pub fn new_typed_literal(value: impl Into<String>, datatype: impl Into<NamedNode>) -> Self {
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
    pub fn new_language_tagged_literal(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Result<Self, LanguageTagParseError> {
        let mut language = language.into();
        language.make_ascii_lowercase();
        Ok(Self::new_language_tagged_literal_unchecked(
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
    /// [`Literal::new_language_tagged_literal()`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub fn new_language_tagged_literal_unchecked(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Self(LiteralContent::LanguageTaggedString {
            value: value.into(),
            language: language.into(),
        })
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form).
    #[inline]
    pub fn value(&self) -> &str {
        self.as_ref().value()
    }

    /// The literal [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag) if it is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    ///
    /// Language tags are defined by the [BCP47](https://tools.ietf.org/html/bcp47).
    /// They are normalized to lowercase by this implementation.
    #[inline]
    pub fn language(&self) -> Option<&str> {
        self.as_ref().language()
    }

    /// The literal [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    ///
    /// The datatype of [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string) is always [rdf:langString](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    /// The datatype of [simple literals](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal) is [xsd:string](https://www.w3.org/TR/xmlschema11-2/#string).
    #[inline]
    pub fn datatype(&self) -> NamedNodeRef<'_> {
        self.as_ref().datatype()
    }

    /// Checks if this literal could be seen as an RDF 1.0 [plain literal](https://www.w3.org/TR/2004/REC-rdf-concepts-20040210/#dfn-plain-literal).
    ///
    /// It returns true if the literal is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    /// or has the datatype [xsd:string](https://www.w3.org/TR/xmlschema11-2/#string).
    #[inline]
    pub fn is_plain(&self) -> bool {
        self.as_ref().is_plain()
    }

    #[inline]
    pub fn as_ref(&self) -> LiteralRef<'_> {
        LiteralRef(match &self.0 {
            LiteralContent::String(value) => LiteralRefContent::String(value),
            LiteralContent::LanguageTaggedString { value, language } => {
                LiteralRefContent::LanguageTaggedString { value, language }
            }
            LiteralContent::TypedLiteral { value, datatype } => LiteralRefContent::TypedLiteral {
                value,
                datatype: datatype.as_ref(),
            },
        })
    }

    /// Extract components from this literal (value, datatype and language tag).
    #[inline]
    pub fn destruct(self) -> (String, Option<NamedNode>, Option<String>) {
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
        self.as_ref().fmt(f)
    }
}

#[cfg(feature = "serde")]
impl Serialize for Literal {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map: <S as Serializer>::SerializeStruct;
        match self {
            Self(LiteralContent::String(value)) => {
                map = serializer.serialize_struct("Literal", 1)?;
                map.serialize_field("value", value)?;
            }
            Self(LiteralContent::LanguageTaggedString { value, language }) => {
                map = serializer.serialize_struct("Literal", 2)?;
                map.serialize_field("value", value)?;
                map.serialize_field("language", language)?;
            }
            Self(LiteralContent::TypedLiteral { value, datatype }) => {
                map = serializer.serialize_struct("Literal", 2)?;
                map.serialize_field("value", value)?;
                map.serialize_field("datatype", datatype.as_str())?;
            }
        };
        map.end()
    }
}

#[cfg(feature = "serde")]
#[derive(Deserialize)]
struct LiteralValue {
    value: String,
    language: Option<String>,
    datatype: Option<String>,
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Literal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let LiteralValue {
            value,
            language,
            datatype,
        } = LiteralValue::deserialize(deserializer)?;
        match (datatype, language) {
            (Some(datatype), None) => Ok(Literal::new_typed_literal(
                value,
                if cfg!(feature = "serde-unvalidated") {
                    NamedNode::new_unchecked(datatype)
                } else {
                    NamedNode::new(datatype).map_err(de::Error::custom)?
                },
            )),
            (None, Some(language)) => {
                if cfg!(feature = "serde-unvalidated") {
                    Ok(Literal::new_language_tagged_literal_unchecked(
                        value, language,
                    ))
                } else {
                    Literal::new_language_tagged_literal(value, language).map_err(de::Error::custom)
                }
            }
            _ => Ok(Literal::new_simple_literal(value)),
        }
    }
}

impl<'a> From<&'a str> for Literal {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self(LiteralContent::String(value.into()))
    }
}

impl From<String> for Literal {
    #[inline]
    fn from(value: String) -> Self {
        Self(LiteralContent::String(value))
    }
}

impl<'a> From<Cow<'a, str>> for Literal {
    #[inline]
    fn from(value: Cow<'a, str>) -> Self {
        Self(LiteralContent::String(value.into()))
    }
}

impl From<bool> for Literal {
    #[inline]
    fn from(value: bool) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::BOOLEAN.into(),
        })
    }
}

impl From<i128> for Literal {
    #[inline]
    fn from(value: i128) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.into(),
        })
    }
}

impl From<i64> for Literal {
    #[inline]
    fn from(value: i64) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.into(),
        })
    }
}

impl From<i32> for Literal {
    #[inline]
    fn from(value: i32) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.into(),
        })
    }
}

impl From<i16> for Literal {
    #[inline]
    fn from(value: i16) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.into(),
        })
    }
}

impl From<u64> for Literal {
    #[inline]
    fn from(value: u64) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.into(),
        })
    }
}

impl From<u32> for Literal {
    #[inline]
    fn from(value: u32) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.into(),
        })
    }
}

impl From<u16> for Literal {
    #[inline]
    fn from(value: u16) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.into(),
        })
    }
}

impl From<f32> for Literal {
    #[inline]
    fn from(value: f32) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: if value == f32::INFINITY {
                "INF".to_owned()
            } else if value == f32::NEG_INFINITY {
                "-INF".to_owned()
            } else {
                value.to_string()
            },
            datatype: xsd::FLOAT.into(),
        })
    }
}

impl From<f64> for Literal {
    #[inline]
    fn from(value: f64) -> Self {
        Self(LiteralContent::TypedLiteral {
            value: if value == f64::INFINITY {
                "INF".to_owned()
            } else if value == f64::NEG_INFINITY {
                "-INF".to_owned()
            } else {
                value.to_string()
            },
            datatype: xsd::DOUBLE.into(),
        })
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Boolean> for Literal {
    #[inline]
    fn from(value: Boolean) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::BOOLEAN)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Float> for Literal {
    #[inline]
    fn from(value: Float) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::FLOAT)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Double> for Literal {
    #[inline]
    fn from(value: Double) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DOUBLE)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Integer> for Literal {
    #[inline]
    fn from(value: Integer) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::INTEGER)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Decimal> for Literal {
    #[inline]
    fn from(value: Decimal) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DECIMAL)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<DateTime> for Literal {
    #[inline]
    fn from(value: DateTime) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DATE_TIME)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Time> for Literal {
    #[inline]
    fn from(value: Time) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::TIME)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Date> for Literal {
    #[inline]
    fn from(value: Date) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DATE)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GYearMonth> for Literal {
    #[inline]
    fn from(value: GYearMonth) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_YEAR_MONTH)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GYear> for Literal {
    #[inline]
    fn from(value: GYear) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_YEAR)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GMonthDay> for Literal {
    #[inline]
    fn from(value: GMonthDay) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_MONTH_DAY)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GMonth> for Literal {
    #[inline]
    fn from(value: GMonth) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_MONTH)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<GDay> for Literal {
    #[inline]
    fn from(value: GDay) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_DAY)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<Duration> for Literal {
    #[inline]
    fn from(value: Duration) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DURATION)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<YearMonthDuration> for Literal {
    #[inline]
    fn from(value: YearMonthDuration) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::YEAR_MONTH_DURATION)
    }
}

#[cfg(feature = "oxsdatatypes")]
impl From<DayTimeDuration> for Literal {
    #[inline]
    fn from(value: DayTimeDuration) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DAY_TIME_DURATION)
    }
}

/// A borrowed RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// use oxrdf::vocab::xsd;
/// use oxrdf::LiteralRef;
///
/// assert_eq!(
///     "\"foo\\nbar\"",
///     LiteralRef::new_simple_literal("foo\nbar").to_string()
/// );
///
/// assert_eq!(
///     r#""1999-01-01"^^<http://www.w3.org/2001/XMLSchema#date>"#,
///     LiteralRef::new_typed_literal("1999-01-01", xsd::DATE).to_string()
/// );
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct LiteralRef<'a>(LiteralRefContent<'a>);

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
enum LiteralRefContent<'a> {
    String(&'a str),
    LanguageTaggedString {
        value: &'a str,
        language: &'a str,
    },
    TypedLiteral {
        value: &'a str,
        datatype: NamedNodeRef<'a>,
    },
}

impl<'a> LiteralRef<'a> {
    /// Builds an RDF [simple literal](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal).
    #[inline]
    pub const fn new_simple_literal(value: &'a str) -> Self {
        LiteralRef(LiteralRefContent::String(value))
    }

    /// Builds an RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) with a [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    #[inline]
    pub fn new_typed_literal(value: &'a str, datatype: impl Into<NamedNodeRef<'a>>) -> Self {
        let datatype = datatype.into();
        LiteralRef(if datatype == xsd::STRING {
            LiteralRefContent::String(value)
        } else {
            LiteralRefContent::TypedLiteral { value, datatype }
        })
    }

    /// Builds an RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    ///
    /// It is the responsibility of the caller to check that `language`
    /// is valid [BCP47](https://tools.ietf.org/html/bcp47) language tag,
    /// and is lowercase.
    ///
    /// [`Literal::new_language_tagged_literal()`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub const fn new_language_tagged_literal_unchecked(value: &'a str, language: &'a str) -> Self {
        LiteralRef(LiteralRefContent::LanguageTaggedString { value, language })
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form)
    #[inline]
    pub const fn value(self) -> &'a str {
        match self.0 {
            LiteralRefContent::String(value)
            | LiteralRefContent::LanguageTaggedString { value, .. }
            | LiteralRefContent::TypedLiteral { value, .. } => value,
        }
    }

    /// The literal [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag) if it is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    ///
    /// Language tags are defined by the [BCP47](https://tools.ietf.org/html/bcp47).
    /// They are normalized to lowercase by this implementation.
    #[inline]
    pub const fn language(self) -> Option<&'a str> {
        match self.0 {
            LiteralRefContent::LanguageTaggedString { language, .. } => Some(language),
            _ => None,
        }
    }

    /// The literal [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    ///
    /// The datatype of [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string) is always [rdf:langString](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    /// The datatype of [simple literals](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal) is [xsd:string](https://www.w3.org/TR/xmlschema11-2/#string).
    #[inline]
    pub const fn datatype(self) -> NamedNodeRef<'a> {
        match self.0 {
            LiteralRefContent::String(_) => xsd::STRING,
            LiteralRefContent::LanguageTaggedString { .. } => rdf::LANG_STRING,
            LiteralRefContent::TypedLiteral { datatype, .. } => datatype,
        }
    }

    /// Checks if this literal could be seen as an RDF 1.0 [plain literal](https://www.w3.org/TR/2004/REC-rdf-concepts-20040210/#dfn-plain-literal).
    ///
    /// It returns true if the literal is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    /// or has the datatype [xsd:string](https://www.w3.org/TR/xmlschema11-2/#string).
    #[inline]
    pub const fn is_plain(self) -> bool {
        matches!(
            self.0,
            LiteralRefContent::String(_) | LiteralRefContent::LanguageTaggedString { .. }
        )
    }

    #[inline]
    pub fn into_owned(self) -> Literal {
        Literal(match self.0 {
            LiteralRefContent::String(value) => LiteralContent::String(value.to_owned()),
            LiteralRefContent::LanguageTaggedString { value, language } => {
                LiteralContent::LanguageTaggedString {
                    value: value.to_owned(),
                    language: language.to_owned(),
                }
            }
            LiteralRefContent::TypedLiteral { value, datatype } => LiteralContent::TypedLiteral {
                value: value.to_owned(),
                datatype: datatype.into_owned(),
            },
        })
    }

    /// Extract components from this literal
    #[inline]
    pub const fn destruct(self) -> (&'a str, Option<NamedNodeRef<'a>>, Option<&'a str>) {
        match self.0 {
            LiteralRefContent::String(s) => (s, None, None),
            LiteralRefContent::LanguageTaggedString { value, language } => {
                (value, None, Some(language))
            }
            LiteralRefContent::TypedLiteral { value, datatype } => (value, Some(datatype), None),
        }
    }
}

impl fmt::Display for LiteralRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            LiteralRefContent::String(value) => print_quoted_str(value, f),
            LiteralRefContent::LanguageTaggedString { value, language } => {
                print_quoted_str(value, f)?;
                write!(f, "@{language}")
            }
            LiteralRefContent::TypedLiteral { value, datatype } => {
                print_quoted_str(value, f)?;
                write!(f, "^^{datatype}")
            }
        }
    }
}

impl<'a> From<&'a Literal> for LiteralRef<'a> {
    #[inline]
    fn from(node: &'a Literal) -> Self {
        node.as_ref()
    }
}

impl<'a> From<LiteralRef<'a>> for Literal {
    #[inline]
    fn from(node: LiteralRef<'a>) -> Self {
        node.into_owned()
    }
}

impl<'a> From<&'a str> for LiteralRef<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        LiteralRef(LiteralRefContent::String(value))
    }
}

impl PartialEq<Literal> for LiteralRef<'_> {
    #[inline]
    fn eq(&self, other: &Literal) -> bool {
        *self == other.as_ref()
    }
}

impl PartialEq<LiteralRef<'_>> for Literal {
    #[inline]
    fn eq(&self, other: &LiteralRef<'_>) -> bool {
        self.as_ref() == *other
    }
}

#[inline]
pub fn print_quoted_str(string: &str, f: &mut impl Write) -> fmt::Result {
    f.write_char('"')?;
    for c in string.chars() {
        match c {
            '\u{08}' => f.write_str("\\b"),
            '\t' => f.write_str("\\t"),
            '\n' => f.write_str("\\n"),
            '\u{0C}' => f.write_str("\\f"),
            '\r' => f.write_str("\\r"),
            '"' => f.write_str("\\\""),
            '\\' => f.write_str("\\\\"),
            '\0'..='\u{1F}' | '\u{7F}' => write!(f, "\\u{:04X}", u32::from(c)),
            _ => f.write_char(c),
        }?;
    }
    f.write_char('"')
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    #[cfg(feature = "serde")]
    use serde::de::DeserializeOwned;

    #[test]
    fn test_simple_literal_equality() {
        assert_eq!(
            Literal::new_simple_literal("foo"),
            Literal::new_typed_literal("foo", xsd::STRING)
        );
        assert_eq!(
            Literal::new_simple_literal("foo"),
            LiteralRef::new_typed_literal("foo", xsd::STRING)
        );
        assert_eq!(
            LiteralRef::new_simple_literal("foo"),
            Literal::new_typed_literal("foo", xsd::STRING)
        );
        assert_eq!(
            LiteralRef::new_simple_literal("foo"),
            LiteralRef::new_typed_literal("foo", xsd::STRING)
        );
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
        let j = serde_json::to_string(&Literal::new_simple_literal("foo")).unwrap();
        assert_eq!("{\"value\":\"foo\"}", j);

        let mut de = serde_json::Deserializer::from_str(&j);
        let node: Literal = Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(node, Literal::new_simple_literal("foo"));

        // Typed literal
        let j = serde_json::to_string(&Literal::new_typed_literal("true", xsd::BOOLEAN)).unwrap();

        assert_eq!(
            "{\"value\":\"true\",\"datatype\":\"http://www.w3.org/2001/XMLSchema#boolean\"}",
            j
        );

        let mut de = serde_json::Deserializer::from_str(&j);
        let node: Literal = Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(node, Literal::new_typed_literal("true", xsd::BOOLEAN));

        // Language-tagged string
        let j = serde_json::to_string(&Literal::new_language_tagged_literal("foo", "en").unwrap())
            .unwrap();
        assert_eq!("{\"value\":\"foo\",\"language\":\"en\"}", j);
        let mut de = serde_json::Deserializer::from_str(&j);
        let node: Literal = Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(node, Literal::new_language_tagged_literal("foo", "en").unwrap());
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_from_reader() {
        // Simple literal
        let j = serde_json::to_string(&Literal::new_simple_literal("foo")).unwrap();
        assert_eq!("{\"value\":\"foo\"}", j);

        let mut de = serde_json::Deserializer::from_reader(j.as_bytes());
        let node: Literal = Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(node, Literal::new_simple_literal("foo"));

        // Typed literal
        let j = serde_json::to_string(&Literal::new_typed_literal("true", xsd::BOOLEAN)).unwrap();

        assert_eq!(
            "{\"value\":\"true\",\"datatype\":\"http://www.w3.org/2001/XMLSchema#boolean\"}",
            j
        );

        let mut de = serde_json::Deserializer::from_reader(j.as_bytes());
        let node: Literal = Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(node, Literal::new_typed_literal("true", xsd::BOOLEAN));

        // Language-tagged string
        let j = serde_json::to_string(&Literal::new_language_tagged_literal("foo", "en").unwrap())
            .unwrap();
        assert_eq!("{\"value\":\"foo\",\"language\":\"en\"}", j);
        let mut de = serde_json::Deserializer::from_reader(j.as_bytes());
        let node: Literal = Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(node, Literal::new_language_tagged_literal("foo", "en").unwrap());
    }

    // Test for serde validation errors
    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_validation() {
        let j = r#"{"value":"true","datatype":"boo"}"#;
        let mut de = serde_json::Deserializer::from_str(j);
        let deserialized = Literal::deserialize(&mut de);

        if cfg!(feature = "serde-unvalidated") {
            assert!(deserialized.is_ok());
        } else {
            assert!(deserialized.is_err());
        }

        let j = r#"{"value":"true","language":"bo2"}"#;
        let mut de = serde_json::Deserializer::from_str(j);
        let deserialized = Literal::deserialize(&mut de);

        if cfg!(feature = "serde-unvalidated") {
            assert!(deserialized.is_ok());
        } else {
            assert!(deserialized.is_err());
        }
    }

    // This helper function will only compile if T implements DeserializeOwned.
    #[cfg(feature = "serde")]
    fn assert_deserialize_owned<T: DeserializeOwned>() {}

    #[test]
    #[cfg(feature = "serde")]
    fn test_literal_deserialize_owned() {
        // If Literal does not implement DeserializeOwned, this call will fail to compile.
        assert_deserialize_owned::<Literal>();
    }
}
