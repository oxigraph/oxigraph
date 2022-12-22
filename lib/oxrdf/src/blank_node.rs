use rand::random;
use std::error::Error;
use std::fmt;
use std::io::Write;
use std::str;

/// An owned RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
///
/// The common way to create a new blank node is to use the [`BlankNode::default()`] function.
///
/// It is also possible to create a blank node from a blank node identifier using the [`BlankNode::new()`] function.
/// The blank node identifier must be valid according to N-Triples, Turtle, and SPARQL grammars.
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// use oxrdf::BlankNode;
///
/// assert_eq!(
///     "_:a122",
///     BlankNode::new("a122")?.to_string()
/// );
/// # Result::<_,oxrdf::BlankNodeIdParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct BlankNode(BlankNodeContent);

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
enum BlankNodeContent {
    Named(String),
    Anonymous { id: u128, str: IdStr },
}

impl BlankNode {
    /// Creates a blank node from a unique identifier.
    ///
    /// The blank node identifier must be valid according to N-Triples, Turtle, and SPARQL grammars.
    ///
    /// In most cases, it is much more convenient to create a blank node using [`BlankNode::default()`]
    ///that creates a random ID that could be easily inlined by Oxigraph stores.
    pub fn new(id: impl Into<String>) -> Result<Self, BlankNodeIdParseError> {
        let id = id.into();
        validate_blank_node_identifier(&id)?;
        Ok(Self::new_unchecked(id))
    }

    /// Creates a blank node from a unique identifier without validation.
    ///
    /// It is the caller's responsibility to ensure that `id` is a valid blank node identifier
    /// according to N-Triples, Turtle, and SPARQL grammars.
    ///
    /// [`BlankNode::new()`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        let id = id.into();
        if let Some(numerical_id) = to_integer_id(&id) {
            Self::new_from_unique_id(numerical_id)
        } else {
            Self(BlankNodeContent::Named(id))
        }
    }

    /// Creates a blank node from a unique numerical id.
    ///
    /// In most cases, it is much more convenient to create a blank node using [`BlankNode::default()`].
    #[inline]
    pub fn new_from_unique_id(id: u128) -> Self {
        Self(BlankNodeContent::Anonymous {
            id,
            str: IdStr::new(id),
        })
    }

    /// Returns the underlying ID of this blank node.
    #[inline]
    pub fn as_str(&self) -> &str {
        match &self.0 {
            BlankNodeContent::Named(id) => id,
            BlankNodeContent::Anonymous { str, .. } => str.as_str(),
        }
    }

    /// Returns the underlying ID of this blank node.
    #[inline]
    pub fn into_string(self) -> String {
        match self.0 {
            BlankNodeContent::Named(id) => id,
            BlankNodeContent::Anonymous { str, .. } => str.as_str().to_owned(),
        }
    }

    #[inline]
    pub fn as_ref(&self) -> BlankNodeRef<'_> {
        BlankNodeRef(match &self.0 {
            BlankNodeContent::Named(id) => BlankNodeRefContent::Named(id.as_str()),
            BlankNodeContent::Anonymous { id, str } => BlankNodeRefContent::Anonymous {
                id: *id,
                str: str.as_str(),
            },
        })
    }
}

impl fmt::Display for BlankNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl Default for BlankNode {
    /// Builds a new RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) with a unique id.
    #[inline]
    fn default() -> Self {
        Self::new_from_unique_id(random::<u128>())
    }
}

