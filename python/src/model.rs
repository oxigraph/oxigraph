use oxigraph::model::*;
use oxigraph::sparql::Variable;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyIndexError, PyNotImplementedError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::PyTypeInfo;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::vec::IntoIter;

/// An RDF `node identified by an IRI <https://www.w3.org/TR/rdf11-concepts/#dfn-iri>`_.
///
/// :param value: the IRI as a string.
/// :type value: str
/// :raises ValueError: if the IRI is not valid according to `RFC 3987 <https://tools.ietf.org/rfc/rfc3987>`_.
///
/// The :py:func:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
///
/// >>> str(NamedNode('http://example.com'))
/// '<http://example.com>'
#[pyclass(name = "NamedNode", module = "oxigraph")]
#[pyo3(text_signature = "(value)")]
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct PyNamedNode {
    inner: NamedNode,
}

impl From<NamedNode> for PyNamedNode {
    fn from(inner: NamedNode) -> Self {
        Self { inner }
    }
}

impl From<PyNamedNode> for NamedNode {
    fn from(node: PyNamedNode) -> Self {
        node.inner
    }
}

impl From<PyNamedNode> for NamedOrBlankNode {
    fn from(node: PyNamedNode) -> Self {
        node.inner.into()
    }
}

impl From<PyNamedNode> for Subject {
    fn from(node: PyNamedNode) -> Self {
        node.inner.into()
    }
}

impl From<PyNamedNode> for Term {
    fn from(node: PyNamedNode) -> Self {
        node.inner.into()
    }
}

impl From<PyNamedNode> for GraphName {
    fn from(node: PyNamedNode) -> Self {
        node.inner.into()
    }
}

#[pymethods]
impl PyNamedNode {
    #[new]
    fn new(value: String) -> PyResult<Self> {
        Ok(NamedNode::new(value)
            .map_err(|e| PyValueError::new_err(e.to_string()))?
            .into())
    }

    /// :return: the named node IRI.
    /// :rtype: str
    ///
    /// >>> NamedNode("http://example.com").value
    /// 'http://example.com'
    #[getter]
    fn value(&self) -> &str {
        self.inner.as_str()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        let mut buffer = String::new();
        named_node_repr(self.inner.as_ref(), &mut buffer);
        buffer
    }

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __richcmp__(&self, other: &PyAny, op: CompareOp) -> PyResult<bool> {
        if let Ok(other) = other.downcast::<PyCell<Self>>() {
            Ok(eq_ord_compare(self, &other.borrow(), op))
        } else if PyBlankNode::is_type_of(other)
            || PyLiteral::is_type_of(other)
            || PyDefaultGraph::is_type_of(other)
        {
            eq_compare_other_type(op)
        } else {
            Err(PyTypeError::new_err(
                "NamedNode could only be compared with RDF terms",
            ))
        }
    }
}

/// An RDF `blank node <https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node>`_.
///
/// :param value: the `blank node ID <https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node-identifier>`_ (if not present, a random blank node ID is automatically generated).
/// :type value: str, optional
/// :raises ValueError: if the blank node ID is invalid according to NTriples, Turtle, and SPARQL grammars.
///
/// The :py:func:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
///
/// >>> str(BlankNode('ex'))
/// '_:ex'
#[pyclass(name = "BlankNode", module = "oxigraph")]
#[pyo3(text_signature = "(value)")]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct PyBlankNode {
    inner: BlankNode,
}

impl From<BlankNode> for PyBlankNode {
    fn from(inner: BlankNode) -> Self {
        Self { inner }
    }
}

impl From<PyBlankNode> for BlankNode {
    fn from(node: PyBlankNode) -> Self {
        node.inner
    }
}

impl From<PyBlankNode> for NamedOrBlankNode {
    fn from(node: PyBlankNode) -> Self {
        node.inner.into()
    }
}

impl From<PyBlankNode> for Subject {
    fn from(node: PyBlankNode) -> Self {
        node.inner.into()
    }
}

impl From<PyBlankNode> for Term {
    fn from(node: PyBlankNode) -> Self {
        node.inner.into()
    }
}

impl From<PyBlankNode> for GraphName {
    fn from(node: PyBlankNode) -> Self {
        node.inner.into()
    }
}

#[pymethods]
impl PyBlankNode {
    #[new]
    fn new(value: Option<String>) -> PyResult<Self> {
        Ok(if let Some(value) = value {
            BlankNode::new(value).map_err(|e| PyValueError::new_err(e.to_string()))?
        } else {
            BlankNode::default()
        }
        .into())
    }

