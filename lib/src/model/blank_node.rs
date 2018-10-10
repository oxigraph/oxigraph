use std::fmt;
use uuid::Uuid;

/// A RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
///
/// This implementation enforces that the blank node id is an UUID to easily ensure
/// that it is not possible for two blank nodes to share an id.
///
/// The common way to create a new blank node is to use the `Default::default` trait method.
///
/// The default string formatter is returning a N-Triples, Turtle and SPARQL compatible representation.
/// `BlankNode::default().to_string()` should return something like `_:00112233445566778899aabbccddeeff`
///
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct BlankNode {
    id: Uuid,
}

impl BlankNode {
    /// Returns the underlying UUID of this blank node
    pub fn as_uuid(&self) -> &Uuid {
        &self.id
    }
}

impl fmt::Display for BlankNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "_:{}", self.id.to_simple())
    }
}

impl Default for BlankNode {
    /// Builds a new RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) with a unique id
    fn default() -> Self {
        Self { id: Uuid::new_v4() }
    }
}

impl From<Uuid> for BlankNode {
    fn from(id: Uuid) -> Self {
        Self { id }
    }
}
