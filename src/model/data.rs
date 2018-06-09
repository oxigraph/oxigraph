use std::borrow::Cow;
///! Implements data structures for https://www.w3.org/TR/rdf11-concepts/
///! Inspired by [RDFjs](http://rdf.js.org/)
use std::fmt;
use std::ops::Deref;
use std::option::Option;
use std::str::FromStr;
use std::sync::Arc;
use url::ParseError;
use url::Url;
use utils::Escaper;
use uuid::Uuid;

/// A RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri)
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct NamedNode {
    iri: Arc<Url>,
}

impl NamedNode {
    /// Builds a RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri)
    pub fn new(iri: impl Into<Url>) -> Self {
        Self {
            iri: Arc::new(iri.into()),
        }
    }

    pub fn value(&self) -> &str {
        self.iri.as_str()
    }

    pub fn url(&self) -> &Url {
        &self.iri
    }
}

impl Deref for NamedNode {
    type Target = Url;

    fn deref(&self) -> &Url {
        &self.iri
    }
}

impl fmt::Display for NamedNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<{}>", self.iri)
    }
}

impl FromStr for NamedNode {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(NamedNode::new(Url::parse(s)?))
    }
}

/// A RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node)
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct BlankNode {
    id: Uuid,
}

impl Deref for BlankNode {
    type Target = Uuid;

    fn deref(&self) -> &Uuid {
        &self.id
    }
}

impl fmt::Display for BlankNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "_:{}", self.id)
    }
}

impl Default for BlankNode {
    /// Builds a new RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) with a unique id
    fn default() -> Self {
        BlankNode { id: Uuid::new_v4() }
    }
}

/// A RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal)
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Literal {
    SimpleLiteral(String),
    String(String),
    LanguageTaggedString { value: String, language: String },
    Boolean(bool),
    TypedLiteral { value: String, datatype: NamedNode },
}

lazy_static! {
    static ref XSD_BOOLEAN: NamedNode =
        NamedNode::from_str("http://www.w3.org/2001/XMLSchema#boolean").unwrap();
    static ref XSD_DOUBLE: NamedNode =
        NamedNode::from_str("http://www.w3.org/2001/XMLSchema#double").unwrap();
    static ref XSD_FLOAT: NamedNode =
        NamedNode::from_str("http://www.w3.org/2001/XMLSchema#float").unwrap();
    static ref XSD_INTEGER: NamedNode =
        NamedNode::from_str("http://www.w3.org/2001/XMLSchema#integer").unwrap();
    static ref XSD_STRING: NamedNode =
        NamedNode::from_str("http://www.w3.org/2001/XMLSchema#string").unwrap();
    static ref RDF_LANG_STRING: NamedNode =
        NamedNode::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString").unwrap();
}

impl Literal {
    /// Builds a RDF [simple literal](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal)
    pub fn new_simple_literal(value: impl Into<String>) -> Self {
        Literal::SimpleLiteral(value.into())
    }

    /// Builds a RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) with a [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri)
    pub fn new_typed_literal(value: impl Into<String>, datatype: impl Into<NamedNode>) -> Self {
        //TODO: proper casts
        Literal::TypedLiteral {
            value: value.into(),
            datatype: datatype.into(),
        }
    }

    /// Builds a RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    pub fn new_language_tagged_literal(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Literal::LanguageTaggedString {
            value: value.into(),
            language: language.into(),
        }
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form)
    pub fn value<'a>(&'a self) -> Cow<'a, String> {
        match self {
            Literal::SimpleLiteral(value) => Cow::Borrowed(value),
            Literal::String(value) => Cow::Borrowed(value),
            Literal::LanguageTaggedString { value, .. } => Cow::Borrowed(value),
            Literal::Boolean(value) => Cow::Owned(value.to_string()),
            Literal::TypedLiteral { value, .. } => Cow::Borrowed(value),
        }
    }

    /// The literal [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag) if it is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    pub fn language(&self) -> Option<&str> {
        match self {
            Literal::LanguageTaggedString { language, .. } => Some(language),
            _ => None,
        }
    }

    /// The literal [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri)
    /// The datatype of [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string) is always http://www.w3.org/1999/02/22-rdf-syntax-ns#langString
    pub fn datatype(&self) -> &NamedNode {
        match self {
            Literal::SimpleLiteral(_) => &XSD_STRING,
            Literal::String(_) => &XSD_STRING,
            Literal::LanguageTaggedString { .. } => &RDF_LANG_STRING,
            Literal::Boolean(_) => &XSD_BOOLEAN,
            Literal::TypedLiteral { datatype, .. } => datatype,
        }
    }

    pub fn is_plain(&self) -> bool {
        match self {
            Literal::SimpleLiteral(_) => true,
            Literal::LanguageTaggedString { .. } => true,
            _ => false,
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_plain() {
            self.language()
                .map(|lang| write!(f, "\"{}\"@{}", self.value().escape(), lang))
                .unwrap_or_else(|| write!(f, "\"{}\"", self.value().escape()))
        } else {
            write!(f, "\"{}\"^^{}", self.value().escape(), self.datatype())
        }
    }
}

