#![allow(clippy::multiple_inherent_impl)]

use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::*;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyInt, PyTuple};
use std::vec::IntoIter;

/// An RDF `node identified by an IRI <https://www.w3.org/TR/rdf11-concepts/#dfn-iri>`_.
///
/// :param value: the IRI as a string.
/// :type value: str
/// :raises ValueError: if the IRI is not valid according to `RFC 3987 <https://tools.ietf.org/rfc/rfc3987>`_.
///
/// The :py:class:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
///
/// >>> str(NamedNode('http://example.com'))
/// '<http://example.com>'
#[pyclass(frozen, name = "NamedNode", module = "pyoxigraph", eq, ord, hash)]
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

    /// :rtype: typing.Any
    fn __getnewargs__(&self) -> (&str,) {
        (self.value(),)
    }

    /// :rtype: NamedNode
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: NamedNode
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }
}

/// An RDF `blank node <https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node>`_.
///
/// :param value: the `blank node identifier <https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node-identifier>`_ (if not present, a random blank node identifier is automatically generated).
/// :type value: str or None, optional
/// :raises ValueError: if the blank node identifier is invalid according to NTriples, Turtle, and SPARQL grammars.
///
/// The :py:class:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
///
/// >>> str(BlankNode('ex'))
/// '_:ex'
#[pyclass(frozen, name = "BlankNode", module = "pyoxigraph", eq, hash)]
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
    #[pyo3(signature = (value = None))]
    fn new(value: Option<String>) -> PyResult<Self> {
        Ok(if let Some(value) = value {
            BlankNode::new(value).map_err(|e| PyValueError::new_err(e.to_string()))?
        } else {
            BlankNode::default()
        }
        .into())
    }

    /// :return: the `blank node identifier <https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node-identifier>`_.
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

    /// :rtype: typing.Any
    fn __getnewargs__(&self) -> (&str,) {
        (self.value(),)
    }

    /// :rtype: BlankNode
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: BlankNode
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }
}

/// An RDF `literal <https://www.w3.org/TR/rdf11-concepts/#dfn-literal>`_.
///
/// :param value: the literal value or `lexical form <https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form>`_.
/// :type value: str or int or float or bool
/// :param datatype: the literal `datatype IRI <https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri>`_.
/// :type datatype: NamedNode or None, optional
/// :param language: the literal `language tag <https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag>`_.
/// :type language: str or None, optional
/// :param direction: the literal `base direction <https://www.w3.org/TR/rdf12-concepts/#dfn-base-direction>`_.
/// :type direction: BaseDirection or None, optional
/// :raises ValueError: if the language tag is not valid according to `RFC 5646 <https://tools.ietf.org/rfc/rfc5646>`_ (`BCP 47 <https://tools.ietf.org/rfc/bcp/bcp47>`_).
///
/// The :py:class:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
///
/// >>> str(Literal('example'))
/// '"example"'
/// >>> str(Literal('example', language='en'))
/// '"example"@en'
/// >>> str(Literal('11', datatype=NamedNode('http://www.w3.org/2001/XMLSchema#integer')))
/// '"11"^^<http://www.w3.org/2001/XMLSchema#integer>'
/// >>> str(Literal(11))
/// '"11"^^<http://www.w3.org/2001/XMLSchema#integer>'
#[pyclass(frozen, name = "Literal", module = "pyoxigraph", eq, hash)]
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

impl PyLiteral {
    fn from_value(value: &Bound<'_, PyAny>, datatype: Option<PyNamedNode>) -> PyResult<Self> {
        Ok(if let Some(datatype) = datatype {
            Literal::new_typed_literal(value.extract::<String>()?, datatype)
        } else if let Ok(value) = value.extract::<String>() {
            value.into()
        } else if let Ok(value) = value.extract::<bool>() {
            value.into()
        } else if let Ok(value) = value.extract::<Bound<'_, PyInt>>() {
            Literal::new_typed_literal(value.to_string(), xsd::INTEGER)
        } else if let Ok(value) = value.extract::<f64>() {
            value.into()
        } else {
            return Err(PyValueError::new_err(
                "The literal value must be a str, an int, a float or a bool",
            ));
        }
        .into())
    }
}