    /// :return: the `blank node ID <https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node-identifier>`_.
    /// :rtype: str
    ///
    /// >>> BlankNode("ex").value
    /// 'ex'
    #[getter]
    fn value(&self) -> &str {
        self.inner.as_str()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        let mut buffer = String::new();
        blank_node_repr(self.inner.as_ref(), &mut buffer);
        buffer
    }

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __richcmp__(&self, other: &PyAny, op: CompareOp) -> PyResult<bool> {
        if let Ok(other) = other.downcast::<PyCell<Self>>() {
            eq_compare(self, &other.borrow(), op)
        } else if PyNamedNode::is_type_of(other)
            || PyLiteral::is_type_of(other)
            || PyDefaultGraph::is_type_of(other)
        {
            eq_compare_other_type(op)
        } else {
            Err(PyTypeError::new_err(
                "BlankNode could only be compared with RDF terms",
            ))
        }
    }
}

/// An RDF `literal <https://www.w3.org/TR/rdf11-concepts/#dfn-literal>`_.
///
/// :param value: the literal value or `lexical form <https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form>`_.
/// :type value: str
/// :param datatype: the literal `datatype IRI <https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri>`_.
/// :type datatype: NamedNode, optional
/// :param language: the literal `language tag <https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag>`_.
/// :type language: str, optional
/// :raises ValueError: if the language tag is not valid according to `RFC 5646 <https://tools.ietf.org/rfc/rfc5646>`_ (`BCP 47 <https://tools.ietf.org/rfc/bcp/bcp47>`_).
///
/// The :py:func:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
///
/// >>> str(Literal('example'))
/// '"example"'
/// >>> str(Literal('example', language='en'))
/// '"example"@en'
/// >>> str(Literal('11', datatype=NamedNode('http://www.w3.org/2001/XMLSchema#integer')))
/// '"11"^^<http://www.w3.org/2001/XMLSchema#integer>'
#[pyclass(name = "Literal", module = "oxigraph")]
#[pyo3(text_signature = "(value, *, datatype = None, language = None)")]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct PyLiteral {
    inner: Literal,
}

impl From<Literal> for PyLiteral {
    fn from(inner: Literal) -> Self {
        Self { inner }
    }
}

impl From<PyLiteral> for Literal {
    fn from(literal: PyLiteral) -> Self {
        literal.inner
    }
}

impl From<PyLiteral> for Term {
    fn from(node: PyLiteral) -> Self {
        node.inner.into()
    }
}

#[pymethods]
impl PyLiteral {
    #[new]
    #[args(value, "*", datatype = "None", language = "None")]
    fn new(
        value: String,
        language: Option<String>,
        datatype: Option<PyNamedNode>,
    ) -> PyResult<Self> {
        Ok(if let Some(language) = language {
            if let Some(datatype) = datatype {
                if datatype.value() != "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString" {
                    return Err(PyValueError::new_err(
                        "The literals with a language tag must use the rdf:langString datatype",
                    ));
                }
            }
            Literal::new_language_tagged_literal(value, language)
                .map_err(|e| PyValueError::new_err(e.to_string()))?
        } else if let Some(datatype) = datatype {
            Literal::new_typed_literal(value, datatype)
        } else {
            Literal::new_simple_literal(value)
        }
        .into())
    }

    /// :return: the literal value or `lexical form <https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form>`_.
    /// :rtype: str
    ///
    /// >>> Literal("example").value
    /// 'example'
    #[getter]
    fn value(&self) -> &str {
        self.inner.value()
    }

    /// :return: the literal `language tag <https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag>`_.
    /// :rtype: str or None
    ///
    /// >>> Literal('example', language='en').language
    /// 'en'
    /// >>> Literal('example').language
    ///
    #[getter]
    fn language(&self) -> Option<&str> {
        self.inner.language()
    }

    /// :return: the literal `datatype IRI <https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri>`_.
    /// :rtype: NamedNode
    ///
    /// >>> Literal('11', datatype=NamedNode('http://www.w3.org/2001/XMLSchema#integer')).datatype
    /// <NamedNode value=http://www.w3.org/2001/XMLSchema#integer>
    /// >>> Literal('example').datatype
    /// <NamedNode value=http://www.w3.org/2001/XMLSchema#string>
    /// >>> Literal('example', language='en').datatype
    /// <NamedNode value=http://www.w3.org/1999/02/22-rdf-syntax-ns#langString>
    #[getter]
    fn datatype(&self) -> PyNamedNode {
        self.inner.datatype().into_owned().into()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        let mut buffer = String::new();
        literal_repr(self.inner.as_ref(), &mut buffer);
        buffer
    }

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __richcmp__(&self, other: &PyAny, op: CompareOp) -> PyResult<bool> {
        if let Ok(other) = other.downcast::<PyCell<Self>>() {
            eq_compare(self, &other.borrow(), op)
        } else if PyNamedNode::is_type_of(other)
            || PyBlankNode::is_type_of(other)
            || PyDefaultGraph::is_type_of(other)
        {
            eq_compare_other_type(op)
        } else {
            Err(PyTypeError::new_err(
                "Literal could only be compared with RDF terms",
            ))
        }
    }
}

