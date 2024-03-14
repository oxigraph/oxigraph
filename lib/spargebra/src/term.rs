//! Data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/) like IRI, literal or triples.

pub use oxrdf::{BlankNode, Literal, NamedNode, Subject, Term, Triple, Variable};
use std::fmt;
use std::fmt::Write;

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [triples](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GroundSubject {
    NamedNode(NamedNode),
    #[cfg(feature = "rdf-star")]
    Triple(Box<GroundTriple>),
}

impl fmt::Display for GroundSubject {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => write!(
                f,
                "<<{} {} {}>>",
                triple.subject, triple.predicate, triple.object
            ),
        }
    }
}

impl From<NamedNode> for GroundSubject {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

#[cfg(feature = "rdf-star")]
impl From<GroundTriple> for GroundSubject {
    #[inline]
    fn from(triple: GroundTriple) -> Self {
        Self::Triple(Box::new(triple))
    }
}

impl TryFrom<Subject> for GroundSubject {
    type Error = ();

    #[inline]
    fn try_from(subject: Subject) -> Result<Self, Self::Error> {
        match subject {
            Subject::NamedNode(t) => Ok(t.into()),
            Subject::BlankNode(_) => Err(()),
            #[cfg(feature = "rdf-star")]
            Subject::Triple(t) => Ok(GroundTriple::try_from(*t)?.into()),
        }
    }
}

impl TryFrom<GroundTerm> for GroundSubject {
    type Error = ();

    #[inline]
    fn try_from(term: GroundTerm) -> Result<Self, Self::Error> {
        match term {
            GroundTerm::NamedNode(t) => Ok(t.into()),
            GroundTerm::Literal(_) => Err(()),
            #[cfg(feature = "rdf-star")]
            GroundTerm::Triple(t) => Ok((*t).into()),
        }
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) and [triples](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple).
///
/// The default string formatter is returning an N-Triples, Turtle, and SPARQL compatible representation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GroundTerm {
    NamedNode(NamedNode),
    Literal(Literal),
    #[cfg(feature = "rdf-star")]
    Triple(Box<GroundTriple>),
}

impl fmt::Display for GroundTerm {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::Literal(literal) => literal.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => write!(
                f,
                "<<{} {} {}>>",
                triple.subject, triple.predicate, triple.object
            ),
        }
    }
}

impl From<NamedNode> for GroundTerm {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<Literal> for GroundTerm {
    #[inline]
    fn from(literal: Literal) -> Self {
        Self::Literal(literal)
    }
}

#[cfg(feature = "rdf-star")]
impl From<GroundTriple> for GroundTerm {
    #[inline]
    fn from(triple: GroundTriple) -> Self {
        Self::Triple(Box::new(triple))
    }
}

impl TryFrom<Term> for GroundTerm {
    type Error = ();

    #[inline]
    fn try_from(term: Term) -> Result<Self, Self::Error> {
        match term {
            Term::NamedNode(t) => Ok(t.into()),
            Term::BlankNode(_) => Err(()),
            Term::Literal(t) => Ok(t.into()),
            #[cfg(feature = "rdf-star")]
            Term::Triple(t) => Ok(GroundTriple::try_from(*t)?.into()),
        }
    }
}

/// A [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) without blank nodes.
///
/// The default string formatter is returning a N-Quads representation.
///
/// ```
/// use spargebra::term::{GroundTriple, NamedNode};
///
/// assert_eq!(
///     "<http://example.com/s> <http://example.com/p> <http://example.com/o>",
///     GroundTriple {
///         subject: NamedNode::new("http://example.com/s")?.into(),
///         predicate: NamedNode::new("http://example.com/p")?,
///         object: NamedNode::new("http://example.com/o")?.into(),
///     }
///     .to_string()
/// );
/// # Result::<_,oxrdf::IriParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct GroundTriple {
    pub subject: GroundSubject,
    pub predicate: NamedNode,
    pub object: GroundTerm,
}

impl fmt::Display for GroundTriple {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}

impl TryFrom<Triple> for GroundTriple {
    type Error = ();

    #[inline]
    fn try_from(triple: Triple) -> Result<Self, Self::Error> {
        Ok(Self {
            subject: triple.subject.try_into()?,
            predicate: triple.predicate,
            object: triple.object.try_into()?,
        })
    }
}

/// A possible graph name.
///
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and the [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph).
#[derive(Eq, PartialEq, Debug, Clone, Hash, Default)]
pub enum GraphName {
    NamedNode(NamedNode),
    #[default]
    DefaultGraph,
}

