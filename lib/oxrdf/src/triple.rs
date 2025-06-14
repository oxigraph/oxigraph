use crate::blank_node::BlankNode;
use crate::literal::Literal;
use crate::named_node::NamedNode;
use crate::{BlankNodeRef, LiteralRef, NamedNodeRef};
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
        self.as_ref().is_named_node()
    }

    #[inline]
    pub fn is_blank_node(&self) -> bool {
        self.as_ref().is_blank_node()
    }

    #[inline]
    pub fn as_ref(&self) -> NamedOrBlankNodeRef<'_> {
        match self {
            Self::NamedNode(node) => NamedOrBlankNodeRef::NamedNode(node.as_ref()),
            Self::BlankNode(node) => NamedOrBlankNodeRef::BlankNode(node.as_ref()),
        }
    }
}

impl fmt::Display for NamedOrBlankNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl From<NamedNode> for NamedOrBlankNode {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<NamedNodeRef<'_>> for NamedOrBlankNode {
    #[inline]
    fn from(node: NamedNodeRef<'_>) -> Self {
        node.into_owned().into()
    }
}

impl From<BlankNode> for NamedOrBlankNode {
    #[inline]
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

impl From<BlankNodeRef<'_>> for NamedOrBlankNode {
    #[inline]
    fn from(node: BlankNodeRef<'_>) -> Self {
        node.into_owned().into()
    }
}

/// The borrowed union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "lowercase"))]
pub enum NamedOrBlankNodeRef<'a> {
    #[cfg_attr(feature = "serde", serde(rename = "uri"))]
    NamedNode(NamedNodeRef<'a>),
    #[cfg_attr(feature = "serde", serde(rename = "bnode"))]
    BlankNode(BlankNodeRef<'a>),
}

impl NamedOrBlankNodeRef<'_> {
    #[inline]
    pub fn is_named_node(&self) -> bool {
        match self {
            Self::NamedNode(_) => true,
            Self::BlankNode(_) => false,
        }
    }

    #[inline]
    pub fn is_blank_node(&self) -> bool {
        match self {
            Self::NamedNode(_) => false,
            Self::BlankNode(_) => true,
        }
    }

    #[inline]
    pub fn into_owned(self) -> NamedOrBlankNode {
        match self {
            Self::NamedNode(node) => NamedOrBlankNode::NamedNode(node.into_owned()),
            Self::BlankNode(node) => NamedOrBlankNode::BlankNode(node.into_owned()),
        }
    }
}

impl fmt::Display for NamedOrBlankNodeRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::BlankNode(node) => node.fmt(f),
        }
    }
}

impl<'a> From<NamedNodeRef<'a>> for NamedOrBlankNodeRef<'a> {
    #[inline]
    fn from(node: NamedNodeRef<'a>) -> Self {
        Self::NamedNode(node)
    }
}

impl<'a> From<&'a NamedNode> for NamedOrBlankNodeRef<'a> {
    #[inline]
    fn from(node: &'a NamedNode) -> Self {
        node.as_ref().into()
    }
}

impl<'a> From<BlankNodeRef<'a>> for NamedOrBlankNodeRef<'a> {
    #[inline]
    fn from(node: BlankNodeRef<'a>) -> Self {
        Self::BlankNode(node)
    }
}

impl<'a> From<&'a BlankNode> for NamedOrBlankNodeRef<'a> {
    #[inline]
    fn from(node: &'a BlankNode) -> Self {
        node.as_ref().into()
    }
}

impl<'a> From<&'a NamedOrBlankNode> for NamedOrBlankNodeRef<'a> {
    #[inline]
    fn from(node: &'a NamedOrBlankNode) -> Self {
        node.as_ref()
    }
}

impl<'a> From<NamedOrBlankNodeRef<'a>> for NamedOrBlankNode {
    #[inline]
    fn from(node: NamedOrBlankNodeRef<'a>) -> Self {
        node.into_owned()
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
        self.as_ref().is_named_node()
    }

