use oxigraph::model::*;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{NotImplementedError, TypeError, ValueError};
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::PyObjectProtocol;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

#[pyclass(name = NamedNode)]
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
            .map_err(|e| ValueError::py_err(e.to_string()))?
            .into())
    }

    #[getter]
    fn value(&self) -> &str {
        self.inner.as_str()
    }
}

#[pyproto]
impl PyObjectProtocol for PyNamedNode {
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("<NamedNode value={}>", self.inner.as_str())
    }

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __richcmp__(&self, other: &PyCell<Self>, op: CompareOp) -> bool {
        eq_ord_compare(self, &other.borrow(), op)
    }
}

#[pyclass(name = BlankNode)]
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
            BlankNode::new(value).map_err(|e| ValueError::py_err(e.to_string()))?
        } else {
            BlankNode::default()
        }
        .into())
    }

    #[getter]
    fn value(&self) -> &str {
        self.inner.as_str()
    }
}

#[pyproto]
impl PyObjectProtocol for PyBlankNode {
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("<BlankNode value={}>", self.inner.as_str())
    }

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __richcmp__(&self, other: &PyCell<Self>, op: CompareOp) -> PyResult<bool> {
        eq_compare(self, &other.borrow(), op)
    }
}

#[pyclass(name = Literal)]
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
    #[args(value, "*", language = "None", datatype = "None")]
    fn new(
        value: String,
        language: Option<String>,
        datatype: Option<PyNamedNode>,
    ) -> PyResult<Self> {
        Ok(if let Some(language) = language {
            if let Some(datatype) = datatype {
                if datatype.value() != "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString" {
                    return Err(ValueError::py_err(
                        "The literals with a language tag must use the rdf:langString datatype",
                    ));
                }
            }
            Literal::new_language_tagged_literal(value, language)
                .map_err(|e| ValueError::py_err(e.to_string()))?
        } else if let Some(datatype) = datatype {
            Literal::new_typed_literal(value, datatype)
        } else {
            Literal::new_simple_literal(value)
        }
        .into())
    }

    #[getter]
    fn value(&self) -> &str {
        self.inner.value()
    }

    #[getter]
    fn language(&self) -> Option<&str> {
        self.inner.language()
    }

    #[getter]
    fn datatype(&self) -> PyNamedNode {
        self.inner.datatype().clone().into()
    }
}

#[pyproto]
impl PyObjectProtocol for PyLiteral {
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "<Literal value={} language={} datatype={}>",
            self.inner.value(),
            self.inner.language().unwrap_or(""),
            self.inner.datatype().as_str()
        )
    }

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __richcmp__(&self, other: &PyCell<Self>, op: CompareOp) -> PyResult<bool> {
        eq_compare(self, &other.borrow(), op)
    }
}

#[pyclass(name = DefaultGraph)]
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
        PyDefaultGraph {}
    }

    #[getter]
    fn value(&self) -> &str {
        ""
    }
}

#[pyproto]
impl PyObjectProtocol for PyDefaultGraph {
    fn __str__(&self) -> &'p str {
        "DEFAULT"
    }

    fn __repr__(&self) -> &'p str {
        "<DefaultGraph>"
    }

    fn __hash__(&self) -> u64 {
        0
    }

    fn __richcmp__(&self, other: &PyCell<Self>, op: CompareOp) -> PyResult<bool> {
        eq_compare(self, &other.borrow(), op)
    }
}

pub fn extract_named_node(py: &PyAny) -> PyResult<NamedNode> {
    if let Ok(node) = py.downcast::<PyCell<PyNamedNode>>() {
        Ok(node.borrow().clone().into())
    } else {
        Err(TypeError::py_err(format!(
            "{} is not a RDF named node",
            py.get_type().name(),
        )))
    }
}

pub fn extract_named_or_blank_node(py: &PyAny) -> PyResult<NamedOrBlankNode> {
    if let Ok(node) = py.downcast::<PyCell<PyNamedNode>>() {
        Ok(node.borrow().clone().into())
    } else if let Ok(node) = py.downcast::<PyCell<PyBlankNode>>() {
        Ok(node.borrow().clone().into())
    } else {
        Err(TypeError::py_err(format!(
            "{} is not a RDF named or blank node",
            py.get_type().name(),
        )))
    }
}