impl GraphName {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        match self {
            Self::NamedNode(node) => write!(f, "{node}"),
            Self::DefaultGraph => f.write_str("default"),
        }
    }
}

impl fmt::Display for GraphName {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
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

impl TryFrom<GraphNamePattern> for GraphName {
    type Error = ();

    #[inline]
    fn try_from(pattern: GraphNamePattern) -> Result<Self, Self::Error> {
        match pattern {
            GraphNamePattern::NamedNode(t) => Ok(t.into()),
            GraphNamePattern::DefaultGraph => Ok(Self::DefaultGraph),
            GraphNamePattern::Variable(_) => Err(()),
        }
    }
}

/// A [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
///
/// The default string formatter is returning a N-Quads representation.
///
/// ```
/// use spargebra::term::{NamedNode, Quad};
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
pub struct Quad {
    pub subject: Subject,
    pub predicate: NamedNode,
    pub object: Term,
    pub graph_name: GraphName,
}

impl Quad {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        if self.graph_name != GraphName::DefaultGraph {
            f.write_str("(graph ")?;
            self.graph_name.fmt_sse(f)?;
            f.write_str(" (")?;
        }
        write!(
            f,
            "(triple {} {} {})",
            self.subject, self.predicate, self.object
        )?;
        if self.graph_name != GraphName::DefaultGraph {
            f.write_str("))")?;
        }
        Ok(())
    }
}

impl fmt::Display for Quad {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphName::DefaultGraph {
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

impl TryFrom<QuadPattern> for Quad {
    type Error = ();

    #[inline]
    fn try_from(quad: QuadPattern) -> Result<Self, Self::Error> {
        Ok(Self {
            subject: quad.subject.try_into()?,
            predicate: quad.predicate.try_into()?,
            object: quad.object.try_into()?,
            graph_name: quad.graph_name.try_into()?,
        })
    }
}

/// A [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) without blank nodes.
///
/// The default string formatter is returning a N-Quads representation.
///
/// ```
/// use spargebra::term::{NamedNode, GroundQuad};
///
/// assert_eq!(
///     "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g>",
///     GroundQuad {
///         subject: NamedNode::new("http://example.com/s")?.into(),
///         predicate: NamedNode::new("http://example.com/p")?,
///         object: NamedNode::new("http://example.com/o")?.into(),
///         graph_name: NamedNode::new("http://example.com/g")?.into(),
///     }.to_string()
/// );
/// # Result::<_,oxrdf::IriParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct GroundQuad {
    pub subject: GroundSubject,
    pub predicate: NamedNode,
    pub object: GroundTerm,
    pub graph_name: GraphName,
}

impl GroundQuad {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        if self.graph_name != GraphName::DefaultGraph {
            f.write_str("(graph ")?;
            self.graph_name.fmt_sse(f)?;
            f.write_str(" (")?;
        }
        write!(
            f,
            "(triple {} {} {})",
            self.subject, self.predicate, self.object
        )?;
        if self.graph_name != GraphName::DefaultGraph {
            f.write_str("))")?;
        }
        Ok(())
    }
}

impl fmt::Display for GroundQuad {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphName::DefaultGraph {
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

impl TryFrom<Quad> for GroundQuad {
    type Error = ();

    #[inline]
    fn try_from(quad: Quad) -> Result<Self, Self::Error> {
        Ok(Self {
            subject: quad.subject.try_into()?,
            predicate: quad.predicate,
            object: quad.object.try_into()?,
            graph_name: quad.graph_name,
        })
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum NamedNodePattern {
    NamedNode(NamedNode),
    Variable(Variable),
}

impl NamedNodePattern {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        match self {
            Self::NamedNode(node) => write!(f, "{node}"),
            Self::Variable(var) => write!(f, "{var}"),
        }
    }
}

impl fmt::Display for NamedNodePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::Variable(var) => var.fmt(f),
        }
    }
}

impl From<NamedNode> for NamedNodePattern {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<Variable> for NamedNodePattern {
    #[inline]
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl TryFrom<NamedNodePattern> for NamedNode {
    type Error = ();

    #[inline]
    fn try_from(pattern: NamedNodePattern) -> Result<Self, Self::Error> {
        match pattern {
            NamedNodePattern::NamedNode(t) => Ok(t),
            NamedNodePattern::Variable(_) => Err(()),
        }
    }
}

/// The union of [terms](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum TermPattern {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Literal(Literal),
    #[cfg(feature = "rdf-star")]
    Triple(Box<TriplePattern>),
    Variable(Variable),
}

impl TermPattern {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        match self {
            Self::NamedNode(term) => write!(f, "{term}"),
            Self::BlankNode(term) => write!(f, "{term}"),
            Self::Literal(term) => write!(f, "{term}"),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => triple.fmt_sse(f),
            Self::Variable(var) => write!(f, "{var}"),
        }
    }
}