/// The RDF `default graph name <https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph>`_.
#[pyclass(name = "DefaultGraph", module = "oxigraph")]
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct PyDefaultGraph {}

impl From<PyDefaultGraph> for GraphName {
    fn from(_: PyDefaultGraph) -> Self {
        GraphName::DefaultGraph
    }
}

#[pymethods]
impl PyDefaultGraph {
    #[new]
    fn new() -> Self {
        Self {}
    }

    #[getter]
    fn value(&self) -> &str {
        ""
    }

    fn __str__(&self) -> &str {
        "DEFAULT"
    }

    fn __repr__(&self) -> &str {
        "<DefaultGraph>"
    }

    fn __hash__(&self) -> u64 {
        0
    }

    fn __richcmp__(&self, other: &PyAny, op: CompareOp) -> PyResult<bool> {
        if let Ok(other) = other.downcast::<PyCell<Self>>() {
            eq_compare(self, &other.borrow(), op)
        } else if PyNamedNode::is_type_of(other)
            || PyBlankNode::is_type_of(other)
            || PyLiteral::is_type_of(other)
        {
            eq_compare_other_type(op)
        } else {
            Err(PyTypeError::new_err(
                "DefaultGraph could only be compared with RDF terms",
            ))
        }
    }
}

#[derive(FromPyObject)]
pub enum PyNamedOrBlankNode {
    NamedNode(PyNamedNode),
    BlankNode(PyBlankNode),
}

impl From<PyNamedOrBlankNode> for NamedOrBlankNode {
    fn from(node: PyNamedOrBlankNode) -> Self {
        match node {
            PyNamedOrBlankNode::NamedNode(node) => node.into(),
            PyNamedOrBlankNode::BlankNode(node) => node.into(),
        }
    }
}

impl From<NamedOrBlankNode> for PyNamedOrBlankNode {
    fn from(node: NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(node) => Self::NamedNode(node.into()),
            NamedOrBlankNode::BlankNode(node) => Self::BlankNode(node.into()),
        }
    }
}

impl IntoPy<PyObject> for PyNamedOrBlankNode {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self {
            Self::NamedNode(node) => node.into_py(py),
            Self::BlankNode(node) => node.into_py(py),
        }
    }
}

#[derive(FromPyObject)]
pub enum PySubject {
    NamedNode(PyNamedNode),
    BlankNode(PyBlankNode),
    Triple(PyTriple),
}

impl From<PySubject> for Subject {
    fn from(node: PySubject) -> Self {
        match node {
            PySubject::NamedNode(node) => node.into(),
            PySubject::BlankNode(node) => node.into(),
            PySubject::Triple(triple) => triple.into(),
        }
    }
}

impl From<Subject> for PySubject {
    fn from(node: Subject) -> Self {
        match node {
            Subject::NamedNode(node) => Self::NamedNode(node.into()),
            Subject::BlankNode(node) => Self::BlankNode(node.into()),
            Subject::Triple(triple) => Self::Triple(triple.as_ref().clone().into()),
        }
    }
}

impl IntoPy<PyObject> for PySubject {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self {
            Self::NamedNode(node) => node.into_py(py),
            Self::BlankNode(node) => node.into_py(py),
            Self::Triple(triple) => triple.into_py(py),
        }
    }
}

#[derive(FromPyObject)]
pub enum PyTerm {
    NamedNode(PyNamedNode),
    BlankNode(PyBlankNode),
    Literal(PyLiteral),
    Triple(PyTriple),
}

impl From<PyTerm> for Term {
    fn from(term: PyTerm) -> Self {
        match term {
            PyTerm::NamedNode(node) => node.into(),
            PyTerm::BlankNode(node) => node.into(),
            PyTerm::Literal(literal) => literal.into(),
            PyTerm::Triple(triple) => triple.into(),
        }
    }
}

