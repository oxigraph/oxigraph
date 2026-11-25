use crate::blank_node::BlankNode;
use crate::literal::Literal;
use crate::named_node::NamedNode;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;

/// The owned union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "lowercase"))]
pub enum NamedOrBlankNode {
    #[cfg_attr(feature = "serde", serde(rename = "uri"))]
    NamedNode(NamedNode),
    #[cfg_attr(feature = "serde", serde(rename = "bnode"))]
    BlankNode(BlankNode),
}

impl NamedOrBlankNode {
    #[inline]
    pub fn is_named_node(&self) -> bool {
        matches!(self, Self::NamedNode(_))
    }

    #[inline]
    pub fn is_blank_node(&self) -> bool {
        matches!(self, Self::BlankNode(_))
    }
}

impl PartialEq<NamedNode> for NamedOrBlankNode {
    #[inline]
    fn eq(&self, other: &NamedNode) -> bool {
        if let NamedOrBlankNode::NamedNode(this) = self {
            this == other
        } else {
            false
        }
    }
}

impl PartialEq<NamedOrBlankNode> for NamedNode {
    #[inline]
    fn eq(&self, other: &NamedOrBlankNode) -> bool {
        if let NamedOrBlankNode::NamedNode(other) = other {
            self == other
        } else {
            false
        }
    }
}

impl PartialEq<BlankNode> for NamedOrBlankNode {
    #[inline]
    fn eq(&self, other: &BlankNode) -> bool {
        if let NamedOrBlankNode::BlankNode(this) = self {
            this == other
        } else {
            false
        }
    }
}

impl PartialEq<NamedOrBlankNode> for BlankNode {
    #[inline]
    fn eq(&self, other: &NamedOrBlankNode) -> bool {
        if let NamedOrBlankNode::BlankNode(other) = other {
            self == other
        } else {
            false
        }
    }
}

impl fmt::Display for NamedOrBlankNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::BlankNode(node) => node.fmt(f),
        }
    }
}

impl From<NamedNode> for NamedOrBlankNode {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<BlankNode> for NamedOrBlankNode {
    #[inline]
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

/// An owned RDF [term](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term)
///
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node), [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) and [triples](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) (if the `rdf-12` feature is enabled).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "lowercase"))]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Term {
    #[cfg_attr(feature = "serde", serde(rename = "uri"))]
    NamedNode(NamedNode),
    #[cfg_attr(feature = "serde", serde(rename = "bnode"))]
    BlankNode(BlankNode),
    Literal(Literal),
    #[cfg(feature = "rdf-12")]
    Triple(Box<Triple>),
}

impl Term {
    #[inline]
    pub fn is_named_node(&self) -> bool {
        matches!(self, Self::NamedNode(_))
    }

    #[inline]
    pub fn is_blank_node(&self) -> bool {
        matches!(self, Self::BlankNode(_))
    }

    #[inline]
    pub fn is_literal(&self) -> bool {
        matches!(self, Self::Literal(_))
    }

    #[cfg(feature = "rdf-12")]
    #[inline]
    pub fn is_triple(&self) -> bool {
        matches!(self, Self::Triple(_))
    }
}

impl PartialEq<NamedNode> for Term {
    #[inline]
    fn eq(&self, other: &NamedNode) -> bool {
        if let Term::NamedNode(this) = self {
            this == other
        } else {
            false
        }
    }
}

impl PartialEq<Term> for NamedNode {
    #[inline]
    fn eq(&self, other: &Term) -> bool {
        if let Term::NamedNode(other) = other {
            self == other
        } else {
            false
        }
    }
}

impl PartialEq<BlankNode> for Term {
    #[inline]
    fn eq(&self, other: &BlankNode) -> bool {
        if let Term::BlankNode(this) = self {
            this == other
        } else {
            false
        }
    }
}

impl PartialEq<Term> for BlankNode {
    #[inline]
    fn eq(&self, other: &Term) -> bool {
        if let Term::BlankNode(other) = other {
            self == other
        } else {
            false
        }
    }
}

impl PartialEq<Literal> for Term {
    #[inline]
    fn eq(&self, other: &Literal) -> bool {
        if let Term::Literal(this) = self {
            this == other
        } else {
            false
        }
    }
}

impl PartialEq<Term> for Literal {
    #[inline]
    fn eq(&self, other: &Term) -> bool {
        if let Term::Literal(other) = other {
            self == other
        } else {
            false
        }
    }
}

#[cfg(feature = "rdf-12")]
impl PartialEq<Triple> for Term {
    #[inline]
    fn eq(&self, other: &Triple) -> bool {
        if let Term::Triple(this) = self {
            **this == *other
        } else {
            false
        }
    }
}