    #[inline]
    pub fn is_blank_node(&self) -> bool {
        self.as_ref().is_blank_node()
    }

    #[inline]
    pub fn is_literal(&self) -> bool {
        self.as_ref().is_literal()
    }

    #[cfg(feature = "rdf-12")]
    #[inline]
    pub fn is_triple(&self) -> bool {
        self.as_ref().is_triple()
    }

    #[inline]
    pub fn as_ref(&self) -> TermRef<'_> {
        match self {
            Self::NamedNode(node) => TermRef::NamedNode(node.as_ref()),
            Self::BlankNode(node) => TermRef::BlankNode(node.as_ref()),
            Self::Literal(literal) => TermRef::Literal(literal.as_ref()),
            #[cfg(feature = "rdf-12")]
            Self::Triple(triple) => TermRef::Triple(triple),
        }
    }
}

impl fmt::Display for Term {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl From<NamedNode> for Term {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<NamedNodeRef<'_>> for Term {
    #[inline]
    fn from(node: NamedNodeRef<'_>) -> Self {
        node.into_owned().into()
    }
}

impl From<BlankNode> for Term {
    #[inline]
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

impl From<BlankNodeRef<'_>> for Term {
    #[inline]
    fn from(node: BlankNodeRef<'_>) -> Self {
        node.into_owned().into()
    }
}

impl From<Literal> for Term {
    #[inline]
    fn from(literal: Literal) -> Self {
        Self::Literal(literal)
    }
}

impl From<LiteralRef<'_>> for Term {
    #[inline]
    fn from(literal: LiteralRef<'_>) -> Self {
        literal.into_owned().into()
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

#[cfg(feature = "rdf-12")]
impl From<TripleRef<'_>> for Term {
    #[inline]
    fn from(triple: TripleRef<'_>) -> Self {
        triple.into_owned().into()
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

impl From<NamedOrBlankNodeRef<'_>> for Term {
    #[inline]
    fn from(node: NamedOrBlankNodeRef<'_>) -> Self {
        node.into_owned().into()
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

/// A borrowed RDF [term](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term)
///
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node), [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) and [triples](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) (if the `rdf-12` feature is enabled).
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "lowercase"))]
pub enum TermRef<'a> {
    #[cfg_attr(feature = "serde", serde(rename = "uri"))]
    NamedNode(NamedNodeRef<'a>),
    #[cfg_attr(feature = "serde", serde(rename = "bnode"))]
    BlankNode(BlankNodeRef<'a>),
    Literal(LiteralRef<'a>),
    #[cfg(feature = "rdf-12")]
    Triple(&'a Triple),
}

impl TermRef<'_> {
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

    #[inline]
    pub fn into_owned(self) -> Term {
        match self {
            Self::NamedNode(node) => Term::NamedNode(node.into_owned()),
            Self::BlankNode(node) => Term::BlankNode(node.into_owned()),
            Self::Literal(literal) => Term::Literal(literal.into_owned()),
            #[cfg(feature = "rdf-12")]
            Self::Triple(triple) => Term::Triple(Box::new(triple.clone())),
        }
    }
}

impl fmt::Display for TermRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::BlankNode(node) => node.fmt(f),
            Self::Literal(literal) => literal.fmt(f),
            #[cfg(feature = "rdf-12")]
            Self::Triple(triple) => {
                write!(f, "<<( {triple} )>>")
            }
        }
    }
}

impl<'a> From<NamedNodeRef<'a>> for TermRef<'a> {
    #[inline]
    fn from(node: NamedNodeRef<'a>) -> Self {
        Self::NamedNode(node)
    }
}

impl<'a> From<&'a NamedNode> for TermRef<'a> {
    #[inline]
    fn from(node: &'a NamedNode) -> Self {
        node.as_ref().into()
    }
}

impl<'a> From<BlankNodeRef<'a>> for TermRef<'a> {
    #[inline]
    fn from(node: BlankNodeRef<'a>) -> Self {
        Self::BlankNode(node)
    }
}

