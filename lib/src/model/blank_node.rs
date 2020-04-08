use rand::random;
use rio_api::model as rio;
use std::fmt;
use std::io::Write;
use std::str;

/// A RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
///
/// This implementation enforces that the blank node id is a uniquely generated ID to easily ensure
/// that it is not possible for two blank nodes to share an id.
///
/// The common way to create a new blank node is to use the `Default::default` trait method.
///
/// The default string formatter is returning a N-Triples, Turtle and SPARQL compatible representation.
/// `BlankNode::default().to_string()` should return something like `_:00112233445566778899aabbccddeeff`
///
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct BlankNode {
    id: u128,
    str: [u8; 32],
}

impl BlankNode {
    /// Creates a blank node from a unique id
    ///
    /// In most cases, you **should not*** create a blank node this way,
    /// but should use `Default::default` instead.
    ///
    /// This method is only exposed for low-level library,
    /// in particular bindings to other languages or APIs.
    pub fn new_from_unique_id(id: u128) -> Self {
        let mut str = [0; 32];
        write!(&mut str[..], "{:x}", id).unwrap();
        Self { id, str }
    }

    /// Returns the underlying ID of this blank node
    pub fn as_str(&self) -> &str {
        let len = self.str.iter().position(|x| x == &0).unwrap_or(32);
        str::from_utf8(&self.str[..len]).unwrap()
    }

    /// Returns the internal ID of this blank node
    pub const fn id(&self) -> u128 {
        self.id
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
        Self::new_from_unique_id(random::<u128>())
    }
}

impl<'a> From<&'a BlankNode> for rio::BlankNode<'a> {
    fn from(node: &'a BlankNode) -> Self {
        rio::BlankNode { id: node.as_str() }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn as_str_partial() {
        let b = BlankNode::new_from_unique_id(0x42);
        assert_eq!(b.as_str(), "42");
    }

    #[test]
    fn as_str_full() {
        let b = BlankNode::new_from_unique_id(0x77776666555544443333222211110000);
        assert_eq!(b.as_str(), "77776666555544443333222211110000");
    }
}