#[cfg(feature = "rdf-12")]
impl PartialEq<Term> for Triple {
    #[inline]
    fn eq(&self, other: &Term) -> bool {
        if let Term::Triple(other) = other {
            *self == **other
        } else {
            false
        }
    }
}

impl fmt::Display for Term {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::BlankNode(node) => node.fmt(f),
            Self::Literal(literal) => literal.fmt(f),
            #[cfg(feature = "rdf-12")]
            Self::Triple(triple) => write!(f, "<<( {triple} )>>"),
        }
    }
}

impl From<NamedNode> for Term {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<BlankNode> for Term {
    #[inline]
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

impl From<Literal> for Term {
    #[inline]
    fn from(literal: Literal) -> Self {
        Self::Literal(literal)
    }
}

#[cfg(feature = "rdf-12")]
impl From<Triple> for Term {
    #[inline]
    fn from(triple: Triple) -> Self {
        Self::Triple(Box::new(triple))
    }
}

#[cfg(feature = "rdf-12")]
impl From<Box<Triple>> for Term {
    #[inline]
    fn from(node: Box<Triple>) -> Self {
        Self::Triple(node)
    }
}

impl From<NamedOrBlankNode> for Term {
    #[inline]
    fn from(node: NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(node) => node.into(),
            NamedOrBlankNode::BlankNode(node) => node.into(),
        }
    }
}

impl TryFrom<Term> for NamedNode {
    type Error = TryFromTermError;

    #[inline]
    fn try_from(term: Term) -> Result<Self, Self::Error> {
        if let Term::NamedNode(node) = term {
            Ok(node)
        } else {
            Err(TryFromTermError {
                term,
                target: "NamedNode",
            })
        }
    }
}

impl TryFrom<Term> for BlankNode {
    type Error = TryFromTermError;

    #[inline]
    fn try_from(term: Term) -> Result<Self, Self::Error> {
        if let Term::BlankNode(node) = term {
            Ok(node)
        } else {
            Err(TryFromTermError {
                term,
                target: "BlankNode",
            })
        }
    }
}

impl TryFrom<Term> for Literal {
    type Error = TryFromTermError;

    #[inline]
    fn try_from(term: Term) -> Result<Self, Self::Error> {
        if let Term::Literal(node) = term {
            Ok(node)
        } else {
            Err(TryFromTermError {
                term,
                target: "Literal",
            })
        }
    }
}

impl TryFrom<Term> for NamedOrBlankNode {
    type Error = TryFromTermError;

    #[inline]
    fn try_from(term: Term) -> Result<Self, Self::Error> {
        match term {
            Term::NamedNode(term) => Ok(Self::NamedNode(term)),
            Term::BlankNode(term) => Ok(Self::BlankNode(term)),
            Term::Literal(_) => Err(TryFromTermError {
                term,
                target: "NamedOrBlankNode",
            }),
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => Err(TryFromTermError {
                term,
                target: "Triple",
            }),
        }
    }
}

/// An owned [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// use oxrdf::{NamedNode, Triple};
///
/// assert_eq!(
///     "<http://example.com/s> <http://example.com/p> <http://example.com/o>",
///     Triple {
///         subject: NamedNode::new("http://example.com/s")?.into(),
///         predicate: NamedNode::new("http://example.com/p")?,
///         object: NamedNode::new("http://example.com/o")?.into(),
///     }
///     .to_string()
/// );
/// # Result::<_,oxrdf::IriParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct Triple {
    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple.
    pub subject: NamedOrBlankNode,

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple.
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_predicate"))]
    pub predicate: NamedNode,

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple.
    pub object: Term,
}

impl Triple {
    /// Builds an RDF [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple).
    #[inline]
    pub fn new(
        subject: impl Into<NamedOrBlankNode>,
        predicate: impl Into<NamedNode>,
        object: impl Into<Term>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
        }
    }

    /// Builds an RDF [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) from [`Term`]s.
    ///
    /// Returns a [`TryFromTermError`] error if the generated triple would be ill-formed.
    #[inline]
    pub fn from_terms(
        subject: impl Into<Term>,
        predicate: impl Into<Term>,
        object: impl Into<Term>,
    ) -> Result<Self, TryFromTermError> {
        Ok(Self {
            subject: subject.into().try_into()?,
            predicate: predicate.into().try_into()?,
            object: object.into(),
        })
    }

    /// Encodes that this triple is in an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
    #[inline]
    pub fn in_graph(self, graph_name: impl Into<GraphName>) -> Quad {
        Quad {
            subject: self.subject,
            predicate: self.predicate,
            object: self.object,
            graph_name: graph_name.into(),
        }
    }
}