impl<'a> From<&'a BlankNode> for TermRef<'a> {
    #[inline]
    fn from(node: &'a BlankNode) -> Self {
        node.as_ref().into()
    }
}

impl<'a> From<LiteralRef<'a>> for TermRef<'a> {
    #[inline]
    fn from(literal: LiteralRef<'a>) -> Self {
        Self::Literal(literal)
    }
}

impl<'a> From<&'a Literal> for TermRef<'a> {
    #[inline]
    fn from(literal: &'a Literal) -> Self {
        literal.as_ref().into()
    }
}

#[cfg(feature = "rdf-12")]
impl<'a> From<&'a Triple> for TermRef<'a> {
    #[inline]
    fn from(node: &'a Triple) -> Self {
        Self::Triple(node)
    }
}

impl<'a> From<NamedOrBlankNodeRef<'a>> for TermRef<'a> {
    #[inline]
    fn from(node: NamedOrBlankNodeRef<'a>) -> Self {
        match node {
            NamedOrBlankNodeRef::NamedNode(node) => node.into(),
            NamedOrBlankNodeRef::BlankNode(node) => node.into(),
        }
    }
}

impl<'a> From<&'a NamedOrBlankNode> for TermRef<'a> {
    #[inline]
    fn from(node: &'a NamedOrBlankNode) -> Self {
        node.as_ref().into()
    }
}

impl<'a> From<&'a Term> for TermRef<'a> {
    #[inline]
    fn from(node: &'a Term) -> Self {
        node.as_ref()
    }
}

impl<'a> From<TermRef<'a>> for Term {
    #[inline]
    fn from(node: TermRef<'a>) -> Self {
        node.into_owned()
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

    #[inline]
    pub fn as_ref(&self) -> TripleRef<'_> {
        TripleRef {
            subject: self.subject.as_ref(),
            predicate: self.predicate.as_ref(),
            object: self.object.as_ref(),
        }
    }
}

impl fmt::Display for Triple {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

/// A borrowed [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation:
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
///
/// assert_eq!(
///     "<http://example.com/s> <http://example.com/p> <http://example.com/o>",
///     TripleRef {
///         subject: NamedNodeRef::new("http://example.com/s")?.into(),
///         predicate: NamedNodeRef::new("http://example.com/p")?,
///         object: NamedNodeRef::new("http://example.com/o")?.into(),
///     }
///     .to_string()
/// );
/// # Result::<_,oxrdf::IriParseError>::Ok(())
/// ```

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct TripleRef<'a> {
    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple.
    pub subject: NamedOrBlankNodeRef<'a>,

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple.
    pub predicate: NamedNodeRef<'a>,

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple.
    pub object: TermRef<'a>,
}

impl<'a> TripleRef<'a> {
    /// Builds an RDF [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple).
    #[inline]
    pub fn new(
        subject: impl Into<NamedOrBlankNodeRef<'a>>,
        predicate: impl Into<NamedNodeRef<'a>>,
        object: impl Into<TermRef<'a>>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
        }
    }

    /// Encodes that this triple is in an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
    #[inline]
    pub fn in_graph(self, graph_name: impl Into<GraphNameRef<'a>>) -> QuadRef<'a> {
        QuadRef {
            subject: self.subject,
            predicate: self.predicate,
            object: self.object,
            graph_name: graph_name.into(),
        }
    }

    #[inline]
    pub fn into_owned(self) -> Triple {
        Triple {
            subject: self.subject.into_owned(),
            predicate: self.predicate.into_owned(),
            object: self.object.into_owned(),
        }
    }
}

impl fmt::Display for TripleRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}

impl<'a> From<&'a Triple> for TripleRef<'a> {
    #[inline]
    fn from(triple: &'a Triple) -> Self {
        triple.as_ref()
    }
}

impl<'a> From<TripleRef<'a>> for Triple {
    #[inline]
    fn from(triple: TripleRef<'a>) -> Self {
        triple.into_owned()
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
        self.as_ref().is_named_node()
    }

