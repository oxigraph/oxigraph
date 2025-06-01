use oxiri::{Iri, IriParseError};
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::cmp::Ordering;
use std::fmt;

/// An owned RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// use oxrdf::NamedNode;
///
/// assert_eq!(
///     "<http://example.com/foo>",
///     NamedNode::new("http://example.com/foo")?.to_string()
/// );
/// # Result::<_,oxrdf::IriParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct NamedNode {
    iri: String,
}

impl NamedNode {
    /// Builds and validate an RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri).
    pub fn new(iri: impl Into<String>) -> Result<Self, IriParseError> {
        Ok(Self::new_from_iri(Iri::parse(iri.into())?))
    }

    #[inline]
    pub(crate) fn new_from_iri(iri: Iri<String>) -> Self {
        Self::new_unchecked(iri.into_inner())
    }

    /// Builds an RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) from a string.
    ///
    /// It is the caller's responsibility to ensure that `iri` is a valid IRI.
    ///
    /// [`NamedNode::new()`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub fn new_unchecked(iri: impl Into<String>) -> Self {
        Self { iri: iri.into() }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.iri.as_str()
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.iri
    }

    #[inline]
    pub fn as_ref(&self) -> NamedNodeRef<'_> {
        NamedNodeRef::new_unchecked(&self.iri)
    }
}

impl fmt::Display for NamedNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl PartialEq<str> for NamedNode {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<NamedNode> for str {
    #[inline]
    fn eq(&self, other: &NamedNode) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<&str> for NamedNode {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl PartialEq<NamedNode> for &str {
    #[inline]
    fn eq(&self, other: &NamedNode) -> bool {
        *self == other
    }
}

/// A borrowed RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// use oxrdf::NamedNodeRef;
///
/// assert_eq!(
///     "<http://example.com/foo>",
///     NamedNodeRef::new("http://example.com/foo")?.to_string()
/// );
/// # Result::<_,oxrdf::IriParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct NamedNodeRef<'a> {
    iri: &'a str,
}

impl<'a> NamedNodeRef<'a> {
    /// Builds and validate an RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri)
    pub fn new(iri: &'a str) -> Result<Self, IriParseError> {
        Ok(Self::new_from_iri(Iri::parse(iri)?))
    }

    #[inline]
    pub(crate) fn new_from_iri(iri: Iri<&'a str>) -> Self {
        Self::new_unchecked(iri.into_inner())
    }

    /// Builds an RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) from a string.
    ///
    /// It is the caller's responsibility to ensure that `iri` is a valid IRI.
    ///
    /// [`NamedNode::new()`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub const fn new_unchecked(iri: &'a str) -> Self {
        Self { iri }
    }

    #[inline]
    pub const fn as_str(self) -> &'a str {
        self.iri
    }

    #[inline]
    pub fn into_owned(self) -> NamedNode {
        NamedNode::new_unchecked(self.iri)
    }
}

impl fmt::Display for NamedNodeRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.as_str())
    }
}

impl From<NamedNodeRef<'_>> for NamedNode {
    #[inline]
    fn from(node: NamedNodeRef<'_>) -> Self {
        node.into_owned()
    }
}

impl<'a> From<&'a NamedNode> for NamedNodeRef<'a> {
    #[inline]
    fn from(node: &'a NamedNode) -> Self {
        node.as_ref()
    }
}

impl PartialEq<NamedNode> for NamedNodeRef<'_> {
    #[inline]
    fn eq(&self, other: &NamedNode) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<NamedNodeRef<'_>> for NamedNode {
    #[inline]
    fn eq(&self, other: &NamedNodeRef<'_>) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<str> for NamedNodeRef<'_> {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<NamedNodeRef<'_>> for str {
    #[inline]
    fn eq(&self, other: &NamedNodeRef<'_>) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<&str> for NamedNodeRef<'_> {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl PartialEq<NamedNodeRef<'_>> for &str {
    #[inline]
    fn eq(&self, other: &NamedNodeRef<'_>) -> bool {
        *self == other
    }
}

impl PartialOrd<NamedNode> for NamedNodeRef<'_> {
    #[inline]
    fn partial_cmp(&self, other: &NamedNode) -> Option<Ordering> {
        self.partial_cmp(&other.as_ref())
    }
}

impl PartialOrd<NamedNodeRef<'_>> for NamedNode {
    #[inline]
    fn partial_cmp(&self, other: &NamedNodeRef<'_>) -> Option<Ordering> {
        self.as_ref().partial_cmp(other)
    }
}

impl From<Iri<String>> for NamedNode {
    #[inline]
    fn from(iri: Iri<String>) -> Self {
        Self {
            iri: iri.into_inner(),
        }
    }
}

impl<'a> From<Iri<&'a str>> for NamedNodeRef<'a> {
    #[inline]
    fn from(iri: Iri<&'a str>) -> Self {
        Self {
            iri: iri.into_inner(),
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for NamedNode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_ref().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl Serialize for NamedNodeRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "NamedNode")]
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
impl<'de> Deserialize<'de> for NamedNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename = "NamedNode")]
        struct Value {
            value: String,
        }
        Self::new(Value::deserialize(deserializer)?.value).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_node_construction() {
        assert_eq!(
            "http://example.org/",
            NamedNode::new("http://example.org/").unwrap().iri
        );
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde() {
        let n = NamedNode::new("http://example.com/foo").unwrap();
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(json, "{\"value\":\"http://example.com/foo\"}");
        let n2: NamedNode = serde_json::from_str(&json).unwrap();
        assert_eq!(n2, n);
    }
}