impl From<Term> for PyTerm {
    fn from(term: Term) -> Self {
        match term {
            Term::NamedNode(node) => Self::NamedNode(node.into()),
            Term::BlankNode(node) => Self::BlankNode(node.into()),
            Term::Literal(literal) => Self::Literal(literal.into()),
            Term::Triple(triple) => Self::Triple(triple.as_ref().clone().into()),
        }
    }
}

impl IntoPy<PyObject> for PyTerm {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self {
            Self::NamedNode(node) => node.into_py(py),
            Self::BlankNode(node) => node.into_py(py),
            Self::Literal(literal) => literal.into_py(py),
            Self::Triple(triple) => triple.into_py(py),
        }
    }
}

/// An RDF `triple <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple>`_.
///
/// :param subject: the triple subject.
/// :type subject: NamedNode or BlankNode or Triple
/// :param predicate: the triple predicate.
/// :type predicate: NamedNode
/// :param object: the triple object.
/// :type object: NamedNode or BlankNode or Literal or Triple
///
/// The :py:func:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
///
/// >>> str(Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// '<http://example.com> <http://example.com/p> "1"'
///
/// A triple could also be easily destructed into its components:
///
/// >>> (s, p, o) = Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))
#[pyclass(name = "Triple", module = "oxigraph")]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
#[pyo3(text_signature = "(subject, predicate, object)")]
pub struct PyTriple {
    inner: Triple,
}

impl From<Triple> for PyTriple {
    fn from(inner: Triple) -> Self {
        Self { inner }
    }
}

impl From<PyTriple> for Triple {
    fn from(triple: PyTriple) -> Self {
        triple.inner
    }
}

impl<'a> From<&'a PyTriple> for TripleRef<'a> {
    fn from(triple: &'a PyTriple) -> Self {
        triple.inner.as_ref()
    }
}

impl From<PyTriple> for Subject {
    fn from(triple: PyTriple) -> Self {
        triple.inner.into()
    }
}

impl From<PyTriple> for Term {
    fn from(triple: PyTriple) -> Self {
        triple.inner.into()
    }
}

#[pymethods]
impl PyTriple {
    #[new]
    fn new(subject: PySubject, predicate: PyNamedNode, object: PyTerm) -> Self {
        Triple::new(subject, predicate, object).into()
    }

    /// :return: the triple subject.
    /// :rtype: NamedNode or BlankNode or Triple
    ///
    /// >>> Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')).subject
    /// <NamedNode value=http://example.com>
    #[getter]
    fn subject(&self) -> PySubject {
        self.inner.subject.clone().into()
    }

    /// :return: the triple predicate.
    /// :rtype: NamedNode
    ///
    /// >>> Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')).predicate
    /// <NamedNode value=http://example.com/p>
    #[getter]
    fn predicate(&self) -> PyNamedNode {
        self.inner.predicate.clone().into()
    }

    /// :return: the triple object.
    /// :rtype: NamedNode or BlankNode or Literal or Triple
    ///
    /// >>> Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')).object
    /// <Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>
    #[getter]
    fn object(&self) -> PyTerm {
        self.inner.object.clone().into()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        let mut buffer = String::new();
        triple_repr(self.inner.as_ref(), &mut buffer);
        buffer
    }

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __richcmp__(&self, other: &Self, op: CompareOp) -> PyResult<bool> {
        eq_compare(self, other, op)
    }

    fn __len__(&self) -> usize {
        3
    }

    fn __getitem__(&self, input: usize) -> PyResult<PyTerm> {
        match input {
            0 => Ok(Term::from(self.inner.subject.clone()).into()),
            1 => Ok(Term::from(self.inner.predicate.clone()).into()),
            2 => Ok(self.inner.object.clone().into()),
            _ => Err(PyIndexError::new_err("A triple has only 3 elements")),
        }
    }

    fn __iter__(&self) -> TripleComponentsIter {
        TripleComponentsIter {
            inner: vec![
                self.inner.subject.clone().into(),
                self.inner.predicate.clone().into(),
                self.inner.object.clone(),
            ]
            .into_iter(),
        }
    }
}

#[derive(FromPyObject)]
pub enum PyGraphName {
    NamedNode(PyNamedNode),
    BlankNode(PyBlankNode),
    DefaultGraph(PyDefaultGraph),
}

impl From<PyGraphName> for GraphName {
    fn from(graph_name: PyGraphName) -> Self {
        match graph_name {
            PyGraphName::NamedNode(node) => node.into(),
            PyGraphName::BlankNode(node) => node.into(),
            PyGraphName::DefaultGraph(default_graph) => default_graph.into(),
        }
    }
}