impl fmt::Display for Triple {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}

/// An owned graph name
///
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node), and the [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph).
#[derive(Eq, PartialEq, Debug, Clone, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "lowercase"))]
pub enum GraphName {
    #[cfg_attr(feature = "serde", serde(rename = "uri"))]
    NamedNode(NamedNode),
    #[cfg_attr(feature = "serde", serde(rename = "bnode"))]
    BlankNode(BlankNode),
    #[default]
    #[cfg_attr(feature = "serde", serde(rename = "default"))]
    DefaultGraph,
}

impl GraphName {
    #[inline]
    pub fn is_named_node(&self) -> bool {
        matches!(self, Self::NamedNode(_))
    }

    #[inline]
    pub fn is_blank_node(&self) -> bool {
        matches!(self, Self::BlankNode(_))
    }

    #[inline]
    pub fn is_default_graph(&self) -> bool {
        matches!(self, Self::DefaultGraph)
    }
}

impl PartialEq<NamedNode> for GraphName {
    #[inline]
    fn eq(&self, other: &NamedNode) -> bool {
        if let GraphName::NamedNode(this) = self {
            this == other
        } else {
            false
        }
    }
}

impl PartialEq<GraphName> for NamedNode {
    #[inline]
    fn eq(&self, other: &GraphName) -> bool {
        if let GraphName::NamedNode(other) = other {
            self == other
        } else {
            false
        }
    }
}

impl PartialEq<BlankNode> for GraphName {
    #[inline]
    fn eq(&self, other: &BlankNode) -> bool {
        if let GraphName::BlankNode(this) = self {
            this == other
        } else {
            false
        }
    }
}

impl PartialEq<GraphName> for BlankNode {
    #[inline]
    fn eq(&self, other: &GraphName) -> bool {
        if let GraphName::BlankNode(other) = other {
            self == other
        } else {
            false
        }
    }
}

impl fmt::Display for GraphName {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::BlankNode(node) => node.fmt(f),
            Self::DefaultGraph => f.write_str("DEFAULT"),
        }
    }
}

impl From<NamedNode> for GraphName {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<BlankNode> for GraphName {
    #[inline]
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

impl From<NamedOrBlankNode> for GraphName {
    #[inline]
    fn from(node: NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(node) => node.into(),
            NamedOrBlankNode::BlankNode(node) => node.into(),
        }
    }
}

/// An owned [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
///
/// The default string formatter is returning an N-Quads compatible representation:
/// ```
/// use oxrdf::{Quad, NamedNode};
///
/// assert_eq!(
///     "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g>",
///     Quad {
///         subject: NamedNode::new("http://example.com/s")?.into(),
///         predicate: NamedNode::new("http://example.com/p")?,
///         object: NamedNode::new("http://example.com/o")?.into(),
///         graph_name: NamedNode::new("http://example.com/g")?.into(),
///     }.to_string()
/// );
/// # Result::<_,oxrdf::IriParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
#[cfg_attr(feature = "serde", expect(clippy::unsafe_derive_deserialize))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct Quad {
    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple.
    pub subject: NamedOrBlankNode,

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple.
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_predicate"))]
    pub predicate: NamedNode,

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple.
    pub object: Term,

    /// The name of the RDF [graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) in which the triple is.
    #[cfg_attr(feature = "serde", serde(rename = "graph"))]
    pub graph_name: GraphName,
}

impl Quad {
    /// Builds an RDF [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
    #[inline]
    pub fn new(
        subject: impl Into<NamedOrBlankNode>,
        predicate: impl Into<NamedNode>,
        object: impl Into<Term>,
        graph_name: impl Into<GraphName>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            graph_name: graph_name.into(),
        }
    }

    #[inline]
    #[expect(unsafe_code)]
    pub fn as_triple(&self) -> &Triple {
        let this: *const Quad = self;
        // SAFETY: Both Quad and Triple are repr(C) and Triple is a prefix of Quad.
        unsafe { &*this.cast::<Triple>() }
    }
}

impl fmt::Display for Quad {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_triple().fmt(f)?;
        if !self.graph_name.is_default_graph() {
            write!(f, " {}", self.graph_name)?;
        }
        Ok(())
    }
}

impl From<Quad> for Triple {
    #[inline]
    fn from(quad: Quad) -> Self {
        Self {
            subject: quad.subject,
            predicate: quad.predicate,
            object: quad.object,
        }
    }
}

