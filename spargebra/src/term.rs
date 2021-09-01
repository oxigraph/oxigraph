//! Data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/) like IRI, literal or triples.

use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::fmt::Write;

/// An RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri).
///
/// The default string formatter is returning an N-Triples, Turtle and SPARQL compatible representation.
///
/// ```
/// use spargebra::term::NamedNode;
///
/// assert_eq!(
///     "<http://example.com/foo>",
///     NamedNode { iri: "http://example.com/foo".into() }.to_string()
/// )
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Hash)]
pub struct NamedNode {
    /// The [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) itself.
    pub iri: String,
}

impl fmt::Debug for NamedNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.iri)
    }
}

impl fmt::Display for NamedNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.iri)
    }
}

impl TryFrom<NamedNodePattern> for NamedNode {
    type Error = ();

    #[inline]
    fn try_from(pattern: NamedNodePattern) -> Result<Self, ()> {
        match pattern {
            NamedNodePattern::NamedNode(t) => Ok(t),
            NamedNodePattern::Variable(_) => Err(()),
        }
    }
}

/// An RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
///
///
/// The default string formatter is returning an N-Triples, Turtle and SPARQL compatible representation.
///
/// ```
/// use spargebra::term::BlankNode;
///
/// assert_eq!(
///     "_:a1",
///     BlankNode { id: "a1".into() }.to_string()
/// )
/// ```
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct BlankNode {
    /// The [blank node identifier](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node-identifier).
    pub id: String,
}

impl fmt::Debug for BlankNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_:{}", self.id)
    }
}

impl fmt::Display for BlankNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_:{}", self.id)
    }
}

/// An RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
///
/// The default string formatter is returning an N-Triples, Turtle and SPARQL compatible representation.
///
/// The language tags should be lowercased  [as suggested by the RDF specification](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
///
/// ```
/// use spargebra::term::NamedNode;
/// use spargebra::term::Literal;
///
/// assert_eq!(
///     "\"foo\\nbar\"",
///     Literal::Simple { value: "foo\nbar".into() }.to_string()
/// );
///
/// assert_eq!(
///     "\"1999-01-01\"^^<http://www.w3.org/2001/XMLSchema#date>",
///     Literal::Typed { value: "1999-01-01".into(), datatype: NamedNode { iri: "http://www.w3.org/2001/XMLSchema#date".into() }}.to_string()
/// );
///
/// assert_eq!(
///     "\"foo\"@en",
///     Literal::LanguageTaggedString { value: "foo".into(), language: "en".into() }.to_string()
/// );
/// ```
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum Literal {
    /// A [simple literal](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal) without datatype or language form.
    Simple {
        /// The [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form).
        value: String,
    },
    /// A [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    LanguageTaggedString {
        /// The [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form).
        value: String,
        /// The [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag).
        language: String,
    },
    /// A literal with an explicit datatype
    Typed {
        /// The [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form).
        value: String,
        /// The [datatype IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
        datatype: NamedNode,
    },
}

impl fmt::Debug for Literal {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Simple { value } => print_quoted_str(value, f),
            Literal::LanguageTaggedString { value, language } => {
                print_quoted_str(value, f)?;
                write!(f, "@{}", language)
            }
            Literal::Typed { value, datatype } => {
                print_quoted_str(value, f)?;
                write!(f, "^^{}", datatype)
            }
        }
    }
}

impl fmt::Display for Literal {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Simple { value } => print_quoted_str(value, f),
            Literal::LanguageTaggedString { value, language } => {
                print_quoted_str(value, f)?;
                write!(f, "@{}", language)
            }
            Literal::Typed { value, datatype } => {
                print_quoted_str(value, f)?;
                write!(f, "^^{}", datatype)
            }
        }
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
///
/// The default string formatter is returning an N-Triples, Turtle and SPARQL compatible representation.
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum Subject {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    #[cfg(feature = "rdf-star")]
    Triple(Box<Triple>),
}

impl fmt::Debug for Subject {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::BlankNode(node) => node.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => triple.fmt(f),
        }
    }
}

impl fmt::Display for Subject {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::BlankNode(node) => node.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => write!(
                f,
                "<<{} {} {}>>",
                triple.subject, triple.predicate, triple.object
            ),
        }
    }
}

impl From<NamedNode> for Subject {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<BlankNode> for Subject {
    #[inline]
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

#[cfg(feature = "rdf-star")]
impl From<Triple> for Subject {
    #[inline]
    fn from(triple: Triple) -> Self {
        Self::Triple(Box::new(triple))
    }
}

impl TryFrom<TermPattern> for Subject {
    type Error = ();