impl fmt::Display for TermPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(term) => term.fmt(f),
            Self::BlankNode(term) => term.fmt(f),
            Self::Literal(term) => term.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => write!(f, "<<{triple}>>"),
            Self::Variable(var) => var.fmt(f),
        }
    }
}

impl From<NamedNode> for TermPattern {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<BlankNode> for TermPattern {
    #[inline]
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

impl From<Literal> for TermPattern {
    #[inline]
    fn from(literal: Literal) -> Self {
        Self::Literal(literal)
    }
}

#[cfg(feature = "rdf-star")]
impl From<TriplePattern> for TermPattern {
    #[inline]
    fn from(triple: TriplePattern) -> Self {
        Self::Triple(Box::new(triple))
    }
}

impl From<Variable> for TermPattern {
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl From<Subject> for TermPattern {
    #[inline]
    fn from(subject: Subject) -> Self {
        match subject {
            Subject::NamedNode(node) => node.into(),
            Subject::BlankNode(node) => node.into(),
            #[cfg(feature = "rdf-star")]
            Subject::Triple(t) => TriplePattern::from(*t).into(),
        }
    }
}

impl From<Term> for TermPattern {
    #[inline]
    fn from(term: Term) -> Self {
        match term {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            Term::Literal(literal) => literal.into(),
            #[cfg(feature = "rdf-star")]
            Term::Triple(t) => TriplePattern::from(*t).into(),
        }
    }
}

impl From<NamedNodePattern> for TermPattern {
    #[inline]
    fn from(element: NamedNodePattern) -> Self {
        match element {
            NamedNodePattern::NamedNode(node) => node.into(),
            NamedNodePattern::Variable(var) => var.into(),
        }
    }
}

impl From<GroundTermPattern> for TermPattern {
    #[inline]
    fn from(element: GroundTermPattern) -> Self {
        match element {
            GroundTermPattern::NamedNode(node) => node.into(),
            GroundTermPattern::Literal(literal) => literal.into(),
            #[cfg(feature = "rdf-star")]
            GroundTermPattern::Triple(t) => TriplePattern::from(*t).into(),
            GroundTermPattern::Variable(variable) => variable.into(),
        }
    }
}

impl TryFrom<TermPattern> for Subject {
    type Error = ();

    #[inline]
    fn try_from(term: TermPattern) -> Result<Self, Self::Error> {
        match term {
            TermPattern::NamedNode(t) => Ok(t.into()),
            TermPattern::BlankNode(t) => Ok(t.into()),
            #[cfg(feature = "rdf-star")]
            TermPattern::Triple(t) => Ok(Triple::try_from(*t)?.into()),
            TermPattern::Literal(_) | TermPattern::Variable(_) => Err(()),
        }
    }
}

impl TryFrom<TermPattern> for Term {
    type Error = ();

    #[inline]
    fn try_from(pattern: TermPattern) -> Result<Self, Self::Error> {
        match pattern {
            TermPattern::NamedNode(t) => Ok(t.into()),
            TermPattern::BlankNode(t) => Ok(t.into()),
            TermPattern::Literal(t) => Ok(t.into()),
            #[cfg(feature = "rdf-star")]
            TermPattern::Triple(t) => Ok(Triple::try_from(*t)?.into()),
            TermPattern::Variable(_) => Err(()),
        }
    }
}
/// The union of [terms](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables) without blank nodes.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GroundTermPattern {
    NamedNode(NamedNode),
    Literal(Literal),
    Variable(Variable),
    #[cfg(feature = "rdf-star")]
    Triple(Box<GroundTriplePattern>),
}

impl GroundTermPattern {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        match self {
            Self::NamedNode(term) => write!(f, "{term}"),
            Self::Literal(term) => write!(f, "{term}"),
            Self::Variable(var) => write!(f, "{var}"),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => triple.fmt_sse(f),
        }
    }
}

impl fmt::Display for GroundTermPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(term) => term.fmt(f),
            Self::Literal(term) => term.fmt(f),
            Self::Variable(var) => var.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => write!(f, "<<{triple}>>"),
        }
    }
}

