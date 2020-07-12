use crate::model::blank_node::BlankNode;
use crate::model::literal::Literal;
use crate::model::named_node::NamedNode;
use rio_api::model as rio;
use std::fmt;

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum NamedOrBlankNode {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
}

impl NamedOrBlankNode {
    pub fn is_named_node(&self) -> bool {
        match self {
            NamedOrBlankNode::NamedNode(_) => true,
            NamedOrBlankNode::BlankNode(_) => false,
        }
    }

    pub fn is_blank_node(&self) -> bool {
        match self {
            NamedOrBlankNode::NamedNode(_) => false,
            NamedOrBlankNode::BlankNode(_) => true,
        }
    }
}

impl fmt::Display for NamedOrBlankNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamedOrBlankNode::NamedNode(node) => node.fmt(f),
            NamedOrBlankNode::BlankNode(node) => node.fmt(f),
        }
    }
}

impl From<NamedNode> for NamedOrBlankNode {
    fn from(node: NamedNode) -> Self {
        NamedOrBlankNode::NamedNode(node)
    }
}

impl From<BlankNode> for NamedOrBlankNode {
    fn from(node: BlankNode) -> Self {
        NamedOrBlankNode::BlankNode(node)
    }
}

impl<'a> From<&'a NamedOrBlankNode> for rio::NamedOrBlankNode<'a> {
    fn from(node: &'a NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(node) => rio::NamedNode::from(node).into(),
            NamedOrBlankNode::BlankNode(node) => rio::BlankNode::from(node).into(),
        }
    }
}

/// An RDF [term](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term)
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) and [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Term {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Literal(Literal),
}

impl Term {
    pub fn is_named_node(&self) -> bool {
        match self {
            Term::NamedNode(_) => true,
            _ => false,
        }
    }

    pub fn is_blank_node(&self) -> bool {
        match self {
            Term::BlankNode(_) => true,
            _ => false,
        }
    }

    pub fn is_literal(&self) -> bool {
        match self {
            Term::Literal(_) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::NamedNode(node) => node.fmt(f),
            Term::BlankNode(node) => node.fmt(f),
            Term::Literal(literal) => literal.fmt(f),
        }
    }
}

impl From<NamedNode> for Term {
    fn from(node: NamedNode) -> Self {
        Term::NamedNode(node)
    }
}

impl From<BlankNode> for Term {
    fn from(node: BlankNode) -> Self {
        Term::BlankNode(node)
    }
}

impl From<Literal> for Term {
    fn from(literal: Literal) -> Self {
        Term::Literal(literal)
    }
}

impl From<NamedOrBlankNode> for Term {
    fn from(resource: NamedOrBlankNode) -> Self {
        match resource {
            NamedOrBlankNode::NamedNode(node) => node.into(),
            NamedOrBlankNode::BlankNode(node) => node.into(),
        }
    }
}

impl<'a> From<&'a Term> for rio::Term<'a> {
    fn from(node: &'a Term) -> Self {
        match node {
            Term::NamedNode(node) => rio::NamedNode::from(node).into(),
            Term::BlankNode(node) => rio::BlankNode::from(node).into(),
            Term::Literal(node) => rio::Literal::from(node).into(),
        }
    }
}

/// A [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Triple {
    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple
    pub subject: NamedOrBlankNode,

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple
    pub predicate: NamedNode,

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple
    pub object: Term,
}

impl Triple {
    /// Builds an RDF [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple)
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

    #[deprecated(note = "Use directly the `subject` field")]
    pub const fn subject(&self) -> &NamedOrBlankNode {
        &self.subject
    }

    #[deprecated(note = "Use directly the `subject` field")]
    pub fn subject_owned(self) -> NamedOrBlankNode {
        self.subject
    }

    #[deprecated(note = "Use directly the `predicate` field")]
    pub const fn predicate(&self) -> &NamedNode {
        &self.predicate
    }

    #[deprecated(note = "Use directly the `predicate` field")]
    pub fn predicate_owned(self) -> NamedNode {
        self.predicate
    }

    #[deprecated(note = "Use directly the `object` field")]
    pub const fn object(&self) -> &Term {
        &self.object
    }

    #[deprecated(note = "Use directly the `object` field")]
    pub fn object_owned(self) -> Term {
        self.object
    }

    /// Encodes that this triple is in a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        rio::Triple::from(self).fmt(f)
    }
}

impl<'a> From<&'a Triple> for rio::Triple<'a> {
    fn from(node: &'a Triple) -> Self {
        rio::Triple {
            subject: (&node.subject).into(),
            predicate: (&node.predicate).into(),
            object: (&node.object).into(),
        }
    }
}

