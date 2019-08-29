use rio_api::model as rio;
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
    uuid: Uuid,
    id: String,
}

impl BlankNode {
    /// Returns the underlying ID of this blank node
    pub fn as_str(&self) -> &str {
        &self.id
    }

    /// Returns the underlying UUID of this blank node
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }
}

impl fmt::Display for BlankNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        rio::BlankNode::from(self).fmt(f)
    }
}

impl Default for BlankNode {
    /// Builds a new RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) with a unique id
    fn default() -> Self {
        Self::from(Uuid::new_v4())
    }
}

impl From<Uuid> for BlankNode {
    fn from(id: Uuid) -> Self {
        Self {
            uuid: id,
            id: id.to_simple().to_string(),
        }
    }
}

impl<'a> From<&'a BlankNode> for rio::BlankNode<'a> {
    fn from(node: &'a BlankNode) -> Self {
        rio::BlankNode { id: node.as_str() }
    }
}
