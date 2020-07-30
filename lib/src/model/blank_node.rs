use rand::random;
use rio_api::model as rio;
use std::error::Error;
use std::fmt;
use std::io::Write;
use std::str;

/// An RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
///
/// The common way to create a new blank node is to use the `BlankNode::default` trait method.
///
/// It is also possible to create a blank node from a blank node identifier using the `BlankNode::new` method.
/// The blank node identifier must be valid according to N-Triples, Turtle and SPARQL grammars.
///
/// The default string formatter is returning a N-Triples, Turtle and SPARQL compatible representation:
/// ```
/// use oxigraph::model::BlankNode;
///
/// assert_eq!(
///     "_:a122",
///     BlankNode::new("a122")?.to_string()
/// );
/// # Result::<_,oxigraph::model::BlankNodeIdParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct BlankNode(BlankNodeContent);

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
enum BlankNodeContent {
    Named(String),
    Anonymous { id: u128, str: [u8; 32] },
}

impl BlankNode {
    /// Creates a blank node from a unique identifier.
    ///
    /// The blank node identifier must be valid according to N-Triples, Turtle and SPARQL grammars.
    ///
    /// In most cases, it is much more convenient to create a blank node using `BlankNode::default()`.
    /// `BlankNode::default()` creates a random ID that could be easily inlined by Oxigraph stores.
    pub fn new(id: impl Into<String>) -> Result<Self, BlankNodeIdParseError> {
        let id = id.into();
        validate_blank_node_identifier(&id)?;
        Ok(Self::new_unchecked(id))
    }

    /// Creates a blank node from a unique identifier without validation.
    ///
    /// It is the caller's responsibility to ensure that `id` is a valid blank node identifier
    /// according to N-Triples, Turtle and SPARQL grammars.
    ///
    /// Except if you really know what you do, you should use [`new`](#method.new).
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        let id = id.into();
        if let Ok(numerical_id) = u128::from_str_radix(&id, 16) {
            let result = Self::new_from_unique_id(numerical_id);
            if result.as_str() == id {
                result
            } else {
                Self(BlankNodeContent::Named(id))
            }
        } else {
            Self(BlankNodeContent::Named(id))
        }
    }

    /// Creates a blank node from a unique numerical id
    ///
    /// In most cases, it is much more convenient to create a blank node using `BlankNode::default()`.
    pub fn new_from_unique_id(id: impl Into<u128>) -> Self {
        let id = id.into();
        let mut str = [0; 32];
        write!(&mut str[..], "{:x}", id).unwrap();
        Self(BlankNodeContent::Anonymous { id, str })
    }

    /// Returns the underlying ID of this blank node
    pub fn as_str(&self) -> &str {
        match &self.0 {
            BlankNodeContent::Named(id) => id,
            BlankNodeContent::Anonymous { str, .. } => {
                let len = str.iter().position(|x| x == &0).unwrap_or(32);
                str::from_utf8(&str[..len]).unwrap()
            }
        }
    }

    /// Returns the underlying ID of this blank node
    pub fn into_string(self) -> String {
        match self.0 {
            BlankNodeContent::Named(id) => id,
            BlankNodeContent::Anonymous { str, .. } => {
                let len = str.iter().position(|x| x == &0).unwrap_or(32);
                str::from_utf8(&str[..len]).unwrap().to_owned()
            }
        }
    }

    /// Returns the internal numerical ID of this blank node, if it exists
    pub(crate) fn id(&self) -> Option<u128> {
        match self.0 {
            BlankNodeContent::Named(_) => None,
            BlankNodeContent::Anonymous { id, .. } => Some(id),
        }
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

fn validate_blank_node_identifier(id: &str) -> Result<(), BlankNodeIdParseError> {
    let mut chars = id.chars();
    let front = chars.next().ok_or(BlankNodeIdParseError {})?;
    match front {
        '0'..='9'
        | '_'
        | ':'
        | 'A'..='Z'
        | 'a'..='z'
        | '\u{00C0}'..='\u{00D6}'
        | '\u{00D8}'..='\u{00F6}'
        | '\u{00F8}'..='\u{02FF}'
        | '\u{0370}'..='\u{037D}'
        | '\u{037F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}'
        | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}' => (),
        _ => return Err(BlankNodeIdParseError {}),
    }
    for c in chars {
        match c {
            '.' // validated later
            | '-'
            | '0'..='9'
            | '\u{00B7}'
            | '\u{0300}'..='\u{036F}'
            | '\u{203F}'..='\u{2040}'
            | '_'
            | ':'
            | 'A'..='Z'
            | 'a'..='z'
            | '\u{00C0}'..='\u{00D6}'
            | '\u{00D8}'..='\u{00F6}'
            | '\u{00F8}'..='\u{02FF}'
            | '\u{0370}'..='\u{037D}'
            | '\u{037F}'..='\u{1FFF}'
            | '\u{200C}'..='\u{200D}'
            | '\u{2070}'..='\u{218F}'
            | '\u{2C00}'..='\u{2FEF}'
            | '\u{3001}'..='\u{D7FF}'
            | '\u{F900}'..='\u{FDCF}'
            | '\u{FDF0}'..='\u{FFFD}'
            | '\u{10000}'..='\u{EFFFF}' => (),
            _ => return Err(BlankNodeIdParseError {}),
        }
    }

    // Could not end with a dot
    if id.ends_with('.') {
        Err(BlankNodeIdParseError {})
    } else {
        Ok(())
    }
}

/// An error raised during `BlankNode` validation.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct BlankNodeIdParseError {}

impl fmt::Display for BlankNodeIdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "The blank node identifier is invalid")
    }
}

impl Error for BlankNodeIdParseError {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn as_str_partial() {
        let b = BlankNode::new_from_unique_id(0x42_u128);
        assert_eq!(b.as_str(), "42");
    }

    #[test]
    fn as_str_full() {
        let b = BlankNode::new_from_unique_id(0x7777_6666_5555_4444_3333_2222_1111_0000_u128);
        assert_eq!(b.as_str(), "77776666555544443333222211110000");
    }

    #[test]
    fn new_validation() {
        assert!(BlankNode::new("").is_err());
        assert!(BlankNode::new("a").is_ok());
        assert!(BlankNode::new("-").is_err());
        assert!(BlankNode::new("a-").is_ok());
        assert!(BlankNode::new(".").is_err());
        assert!(BlankNode::new("a.").is_err());
        assert!(BlankNode::new("a.a").is_ok());
    }

    #[test]
    fn new_numerical() {
        assert_eq!(
            BlankNode::new("100a").unwrap(),
            BlankNode::new_from_unique_id(0x100a_u128),
        );
        assert_ne!(
            BlankNode::new("100A").unwrap(),
            BlankNode::new_from_unique_id(0x100a_u128)
        );
    }
}