impl<'a> From<&'a Quad> for &'a Triple {
    #[inline]
    fn from(quad: &'a Quad) -> Self {
        quad.as_triple()
    }
}

/// An error return by some [`TryFrom<Term>`](TryFrom)  implementations.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{term} can not be converted to a {target}")]
pub struct TryFromTermError {
    term: Term,
    target: &'static str,
}

impl TryFromTermError {
    /// The term that can't be converted
    #[inline]
    pub fn into_term(self) -> Term {
        self.term
    }
}
#[cfg(feature = "serde")]
fn serialize_predicate<S>(node: &NamedNode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    #[serde(rename = "uri", tag = "type")]
    struct Value<'a> {
        value: &'a str,
    }
    Value {
        value: node.as_str(),
    }
    .serialize(serializer)
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    #[cfg(feature = "serde")]
    use serde::de::DeserializeOwned;

    #[test]
    fn triple_from_terms() -> Result<(), TryFromTermError> {
        assert_eq!(
            Triple::from_terms(
                NamedNode::new_unchecked("http://example.com/s"),
                NamedNode::new_unchecked("http://example.com/p"),
                NamedNode::new_unchecked("http://example.com/o"),
            )?,
            Triple::new(
                NamedNode::new_unchecked("http://example.com/s"),
                NamedNode::new_unchecked("http://example.com/p"),
                NamedNode::new_unchecked("http://example.com/o"),
            )
        );
        assert_eq!(
            Triple::from_terms(
                Literal::new_simple_literal("foo"),
                NamedNode::new_unchecked("http://example.com/p"),
                NamedNode::new_unchecked("http://example.com/o"),
            )
            .unwrap_err()
            .into_term(),
            Term::from(Literal::new_simple_literal("foo"))
        );
        assert_eq!(
            Triple::from_terms(
                NamedNode::new_unchecked("http://example.com/s"),
                Literal::new_simple_literal("foo"),
                NamedNode::new_unchecked("http://example.com/o"),
            )
            .unwrap_err()
            .into_term(),
            Term::from(Literal::new_simple_literal("foo"))
        );
        Ok(())
    }

    #[test]
    #[cfg(feature = "serde")]
    fn serde() -> Result<(), serde_json::Error> {
        let triple = Triple::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
        );
        let jsn = serde_json::to_string(&triple)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"uri","value":"http://example.com/o"}}"#
        );
        let deserialized: Triple = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, triple);

        // Test triples with all possible combinations of terms
        let triple = Triple::new(
            BlankNode::new_unchecked("s"),
            NamedNode::new_unchecked("http://example.com/p"),
            Literal::new_simple_literal("foo"),
        );
        let jsn = serde_json::to_string(&triple)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"bnode","value":"s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"literal","value":"foo"}}"#
        );
        let deserialized: Triple = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, triple);

        let triple = Triple::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            BlankNode::new_unchecked("foo"),
        );

        let jsn = serde_json::to_string(&triple)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"bnode","value":"foo"}}"#
        );
        let deserialized: Triple = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, triple);

        Ok(())
    }

    #[test]
    #[cfg(feature = "serde")]
    fn serde_from_reader() -> Result<(), serde_json::Error> {
        let triple = Triple::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
        );
        let jsn = serde_json::to_string(&triple)?;
        let deserialized: Triple = serde_json::from_reader(jsn.as_bytes())?;
        assert_eq!(deserialized, triple);

        // Test triples with all possible combinations of terms
        let triple = Triple::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            Literal::new_simple_literal("foo"),
        );
        let jsn = serde_json::to_string(&triple)?;
        let deserialized: Triple = serde_json::from_reader(jsn.as_bytes())?;
        assert_eq!(deserialized, triple);

        let triple = Triple::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            BlankNode::new_unchecked("foo"),
        );

        let jsn = serde_json::to_string(&triple)?;
        let deserialized: Triple = serde_json::from_reader(jsn.as_bytes())?;
        assert_eq!(deserialized, triple);

        Ok(())
    }

    #[test]
    #[cfg(feature = "serde")]
    #[cfg(feature = "rdf-12")]
    fn serde_star() -> Result<(), serde_json::Error> {
        let triple = Triple::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            Term::Triple(Box::new(Triple::new(
                NamedNode::new_unchecked("http://example.com/s"),
                NamedNode::new_unchecked("http://example.com/p"),
                NamedNode::new_unchecked("http://example.com/o"),
            ))),
        );

        let jsn = serde_json::to_string(&triple)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"triple","subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"uri","value":"http://example.com/o"}}}"#
        );
        let deserialized: Triple = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, triple);

        Ok(())
    }

    #[test]
    #[cfg(feature = "serde")]
    fn serde_quad() -> Result<(), serde_json::Error> {
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
            NamedNode::new_unchecked("http://example.com/g"),
        );
        let jsn = serde_json::to_string(&quad)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"uri","value":"http://example.com/o"},"graph":{"type":"uri","value":"http://example.com/g"}}"#
        );
        let deserialized: Quad = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, quad);

        // Test quads with all possible combinations of terms
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            Literal::new_simple_literal("foo"),
            NamedNode::new_unchecked("http://example.com/g"),
        );
        let jsn = serde_json::to_string(&quad)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"literal","value":"foo"},"graph":{"type":"uri","value":"http://example.com/g"}}"#
        );
        let deserialized: Quad = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, quad);

        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            BlankNode::new_unchecked("foo"),
            NamedNode::new_unchecked("http://example.com/g"),
        );

        let jsn = serde_json::to_string(&quad)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"bnode","value":"foo"},"graph":{"type":"uri","value":"http://example.com/g"}}"#
        );
        let deserialized: Quad = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, quad);

        // Test quads with a blank node graph name
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
            BlankNode::new_unchecked("foo"),
        );
        let jsn = serde_json::to_string(&quad)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"uri","value":"http://example.com/o"},"graph":{"type":"bnode","value":"foo"}}"#
        );
        let deserialized: Quad = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, quad);

        // Test quads with the default graph name
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
            GraphName::DefaultGraph,
        );
        let jsn = serde_json::to_string(&quad)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"uri","value":"http://example.com/o"},"graph":{"type":"default"}}"#
        );
        let deserialized: Quad = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, quad);

        Ok(())
    }

    #[test]
    #[cfg(feature = "serde")]
    fn serde_quad_from_reader() -> Result<(), serde_json::Error> {
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
            NamedNode::new_unchecked("http://example.com/g"),
        );
        let jsn = serde_json::to_string(&quad)?;
        let deserialized: Quad = serde_json::from_reader(jsn.as_bytes())?;
        assert_eq!(deserialized, quad);

        // Test quads with all possible combinations of terms
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            Literal::new_simple_literal("foo"),
            NamedNode::new_unchecked("http://example.com/g"),
        );
        let jsn = serde_json::to_string(&quad)?;
        let deserialized: Quad = serde_json::from_reader(jsn.as_bytes())?;
        assert_eq!(deserialized, quad);

        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            BlankNode::new_unchecked("foo"),
            NamedNode::new_unchecked("http://example.com/g"),
        );

        let jsn = serde_json::to_string(&quad)?;
        let deserialized: Quad = serde_json::from_reader(jsn.as_bytes())?;
        assert_eq!(deserialized, quad);

        // Test quads with a blank node graph name
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
            BlankNode::new_unchecked("foo"),
        );
        let jsn = serde_json::to_string(&quad)?;
        let deserialized: Quad = serde_json::from_reader(jsn.as_bytes())?;
        assert_eq!(deserialized, quad);

        // Test quads with the default graph name
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
            GraphName::DefaultGraph,
        );
        let jsn = serde_json::to_string(&quad)?;
        let deserialized: Quad = serde_json::from_reader(jsn.as_bytes())?;
        assert_eq!(deserialized, quad);

        Ok(())
    }

    #[test]
    #[cfg(feature = "serde")]
    #[cfg(feature = "rdf-12")]
    fn serde_quad_star() -> Result<(), serde_json::Error> {
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            Term::Triple(Box::new(Triple::new(
                NamedNode::new_unchecked("http://example.com/s"),
                NamedNode::new_unchecked("http://example.com/p"),
                NamedNode::new_unchecked("http://example.com/o"),
            ))),
            NamedNode::new_unchecked("http://example.com/g"),
        );

        let jsn = serde_json::to_string(&quad)?;
        assert_eq!(
            jsn,
            r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"triple","subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"uri","value":"http://example.com/o"}},"graph":{"type":"uri","value":"http://example.com/g"}}"#
        );
        let deserialized: Quad = serde_json::from_str(&jsn)?;
        assert_eq!(deserialized, quad);

        Ok(())
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_deserialize_owned() {
        fn assert_deserialize_owned<T: DeserializeOwned>() {}

        // If the type does not implement DeserializeOwned, this call will fail to compile.
        assert_deserialize_owned::<Term>();
        assert_deserialize_owned::<Triple>();
        assert_deserialize_owned::<Quad>();
    }
}