    #[inline]
    pub fn is_blank_node(&self) -> bool {
        self.as_ref().is_blank_node()
    }

    #[inline]
    pub fn is_default_graph(&self) -> bool {
        self.as_ref().is_default_graph()
    }

    #[inline]
    pub fn as_ref(&self) -> GraphNameRef<'_> {
        match self {
            Self::NamedNode(node) => GraphNameRef::NamedNode(node.as_ref()),
            Self::BlankNode(node) => GraphNameRef::BlankNode(node.as_ref()),
            Self::DefaultGraph => GraphNameRef::DefaultGraph,
        }
    }
}

impl fmt::Display for GraphName {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl From<NamedNode> for GraphName {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<NamedNodeRef<'_>> for GraphName {
    #[inline]
    fn from(node: NamedNodeRef<'_>) -> Self {
        node.into_owned().into()
    }
}

impl From<BlankNode> for GraphName {
    #[inline]
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

impl From<BlankNodeRef<'_>> for GraphName {
    #[inline]
    fn from(node: BlankNodeRef<'_>) -> Self {
        node.into_owned().into()
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

impl From<NamedOrBlankNodeRef<'_>> for GraphName {
    #[inline]
    fn from(node: NamedOrBlankNodeRef<'_>) -> Self {
        node.into_owned().into()
    }
}

/// A borrowed graph name
///
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node), and the [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph).
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum GraphNameRef<'a> {
    #[cfg_attr(feature = "serde", serde(rename = "uri"))]
    NamedNode(NamedNodeRef<'a>),
    #[cfg_attr(feature = "serde", serde(rename = "bnode"))]
    BlankNode(BlankNodeRef<'a>),
    #[default]
    #[cfg_attr(feature = "serde", serde(rename = "default"))]
    DefaultGraph,
}

impl GraphNameRef<'_> {
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

    #[inline]
    pub fn into_owned(self) -> GraphName {
        match self {
            Self::NamedNode(node) => GraphName::NamedNode(node.into_owned()),
            Self::BlankNode(node) => GraphName::BlankNode(node.into_owned()),
            Self::DefaultGraph => GraphName::DefaultGraph,
        }
    }
}

impl fmt::Display for GraphNameRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::BlankNode(node) => node.fmt(f),
            Self::DefaultGraph => f.write_str("DEFAULT"),
        }
    }
}

impl<'a> From<NamedNodeRef<'a>> for GraphNameRef<'a> {
    #[inline]
    fn from(node: NamedNodeRef<'a>) -> Self {
        Self::NamedNode(node)
    }
}

impl<'a> From<&'a NamedNode> for GraphNameRef<'a> {
    #[inline]
    fn from(node: &'a NamedNode) -> Self {
        node.as_ref().into()
    }
}

impl<'a> From<BlankNodeRef<'a>> for GraphNameRef<'a> {
    #[inline]
    fn from(node: BlankNodeRef<'a>) -> Self {
        Self::BlankNode(node)
    }
}

impl<'a> From<&'a BlankNode> for GraphNameRef<'a> {
    #[inline]
    fn from(node: &'a BlankNode) -> Self {
        node.as_ref().into()
    }
}

impl<'a> From<NamedOrBlankNodeRef<'a>> for GraphNameRef<'a> {
    #[inline]
    fn from(node: NamedOrBlankNodeRef<'a>) -> Self {
        match node {
            NamedOrBlankNodeRef::NamedNode(node) => node.into(),
            NamedOrBlankNodeRef::BlankNode(node) => node.into(),
        }
    }
}

impl<'a> From<&'a NamedOrBlankNode> for GraphNameRef<'a> {
    #[inline]
    fn from(node: &'a NamedOrBlankNode) -> Self {
        node.as_ref().into()
    }
}

impl<'a> From<&'a GraphName> for GraphNameRef<'a> {
    #[inline]
    fn from(node: &'a GraphName) -> Self {
        node.as_ref()
    }
}

