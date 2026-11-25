use oxiri::{Iri, IriParseError};
use oxstr::OxString;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;
use std::fmt::Write;

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
    iri: OxString,
}

impl NamedNode {
    /// Builds and validate an RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri).
    pub fn new(iri: impl Into<OxString>) -> Result<Self, IriParseError> {
        Ok(Self::new_from_iri(Iri::parse(iri.into())?))
    }

    #[inline]
    pub(crate) fn new_from_iri(iri: Iri<OxString>) -> Self {
        Self::new_unchecked(iri.into_inner())
    }

    /// Builds an RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) from a string.
    ///
    /// It is the caller's responsibility to ensure that `iri` is a valid IRI.
    ///
    /// [`NamedNode::new`] is a safe version of this constructor and should be used for untrusted data
    /// and [`NamedNode::new_const_unchecked`] in const contexts on trusted data.
    #[inline]
    pub fn new_unchecked(iri: impl Into<OxString>) -> Self {
        Self { iri: iri.into() }
    }

    /// Builds an RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) from a static string without allocating.
    ///
    /// It is the caller's responsibility to ensure that `iri` is a valid IRI.
    ///
    /// [`NamedNode::new`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub const fn new_const_unchecked(iri: &'static str) -> Self {
        Self {
            iri: OxString::new(iri),
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.iri.as_str()
    }

    #[inline]
    pub fn into_string(self) -> OxString {
        self.iri
    }
}

impl fmt::Display for NamedNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('<')?;
        f.write_str(&self.iri)?;
        f.write_char('>')
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

impl From<Iri<OxString>> for NamedNode {
    #[inline]
    fn from(iri: Iri<OxString>) -> Self {
        Self {
            iri: iri.into_inner(),
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for NamedNode {
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
            value: OxString,
        }
        Self::new(Value::deserialize(deserializer)?.value).map_err(de::Error::custom)
    }
}

#[cfg(feature = "serde")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let n = NamedNode::new("http://example.com/foo").unwrap();
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(json, "{\"value\":\"http://example.com/foo\"}");
        let n2: NamedNode = serde_json::from_str(&json).unwrap();
        assert_eq!(n2, n);
    }
}