impl From<GraphName> for PyGraphName {
    fn from(graph_name: GraphName) -> Self {
        match graph_name {
            GraphName::NamedNode(node) => Self::NamedNode(node.into()),
            GraphName::BlankNode(node) => Self::BlankNode(node.into()),
            GraphName::DefaultGraph => Self::DefaultGraph(PyDefaultGraph::new()),
        }
    }
}

impl IntoPy<PyObject> for PyGraphName {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self {
            Self::NamedNode(node) => node.into_py(py),
            Self::BlankNode(node) => node.into_py(py),
            Self::DefaultGraph(node) => node.into_py(py),
        }
    }
}

/// An RDF `triple <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple>`_.
/// in a `RDF dataset <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_.
///
/// :param subject: the quad subject.
/// :type subject: NamedNode or BlankNode or Triple
/// :param predicate: the quad predicate.
/// :type predicate: NamedNode
/// :param object: the quad object.
/// :type object: NamedNode or BlankNode or Literal or Triple
/// :param graph: the quad graph name. If not present, the default graph is assumed.
/// :type graph: NamedNode or BlankNode or DefaultGraph or None, optional
///
/// The :py:func:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
///
/// >>> str(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
/// '<http://example.com> <http://example.com/p> "1" <http://example.com/g>'
///
/// >>> str(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), DefaultGraph()))
/// '<http://example.com> <http://example.com/p> "1"'
///
/// A quad could also be easily destructed into its components:
///
/// >>> (s, p, o, g) = Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))
#[pyclass(name = "Quad", module = "oxigraph")]
#[pyo3(text_signature = "(subject, predicate, object, graph_name = None)")]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct PyQuad {
    inner: Quad,
}

impl From<Quad> for PyQuad {
    fn from(inner: Quad) -> Self {
        Self { inner }
    }
}

impl From<PyQuad> for Quad {
    fn from(node: PyQuad) -> Self {
        node.inner
    }
}

impl<'a> From<&'a PyQuad> for QuadRef<'a> {
    fn from(node: &'a PyQuad) -> Self {
        node.inner.as_ref()
    }
}

#[pymethods]
impl PyQuad {
    #[new]
    fn new(
        subject: PySubject,
        predicate: PyNamedNode,
        object: PyTerm,
        graph_name: Option<PyGraphName>,
    ) -> Self {
        Quad::new(
            subject,
            predicate,
            object,
            graph_name.unwrap_or(PyGraphName::DefaultGraph(PyDefaultGraph {})),
        )
        .into()
    }

    /// :return: the quad subject.
    /// :rtype: NamedNode or BlankNode or Triple
    ///
    /// >>> Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')).subject
    /// <NamedNode value=http://example.com>
    #[getter]
    fn subject(&self) -> PySubject {
        self.inner.subject.clone().into()
    }

    /// :return: the quad predicate.
    /// :rtype: NamedNode
    ///
    /// >>> Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')).predicate
    /// <NamedNode value=http://example.com/p>
    #[getter]
    fn predicate(&self) -> PyNamedNode {
        self.inner.predicate.clone().into()
    }

    /// :return: the quad object.
    /// :rtype: NamedNode or BlankNode or Literal or Triple
    ///
    /// >>> Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')).object
    /// <Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>
    #[getter]
    fn object(&self) -> PyTerm {
        self.inner.object.clone().into()
    }

    /// :return: the quad graph name.
    /// :rtype: NamedNode or BlankNode or DefaultGraph
    ///
    /// >>> Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')).graph_name
    /// <NamedNode value=http://example.com/g>
    #[getter]
    fn graph_name(&self) -> PyGraphName {
        self.inner.graph_name.clone().into()
    }

    /// :return: the quad underlying triple.
    /// :rtype: Triple
    ///
    /// >>> Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')).triple
    /// <Triple subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>
    #[getter]
    fn triple(&self) -> PyTriple {
        Triple::from(self.inner.clone()).into()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        let mut buffer = String::new();
        buffer.push_str("<Quad subject=");
        term_repr(self.inner.subject.as_ref().into(), &mut buffer);
        buffer.push_str(" predicate=");
        named_node_repr(self.inner.predicate.as_ref(), &mut buffer);
        buffer.push_str(" object=");
        term_repr(self.inner.object.as_ref(), &mut buffer);
        buffer.push_str(" graph_name=");
        graph_name_repr(self.inner.graph_name.as_ref(), &mut buffer);
        buffer.push('>');
        buffer
    }

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __richcmp__(&self, other: &Self, op: CompareOp) -> PyResult<bool> {
        eq_compare(self, other, op)
    }