#[pymethods]
impl PyLiteral {
    #[cfg(feature = "rdf-12")]
    #[new]
    #[pyo3(signature = (value, *, datatype = None, language = None, direction = None))]
    fn new(
        value: &Bound<'_, PyAny>,
        datatype: Option<PyNamedNode>,
        language: Option<String>,
        direction: Option<PyBaseDirection>,
    ) -> PyResult<Self> {
        if let Some(language) = language {
            if let Some(direction) = direction {
                if let Some(datatype) = datatype {
                    if datatype.value() != rdf::DIR_LANG_STRING.as_str() {
                        return Err(PyValueError::new_err(
                            "The literals with a language tag and a base direction must use the rdf:dirLangString datatype",
                        ));
                    }
                }
                return Ok(Literal::new_directional_language_tagged_literal(
                    value.extract::<String>()?,
                    language,
                    direction,
                )
                .map_err(|e| PyValueError::new_err(e.to_string()))?
                .into());
            }
            if let Some(datatype) = datatype {
                if datatype.value() != rdf::LANG_STRING.as_str() {
                    return Err(PyValueError::new_err(
                        "The literals with a language tag must use the rdf:langString datatype",
                    ));
                }
            }
            return Ok(
                Literal::new_language_tagged_literal(value.extract::<String>()?, language)
                    .map_err(|e| PyValueError::new_err(e.to_string()))?
                    .into(),
            );
        }
        if direction.is_some() {
            return Err(PyValueError::new_err(
                "The direction parameter can be set only when the language parameter is set",
            ));
        }
        Self::from_value(value, datatype)
    }

    #[cfg(not(feature = "rdf-12"))]
    #[new]
    #[pyo3(signature = (value, *, datatype = None, language = None))]
    fn new(
        value: &Bound<'_, PyAny>,
        datatype: Option<PyNamedNode>,
        language: Option<String>,
    ) -> PyResult<Self> {
        if let Some(language) = language {
            if let Some(datatype) = datatype {
                if datatype.value() != rdf::LANG_STRING.as_str() {
                    return Err(PyValueError::new_err(
                        "The literals with a language tag must use the rdf:langString datatype",
                    ));
                }
            }
            Ok(
                Literal::new_language_tagged_literal(value.extract::<String>()?, language)
                    .map_err(|e| PyValueError::new_err(e.to_string()))?
                    .into(),
            )
        } else {
            Self::from_value(value, datatype)
        }
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
    #[getter]
    fn language(&self) -> Option<&str> {
        self.inner.language()
    }

    /// :return: the literal `base direction <https://www.w3.org/TR/rdf12-concepts/#dfn-base-direction>`_.
    /// :rtype: BaseDirection or None
    ///
    /// >>> Literal('example', language='en', direction=BaseDirection.LTR).direction
    /// <LtrBaseDirection>
    /// >>> Literal('example', language='en').direction
    #[cfg(feature = "rdf-12")]
    #[getter]
    fn direction(&self) -> Option<PyBaseDirection> {
        Some(self.inner.direction()?.into())
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

    /// :rtype: typing.Any
    fn __getnewargs_ex__<'a, 'py>(
        &'a self,
        py: Python<'py>,
    ) -> PyResult<((&'a str,), Bound<'py, PyDict>)> {
        let kwargs = PyDict::new(py);
        if let Some(language) = self.inner.language() {
            kwargs.set_item("language", language)?;
            #[cfg(feature = "rdf-12")]
            if let Some(direction) = self.inner.direction() {
                kwargs.set_item("direction", PyBaseDirection::from(direction))?;
            }
        } else if self.inner.datatype() != xsd::STRING {
            kwargs.set_item(
                "datatype",
                PyNamedNode::from(self.inner.datatype().into_owned()),
            )?;
        }
        Ok(((self.value(),), kwargs))
    }

    /// :rtype: Literal
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: Literal
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }
}

/// A `directional language-tagged string <https://www.w3.org/TR/rdf12-concepts/#dfn-dir-lang-string>`_ `base-direction <https://www.w3.org/TR/rdf12-concepts/#dfn-base-direction>`_
///
/// :param value: the direction as a string (`ltr` or `rtl`).
/// :type value: str
///
/// >>> str(BaseDirection.LTR)
/// 'ltr'
/// >>> str(BaseDirection("ltr"))
/// 'ltr'
#[cfg(feature = "rdf-12")]
#[pyclass(frozen, name = "BaseDirection", module = "pyoxigraph", eq, hash)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct PyBaseDirection {
    inner: BaseDirection,
}

#[cfg(feature = "rdf-12")]
impl From<BaseDirection> for PyBaseDirection {
    fn from(inner: BaseDirection) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "rdf-12")]