impl From<NamedNode> for GroundTermPattern {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<Literal> for GroundTermPattern {
    #[inline]
    fn from(literal: Literal) -> Self {
        Self::Literal(literal)
    }
}

#[cfg(feature = "rdf-star")]
impl From<GroundTriplePattern> for GroundTermPattern {
    #[inline]
    fn from(triple: GroundTriplePattern) -> Self {
        Self::Triple(Box::new(triple))
    }
}

impl From<Variable> for GroundTermPattern {
    #[inline]
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl From<GroundSubject> for GroundTermPattern {
    #[inline]
    fn from(term: GroundSubject) -> Self {
        match term {
            GroundSubject::NamedNode(node) => node.into(),
            #[cfg(feature = "rdf-star")]
            GroundSubject::Triple(triple) => GroundTriplePattern::from(*triple).into(),
        }
    }
}
impl From<GroundTerm> for GroundTermPattern {
    #[inline]
    fn from(term: GroundTerm) -> Self {
        match term {
            GroundTerm::NamedNode(node) => node.into(),
            GroundTerm::Literal(literal) => literal.into(),
            #[cfg(feature = "rdf-star")]
            GroundTerm::Triple(triple) => GroundTriplePattern::from(*triple).into(),
        }
    }
}

impl From<NamedNodePattern> for GroundTermPattern {
    #[inline]
    fn from(element: NamedNodePattern) -> Self {
        match element {
            NamedNodePattern::NamedNode(node) => node.into(),
            NamedNodePattern::Variable(var) => var.into(),
        }
    }
}

impl TryFrom<TermPattern> for GroundTermPattern {
    type Error = ();

    #[inline]
    fn try_from(pattern: TermPattern) -> Result<Self, Self::Error> {
        Ok(match pattern {
            TermPattern::NamedNode(named_node) => named_node.into(),
            TermPattern::BlankNode(_) => return Err(()),
            TermPattern::Literal(literal) => literal.into(),
            #[cfg(feature = "rdf-star")]
            TermPattern::Triple(triple) => GroundTriplePattern::try_from(*triple)?.into(),
            TermPattern::Variable(variable) => variable.into(),
        })
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphNamePattern {
    NamedNode(NamedNode),
    DefaultGraph,
    Variable(Variable),
}

impl GraphNamePattern {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        match self {
            Self::NamedNode(node) => write!(f, "{node}"),
            Self::DefaultGraph => f.write_str("default"),
            Self::Variable(var) => write!(f, "{var}"),
        }
    }
}

impl fmt::Display for GraphNamePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::DefaultGraph => f.write_str("DEFAULT"),
            Self::Variable(var) => var.fmt(f),
        }
    }
}

impl From<NamedNode> for GraphNamePattern {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<Variable> for GraphNamePattern {
    #[inline]
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl From<GraphName> for GraphNamePattern {
    #[inline]
    fn from(graph_name: GraphName) -> Self {
        match graph_name {
            GraphName::NamedNode(node) => node.into(),
            GraphName::DefaultGraph => Self::DefaultGraph,
        }
    }
}

impl From<NamedNodePattern> for GraphNamePattern {
    #[inline]
    fn from(graph_name: NamedNodePattern) -> Self {
        match graph_name {
            NamedNodePattern::NamedNode(node) => node.into(),
            NamedNodePattern::Variable(var) => var.into(),
        }
    }
}

/// A [triple pattern](https://www.w3.org/TR/sparql11-query/#defn_TriplePattern)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct TriplePattern {
    pub subject: TermPattern,
    pub predicate: NamedNodePattern,
    pub object: TermPattern,
}

impl TriplePattern {
    pub(crate) fn new(
        subject: impl Into<TermPattern>,
        predicate: impl Into<NamedNodePattern>,
        object: impl Into<TermPattern>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
        }
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        f.write_str("(triple ")?;
        self.subject.fmt_sse(f)?;
        f.write_str(" ")?;
        self.predicate.fmt_sse(f)?;
        f.write_str(" ")?;
        self.object.fmt_sse(f)?;
        f.write_str(")")
    }
}

impl fmt::Display for TriplePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}

impl From<Triple> for TriplePattern {
    #[inline]
    fn from(triple: Triple) -> Self {
        Self {
            subject: triple.subject.into(),
            predicate: triple.predicate.into(),
            object: triple.object.into(),
        }
    }
}

impl From<GroundTriplePattern> for TriplePattern {
    #[inline]
    fn from(triple: GroundTriplePattern) -> Self {
        Self {
            subject: triple.subject.into(),
            predicate: triple.predicate,
            object: triple.object.into(),
        }
    }
}

impl TryFrom<TriplePattern> for Triple {
    type Error = ();

    #[inline]
    fn try_from(triple: TriplePattern) -> Result<Self, Self::Error> {
        Ok(Self {
            subject: triple.subject.try_into()?,
            predicate: triple.predicate.try_into()?,
            object: triple.object.try_into()?,
        })
    }
}