    fn __len__(&self) -> usize {
        4
    }

    fn __getitem__(&self, input: usize) -> PyResult<PyObject> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        match input {
            0 => Ok(PySubject::from(self.inner.subject.clone()).into_py(py)),
            1 => Ok(PyNamedNode::from(self.inner.predicate.clone()).into_py(py)),
            2 => Ok(PyTerm::from(self.inner.object.clone()).into_py(py)),
            3 => Ok(PyGraphName::from(self.inner.graph_name.clone()).into_py(py)),
            _ => Err(PyIndexError::new_err("A quad has only 4 elements")),
        }
    }

    fn __iter__(slf: PyRef<'_, Self>) -> QuadComponentsIter {
        QuadComponentsIter {
            inner: vec![
                Some(slf.inner.subject.clone().into()),
                Some(slf.inner.predicate.clone().into()),
                Some(slf.inner.object.clone()),
                match slf.inner.graph_name.clone() {
                    GraphName::NamedNode(node) => Some(node.into()),
                    GraphName::BlankNode(node) => Some(node.into()),
                    GraphName::DefaultGraph => None,
                },
            ]
            .into_iter(),
        }
    }
}

/// A SPARQL query variable.
///
/// :param value: the variable name as a string.
/// :type value: str
/// :raises ValueError: if the variable name is invalid according to the SPARQL grammar.
///
/// The :py:func:`str` function provides a serialization compatible with SPARQL:
///
/// >>> str(Variable('foo'))
/// '?foo'
#[pyclass(name = "Variable", module = "oxigraph")]
#[pyo3(text_signature = "(value)")]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct PyVariable {
    inner: Variable,
}

impl From<Variable> for PyVariable {
    fn from(inner: Variable) -> Self {
        Self { inner }
    }
}

impl From<PyVariable> for Variable {
    fn from(variable: PyVariable) -> Self {
        variable.inner
    }
}

impl<'a> From<&'a PyVariable> for &'a Variable {
    fn from(variable: &'a PyVariable) -> Self {
        &variable.inner
    }
}

#[pymethods]
impl PyVariable {
    #[new]
    fn new(value: String) -> PyResult<Self> {
        Ok(Variable::new(value)
            .map_err(|e| PyValueError::new_err(e.to_string()))?
            .into())
    }

    /// :return: the variable name.
    /// :rtype: str
    ///
    /// >>> Variable("foo").value
    /// 'foo'
    #[getter]
    fn value(&self) -> &str {
        self.inner.as_str()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("<Variable value={}>", self.inner.as_str())
    }

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __richcmp__(&self, other: &Self, op: CompareOp) -> PyResult<bool> {
        eq_compare(self, other, op)
    }
}

pub struct PyNamedNodeRef<'a>(PyRef<'a, PyNamedNode>);

impl<'a> From<&'a PyNamedNodeRef<'a>> for NamedNodeRef<'a> {
    fn from(value: &'a PyNamedNodeRef<'a>) -> Self {
        value.0.inner.as_ref()
    }
}

impl<'a> TryFrom<&'a PyAny> for PyNamedNodeRef<'a> {
    type Error = PyErr;

    fn try_from(value: &'a PyAny) -> PyResult<Self> {
        if let Ok(node) = value.downcast::<PyCell<PyNamedNode>>() {
            Ok(Self(node.borrow()))
        } else {
            Err(PyTypeError::new_err(format!(
                "{} is not an RDF named node",
                value.get_type().name()?,
            )))
        }
    }
}

pub enum PyNamedOrBlankNodeRef<'a> {
    NamedNode(PyRef<'a, PyNamedNode>),
    BlankNode(PyRef<'a, PyBlankNode>),
}

impl<'a> From<&'a PyNamedOrBlankNodeRef<'a>> for NamedOrBlankNodeRef<'a> {
    fn from(value: &'a PyNamedOrBlankNodeRef<'a>) -> Self {
        match value {
            PyNamedOrBlankNodeRef::NamedNode(value) => value.inner.as_ref().into(),
            PyNamedOrBlankNodeRef::BlankNode(value) => value.inner.as_ref().into(),
        }
    }
}

impl<'a> TryFrom<&'a PyAny> for PyNamedOrBlankNodeRef<'a> {
    type Error = PyErr;

    fn try_from(value: &'a PyAny) -> PyResult<Self> {
        if let Ok(node) = value.downcast::<PyCell<PyNamedNode>>() {
            Ok(Self::NamedNode(node.borrow()))
        } else if let Ok(node) = value.downcast::<PyCell<PyBlankNode>>() {
            Ok(Self::BlankNode(node.borrow()))
        } else {
            Err(PyTypeError::new_err(format!(
                "{} is not an RDF named or blank node",
                value.get_type().name()?,
            )))
        }
    }
}

