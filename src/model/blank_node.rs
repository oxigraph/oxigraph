use std::fmt;
use std::ops::Deref;
use uuid::Uuid;

/// A RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node)
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct BlankNode {
    id: Uuid,
}

impl Deref for BlankNode {
    type Target = Uuid;

    fn deref(&self) -> &Uuid {
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
        BlankNode { id: Uuid::new_v4() }
    }
}

impl From<Uuid> for BlankNode {
    fn from(id: Uuid) -> Self {
        Self { id }
    }
}