impl<'a> From<GraphNameRef<'a>> for GraphName {
    #[inline]
    fn from(node: GraphNameRef<'a>) -> Self {
        node.into_owned()
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    pub fn as_ref(&self) -> QuadRef<'_> {
        QuadRef {
            subject: self.subject.as_ref(),
            predicate: self.predicate.as_ref(),
            object: self.object.as_ref(),
            graph_name: self.graph_name.as_ref(),
        }
    }
}

impl fmt::Display for Quad {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
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

/// A borrowed [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
///
/// The default string formatter is returning an N-Quads compatible representation:
/// ```
/// use oxrdf::{QuadRef, NamedNodeRef};
///
/// assert_eq!(
///     "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g>",
///     QuadRef {
///         subject: NamedNodeRef::new("http://example.com/s")?.into(),
///         predicate: NamedNodeRef::new("http://example.com/p")?,
///         object: NamedNodeRef::new("http://example.com/o")?.into(),
///         graph_name: NamedNodeRef::new("http://example.com/g")?.into(),
///     }.to_string()
/// );
/// # Result::<_,oxrdf::IriParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct QuadRef<'a> {
    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple.
    pub subject: NamedOrBlankNodeRef<'a>,

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple.
    pub predicate: NamedNodeRef<'a>,

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple.
    pub object: TermRef<'a>,

    /// The name of the RDF [graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) in which the triple is.
    pub graph_name: GraphNameRef<'a>,
}

impl<'a> QuadRef<'a> {
    /// Builds an RDF [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
    #[inline]
    pub fn new(
        subject: impl Into<NamedOrBlankNodeRef<'a>>,
        predicate: impl Into<NamedNodeRef<'a>>,
        object: impl Into<TermRef<'a>>,
        graph_name: impl Into<GraphNameRef<'a>>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            graph_name: graph_name.into(),
        }
    }

    #[inline]
    pub fn into_owned(self) -> Quad {
        Quad {
            subject: self.subject.into_owned(),
            predicate: self.predicate.into_owned(),
            object: self.object.into_owned(),
            graph_name: self.graph_name.into_owned(),
        }
    }
}

impl fmt::Display for QuadRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name.is_default_graph() {
            write!(f, "{} {} {}", self.subject, self.predicate, self.object)
        } else {
            write!(
                f,
                "{} {} {} {}",
                self.subject, self.predicate, self.object, self.graph_name
            )
        }
    }
}

impl<'a> From<QuadRef<'a>> for TripleRef<'a> {
    #[inline]
    fn from(quad: QuadRef<'a>) -> Self {
        Self {
            subject: quad.subject,
            predicate: quad.predicate,
            object: quad.object,
        }
    }
}

impl<'a> From<&'a Quad> for QuadRef<'a> {
    #[inline]
    fn from(quad: &'a Quad) -> Self {
        quad.as_ref()
    }
}

impl<'a> From<QuadRef<'a>> for Quad {
    #[inline]
    fn from(quad: QuadRef<'a>) -> Self {
        quad.into_owned()
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
            BlankNode::new("foo").unwrap(),
        );

        let jsn = serde_json::to_string(&triple).unwrap();
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
            BlankNode::new("foo").unwrap(),
        );

        let jsn = serde_json::to_string(&triple).unwrap();
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
            BlankNode::new("foo").unwrap(),
            NamedNode::new_unchecked("http://example.com/g"),
        );

        let jsn = serde_json::to_string(&quad).unwrap();
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
            BlankNode::new("foo").unwrap(),
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
            BlankNode::new("foo").unwrap(),
            NamedNode::new_unchecked("http://example.com/g"),
        );

        let jsn = serde_json::to_string(&quad).unwrap();
        let deserialized: Quad = serde_json::from_reader(jsn.as_bytes())?;
        assert_eq!(deserialized, quad);

        // Test quads with a blank node graph name
        let quad = Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
            BlankNode::new("foo").unwrap(),
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