    #[inline]
    fn try_from(term: TermPattern) -> Result<Self, ()> {
        match term {
            TermPattern::NamedNode(t) => Ok(t.into()),
            TermPattern::BlankNode(t) => Ok(t.into()),
            #[cfg(feature = "rdf-star")]
            TermPattern::Triple(t) => Ok(Triple::try_from(*t)?.into()),
            TermPattern::Literal(_) | TermPattern::Variable(_) => Err(()),
        }
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [triples](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple).
///
/// The default string formatter is returning an N-Triples, Turtle and SPARQL compatible representation.
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum GroundSubject {
    NamedNode(NamedNode),
    #[cfg(feature = "rdf-star")]
    Triple(Box<GroundTriple>),
}

impl fmt::Debug for GroundSubject {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => triple.fmt(f),
        }
    }
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
    fn try_from(subject: Subject) -> Result<Self, ()> {
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
    fn try_from(term: GroundTerm) -> Result<Self, ()> {
        match term {
            GroundTerm::NamedNode(t) => Ok(t.into()),
            GroundTerm::Literal(_) => Err(()),
            #[cfg(feature = "rdf-star")]
            GroundTerm::Triple(t) => Ok((*t).into()),
        }
    }
}

/// An RDF [term](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term).
///
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) and [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
///
/// The default string formatter is returning an N-Triples, Turtle and SPARQL compatible representation.
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum Term {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Literal(Literal),
    #[cfg(feature = "rdf-star")]
    Triple(Box<Triple>),
}

impl fmt::Debug for Term {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::BlankNode(node) => node.fmt(f),
            Self::Literal(literal) => literal.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => triple.fmt(f),
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
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => write!(
                f,
                "<<{} {} {}>>",
                triple.subject, triple.predicate, triple.object
            ),
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

#[cfg(feature = "rdf-star")]
impl From<Triple> for Term {
    #[inline]
    fn from(triple: Triple) -> Self {
        Self::Triple(Box::new(triple))
    }
}

impl From<Subject> for Term {
    #[inline]
    fn from(resource: Subject) -> Self {
        match resource {
            Subject::NamedNode(node) => node.into(),
            Subject::BlankNode(node) => node.into(),
            #[cfg(feature = "rdf-star")]
            Subject::Triple(t) => (*t).into(),
        }
    }
}

impl TryFrom<TermPattern> for Term {
    type Error = ();

    #[inline]
    fn try_from(pattern: TermPattern) -> Result<Self, ()> {
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

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) and [triples](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple).
///
/// The default string formatter is returning an N-Triples, Turtle and SPARQL compatible representation.
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum GroundTerm {
    NamedNode(NamedNode),
    Literal(Literal),
    #[cfg(feature = "rdf-star")]
    Triple(Box<GroundTriple>),
}

impl fmt::Debug for GroundTerm {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::Literal(literal) => literal.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => triple.fmt(f),
        }
    }
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
    fn try_from(term: Term) -> Result<Self, ()> {
        match term {
            Term::NamedNode(t) => Ok(t.into()),
            Term::BlankNode(_) => Err(()),
            Term::Literal(t) => Ok(t.into()),
            #[cfg(feature = "rdf-star")]
            Term::Triple(t) => Ok(GroundTriple::try_from(*t)?.into()),
        }
    }
}

/// A [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple).
///
/// The default string formatter is returning a N-Quads representation.
///
/// ```
/// use spargebra::term::NamedNode;
/// use spargebra::term::Triple;
///
/// assert_eq!(
///     "<http://example.com/foo> <http://schema.org/sameAs> <http://example.com/foo>",
///     Triple {
///         subject: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         predicate: NamedNode { iri: "http://schema.org/sameAs".into() },
///         object: NamedNode { iri: "http://example.com/foo".into() }.into(),
///     }.to_string()
/// )
/// ```
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct Triple {
    pub subject: Subject,
    pub predicate: NamedNode,
    pub object: Term,
}

impl fmt::Debug for Triple {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(triple {:?} {:?} {:?})",
            self.subject, self.predicate, self.object
        )
    }
}

impl fmt::Display for Triple {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}

impl TryFrom<TriplePattern> for Triple {
    type Error = ();