/// A possible graph name.
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) and the [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphName {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    DefaultGraph,
}

impl GraphName {
    pub fn is_named_node(&self) -> bool {
        match self {
            GraphName::NamedNode(_) => true,
            _ => false,
        }
    }

    pub fn is_blank_node(&self) -> bool {
        match self {
            GraphName::BlankNode(_) => true,
            _ => false,
        }
    }

    pub fn is_default_graph(&self) -> bool {
        match self {
            GraphName::DefaultGraph => true,
            _ => false,
        }
    }
}

impl fmt::Display for GraphName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphName::NamedNode(node) => node.fmt(f),
            GraphName::BlankNode(node) => node.fmt(f),
            GraphName::DefaultGraph => write!(f, "DEFAULT"),
        }
    }
}

impl From<NamedNode> for GraphName {
    fn from(node: NamedNode) -> Self {
        GraphName::NamedNode(node)
    }
}

impl From<BlankNode> for GraphName {
    fn from(node: BlankNode) -> Self {
        GraphName::BlankNode(node)
    }
}

impl From<NamedOrBlankNode> for GraphName {
    fn from(node: NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(node) => node.into(),
            NamedOrBlankNode::BlankNode(node) => node.into(),
        }
    }
}

impl From<Option<NamedOrBlankNode>> for GraphName {
    fn from(name: Option<NamedOrBlankNode>) -> Self {
        if let Some(node) = name {
            node.into()
        } else {
            GraphName::DefaultGraph
        }
    }
}

impl From<GraphName> for Option<NamedOrBlankNode> {
    fn from(name: GraphName) -> Self {
        match name {
            GraphName::NamedNode(node) => Some(node.into()),
            GraphName::BlankNode(node) => Some(node.into()),
            GraphName::DefaultGraph => None,
        }
    }
}

impl<'a> From<&'a GraphName> for Option<rio::NamedOrBlankNode<'a>> {
    fn from(name: &'a GraphName) -> Self {
        match name {
            GraphName::NamedNode(node) => Some(rio::NamedNode::from(node).into()),
            GraphName::BlankNode(node) => Some(rio::BlankNode::from(node).into()),
            GraphName::DefaultGraph => None,
        }
    }
}

/// A [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Quad {
    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple
    pub subject: NamedOrBlankNode,

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple
    pub predicate: NamedNode,

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple
    pub object: Term,

    /// The name of the RDF [graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) in which the triple is.
    pub graph_name: GraphName,
}

impl Quad {
    /// Builds an RDF [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
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

    #[deprecated(note = "Use directly the `subject` field")]
    pub const fn subject(&self) -> &NamedOrBlankNode {
        &self.subject
    }

    #[deprecated(note = "Use directly the `subject` field")]
    pub fn subject_owned(self) -> NamedOrBlankNode {
        self.subject
    }

    #[deprecated(note = "Use directly the `predicate` field")]
    pub const fn predicate(&self) -> &NamedNode {
        &self.predicate
    }

    #[deprecated(note = "Use directly the `predicate` field")]
    pub fn predicate_owned(self) -> NamedNode {
        self.predicate
    }

    #[deprecated(note = "Use directly the `object` field")]
    pub const fn object(&self) -> &Term {
        &self.object
    }

    #[deprecated(note = "Use directly the `object` field")]
    pub fn object_owned(self) -> Term {
        self.object
    }

    #[deprecated(note = "Use directly the `graph_name` field")]
    pub const fn graph_name(&self) -> &GraphName {
        &self.graph_name
    }

    #[deprecated(note = "Use directly the `graph_name` field")]
    pub fn graph_name_owned(self) -> GraphName {
        self.graph_name
    }

    #[deprecated(note = "Use `Triple::from` instead")]
    pub fn into_triple(self) -> Triple {
        Triple::new(self.subject, self.predicate, self.object)
    }

    #[deprecated(note = "Use directly the struct fields")]
    pub fn destruct(self) -> (NamedOrBlankNode, NamedNode, Term, GraphName) {
        (self.subject, self.predicate, self.object, self.graph_name)
    }
}

impl fmt::Display for Quad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        rio::Quad::from(self).fmt(f)
    }
}

impl<'a> From<&'a Quad> for rio::Quad<'a> {
    fn from(node: &'a Quad) -> Self {
        rio::Quad {
            subject: (&node.subject).into(),
            predicate: (&node.predicate).into(),
            object: (&node.object).into(),
            graph_name: (&node.graph_name).into(),
        }
    }
}

impl From<Quad> for Triple {
    fn from(quad: Quad) -> Self {
        Self {
            subject: quad.subject,
            predicate: quad.predicate,
            object: quad.object,
        }
    }
}