/// A [triple pattern](https://www.w3.org/TR/sparql11-query/#defn_TriplePattern) without blank nodes.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct GroundTriplePattern {
    pub subject: GroundTermPattern,
    pub predicate: NamedNodePattern,
    pub object: GroundTermPattern,
}

impl GroundTriplePattern {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    #[allow(dead_code)]
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        f.write_str("(triple ")?;
        self.subject.fmt_sse(f)?;
        f.write_str(" ")?;
        self.predicate.fmt_sse(f)?;
        f.write_str(" ")?;
        self.object.fmt_sse(f)?;
        f.write_str(")")
    }
}

impl fmt::Display for GroundTriplePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}

impl From<GroundTriple> for GroundTriplePattern {
    #[inline]
    fn from(triple: GroundTriple) -> Self {
        Self {
            subject: triple.subject.into(),
            predicate: triple.predicate.into(),
            object: triple.object.into(),
        }
    }
}

impl TryFrom<TriplePattern> for GroundTriplePattern {
    type Error = ();

    #[inline]
    fn try_from(triple: TriplePattern) -> Result<Self, Self::Error> {
        Ok(Self {
            subject: triple.subject.try_into()?,
            predicate: triple.predicate,
            object: triple.object.try_into()?,
        })
    }
}

/// A [triple pattern](https://www.w3.org/TR/sparql11-query/#defn_TriplePattern) in a specific graph
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct QuadPattern {
    pub subject: TermPattern,
    pub predicate: NamedNodePattern,
    pub object: TermPattern,
    pub graph_name: GraphNamePattern,
}

impl QuadPattern {
    pub(crate) fn new(
        subject: impl Into<TermPattern>,
        predicate: impl Into<NamedNodePattern>,
        object: impl Into<TermPattern>,
        graph_name: impl Into<GraphNamePattern>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            graph_name: graph_name.into(),
        }
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        if self.graph_name != GraphNamePattern::DefaultGraph {
            f.write_str("(graph ")?;
            self.graph_name.fmt_sse(f)?;
            f.write_str(" (")?;
        }
        f.write_str("(triple ")?;
        self.subject.fmt_sse(f)?;
        f.write_str(" ")?;
        self.predicate.fmt_sse(f)?;
        f.write_str(" ")?;
        self.object.fmt_sse(f)?;
        f.write_str(")")?;
        if self.graph_name != GraphNamePattern::DefaultGraph {
            f.write_str("))")?;
        }
        Ok(())
    }
}

impl fmt::Display for QuadPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphNamePattern::DefaultGraph {
            write!(f, "{} {} {}", self.subject, self.predicate, self.object)
        } else {
            write!(
                f,
                "GRAPH {} {{ {} {} {} }}",
                self.graph_name, self.subject, self.predicate, self.object
            )
        }
    }
}

/// A [triple pattern](https://www.w3.org/TR/sparql11-query/#defn_TriplePattern) in a specific graph without blank nodes.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct GroundQuadPattern {
    pub subject: GroundTermPattern,
    pub predicate: NamedNodePattern,
    pub object: GroundTermPattern,
    pub graph_name: GraphNamePattern,
}

impl GroundQuadPattern {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl Write) -> fmt::Result {
        if self.graph_name != GraphNamePattern::DefaultGraph {
            f.write_str("(graph ")?;
            self.graph_name.fmt_sse(f)?;
            f.write_str(" (")?;
        }
        f.write_str("(triple ")?;
        self.subject.fmt_sse(f)?;
        f.write_str(" ")?;
        self.predicate.fmt_sse(f)?;
        f.write_str(" ")?;
        self.object.fmt_sse(f)?;
        f.write_str(")")?;
        if self.graph_name != GraphNamePattern::DefaultGraph {
            f.write_str("))")?;
        }
        Ok(())
    }
}

impl fmt::Display for GroundQuadPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphNamePattern::DefaultGraph {
            write!(f, "{} {} {}", self.subject, self.predicate, self.object)
        } else {
            write!(
                f,
                "GRAPH {} {{ {} {} {} }}",
                self.graph_name, self.subject, self.predicate, self.object
            )
        }
    }
}

impl TryFrom<QuadPattern> for GroundQuadPattern {
    type Error = ();

    #[inline]
    fn try_from(pattern: QuadPattern) -> Result<Self, Self::Error> {
        Ok(Self {
            subject: pattern.subject.try_into()?,
            predicate: pattern.predicate,
            object: pattern.object.try_into()?,
            graph_name: pattern.graph_name,
        })
    }
}