/// A borrowed RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
///
/// The common way to create a new blank node is to use the [`BlankNode::default`] trait method.
///
/// It is also possible to create a blank node from a blank node identifier using the [`BlankNodeRef::new()`] function.
/// The blank node identifier must be valid according to N-Triples, Turtle, and SPARQL grammars.
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// use oxrdf::BlankNodeRef;
///
/// assert_eq!(
///     "_:a122",
///     BlankNodeRef::new("a122")?.to_string()
/// );
/// # Result::<_,oxrdf::BlankNodeIdParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct BlankNodeRef<'a>(BlankNodeRefContent<'a>);

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
enum BlankNodeRefContent<'a> {
    Named(&'a str),
    Anonymous { id: u128, str: &'a str },
}

impl<'a> BlankNodeRef<'a> {
    /// Creates a blank node from a unique identifier.
    ///
    /// The blank node identifier must be valid according to N-Triples, Turtle, and SPARQL grammars.
    ///
    /// In most cases, it is much more convenient to create a blank node using [`BlankNode::default()`].
    /// that creates a random ID that could be easily inlined by Oxigraph stores.
    pub fn new(id: &'a str) -> Result<Self, BlankNodeIdParseError> {
        validate_blank_node_identifier(id)?;
        Ok(Self::new_unchecked(id))
    }

    /// Creates a blank node from a unique identifier without validation.
    ///
    /// It is the caller's responsibility to ensure that `id` is a valid blank node identifier
    /// according to N-Triples, Turtle, and SPARQL grammars.
    ///
    /// [`BlankNodeRef::new()`) is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub fn new_unchecked(id: &'a str) -> Self {
        if let Some(numerical_id) = to_integer_id(id) {
            Self(BlankNodeRefContent::Anonymous {
                id: numerical_id,
                str: id,
            })
        } else {
            Self(BlankNodeRefContent::Named(id))
        }
    }

    /// Returns the underlying ID of this blank node.
    #[inline]
    pub fn as_str(self) -> &'a str {
        match self.0 {
            BlankNodeRefContent::Named(id) => id,
            BlankNodeRefContent::Anonymous { str, .. } => str,
        }
    }

    /// Returns the internal numerical ID of this blank node if it has been created using [`BlankNode::new_from_unique_id`].
    ///
    /// ```
    /// use oxrdf::BlankNode;
    ///
    /// assert_eq!(BlankNode::new_from_unique_id(128).as_ref().unique_id(), Some(128));
    /// assert_eq!(BlankNode::new("foo")?.as_ref().unique_id(), None);
    /// # Result::<_,oxrdf::BlankNodeIdParseError>::Ok(())
    /// ```
    #[inline]
    pub fn unique_id(&self) -> Option<u128> {
        match self.0 {
            BlankNodeRefContent::Named(_) => None,
            BlankNodeRefContent::Anonymous { id, .. } => Some(id),
        }
    }

    #[inline]
    pub fn into_owned(self) -> BlankNode {
        BlankNode(match self.0 {
            BlankNodeRefContent::Named(id) => BlankNodeContent::Named(id.to_owned()),
            BlankNodeRefContent::Anonymous { id, .. } => BlankNodeContent::Anonymous {
                id,
                str: IdStr::new(id),
            },
        })
    }
}

impl fmt::Display for BlankNodeRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_:{}", self.as_str())
    }
}

impl<'a> From<&'a BlankNode> for BlankNodeRef<'a> {
    #[inline]
    fn from(node: &'a BlankNode) -> Self {
        node.as_ref()
    }
}

impl<'a> From<BlankNodeRef<'a>> for BlankNode {
    #[inline]
    fn from(node: BlankNodeRef<'a>) -> Self {
        node.into_owned()
    }
}

impl PartialEq<BlankNode> for BlankNodeRef<'_> {
    #[inline]
    fn eq(&self, other: &BlankNode) -> bool {
        *self == other.as_ref()
    }
}

impl PartialEq<BlankNodeRef<'_>> for BlankNode {
    #[inline]
    fn eq(&self, other: &BlankNodeRef<'_>) -> bool {
        self.as_ref() == *other
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
struct IdStr([u8; 32]);

impl IdStr {
    #[inline]
    fn new(id: u128) -> Self {
        let mut str = [0; 32];
        write!(&mut str[..], "{id:x}").unwrap();
        Self(str)
    }

    #[inline]
    fn as_str(&self) -> &str {
        let len = self.0.iter().position(|x| x == &0).unwrap_or(32);
        str::from_utf8(&self.0[..len]).unwrap()
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

#[inline]
fn to_integer_id(id: &str) -> Option<u128> {
    let digits = id.as_bytes();
    let mut value: u128 = 0;
    if let None | Some(b'0') = digits.first() {
        return None; // No empty string or leading zeros
    }
    for digit in digits {
        value = value.checked_mul(16)?.checked_add(
            match *digit {
                b'0'..=b'9' => digit - b'0',
                b'a'..=b'f' => digit - b'a' + 10,
                _ => return None,
            }
            .into(),
        )?;
    }
    Some(value)
}

/// An error raised during [`BlankNode`] IDs validation.
#[derive(Debug)]
pub struct BlankNodeIdParseError {}

impl fmt::Display for BlankNodeIdParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "The blank node identifier is invalid")
    }
}

impl Error for BlankNodeIdParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_partial() {
        let b = BlankNode::new_from_unique_id(0x42);
        assert_eq!(b.as_str(), "42");
    }

    #[test]
    fn as_str_full() {
        let b = BlankNode::new_from_unique_id(0x7777_6666_5555_4444_3333_2222_1111_0000);
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
            BlankNode::new_from_unique_id(0x100a),
        );
        assert_ne!(
            BlankNode::new("100A").unwrap(),
            BlankNode::new_from_unique_id(0x100a)
        );
    }

    #[test]
    fn test_equals() {
        assert_eq!(
            BlankNode::new("100a").unwrap(),
            BlankNodeRef::new("100a").unwrap()
        );
        assert_eq!(
            BlankNode::new("zzz").unwrap(),
            BlankNodeRef::new("zzz").unwrap()
        );
    }
}