pub fn named_or_blank_node_to_python(py: Python<'_>, node: NamedOrBlankNode) -> PyObject {
    match node {
        NamedOrBlankNode::NamedNode(node) => PyNamedNode::from(node).into_py(py),
        NamedOrBlankNode::BlankNode(node) => PyBlankNode::from(node).into_py(py),
    }
}

pub fn extract_term(py: &PyAny) -> PyResult<Term> {
    if let Ok(node) = py.downcast::<PyCell<PyNamedNode>>() {
        Ok(node.borrow().clone().into())
    } else if let Ok(node) = py.downcast::<PyCell<PyBlankNode>>() {
        Ok(node.borrow().clone().into())
    } else if let Ok(literal) = py.downcast::<PyCell<PyLiteral>>() {
        Ok(literal.borrow().clone().into())
    } else {
        Err(TypeError::py_err(format!(
            "{} is not a RDF named or blank node",
            py.get_type().name(),
        )))
    }
}

pub fn term_to_python(py: Python<'_>, term: Term) -> PyObject {
    match term {
        Term::NamedNode(node) => PyNamedNode::from(node).into_py(py),
        Term::BlankNode(node) => PyBlankNode::from(node).into_py(py),
        Term::Literal(literal) => PyLiteral::from(literal).into_py(py),
    }
}

pub fn extract_graph_name(py: &PyAny) -> PyResult<GraphName> {
    if let Ok(node) = py.downcast::<PyCell<PyNamedNode>>() {
        Ok(node.borrow().clone().into())
    } else if let Ok(node) = py.downcast::<PyCell<PyBlankNode>>() {
        Ok(node.borrow().clone().into())
    } else if let Ok(node) = py.downcast::<PyCell<PyDefaultGraph>>() {
        Ok(node.borrow().clone().into())
    } else {
        Err(TypeError::py_err(format!(
            "{} is not a valid RDF graph name",
            py.get_type().name(),
        )))
    }
}

pub fn graph_name_to_python(py: Python<'_>, name: GraphName) -> PyObject {
    match name {
        GraphName::NamedNode(node) => PyNamedNode::from(node).into_py(py),
        GraphName::BlankNode(node) => PyBlankNode::from(node).into_py(py),
        GraphName::DefaultGraph => PyDefaultGraph::new().into_py(py),
    }
}

pub fn triple_to_python(py: Python<'_>, triple: Triple) -> (PyObject, PyObject, PyObject) {
    (
        named_or_blank_node_to_python(py, triple.subject),
        PyNamedNode::from(triple.predicate).into_py(py),
        term_to_python(py, triple.object),
    )
}

pub fn extract_quad(tuple: &PyTuple) -> PyResult<Quad> {
    let len = tuple.len();
    if len != 3 && len != 4 {
        return Err(TypeError::py_err(
            "A quad should be tuple with 3 or 4 elements",
        ));
    }
    Ok(Quad {
        subject: extract_named_or_blank_node(tuple.get_item(0))?,
        predicate: extract_named_node(tuple.get_item(1))?,
        object: extract_term(tuple.get_item(2))?,
        graph_name: if len == 4 {
            extract_graph_name(tuple.get_item(3))?
        } else {
            GraphName::DefaultGraph
        },
    })
}

pub fn quad_to_python(py: Python<'_>, quad: Quad) -> (PyObject, PyObject, PyObject, PyObject) {
    (
        named_or_blank_node_to_python(py, quad.subject),
        PyNamedNode::from(quad.predicate).into_py(py),
        term_to_python(py, quad.object),
        graph_name_to_python(py, quad.graph_name),
    )
}

fn eq_compare<T: Eq>(a: &T, b: &T, op: CompareOp) -> PyResult<bool> {
    match op {
        CompareOp::Eq => Ok(a == b),
        CompareOp::Ne => Ok(a != b),
        _ => Err(NotImplementedError::py_err("Ordering is not implemented")),
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