pub enum PySubjectRef<'a> {
    NamedNode(PyRef<'a, PyNamedNode>),
    BlankNode(PyRef<'a, PyBlankNode>),
    Triple(PyRef<'a, PyTriple>),
}

impl<'a> From<&'a PySubjectRef<'a>> for SubjectRef<'a> {
    fn from(value: &'a PySubjectRef<'a>) -> Self {
        match value {
            PySubjectRef::NamedNode(value) => value.inner.as_ref().into(),
            PySubjectRef::BlankNode(value) => value.inner.as_ref().into(),
            PySubjectRef::Triple(value) => (&value.inner).into(),
        }
    }
}

impl<'a> TryFrom<&'a PyAny> for PySubjectRef<'a> {
    type Error = PyErr;

    fn try_from(value: &'a PyAny) -> PyResult<Self> {
        if let Ok(node) = value.downcast::<PyCell<PyNamedNode>>() {
            Ok(Self::NamedNode(node.borrow()))
        } else if let Ok(node) = value.downcast::<PyCell<PyBlankNode>>() {
            Ok(Self::BlankNode(node.borrow()))
        } else if let Ok(node) = value.downcast::<PyCell<PyTriple>>() {
            Ok(Self::Triple(node.borrow()))
        } else {
            Err(PyTypeError::new_err(format!(
                "{} is not an RDF named or blank node",
                value.get_type().name()?,
            )))
        }
    }
}

pub enum PyTermRef<'a> {
    NamedNode(PyRef<'a, PyNamedNode>),
    BlankNode(PyRef<'a, PyBlankNode>),
    Literal(PyRef<'a, PyLiteral>),
    Triple(PyRef<'a, PyTriple>),
}

impl<'a> From<&'a PyTermRef<'a>> for TermRef<'a> {
    fn from(value: &'a PyTermRef<'a>) -> Self {
        match value {
            PyTermRef::NamedNode(value) => value.inner.as_ref().into(),
            PyTermRef::BlankNode(value) => value.inner.as_ref().into(),
            PyTermRef::Literal(value) => value.inner.as_ref().into(),
            PyTermRef::Triple(value) => (&value.inner).into(),
        }
    }
}

impl<'a> From<&'a PyTermRef<'a>> for Term {
    fn from(value: &'a PyTermRef<'a>) -> Self {
        TermRef::from(value).into()
    }
}

impl<'a> TryFrom<&'a PyAny> for PyTermRef<'a> {
    type Error = PyErr;

    fn try_from(value: &'a PyAny) -> PyResult<Self> {
        if let Ok(node) = value.downcast::<PyCell<PyNamedNode>>() {
            Ok(Self::NamedNode(node.borrow()))
        } else if let Ok(node) = value.downcast::<PyCell<PyBlankNode>>() {
            Ok(Self::BlankNode(node.borrow()))
        } else if let Ok(node) = value.downcast::<PyCell<PyLiteral>>() {
            Ok(Self::Literal(node.borrow()))
        } else if let Ok(node) = value.downcast::<PyCell<PyTriple>>() {
            Ok(Self::Triple(node.borrow()))
        } else {
            Err(PyTypeError::new_err(format!(
                "{} is not an RDF term",
                value.get_type().name()?,
            )))
        }
    }
}

pub enum PyGraphNameRef<'a> {
    NamedNode(PyRef<'a, PyNamedNode>),
    BlankNode(PyRef<'a, PyBlankNode>),
    DefaultGraph,
}

impl<'a> From<&'a PyGraphNameRef<'a>> for GraphNameRef<'a> {
    fn from(value: &'a PyGraphNameRef<'a>) -> Self {
        match value {
            PyGraphNameRef::NamedNode(value) => value.inner.as_ref().into(),
            PyGraphNameRef::BlankNode(value) => value.inner.as_ref().into(),
            PyGraphNameRef::DefaultGraph => Self::DefaultGraph,
        }
    }
}

impl<'a> From<&'a PyGraphNameRef<'a>> for GraphName {
    fn from(value: &'a PyGraphNameRef<'a>) -> Self {
        GraphNameRef::from(value).into()
    }
}

impl<'a> TryFrom<&'a PyAny> for PyGraphNameRef<'a> {
    type Error = PyErr;

    fn try_from(value: &'a PyAny) -> PyResult<Self> {
        if let Ok(node) = value.downcast::<PyCell<PyNamedNode>>() {
            Ok(Self::NamedNode(node.borrow()))
        } else if let Ok(node) = value.downcast::<PyCell<PyBlankNode>>() {
            Ok(Self::BlankNode(node.borrow()))
        } else if value.downcast::<PyCell<PyDefaultGraph>>().is_ok() {
            Ok(Self::DefaultGraph)
        } else {
            Err(PyTypeError::new_err(format!(
                "{} is not an RDF graph name",
                value.get_type().name()?,
            )))
        }
    }
}

fn eq_compare<T: Eq>(a: &T, b: &T, op: CompareOp) -> PyResult<bool> {
    match op {
        CompareOp::Eq => Ok(a == b),
        CompareOp::Ne => Ok(a != b),
        _ => Err(PyNotImplementedError::new_err(
            "Ordering is not implemented",
        )),
    }
}

fn eq_compare_other_type(op: CompareOp) -> PyResult<bool> {
    match op {
        CompareOp::Eq => Ok(false),
        CompareOp::Ne => Ok(true),
        _ => Err(PyNotImplementedError::new_err(
            "Ordering is not implemented",
        )),
    }
}

fn eq_ord_compare<T: Eq + Ord>(a: &T, b: &T, op: CompareOp) -> bool {
    match op {
        CompareOp::Lt => a < b,
        CompareOp::Le => a <= b,
        CompareOp::Eq => a == b,
        CompareOp::Ne => a != b,
        CompareOp::Gt => a > b,
        CompareOp::Ge => a >= b,
    }
}

fn hash(t: &impl Hash) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

fn named_node_repr(node: NamedNodeRef<'_>, buffer: &mut String) {
    buffer.push_str("<NamedNode value=");
    buffer.push_str(node.as_str());
    buffer.push('>');
}

fn blank_node_repr(node: BlankNodeRef<'_>, buffer: &mut String) {
    buffer.push_str("<BlankNode value=");
    buffer.push_str(node.as_str());
    buffer.push('>');
}

fn literal_repr(literal: LiteralRef<'_>, buffer: &mut String) {
    buffer.push_str("<Literal value=");
    buffer.push_str(literal.value());
    if let Some(language) = literal.language() {
        buffer.push_str(" language=");
        buffer.push_str(language);
    } else {
        buffer.push_str(" datatype=");
        named_node_repr(literal.datatype(), buffer);
    }
    buffer.push('>');
}

pub fn term_repr(term: TermRef<'_>, buffer: &mut String) {
    match term {
        TermRef::NamedNode(node) => named_node_repr(node, buffer),
        TermRef::BlankNode(node) => blank_node_repr(node, buffer),
        TermRef::Literal(literal) => literal_repr(literal, buffer),
        TermRef::Triple(triple) => triple_repr(triple.as_ref(), buffer),
    }
}

fn graph_name_repr(term: GraphNameRef<'_>, buffer: &mut String) {
    match term {
        GraphNameRef::NamedNode(node) => named_node_repr(node, buffer),
        GraphNameRef::BlankNode(node) => blank_node_repr(node, buffer),
        GraphNameRef::DefaultGraph => buffer.push_str("<DefaultGraph>"),
    }
}

fn triple_repr(triple: TripleRef<'_>, buffer: &mut String) {
    buffer.push_str("<Triple subject=");
    term_repr(triple.subject.into(), buffer);
    buffer.push_str(" predicate=");
    named_node_repr(triple.predicate, buffer);
    buffer.push_str(" object=");
    term_repr(triple.object, buffer);
    buffer.push('>');
}

#[pyclass(module = "oxigraph")]
pub struct TripleComponentsIter {
    inner: IntoIter<Term>,
}

#[pymethods]
impl TripleComponentsIter {
    fn __iter__(slf: PyRef<'_, Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(&mut self) -> Option<PyTerm> {
        self.inner.next().map(PyTerm::from)
    }
}

#[pyclass(module = "oxigraph")]
pub struct QuadComponentsIter {
    inner: IntoIter<Option<Term>>,
}

#[pymethods]
impl QuadComponentsIter {
    fn __iter__(slf: PyRef<'_, Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(&mut self, py: Python<'_>) -> Option<PyObject> {
        self.inner.next().map(move |t| {
            if let Some(t) = t {
                PyTerm::from(t).into_py(py)
            } else {
                PyDefaultGraph {}.into_py(py)
            }
        })
    }
}