    #[inline]
    fn try_from(triple: TriplePattern) -> Result<Self, ()> {
        Ok(Self {
            subject: triple.subject.try_into()?,
            predicate: triple.predicate.try_into()?,
            object: triple.object.try_into()?,
        })
    }
}

/// A [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) without blank nodes.
///
/// The default string formatter is returning a N-Quads representation.
///
/// ```
/// use spargebra::term::NamedNode;
/// use spargebra::term::GroundTriple;
///
/// assert_eq!(
///     "<http://example.com/foo> <http://schema.org/sameAs> <http://example.com/foo>",
///     GroundTriple {
///         subject: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         predicate: NamedNode { iri: "http://schema.org/sameAs".into() },
///         object: NamedNode { iri: "http://example.com/foo".into() }.into(),
///     }.to_string()
/// )
/// ```
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct GroundTriple {
    pub subject: GroundSubject,
    pub predicate: NamedNode,
    pub object: GroundTerm,
}

impl fmt::Debug for GroundTriple {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(triple {:?} {:?} {:?})",
            self.subject, self.predicate, self.object
        )
    }
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
    fn try_from(triple: Triple) -> Result<Self, ()> {
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
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum GraphName {
    NamedNode(NamedNode),
    DefaultGraph,
}

impl fmt::Debug for GraphName {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::DefaultGraph => write!(f, "default"),
        }
    }
}

impl fmt::Display for GraphName {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::DefaultGraph => write!(f, "DEFAULT"),
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
    fn try_from(pattern: GraphNamePattern) -> Result<Self, ()> {
        match pattern {
            GraphNamePattern::NamedNode(t) => Ok(t.into()),
            GraphNamePattern::DefaultGraph => Ok(Self::DefaultGraph),
            GraphNamePattern::Variable(_) => Err(()),
        }
    }
}

/// A [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
///
/// The default string formatter is returning a N-Quads representation.
///
/// ```
/// use spargebra::term::NamedNode;
/// use spargebra::term::Quad;
///
/// assert_eq!(
///     "<http://example.com/foo> <http://schema.org/sameAs> <http://example.com/foo> <http://example.com/>",
///     Quad {
///         subject: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         predicate: NamedNode { iri: "http://schema.org/sameAs".into() },
///         object: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         graph_name: NamedNode { iri: "http://example.com/".into() }.into(),
///     }.to_string()
/// )
/// ```
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct Quad {
    pub subject: Subject,
    pub predicate: NamedNode,
    pub object: Term,
    pub graph_name: GraphName,
}

impl fmt::Debug for Quad {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphName::DefaultGraph {
            write!(
                f,
                "(triple {:?} {:?} {:?})",
                self.subject, self.predicate, self.object
            )
        } else {
            write!(
                f,
                "(graph {:?} ((triple {:?} {:?} {:?})))",
                self.graph_name, self.subject, self.predicate, self.object
            )
        }
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
    fn try_from(quad: QuadPattern) -> Result<Self, ()> {
        Ok(Self {
            subject: quad.subject.try_into()?,
            predicate: quad.predicate.try_into()?,
            object: quad.object.try_into()?,
            graph_name: quad.graph_name.try_into()?,
        })
    }
}

/// A [RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) in a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) without blank nodes.
///
/// The default string formatter is returning a N-Quads representation.
///
/// ```
/// use spargebra::term::NamedNode;
/// use spargebra::term::GroundQuad;
///
/// assert_eq!(
///     "<http://example.com/foo> <http://schema.org/sameAs> <http://example.com/foo> <http://example.com/>",
///     GroundQuad {
///         subject: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         predicate: NamedNode { iri: "http://schema.org/sameAs".into() },
///         object: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         graph_name: NamedNode { iri: "http://example.com/".into() }.into(),
///     }.to_string()
/// )
/// ```
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct GroundQuad {
    pub subject: GroundSubject,
    pub predicate: NamedNode,
    pub object: GroundTerm,
    pub graph_name: GraphName,
}

impl fmt::Debug for GroundQuad {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphName::DefaultGraph {
            write!(
                f,
                "(triple {:?} {:?} {:?})",
                self.subject, self.predicate, self.object
            )
        } else {
            write!(
                f,
                "(graph {:?} ((triple {:?} {:?} {:?})))",
                self.graph_name, self.subject, self.predicate, self.object
            )
        }
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
    fn try_from(quad: Quad) -> Result<Self, ()> {
        Ok(Self {
            subject: quad.subject.try_into()?,
            predicate: quad.predicate,
            object: quad.object.try_into()?,
            graph_name: quad.graph_name,
        })
    }
}

