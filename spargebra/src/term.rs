//! Data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/) like IRI, literal or triples.

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
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct NamedNode {
    /// The [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) itself.
    pub iri: String,
}

impl fmt::Display for NamedNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.iri)
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
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct BlankNode {
    /// The [blank node identifier](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node-identifier).
    pub id: String,
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
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
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
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum NamedOrBlankNode {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
}

impl fmt::Display for NamedOrBlankNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamedOrBlankNode::NamedNode(node) => node.fmt(f),
            NamedOrBlankNode::BlankNode(node) => node.fmt(f),
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

/// An RDF [term](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term).
///
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) and [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
///
/// The default string formatter is returning an N-Triples, Turtle and SPARQL compatible representation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Term {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Literal(Literal),
}

impl fmt::Display for Term {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::NamedNode(node) => node.fmt(f),
            Term::BlankNode(node) => node.fmt(f),
            Term::Literal(literal) => literal.fmt(f),
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

impl From<NamedOrBlankNode> for Term {
    #[inline]
    fn from(resource: NamedOrBlankNode) -> Self {
        match resource {
            NamedOrBlankNode::NamedNode(node) => Self::NamedNode(node),
            NamedOrBlankNode::BlankNode(node) => Self::BlankNode(node),
        }
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
///
/// The default string formatter is returning an N-Triples, Turtle and SPARQL compatible representation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GroundTerm {
    NamedNode(NamedNode),
    Literal(Literal),
}

impl fmt::Display for GroundTerm {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroundTerm::NamedNode(node) => node.fmt(f),
            GroundTerm::Literal(literal) => literal.fmt(f),
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

/// A possible graph name.
///
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and the [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphName {
    NamedNode(NamedNode),
    DefaultGraph,
}

impl fmt::Display for GraphName {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphName::NamedNode(node) => node.fmt(f),
            GraphName::DefaultGraph => write!(f, "DEFAULT"),
        }
    }
}

impl From<NamedNode> for GraphName {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
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
///     "<http://example.com/foo> <http://schema.org/sameAs> <http://example.com/foo> <http://example.com/> .",
///     Quad {
///         subject: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         predicate: NamedNode { iri: "http://schema.org/sameAs".into() },
///         object: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         graph_name: NamedNode { iri: "http://example.com/".into() }.into(),
///     }.to_string()
/// )
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Quad {
    pub subject: NamedOrBlankNode,
    pub predicate: NamedNode,
    pub object: Term,
    pub graph_name: GraphName,
}

impl fmt::Display for Quad {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphName::DefaultGraph {
            write!(f, "{} {} {} .", self.subject, self.predicate, self.object)
        } else {
            write!(
                f,
                "{} {} {} {} .",
                self.subject, self.predicate, self.object, self.graph_name
            )
        }
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
///     "<http://example.com/foo> <http://schema.org/sameAs> <http://example.com/foo> <http://example.com/> .",
///     GroundQuad {
///         subject: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         predicate: NamedNode { iri: "http://schema.org/sameAs".into() },
///         object: NamedNode { iri: "http://example.com/foo".into() }.into(),
///         graph_name: NamedNode { iri: "http://example.com/".into() }.into(),
///     }.to_string()
/// )
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct GroundQuad {
    pub subject: NamedNode,
    pub predicate: NamedNode,
    pub object: GroundTerm,
    pub graph_name: GraphName,
}

impl fmt::Display for GroundQuad {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphName::DefaultGraph {
            write!(f, "{} {} {} .", self.subject, self.predicate, self.object)
        } else {
            write!(
                f,
                "{} {} {} {} .",
                self.subject, self.predicate, self.object, self.graph_name
            )
        }
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
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Variable {
    pub name: String,
}

impl fmt::Display for Variable {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.name)
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum NamedNodePattern {
    NamedNode(NamedNode),
    Variable(Variable),
}

impl fmt::Display for NamedNodePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamedNodePattern::NamedNode(node) => node.fmt(f),
            NamedNodePattern::Variable(var) => var.fmt(f),
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
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum TermPattern {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Literal(Literal),
    Variable(Variable),
}

impl fmt::Display for TermPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TermPattern::NamedNode(term) => term.fmt(f),
            TermPattern::BlankNode(term) => term.fmt(f),
            TermPattern::Literal(term) => term.fmt(f),
            TermPattern::Variable(var) => var.fmt(f),
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

impl From<Variable> for TermPattern {
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl From<Term> for TermPattern {
    #[inline]
    fn from(term: Term) -> Self {
        match term {
            Term::NamedNode(node) => Self::NamedNode(node),
            Term::BlankNode(node) => Self::BlankNode(node),
            Term::Literal(literal) => Self::Literal(literal),
        }
    }
}

impl From<NamedNodePattern> for TermPattern {
    #[inline]
    fn from(element: NamedNodePattern) -> Self {
        match element {
            NamedNodePattern::NamedNode(node) => Self::NamedNode(node),
            NamedNodePattern::Variable(var) => Self::Variable(var),
        }
    }
}

/// The union of [terms](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables) without blank nodes.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GroundTermPattern {
    NamedNode(NamedNode),
    Literal(Literal),
    Variable(Variable),
}

impl fmt::Display for GroundTermPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroundTermPattern::NamedNode(term) => term.fmt(f),
            GroundTermPattern::Literal(term) => term.fmt(f),
            GroundTermPattern::Variable(var) => var.fmt(f),
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

impl From<Variable> for GroundTermPattern {
    #[inline]
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl From<GroundTerm> for GroundTermPattern {
    #[inline]
    fn from(term: GroundTerm) -> Self {
        match term {
            GroundTerm::NamedNode(node) => Self::NamedNode(node),
            GroundTerm::Literal(literal) => Self::Literal(literal),
        }
    }
}

impl From<NamedNodePattern> for GroundTermPattern {
    #[inline]
    fn from(element: NamedNodePattern) -> Self {
        match element {
            NamedNodePattern::NamedNode(node) => Self::NamedNode(node),
            NamedNodePattern::Variable(var) => Self::Variable(var),
        }
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphNamePattern {
    NamedNode(NamedNode),
    DefaultGraph,
    Variable(Variable),
}

impl fmt::Display for GraphNamePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphNamePattern::NamedNode(node) => node.fmt(f),
            GraphNamePattern::DefaultGraph => f.write_str("DEFAULT"),
            GraphNamePattern::Variable(var) => var.fmt(f),
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
            GraphName::NamedNode(node) => Self::NamedNode(node),
            GraphName::DefaultGraph => Self::DefaultGraph,
        }
    }
}

impl From<NamedNodePattern> for GraphNamePattern {
    #[inline]
    fn from(graph_name: NamedNodePattern) -> Self {
        match graph_name {
            NamedNodePattern::NamedNode(node) => Self::NamedNode(node),
            NamedNodePattern::Variable(var) => Self::Variable(var),
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
}

impl fmt::Display for TriplePattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(triple {} {} {})",
            self.subject, self.predicate, self.object
        )
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
}

impl fmt::Display for QuadPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphNamePattern::DefaultGraph {
            write!(
                f,
                "(triple {} {} {})",
                self.subject, self.predicate, self.object
            )
        } else {
            write!(
                f,
                "(graph {} (triple {} {} {}))",
                self.graph_name, self.subject, self.predicate, self.object
            )
        }
    }
}

/// A [triple pattern](https://www.w3.org/TR/sparql11-query/#defn_TriplePattern) in a specific graph without blank nodes
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct GroundQuadPattern {
    pub subject: GroundTermPattern,
    pub predicate: NamedNodePattern,
    pub object: GroundTermPattern,
    pub graph_name: GraphNamePattern,
}

impl fmt::Display for GroundQuadPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphNamePattern::DefaultGraph {
            write!(
                f,
                "(triple {} {} {})",
                self.subject, self.predicate, self.object
            )
        } else {
            write!(
                f,
                "(graph {} (triple {} {} {}))",
                self.graph_name, self.subject, self.predicate, self.object
            )
        }
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
