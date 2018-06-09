use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use url::ParseError;
use url::Url;

/// A RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri)
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

    pub fn value(&self) -> &str {
        self.iri.as_str()
    }

    pub fn url(&self) -> &Url {
        &self.iri
    }
}

impl Deref for NamedNode {
    type Target = Url;

    fn deref(&self) -> &Url {
        &self.iri
    }
}

impl From<Url> for NamedNode {
    fn from(url: Url) -> Self {
        Self { iri: Arc::new(url) }
    }
}

impl FromStr for NamedNode {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(NamedNode::new(Url::parse(s)?))
    }
}
