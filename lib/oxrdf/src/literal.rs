use crate::named_node::NamedNode;
use crate::vocab::rdf;
use crate::vocab::xsd;
use crate::NamedNodeRef;
use oxilangtag::{LanguageTag, LanguageTagParseError};
#[cfg(feature = "oxsdatatypes")]
use oxsdatatypes::*;
use std::borrow::Cow;
use std::fmt;
use std::fmt::Write;
use std::option::Option;

/// An owned RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// # use oxilangtag::LanguageTagParseError;
/// use oxrdf::Literal;
/// use oxrdf::vocab::xsd;
///
/// assert_eq!(
///     "\"foo\\nbar\"",
///     Literal::new_simple_literal("foo\nbar").to_string()
/// );
///
/// assert_eq!(
///     "\"1999-01-01\"^^<http://www.w3.org/2001/XMLSchema#date>",
///     Literal::new_typed_literal("1999-01-01", xsd::DATE).to_string()
/// );
///
/// assert_eq!(
///     "\"foo\"@en",
///     Literal::new_language_tagged_literal("foo", "en")?.to_string()
/// );
/// # Result::<(), LanguageTagParseError>::Ok(())
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
/// use oxrdf::LiteralRef;
/// use oxrdf::vocab::xsd;
///
/// assert_eq!(
///     "\"foo\\nbar\"",
///     LiteralRef::new_simple_literal("foo\nbar").to_string()
/// );
///
/// assert_eq!(
///     "\"1999-01-01\"^^<http://www.w3.org/2001/XMLSchema#date>",
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
    pub fn new_simple_literal(value: &'a str) -> Self {
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
    pub fn new_language_tagged_literal_unchecked(value: &'a str, language: &'a str) -> Self {
        LiteralRef(LiteralRefContent::LanguageTaggedString { value, language })
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form)
    #[inline]
    pub fn value(self) -> &'a str {
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
    pub fn language(self) -> Option<&'a str> {
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
    pub fn datatype(self) -> NamedNodeRef<'a> {
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
    pub fn is_plain(self) -> bool {
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
    pub fn destruct(self) -> (&'a str, Option<NamedNodeRef<'a>>, Option<&'a str>) {
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
            '\u{0c}' => f.write_str("\\f"),
            '\r' => f.write_str("\\r"),
            '"' => f.write_str("\\\""),
            '\\' => f.write_str("\\\\"),
            '\0'..='\u{1f}' | '\u{7f}' => write!(f, "\\u{:04X}", u32::from(c)),
            c => f.write_char(c),
        }?;
    }
    f.write_char('"')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
    fn test_canoincal_escaping() {
        assert_eq!(
            Literal::from_str(r#""\u0000\u0001\u0002\u0003\u0004\u0005\u0006\u0007\u0008\u0009\u000a\u000b\u000c\u000d\u000e\u000f""#).unwrap().to_string(),
            r###""\u0000\u0001\u0002\u0003\u0004\u0005\u0006\u0007\b\t\n\u000B\f\r\u000E\u000F""###
        );
        assert_eq!(
            Literal::from_str(r#""\u0010\u0011\u0012\u0013\u0014\u0015\u0016\u0017\u0018\u0019\u001a\u001b\u001c\u001d\u001e\u001f""#).unwrap().to_string(),
            r###""\u0010\u0011\u0012\u0013\u0014\u0015\u0016\u0017\u0018\u0019\u001A\u001B\u001C\u001D\u001E\u001F""###
        );
        assert_eq!(
            Literal::from_str(r#""\u0020\u0021\u0022\u0023\u0024\u0025\u0026\u0027\u0028\u0029\u002a\u002b\u002c\u002d\u002e\u002f""#).unwrap().to_string(),
            r###"" !\"#$%&'()*+,-./""###
        );
        assert_eq!(
            Literal::from_str(r#""\u0030\u0031\u0032\u0033\u0034\u0035\u0036\u0037\u0038\u0039\u003a\u003b\u003c\u003d\u003e\u003f""#).unwrap().to_string(),
            r###""0123456789:;<=>?""###
        );
        assert_eq!(
            Literal::from_str(r#""\u0040\u0041\u0042\u0043\u0044\u0045\u0046\u0047\u0048\u0049\u004a\u004b\u004c\u004d\u004e\u004f""#).unwrap().to_string(),
            r###""@ABCDEFGHIJKLMNO""###
        );
        assert_eq!(
            Literal::from_str(r#""\u0050\u0051\u0052\u0053\u0054\u0055\u0056\u0057\u0058\u0059\u005a\u005b\u005c\u005d\u005e\u005f""#).unwrap().to_string(),
            r###""PQRSTUVWXYZ[\\]^_""###
        );
        assert_eq!(
            Literal::from_str(r#""\u0060\u0061\u0062\u0063\u0064\u0065\u0066\u0067\u0068\u0069\u006a\u006b\u006c\u006d\u006e\u006f""#).unwrap().to_string(),
            r###""`abcdefghijklmno""###
        );
        assert_eq!(
            Literal::from_str(r#""\u0070\u0071\u0072\u0073\u0074\u0075\u0076\u0077\u0078\u0079\u007a\u007b\u007c\u007d\u007e\u007f""#).unwrap().to_string(),
            r###""pqrstuvwxyz{|}~\u007F""###
        );
        assert_eq!(
            Literal::from_str(r#""\u0080\u0081\u0082\u0083\u0084\u0085\u0086\u0087\u0088\u0089\u008a\u008b\u008c\u008d\u008e\u008f""#).unwrap().to_string(),
            "\"\u{80}\u{81}\u{82}\u{83}\u{84}\u{85}\u{86}\u{87}\u{88}\u{89}\u{8a}\u{8b}\u{8c}\u{8d}\u{8e}\u{8f}\""
        );
    }
}
