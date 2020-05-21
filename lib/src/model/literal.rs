use crate::model::named_node::NamedNode;
use crate::model::vocab::rdf;
use crate::model::vocab::xsd;
use crate::model::xsd::*;
use crate::Result;
use oxilangtag::LanguageTag;
use rio_api::model as rio;
use std::borrow::Cow;
use std::fmt;
use std::option::Option;

/// A RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal)
///
/// The default string formatter is returning a N-Triples, Turtle and SPARQL compatible representation:
/// ```
/// # use oxigraph::Result;
/// use oxigraph::model::Literal;
/// use oxigraph::model::vocab::xsd;
///
/// assert_eq!(
///     "\"foo\\nbar\"",
///     Literal::new_simple_literal("foo\nbar").to_string()
/// );
///
/// assert_eq!(
///     "\"1999-01-01\"^^<http://www.w3.org/2001/XMLSchema#date>",
///     Literal::new_typed_literal("1999-01-01", xsd::DATE.clone()).to_string()
/// );
///
/// assert_eq!(
///     "\"foo\"@en",
///     Literal::new_language_tagged_literal("foo", "en")?.to_string()
/// );
/// # Result::Ok(())
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Literal(LiteralContent);

#[derive(PartialEq, Eq, Ord, PartialOrd, Debug, Clone, Hash)]
enum LiteralContent {
    String(String),
    LanguageTaggedString { value: String, language: String },
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
        Literal(if datatype == *xsd::STRING {
            LiteralContent::String(value)
        } else {
            LiteralContent::TypedLiteral { value, datatype }
        })
    }

    /// Builds a RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    pub fn new_language_tagged_literal(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Result<Self> {
        let mut language = language.into();
        language.make_ascii_lowercase();
        Ok(Literal(LiteralContent::LanguageTaggedString {
            value: value.into(),
            language: LanguageTag::parse(language)?.into_inner(),
        }))
    }

    /// Builds a RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    ///
    /// It is the responsibility of the caller to check that `language`
    /// is valid [BCP47](https://tools.ietf.org/html/bcp47) language tag,
    /// and is lowercase.
    ///
    /// Except if you really know what you do,
    /// you should use [`new_language_tagged_literal`](#method.new_language_tagged_literal).
    pub fn new_language_tagged_literal_unchecked(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Literal(LiteralContent::LanguageTaggedString {
            value: value.into(),
            language: language.into(),
        })
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form)
    pub fn value(&self) -> &str {
        match &self.0 {
            LiteralContent::String(value)
            | LiteralContent::LanguageTaggedString { value, .. }
            | LiteralContent::TypedLiteral { value, .. } => value,
        }
    }

    /// The literal [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag) if it is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    ///
    /// Language tags are defined by the [BCP47](https://tools.ietf.org/html/bcp47).
    /// They are normalized to lowercase by this implementation.
    pub fn language(&self) -> Option<&str> {
        match &self.0 {
            LiteralContent::LanguageTaggedString { language, .. } => Some(language),
            _ => None,
        }
    }

    /// The literal [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    ///
    /// The datatype of [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string) is always [rdf:langString](http://www.w3.org/1999/02/22-rdf-syntax-ns#langString).
    /// The datatype of [simple literals](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal) is [xsd:string](http://www.w3.org/2001/XMLSchema#string).
    pub fn datatype(&self) -> &NamedNode {
        match &self.0 {
            LiteralContent::String(_) => &xsd::STRING,
            LiteralContent::LanguageTaggedString { .. } => &rdf::LANG_STRING,
            LiteralContent::TypedLiteral { datatype, .. } => datatype,
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

    /// Extract components from this literal
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        rio::Literal::from(self).fmt(f)
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
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::BOOLEAN.clone(),
        })
    }
}

impl From<i128> for Literal {
    fn from(value: i128) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.clone(),
        })
    }
}

impl From<i64> for Literal {
    fn from(value: i64) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.clone(),
        })
    }
}

impl From<i32> for Literal {
    fn from(value: i32) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.clone(),
        })
    }
}

impl From<i16> for Literal {
    fn from(value: i16) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.clone(),
        })
    }
}

impl From<u64> for Literal {
    fn from(value: u64) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.clone(),
        })
    }
}

impl From<u32> for Literal {
    fn from(value: u32) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.clone(),
        })
    }
}

impl From<u16> for Literal {
    fn from(value: u16) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.clone(),
        })
    }
}

impl From<f32> for Literal {
    fn from(value: f32) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::FLOAT.clone(),
        })
    }
}

impl From<f64> for Literal {
    fn from(value: f64) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::DOUBLE.clone(),
        })
    }
}

impl From<Decimal> for Literal {
    fn from(value: Decimal) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::DECIMAL.clone(),
        })
    }
}

impl From<Date> for Literal {
    fn from(value: Date) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::DATE.clone(),
        })
    }
}

impl From<Time> for Literal {
    fn from(value: Time) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::TIME.clone(),
        })
    }
}

impl From<DateTime> for Literal {
    fn from(value: DateTime) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::DATE_TIME.clone(),
        })
    }
}

impl From<Duration> for Literal {
    fn from(value: Duration) -> Self {
        Literal(LiteralContent::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::DURATION.clone(),
        })
    }
}

impl<'a> From<&'a Literal> for rio::Literal<'a> {
    fn from(literal: &'a Literal) -> Self {
        if literal.is_plain() {
            literal.language().map_or_else(
                || rio::Literal::Simple {
                    value: literal.value(),
                },
                |lang| rio::Literal::LanguageTaggedString {
                    value: literal.value(),
                    language: lang,
                },
            )
        } else {
            rio::Literal::Typed {
                value: literal.value(),
                datatype: literal.datatype().into(),
            }
        }
    }
}
