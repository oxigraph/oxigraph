use crate::model::blank_node::BlankNode;
use crate::model::literal::Literal;
use crate::model::named_node::NamedNode;
use std::fmt;

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

/// A RDF [term](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term)
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) and [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Term {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Literal(Literal),
}

impl Term {
    pub fn is_named_node(&self) -> bool {
        match self {
            Term::NamedNode(_) => true,
            Term::BlankNode(_) => false,
            Term::Literal(_) => false,
        }
    }

    pub fn is_blank_node(&self) -> bool {
        match self {
            Term::NamedNode(_) => false,
            Term::BlankNode(_) => true,
            Term::Literal(_) => false,
        }
    }

    pub fn is_literal(&self) -> bool {
        match self {
            Term::NamedNode(_) => false,
            Term::BlankNode(_) => false,
            Term::Literal(_) => true,
        }
    }

    /// Returns the [effective boolean value](https://www.w3.org/TR/sparql11-query/#ebv) of the term if it exists
    pub fn to_bool(&self) -> Option<bool> {
        match self {
            Term::Literal(literal) => literal.to_bool(),
            _ => None,
        }
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
            NamedOrBlankNode::NamedNode(node) => Term::NamedNode(node),
            NamedOrBlankNode::BlankNode(node) => Term::BlankNode(node),
        }
    }
}

/// A [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple)
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Triple {
    subject: NamedOrBlankNode,
    predicate: NamedNode,
    object: Term,
}

impl Triple {
    /// Builds a RDF [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple)
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

    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple
    pub fn subject(&self) -> &NamedOrBlankNode {
        &self.subject
    }

    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple
    pub fn subject_owned(self) -> NamedOrBlankNode {
        self.subject
    }

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple
    pub fn predicate(&self) -> &NamedNode {
        &self.predicate
    }

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple
    pub fn predicate_owned(self) -> NamedNode {
        self.predicate
    }

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple
    pub fn object(&self) -> &Term {
        &self.object
    }

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple
    pub fn object_owned(self) -> Term {
        self.object
    }

    /// Encodes that this triple is in a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
    pub fn in_graph(self, graph_name: Option<NamedOrBlankNode>) -> Quad {
        Quad {
            subject: self.subject,
            predicate: self.predicate,
            object: self.object,
            graph_name,
        }
    }
}

impl fmt::Display for Triple {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {} .", self.subject, self.predicate, self.object)
    }
}

/// A [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Quad {
    subject: NamedOrBlankNode,
    predicate: NamedNode,
    object: Term,
    graph_name: Option<NamedOrBlankNode>,
}

impl Quad {
    /// Builds a RDF [triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
    pub fn new(
        subject: impl Into<NamedOrBlankNode>,
        predicate: impl Into<NamedNode>,
        object: impl Into<Term>,
        graph_name: impl Into<Option<NamedOrBlankNode>>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            graph_name: graph_name.into(),
        }
    }

    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple
    pub fn subject(&self) -> &NamedOrBlankNode {
        &self.subject
    }

    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple
    pub fn subject_owned(self) -> NamedOrBlankNode {
        self.subject
    }

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple
    pub fn predicate(&self) -> &NamedNode {
        &self.predicate
    }

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple
    pub fn predicate_owned(self) -> NamedNode {
        self.predicate
    }

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple
    pub fn object(&self) -> &Term {
        &self.object
    }

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple
    pub fn object_owned(self) -> Term {
        self.object
    }

    /// The name of the RDF [graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) in which the triple is or None if it is in the [default graph](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph)
    pub fn graph_name(&self) -> &Option<NamedOrBlankNode> {
        &self.graph_name
    }

    /// The name of the RDF [graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) in which the triple is or None if it is in the [default graph](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph)
    pub fn graph_name_owned(self) -> Option<NamedOrBlankNode> {
        self.graph_name
    }
}

impl fmt::Display for Quad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.graph_name {
            Some(ref graph_name) => write!(
                f,
                "{} {} {} {} .",
                self.subject, self.predicate, self.object, graph_name
            ),
            None => write!(f, "{} {} {} .", self.subject, self.predicate, self.object),
        }
    }
}
