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

/// A possible graph name.
/// It is the union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [blank nodes](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node), and the [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph).
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

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum NamedNodeOrVariable {
    NamedNode(NamedNode),
    Variable(Variable),
}

impl fmt::Display for NamedNodeOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamedNodeOrVariable::NamedNode(node) => node.fmt(f),
            NamedNodeOrVariable::Variable(var) => var.fmt(f),
        }
    }
}

impl From<NamedNode> for NamedNodeOrVariable {
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<Variable> for NamedNodeOrVariable {
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

/// The union of [terms](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GroundTermOrVariable {
    NamedNode(NamedNode),
    Literal(Literal),
    Variable(Variable),
}

impl fmt::Display for GroundTermOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroundTermOrVariable::NamedNode(term) => term.fmt(f),
            GroundTermOrVariable::Literal(term) => term.fmt(f),
            GroundTermOrVariable::Variable(var) => var.fmt(f),
        }
    }
}

impl From<NamedNode> for GroundTermOrVariable {
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<Literal> for GroundTermOrVariable {
    fn from(literal: Literal) -> Self {
        Self::Literal(literal)
    }
}

impl From<Variable> for GroundTermOrVariable {
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl From<GroundTerm> for GroundTermOrVariable {
    fn from(term: GroundTerm) -> Self {
        match term {
            GroundTerm::NamedNode(node) => Self::NamedNode(node),
            GroundTerm::Literal(literal) => Self::Literal(literal),
        }
    }
}

impl From<NamedNodeOrVariable> for GroundTermOrVariable {
    fn from(element: NamedNodeOrVariable) -> Self {
        match element {
            NamedNodeOrVariable::NamedNode(node) => Self::NamedNode(node),
            NamedNodeOrVariable::Variable(var) => Self::Variable(var),
        }
    }
}

/// The union of [terms](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-term) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum TermOrVariable {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Literal(Literal),
    Variable(Variable),
}

impl fmt::Display for TermOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TermOrVariable::NamedNode(term) => term.fmt(f),
            TermOrVariable::BlankNode(term) => term.fmt(f),
            TermOrVariable::Literal(term) => term.fmt(f),
            TermOrVariable::Variable(var) => var.fmt(f),
        }
    }
}

impl From<NamedNode> for TermOrVariable {
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<BlankNode> for TermOrVariable {
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

impl From<Literal> for TermOrVariable {
    fn from(literal: Literal) -> Self {
        Self::Literal(literal)
    }
}

impl From<Variable> for TermOrVariable {
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl From<Term> for TermOrVariable {
    fn from(term: Term) -> Self {
        match term {
            Term::NamedNode(node) => Self::NamedNode(node),
            Term::BlankNode(node) => Self::BlankNode(node),
            Term::Literal(literal) => Self::Literal(literal),
        }
    }
}

impl From<NamedNodeOrVariable> for TermOrVariable {
    fn from(element: NamedNodeOrVariable) -> Self {
        match element {
            NamedNodeOrVariable::NamedNode(node) => Self::NamedNode(node),
            NamedNodeOrVariable::Variable(var) => Self::Variable(var),
        }
    }
}

/// The union of [IRIs](https://www.w3.org/TR/rdf11-concepts/#dfn-iri), [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph) and [variables](https://www.w3.org/TR/sparql11-query/#sparqlQueryVariables).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphNameOrVariable {
    NamedNode(NamedNode),
    DefaultGraph,
    Variable(Variable),
}

impl fmt::Display for GraphNameOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphNameOrVariable::NamedNode(node) => node.fmt(f),
            GraphNameOrVariable::DefaultGraph => f.write_str("DEFAULT"),
            GraphNameOrVariable::Variable(var) => var.fmt(f),
        }
    }
}

impl From<NamedNode> for GraphNameOrVariable {
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<Variable> for GraphNameOrVariable {
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl From<GraphName> for GraphNameOrVariable {
    fn from(graph_name: GraphName) -> Self {
        match graph_name {
            GraphName::NamedNode(node) => Self::NamedNode(node),
            GraphName::DefaultGraph => Self::DefaultGraph,
        }
    }
}

impl From<NamedNodeOrVariable> for GraphNameOrVariable {
    fn from(graph_name: NamedNodeOrVariable) -> Self {
        match graph_name {
            NamedNodeOrVariable::NamedNode(node) => Self::NamedNode(node),
            NamedNodeOrVariable::Variable(var) => Self::Variable(var),
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