impl From<PyBaseDirection> for BaseDirection {
    fn from(direction: PyBaseDirection) -> Self {
        direction.inner
    }
}

#[cfg(feature = "rdf-12")]
#[pymethods]
impl PyBaseDirection {
    /// Left to right
    #[classattr]
    const LTR: Self = Self {
        inner: BaseDirection::Ltr,
    };

    /// Right to left
    #[classattr]
    const RTL: Self = Self {
        inner: BaseDirection::Rtl,
    };

    #[new]
    #[pyo3(signature = (value, *))]
    fn new(value: &str) -> PyResult<Self> {
        match value {
            "ltr" => Ok(Self {
                inner: BaseDirection::Ltr,
            }),
            "rtl" => Ok(Self {
                inner: BaseDirection::Rtl,
            }),
            _ => Err(PyValueError::new_err(
                "The only allowed base direction values are 'ltr' and 'rtl'",
            )),
        }
    }

    /// :return: the base direction as a string
    /// :rtype: str
    ///
    /// >>> BaseDirection("ltr").value
    /// 'ltr'
    #[getter]
    fn value(&self) -> &'static str {
        match self.inner {
            BaseDirection::Ltr => "ltr",
            BaseDirection::Rtl => "rtl",
        }
    }

    fn __str__(&self) -> &'static str {
        self.value()
    }

    fn __repr__(&self) -> &'static str {
        match self.inner {
            BaseDirection::Ltr => "<LtrBaseDirection>",
            BaseDirection::Rtl => "<RtlBaseDirection>",
        }
    }

    /// :rtype: typing.Any
    fn __getnewargs__(&self) -> (&str,) {
        (self.value(),)
    }

    /// :rtype: BaseDirection
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: BaseDirection
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }
}

/// The RDF `default graph name <https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph>`_.
#[pyclass(frozen, name = "DefaultGraph", module = "pyoxigraph", eq, hash)]
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct PyDefaultGraph;

impl From<PyDefaultGraph> for GraphName {
    fn from(_: PyDefaultGraph) -> Self {
        Self::DefaultGraph
    }
}

#[pymethods]
impl PyDefaultGraph {
    #[new]
    fn new() -> Self {
        Self {}
    }

    fn __str__(&self) -> &str {
        "DEFAULT"
    }

    fn __repr__(&self) -> &str {
        "<DefaultGraph>"
    }

    /// :rtype: typing.Any
    fn __getnewargs__<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        PyTuple::empty(py)
    }

    /// :rtype: DefaultGraph
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: DefaultGraph
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }
}

#[derive(FromPyObject, IntoPyObject)]
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

#[derive(FromPyObject, IntoPyObject)]
pub enum PyTerm {
    NamedNode(PyNamedNode),
    BlankNode(PyBlankNode),
    Literal(PyLiteral),
    #[cfg(feature = "rdf-12")]
    Triple(PyTriple),
}

