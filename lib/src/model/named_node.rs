use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use url::Url;
use Error;
use Result;

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
    iri: Arc<Url>,
}

impl fmt::Display for NamedNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<{}>", self.iri)
    }
}

impl NamedNode {
    /// Builds a RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri)
    pub fn new(iri: impl Into<Url>) -> Self {
        Self {
            iri: Arc::new(iri.into()),
        }
    }

    pub fn as_str(&self) -> &str {
        self.iri.as_str()
    }

    pub fn as_url(&self) -> &Url {
        &self.iri
    }
}

impl From<Url> for NamedNode {
    fn from(url: Url) -> Self {
        Self { iri: Arc::new(url) }
    }
}

impl From<NamedNode> for Url {
    fn from(named_node: NamedNode) -> Self {
        Arc::try_unwrap(named_node.iri).unwrap_or_else(|iri| (*iri).clone())
    }
}

impl FromStr for NamedNode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(NamedNode::new(Url::parse(s)?))
    }
}
