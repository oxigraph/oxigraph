use model::named_node::NamedNode;
use model::vocab::rdf;
use model::vocab::xsd;
use std::borrow::Cow;
use std::fmt;
use std::option::Option;
use utils::Escaper;

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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
enum LiteralContent {
    SimpleLiteral(String),
    String(String),
    LanguageTaggedString { value: String, language: String },
    Boolean(bool),
    TypedLiteral { value: String, datatype: NamedNode },
}

impl Literal {
    /// Builds a RDF [simple literal](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal)
    pub fn new_simple_literal(value: impl Into<String>) -> Self {
        Literal(LiteralContent::SimpleLiteral(value.into()))
    }

    /// Builds a RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) with a [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri)
    pub fn new_typed_literal(value: impl Into<String>, datatype: impl Into<NamedNode>) -> Self {
        let value = value.into();
        let datatype = datatype.into();
        if datatype == *xsd::BOOLEAN {
            match value.as_str() {
                "true" | "1" => Literal(LiteralContent::Boolean(true)),
                "false" | "0" => Literal(LiteralContent::Boolean(false)),
                _ => Literal(LiteralContent::TypedLiteral { value, datatype }),
            }
        } else if datatype == *xsd::STRING {
            Literal(LiteralContent::String(value))
        } else {
            Literal(LiteralContent::TypedLiteral { value, datatype })
        }
    }

    /// Builds a RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    pub fn new_language_tagged_literal(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Literal(LiteralContent::LanguageTaggedString {
            value: value.into(),
            language: language.into(),
        })
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form)
    pub fn value(&self) -> Cow<String> {
        match self.0 {
            LiteralContent::SimpleLiteral(ref value) => Cow::Borrowed(value),
            LiteralContent::String(ref value) => Cow::Borrowed(value),
            LiteralContent::LanguageTaggedString { ref value, .. } => Cow::Borrowed(value),
            LiteralContent::Boolean(value) => Cow::Owned(value.to_string()),
            LiteralContent::TypedLiteral { ref value, .. } => Cow::Borrowed(value),
        }
    }

    /// The literal [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag) if it is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    /// Language tags are defined by the [BCP47](https://tools.ietf.org/html/bcp47).
    pub fn language(&self) -> Option<&str> {
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
            LiteralContent::SimpleLiteral(_) => &xsd::STRING,
            LiteralContent::String(_) => &xsd::STRING,
            LiteralContent::LanguageTaggedString { .. } => &rdf::LANG_STRING,
            LiteralContent::Boolean(_) => &xsd::BOOLEAN,
            LiteralContent::TypedLiteral { ref datatype, .. } => datatype,
        }
    }

    /// Checks if it could be considered as an RDF 1.0 [plain literal](https://www.w3.org/TR/rdf-concepts/#dfn-plain-literal).
    ///
    /// It returns true if the literal is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    /// or have been created by `Literal::new_simple_literal`.
    pub fn is_plain(&self) -> bool {
        match self.0 {
            LiteralContent::SimpleLiteral(_) => true,
            LiteralContent::LanguageTaggedString { .. } => true,
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

    /// Checks if the literal has the datatype [xsd:boolean](http://www.w3.org/2001/XMLSchema#string) and is valid
    pub fn is_boolean(&self) -> bool {
        match self.0 {
            LiteralContent::Boolean(_) => true,
            _ => false,
        }
    }

    /// Returns the [effective boolean value](https://www.w3.org/TR/sparql11-query/#ebv) of the literal if it exists
    pub fn to_bool(&self) -> Option<bool> {
        //TODO: numeric literals
        match self.0 {
            LiteralContent::SimpleLiteral(ref value) => Some(!value.is_empty()),
            LiteralContent::String(ref value) => Some(!value.is_empty()),
            LiteralContent::LanguageTaggedString { .. } => None,
            LiteralContent::Boolean(value) => Some(value),
            LiteralContent::TypedLiteral { .. } => None,
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_plain() {
            self.language()
                .map(|lang| write!(f, "\"{}\"@{}", self.value().escape(), lang))
                .unwrap_or_else(|| write!(f, "\"{}\"", self.value().escape()))
        } else {
            write!(f, "\"{}\"^^{}", self.value().escape(), self.datatype())
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

impl From<bool> for Literal {
    fn from(value: bool) -> Self {
        Literal(LiteralContent::Boolean(value))
    }
}

impl From<usize> for Literal {
    fn from(value: usize) -> Self {
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