impl From<PyTerm> for Term {
    fn from(term: PyTerm) -> Self {
        match term {
            PyTerm::NamedNode(node) => node.into(),
            PyTerm::BlankNode(node) => node.into(),
            PyTerm::Literal(literal) => literal.into(),
            #[cfg(feature = "rdf-12")]
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
            #[cfg(feature = "rdf-12")]
            Term::Triple(triple) => Self::Triple(triple.as_ref().clone().into()),
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
/// The :py:class:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
///
/// >>> str(Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// '<http://example.com> <http://example.com/p> "1"'
///
/// A triple could also be easily destructed into its components:
///
/// >>> (s, p, o) = Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))
#[pyclass(frozen, sequence, name = "Triple", module = "pyoxigraph", eq, hash)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
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

#[cfg(feature = "rdf-12")]
impl From<PyTriple> for Term {
    fn from(triple: PyTriple) -> Self {
        triple.inner.into()
    }
}

#[pymethods]
impl PyTriple {
    #[new]
    fn new(subject: PyNamedOrBlankNode, predicate: PyNamedNode, object: PyTerm) -> Self {
        Triple::new(subject, predicate, object).into()
    }

    /// :return: the triple subject.
    /// :rtype: NamedNode or BlankNode or Triple
    ///
    /// >>> Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')).subject
    /// <NamedNode value=http://example.com>
    #[getter]
    fn subject(&self) -> PyNamedOrBlankNode {
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

    /// :rtype: typing.Any
    fn __getnewargs__(&self) -> (PyNamedOrBlankNode, PyNamedNode, PyTerm) {
        (self.subject(), self.predicate(), self.object())
    }

    /// :rtype: Triple
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: Triple
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }

    #[classattr]
    fn __match_args__() -> (&'static str, &'static str, &'static str) {
        ("subject", "predicate", "object")
    }
}

#[derive(FromPyObject, IntoPyObject)]
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

/// An RDF `triple <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple>`_.
/// in a `RDF dataset <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_.
///
/// :param subject: the quad subject.
/// :type subject: NamedNode or BlankNode or Triple
/// :param predicate: the quad predicate.
/// :type predicate: NamedNode
/// :param object: the quad object.
/// :type object: NamedNode or BlankNode or Literal or Triple
/// :param graph_name: the quad graph name. If not present, the default graph is assumed.
/// :type graph_name: NamedNode or BlankNode or DefaultGraph or None, optional
///
/// The :py:class:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:
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
#[pyclass(frozen, sequence, name = "Quad", module = "pyoxigraph", eq, hash)]
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
    #[pyo3(signature = (subject, predicate, object, graph_name = None))]
    fn new(
        subject: PyNamedOrBlankNode,
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
    fn subject(&self) -> PyNamedOrBlankNode {
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

    fn __len__(&self) -> usize {
        4
    }

    fn __getitem__<'a>(&self, input: usize, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        match input {
            0 => PyNamedOrBlankNode::from(self.inner.subject.clone()).into_bound_py_any(py),
            1 => PyNamedNode::from(self.inner.predicate.clone()).into_bound_py_any(py),
            2 => PyTerm::from(self.inner.object.clone()).into_bound_py_any(py),
            3 => PyGraphName::from(self.inner.graph_name.clone()).into_bound_py_any(py),
            _ => Err(PyIndexError::new_err("A quad has only 4 elements")),
        }
    }

    fn __iter__(&self) -> QuadComponentsIter {
        QuadComponentsIter {
            inner: vec![
                Some(self.inner.subject.clone().into()),
                Some(self.inner.predicate.clone().into()),
                Some(self.inner.object.clone()),
                match self.inner.graph_name.clone() {
                    GraphName::NamedNode(node) => Some(node.into()),
                    GraphName::BlankNode(node) => Some(node.into()),
                    GraphName::DefaultGraph => None,
                },
            ]
            .into_iter(),
        }
    }

    /// :rtype: typing.Any
    fn __getnewargs__(&self) -> (PyNamedOrBlankNode, PyNamedNode, PyTerm, PyGraphName) {
        (
            self.subject(),
            self.predicate(),
            self.object(),
            self.graph_name(),
        )
    }

    /// :rtype: Quad
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: Quad
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }

    #[classattr]
    fn __match_args__() -> (&'static str, &'static str, &'static str, &'static str) {
        ("subject", "predicate", "object", "graph_name")
    }
}

/// A SPARQL query variable.
///
/// :param value: the variable name as a string.
/// :type value: str
/// :raises ValueError: if the variable name is invalid according to the SPARQL grammar.
///
/// The :py:class:`str` function provides a serialization compatible with SPARQL:
///
/// >>> str(Variable('foo'))
/// '?foo'
#[pyclass(frozen, name = "Variable", module = "pyoxigraph", eq, hash)]
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