/// A [SPARQL query variable](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
///
/// ```
/// use spargebra::term::Variable;
///
/// assert_eq!(
///     "?foo",
///     Variable { name: "foo".into() }.to_string()
/// );
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Hash)]
pub struct Variable {
    pub name: String,
}

impl fmt::Debug for Variable {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.name)
    }
}

impl fmt::Display for Variable {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.name)
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum NamedNodePattern {
    NamedNode(NamedNode),
    Variable(Variable),
}

impl fmt::Debug for NamedNodePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::Variable(var) => var.fmt(f),
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

/// The union of [terms](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum TermPattern {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Literal(Literal),
    #[cfg(feature = "rdf-star")]
    Triple(Box<TriplePattern>),
    Variable(Variable),
}

impl fmt::Debug for TermPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(term) => term.fmt(f),
            Self::BlankNode(term) => term.fmt(f),
            Self::Literal(term) => term.fmt(f),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => triple.fmt(f),
            Self::Variable(var) => var.fmt(f),
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
            Self::Triple(triple) => write!(f, "<<{}>>", triple),
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

/// The union of [terms](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables) without blank nodes.
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum GroundTermPattern {
    NamedNode(NamedNode),
    Literal(Literal),
    Variable(Variable),
    Triple(Box<GroundTriplePattern>),
}

impl fmt::Debug for GroundTermPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(term) => term.fmt(f),
            Self::Literal(term) => term.fmt(f),
            Self::Variable(var) => var.fmt(f),
            Self::Triple(triple) => write!(f, "<<{}>>", triple),
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
            Self::Triple(triple) => write!(f, "<<{}>>", triple),
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
    fn try_from(pattern: TermPattern) -> Result<Self, ()> {
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
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum GraphNamePattern {
    NamedNode(NamedNode),
    DefaultGraph,
    Variable(Variable),
}

impl fmt::Debug for GraphNamePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::DefaultGraph => write!(f, "default"),
            Self::Variable(var) => var.fmt(f),
        }
    }
}

impl fmt::Display for GraphNamePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::DefaultGraph => write!(f, "DEFAULT"),
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
#[derive(Eq, PartialEq, Clone, Hash)]
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
}

impl fmt::Debug for TriplePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(triple {:?} {:?} {:?})",
            self.subject, self.predicate, self.object
        )
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

/// A [triple pattern](https://www.w3.org/TR/sparql11-query/#defn_TriplePattern) without blank nodes
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct GroundTriplePattern {
    pub subject: GroundTermPattern,
    pub predicate: NamedNodePattern,
    pub object: GroundTermPattern,
}

impl fmt::Debug for GroundTriplePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(triple {:?} {:?} {:?})",
            self.subject, self.predicate, self.object
        )
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
#[derive(Eq, PartialEq, Clone, Hash)]
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
}

impl fmt::Debug for QuadPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphNamePattern::DefaultGraph {
            write!(
                f,
                "(triple {:?} {:?} {:?})",
                self.subject, self.predicate, self.object
            )
        } else {
            write!(
                f,
                "(graph {:?} ((triple {:?} {:?} {:?})))",
                self.graph_name, self.subject, self.predicate, self.object
            )
        }
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

/// A [triple pattern](https://www.w3.org/TR/sparql11-query/#defn_TriplePattern) in a specific graph without blank nodes
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct GroundQuadPattern {
    pub subject: GroundTermPattern,
    pub predicate: NamedNodePattern,
    pub object: GroundTermPattern,
    pub graph_name: GraphNamePattern,
}

impl fmt::Debug for GroundQuadPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphNamePattern::DefaultGraph {
            write!(
                f,
                "(triple {:?} {:?} {:?})",
                self.subject, self.predicate, self.object
            )
        } else {
            write!(
                f,
                "(graph {:?} ((triple {:?} {:?} {:?})))",
                self.graph_name, self.subject, self.predicate, self.object
            )
        }
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
    fn try_from(pattern: QuadPattern) -> Result<Self, ()> {
        Ok(Self {
            subject: pattern.subject.try_into()?,
            predicate: pattern.predicate,
            object: pattern.object.try_into()?,
            graph_name: pattern.graph_name,
        })
    }
}

#[inline]
pub(crate) fn print_quoted_str(string: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_char('"')?;
    for c in string.chars() {
        match c {
            '\n' => f.write_str("\\n"),
            '\r' => f.write_str("\\r"),
            '"' => f.write_str("\\\""),
            '\\' => f.write_str("\\\\"),
            c => f.write_char(c),
        }?;
    }
    f.write_char('"')
}
