use oxstr::OxString;
use rand::random;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::io::Write;
use std::{fmt, str};

/// An owned RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
///
/// The common way to create a new blank node is to use the [`BlankNode::default`] function.
///
/// It is also possible to create a blank node from a blank node identifier using the [`BlankNode::new`] function.
/// The blank node identifier must be valid according to N-Triples, Turtle, and SPARQL grammars.
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// use oxrdf::BlankNode;
///
/// assert_eq!("_:a122", BlankNode::new("a122")?.to_string());
/// # Result::<_,oxrdf::BlankNodeIdParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct BlankNode {
    id: OxString,
}

impl BlankNode {
    /// Creates a blank node from a unique identifier.
    ///
    /// The blank node identifier must be valid according to N-Triples, Turtle, and SPARQL grammars.
    ///
    /// In most cases, it is much more convenient to create a blank node using [`BlankNode::default`]
    /// that creates a random ID that could be easily inlined by Oxigraph stores.
    pub fn new(id: impl Into<OxString>) -> Result<Self, BlankNodeIdParseError> {
        let id = id.into();
        validate_blank_node_identifier(&id)?;
        Ok(Self::new_unchecked(id))
    }

    /// Creates a blank node from a unique identifier without validation.
    ///
    /// It is the caller's responsibility to ensure that `id` is a valid blank node identifier
    /// according to N-Triples, Turtle, and SPARQL grammars.
    ///
    /// [`BlankNode::new`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub fn new_unchecked(id: impl Into<OxString>) -> Self {
        Self { id: id.into() }
    }

    /// Returns the underlying ID of this blank node.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.id.as_str()
    }

    /// Returns the underlying ID of this blank node.
    #[inline]
    pub fn into_string(self) -> OxString {
        self.id
    }
}

impl fmt::Display for BlankNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("_:")?;
        f.write_str(&self.id)
    }
}

impl Default for BlankNode {
    /// Builds a new RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) with a unique id.
    #[inline]
    fn default() -> Self {
        // We ensure the ID does not start with a number to be also valid with RDF/XML
        loop {
            let mut str = [0; 32];
            write!(&mut str[..], "{:x}", random::<u128>()).unwrap();
            if matches!(str[0], b'a'..=b'f') {
                let len = str.iter().position(|x| *x == 0).unwrap_or(32);
                return Self {
                    id: OxString::new_owned(str::from_utf8(&str[..len]).unwrap()),
                };
            }
        }
    }
}

fn validate_blank_node_identifier(id: &str) -> Result<(), BlankNodeIdParseError> {
    let mut chars = id.chars();
    let front = chars.next().ok_or(BlankNodeIdParseError)?;
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
        _ => return Err(BlankNodeIdParseError),
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
            _ => return Err(BlankNodeIdParseError),
        }
    }

    // Could not end with a dot
    if id.ends_with('.') {
        Err(BlankNodeIdParseError)
    } else {
        Ok(())
    }
}

/// An error raised during [`BlankNode`] IDs validation.
#[derive(Debug, thiserror::Error)]
#[error("The blank node identifier is invalid")]
pub struct BlankNodeIdParseError;

#[cfg(feature = "serde")]
impl Serialize for BlankNode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "BlankNode")]
        struct Value<'a> {
            value: &'a str,
        }
        Value {
            value: self.as_str(),
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for BlankNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename = "BlankNode")]
        struct Value {
            value: OxString,
        }
        Self::new(Value::deserialize(deserializer)?.value).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_validation() {
        BlankNode::new("").unwrap_err();
        BlankNode::new("a").unwrap();
        BlankNode::new("-").unwrap_err();
        BlankNode::new("a-").unwrap();
        BlankNode::new(".").unwrap_err();
        BlankNode::new("a.").unwrap_err();
        BlankNode::new("a.a").unwrap();
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde() {
        let b = BlankNode::new("123a").unwrap();
        let json = serde_json::to_string(&b).unwrap();
        assert_eq!(json, "{\"value\":\"123a\"}");
        let b2: BlankNode = serde_json::from_str(&json).unwrap();
        assert_eq!(b2, b);
    }
}