    /// :rtype: typing.Any
    fn __getnewargs__(&self) -> (&str,) {
        (self.value(),)
    }

    /// :rtype: Variable
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: Variable
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }
}

#[derive(FromPyObject)]
pub struct PyNamedNodeRef<'a>(PyRef<'a, PyNamedNode>);

impl<'a> From<&'a PyNamedNodeRef<'a>> for NamedNodeRef<'a> {
    fn from(value: &'a PyNamedNodeRef<'a>) -> Self {
        value.0.inner.as_ref()
    }
}

#[derive(FromPyObject)]
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

#[derive(FromPyObject)]
pub enum PyTermRef<'a> {
    NamedNode(PyRef<'a, PyNamedNode>),
    BlankNode(PyRef<'a, PyBlankNode>),
    Literal(PyRef<'a, PyLiteral>),
    #[cfg(feature = "rdf-12")]
    Triple(PyRef<'a, PyTriple>),
}

impl<'a> From<&'a PyTermRef<'a>> for TermRef<'a> {
    fn from(value: &'a PyTermRef<'a>) -> Self {
        match value {
            PyTermRef::NamedNode(value) => value.inner.as_ref().into(),
            PyTermRef::BlankNode(value) => value.inner.as_ref().into(),
            PyTermRef::Literal(value) => value.inner.as_ref().into(),
            #[cfg(feature = "rdf-12")]
            PyTermRef::Triple(value) => (&value.inner).into(),
        }
    }
}

#[derive(FromPyObject)]
pub enum PyGraphNameRef<'a> {
    NamedNode(PyRef<'a, PyNamedNode>),
    BlankNode(PyRef<'a, PyBlankNode>),
    DefaultGraph(PyRef<'a, PyDefaultGraph>),
}

impl<'a> From<&'a PyGraphNameRef<'a>> for GraphNameRef<'a> {
    fn from(value: &'a PyGraphNameRef<'a>) -> Self {
        match value {
            PyGraphNameRef::NamedNode(value) => value.inner.as_ref().into(),
            PyGraphNameRef::BlankNode(value) => value.inner.as_ref().into(),
            PyGraphNameRef::DefaultGraph(_) => Self::DefaultGraph,
        }
    }
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
        #[cfg(feature = "rdf-12")]
        if let Some(direction) = literal.direction() {
            buffer.push_str(" direction=");
            buffer.push_str(match direction {
                BaseDirection::Ltr => "ltr",
                BaseDirection::Rtl => "rtl",
            });
        }
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
        #[cfg(feature = "rdf-12")]
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

#[pyclass(module = "pyoxigraph")]
pub struct TripleComponentsIter {
    inner: IntoIter<Term>,
}

#[pymethods]
impl TripleComponentsIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<PyTerm> {
        self.inner.next().map(PyTerm::from)
    }
}

#[pyclass(module = "pyoxigraph")]
pub struct QuadComponentsIter {
    inner: IntoIter<Option<Term>>,
}

#[pymethods]
impl QuadComponentsIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'a>(&mut self, py: Python<'a>) -> Option<PyResult<Bound<'a, PyAny>>> {
        self.inner.next().map(move |t| {
            if let Some(t) = t {
                PyTerm::from(t).into_bound_py_any(py)
            } else {
                PyDefaultGraph {}.into_bound_py_any(py)
            }
        })
    }
}