impl<'a> From<&'a str> for Literal {
    fn from(value: &'a str) -> Self {
        Literal::String(value.into())
    }
}

impl From<String> for Literal {
    fn from(value: String) -> Self {
        Literal::String(value)
    }
}

impl From<bool> for Literal {
    fn from(value: bool) -> Self {
        Literal::Boolean(value)
    }
}

impl From<usize> for Literal {
    fn from(value: usize) -> Self {
        Literal::TypedLiteral {
            value: value.to_string(),
            datatype: XSD_INTEGER.clone(),
        }
    }
}

impl From<f32> for Literal {
    fn from(value: f32) -> Self {
        Literal::TypedLiteral {
            value: value.to_string(),
            datatype: XSD_FLOAT.clone(),
        }
    }
}

impl From<f64> for Literal {
    fn from(value: f64) -> Self {
        Literal::TypedLiteral {
            value: value.to_string(),
            datatype: XSD_DOUBLE.clone(),
        }
    }
}

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

/// The interface of containers that looks like [RDF triples](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple)
pub trait TripleLike {
    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple
    fn subject(&self) -> &NamedOrBlankNode;

    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple
    fn subject_owned(self) -> NamedOrBlankNode;

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple
    fn predicate(&self) -> &NamedNode;
    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple

    fn predicate_owned(self) -> NamedNode;

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple
    fn object(&self) -> &Term;

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple
    fn object_owned(self) -> Term;
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
}

impl fmt::Display for Triple {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {} .", self.subject, self.predicate, self.object)
    }
}

impl TripleLike for Triple {
    fn subject(&self) -> &NamedOrBlankNode {
        return &self.subject;
    }

    fn subject_owned(self) -> NamedOrBlankNode {
        return self.subject;
    }

    fn predicate(&self) -> &NamedNode {
        return &self.predicate;
    }

    fn predicate_owned(self) -> NamedNode {
        return self.predicate;
    }

    fn object(&self) -> &Term {
        return &self.object;
    }

    fn object_owned(self) -> Term {
        return self.object;
    }
}

/// The interface of [triples](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) that are in a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
pub trait QuadLike: TripleLike {
    /// The name of the RDF [graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) in which the triple is or None if it is in the [default graph](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph)
    fn graph_name(&self) -> &Option<NamedOrBlankNode>;

    /// The name of the RDF [graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) in which the triple is or None if it is in the [default graph](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph)
    fn graph_name_owned(self) -> Option<NamedOrBlankNode>;
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

impl TripleLike for Quad {
    fn subject(&self) -> &NamedOrBlankNode {
        return &self.subject;
    }

    fn subject_owned(self) -> NamedOrBlankNode {
        return self.subject;
    }

    fn predicate(&self) -> &NamedNode {
        return &self.predicate;
    }

    fn predicate_owned(self) -> NamedNode {
        return self.predicate;
    }

    fn object(&self) -> &Term {
        return &self.object;
    }

    fn object_owned(self) -> Term {
        return self.object;
    }
}

impl QuadLike for Quad {
    fn graph_name(&self) -> &Option<NamedOrBlankNode> {
        return &self.graph_name;
    }

    fn graph_name_owned(self) -> Option<NamedOrBlankNode> {
        return self.graph_name;
    }
}
