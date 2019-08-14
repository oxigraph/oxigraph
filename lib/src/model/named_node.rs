use crate::Error;
use crate::Result;
use rio_api::model as rio;
use std::fmt;
use std::str::FromStr;

/// A RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri)
///
/// The common way to build it is to use the `FromStr::from_str` trait method.
/// This method takes care of usual IRI normalization and validation.
///
/// The default string formatter is returning a N-Triples, Turtle and SPARQL compatible representation:
/// ```
/// use rudf::model::NamedNode;
/// use std::str::FromStr;
///
/// assert_eq!(
///     "<http://example.com/foo>",
///     NamedNode::from_str("http://example.com/foo").unwrap().to_string()
/// )
/// ```
///
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct NamedNode {
    iri: String,
}

impl fmt::Display for NamedNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        rio::NamedNode {
            iri: self.iri.as_str(),
        }
        .fmt(f)
    }
}

impl NamedNode {
    /// Builds a RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri)
    pub fn new(iri: impl Into<String>) -> Self {
        Self { iri: iri.into() }
    }

    pub fn as_str(&self) -> &str {
        self.iri.as_str()
    }

    pub fn into_string(self) -> String {
        self.iri
    }
}

impl FromStr for NamedNode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self::new(s))
    }
}
