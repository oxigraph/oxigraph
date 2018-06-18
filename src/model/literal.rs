use model::named_node::NamedNode;
use model::vocab::rdf;
use model::vocab::xsd;
use std::borrow::Cow;
use std::fmt;
use std::option::Option;
use utils::Escaper;

/// A RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal)
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Literal {
    SimpleLiteral(String),
    String(String),
    LanguageTaggedString { value: String, language: String },
    Boolean(bool),
    TypedLiteral { value: String, datatype: NamedNode },
}

impl Literal {
    /// Builds a RDF [simple literal](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal)
    pub fn new_simple_literal(value: impl Into<String>) -> Self {
        Literal::SimpleLiteral(value.into())
    }

    /// Builds a RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) with a [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri)
    pub fn new_typed_literal(value: impl Into<String>, datatype: impl Into<NamedNode>) -> Self {
        let value = value.into();
        let datatype = datatype.into();
        if datatype == *xsd::BOOLEAN {
            match value.as_str() {
                "true" | "1" => Literal::Boolean(true),
                "false" | "0" => Literal::Boolean(false),
                _ => Literal::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::STRING {
            Literal::String(value)
        } else {
            Literal::TypedLiteral { value, datatype }
        }
    }

    /// Builds a RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    pub fn new_language_tagged_literal(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Literal::LanguageTaggedString {
            value: value.into(),
            language: language.into(),
        }
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form)
    pub fn value<'a>(&'a self) -> Cow<'a, String> {
        match self {
            Literal::SimpleLiteral(value) => Cow::Borrowed(value),
            Literal::String(value) => Cow::Borrowed(value),
            Literal::LanguageTaggedString { value, .. } => Cow::Borrowed(value),
            Literal::Boolean(value) => Cow::Owned(value.to_string()),
            Literal::TypedLiteral { value, .. } => Cow::Borrowed(value),
        }
    }

    /// The literal [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag) if it is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    pub fn language(&self) -> Option<&str> {
        match self {
            Literal::LanguageTaggedString { language, .. } => Some(language),
            _ => None,
        }
    }

    /// The literal [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri)
    /// The datatype of [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string) is always http://www.w3.org/1999/02/22-rdf-syntax-ns#langString
    pub fn datatype(&self) -> &NamedNode {
        match self {
            Literal::SimpleLiteral(_) => &xsd::STRING,
            Literal::String(_) => &xsd::STRING,
            Literal::LanguageTaggedString { .. } => &rdf::LANG_STRING,
            Literal::Boolean(_) => &xsd::BOOLEAN,
            Literal::TypedLiteral { datatype, .. } => datatype,
        }
    }

    pub fn is_plain(&self) -> bool {
        match self {
            Literal::SimpleLiteral(_) => true,
            Literal::LanguageTaggedString { .. } => true,
            _ => false,
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
        Literal::String(value.into())
    }
}

impl From<String> for Literal {
    fn from(value: String) -> Self {
        Literal::String(value)
    }
}

impl From<bool> for Literal {
    fn from(value: bool) -> Self {
        Literal::Boolean(value)
    }
}

impl From<usize> for Literal {
    fn from(value: usize) -> Self {
        Literal::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::INTEGER.clone(),
        }
    }
}

impl From<f32> for Literal {
    fn from(value: f32) -> Self {
        Literal::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::FLOAT.clone(),
        }
    }
}

impl From<f64> for Literal {
    fn from(value: f64) -> Self {
        Literal::TypedLiteral {
            value: value.to_string(),
            datatype: xsd::DOUBLE.clone(),
        }
    }
}
