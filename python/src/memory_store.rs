use crate::io::PyFileLike;
use crate::model::*;
use crate::sparql::*;
use crate::store_utils::*;
use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::*;
use oxigraph::MemoryStore;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{NotImplementedError, ValueError};
use pyo3::prelude::*;
use pyo3::{PyIterProtocol, PyObjectProtocol, PySequenceProtocol};
use std::io::BufReader;

/// In-memory store.
/// It encodes a `RDF dataset <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_ and allows to query it using SPARQL.
///
///
/// The :py:func:`str` function provides a serialization of the store in NQuads:
///
/// >>> store = MemoryStore()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
/// >>> str(store)
/// '<http://example.com> <http://example.com/p> "1" <http://example.com/g> .\n'
#[pyclass(name = MemoryStore)]
#[derive(Eq, PartialEq, Clone)]
#[text_signature = "()"]
pub struct PyMemoryStore {
    inner: MemoryStore,
}

#[pymethods]
impl PyMemoryStore {
    #[new]
    fn new() -> Self {
        Self {
            inner: MemoryStore::new(),
        }
    }

    /// Adds a quad to the store
    ///
    /// :param quad: the quad to add
    /// :type quad: Quad
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[text_signature = "($self, quad)"]
    fn add(&self, quad: PyQuad) {
        self.inner.insert(quad);
    }

    /// Removes a quad from the store
    ///
    /// :param quad: the quad to remove
    /// :type quad: Quad
    ///
    /// >>> store = MemoryStore()
    /// >>> quad = Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))
    /// >>> store.add(quad)
    /// >>> store.remove(quad)
    /// >>> list(store)
    /// []
    #[text_signature = "($self, quad)"]
    fn remove(&self, quad: &PyQuad) {
        self.inner.remove(quad);
    }

    /// Looks for the quads matching a given pattern
    ///
    /// :param subject: the quad subject or :py:const:`None` to match everything.
    /// :type subject: NamedNode or BlankNode or None
    /// :param predicate: the quad predicate or :py:const:`None` to match everything.
    /// :type predicate: NamedNode or None
    /// :param object: the quad object or :py:const:`None` to match everything.
    /// :type object: NamedNode or BlankNode or Literal or None
    /// :param graph: the quad graph name. To match only the default graph, use :py:class:`DefaultGraph`. To match everything use :py:const:`None`.
    /// :type graph: NamedNode or BlankNode or DefaultGraph or None
    /// :return: an iterator of the quads matching the pattern
    /// :rtype: iter(Quad)
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store.quads_for_pattern(NamedNode('http://example.com'), None, None, None))
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[text_signature = "($self, subject, predicate, object, graph_name = None)"]
    fn quads_for_pattern(
        &self,
        subject: &PyAny,
        predicate: &PyAny,
        object: &PyAny,
        graph_name: Option<&PyAny>,
    ) -> PyResult<QuadIter> {
        let (subject, predicate, object, graph_name) =
            extract_quads_pattern(subject, predicate, object, graph_name)?;
        Ok(QuadIter {
            inner: Box::new(self.inner.quads_for_pattern(
                subject.as_ref().map(|t| t.into()),
                predicate.as_ref().map(|t| t.into()),
                object.as_ref().map(|t| t.into()),
                graph_name.as_ref().map(|t| t.into()),
            )),
        })
    }

    /// Executes a `SPARQL 1.1 query <https://www.w3.org/TR/sparql11-query/>`_.
    ///
    /// :param query: the query to execute
    /// :type query: str
    /// :param use_default_graph_as_union: optional, if the SPARQL query should look for triples in all the dataset graphs by default (i.e. without `GRAPH` operations). Disabled by default.
    /// :type use_default_graph_as_union: bool, optional
    /// :param default_graph_uris: optional, list of the named graph URIs that should be used as the query default graph. By default the store default graph is used.
    /// :type default_graph_uris: list(NamedNode) or None, optional
    /// :param named_graph_uris: optional, list of the named graph URIs that could be used in SPARQL `GRAPH` clause. By default all the store default graphs are available.
    /// :type named_graph_uris: list(NamedNode) or None, optional
    /// :return: a :py:class:`bool` for ``ASK`` queries, an iterator of :py:class:`Triple` for ``CONSTRUCT`` and ``DESCRIBE`` queries and an iterator of :py:class:`QuerySolution` for ``SELECT`` queries.
    /// :rtype: QuerySolutions or QueryTriples or bool
    /// :raises SyntaxError: if the provided query is invalid
    ///
    /// ``SELECT`` query:
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> list(solution['s'] for solution in store.query('SELECT ?s WHERE { ?s ?p ?o }'))
    /// [<NamedNode value=http://example.com>]
    ///
    /// ``CONSTRUCT`` query:
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> list(store.query('CONSTRUCT WHERE { ?s ?p ?o }'))
    /// [<Triple subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]
    ///
    /// ``ASK`` query:
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.query('ASK { ?s ?p ?o }')
    /// True
    #[text_signature = "($self, query, *, use_default_graph_as_union, default_graph_uris, named_graph_uris)"]
    #[args(
        query,
        "*",
        use_default_graph_as_union = "false",
        default_graph_uris = "None",
        named_graph_uris = "None"
    )]
    fn query(
        &self,
        query: &str,
        use_default_graph_as_union: bool,
        default_graph_uris: Option<Vec<PyNamedNode>>,
        named_graph_uris: Option<Vec<PyNamedNode>>,
        py: Python<'_>,
    ) -> PyResult<PyObject> {
        let results = py.allow_threads(move || {
            let options = build_query_options(
                use_default_graph_as_union,
                default_graph_uris,
                named_graph_uris,
            )?;
            self.inner
                .query(query, options)
                .map_err(map_evaluation_error)
        })?;
        query_results_to_python(py, results)
    }

    /// Loads an RDF serialization into the store
    ///
    /// It currently supports the following formats:
    ///
    /// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples``)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads``)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle``)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig``)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml``)
    ///
    /// It supports also some MIME type aliases.
    /// For example ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
    ///
    /// :param input: The binary I/O object to read from. For example, it could be a file opened in binary mode with ``open('my_file.ttl', 'rb')``.
    /// :type input: io.RawIOBase or io.BufferedIOBase
    /// :param mime_type: the MIME type of the RDF serialization
    /// :type mime_type: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done
    /// :type base_iri: str or None, optional
    /// :param to_graph: if it is a file composed of triples, the graph in which store the triples. By default, the default graph is used.
    /// :type to_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :raises ValueError: if the MIME type is not supported or the `to_graph` parameter is given with a quad file.
    /// :raises SyntaxError: if the provided data is invalid
    ///
    /// >>> store = MemoryStore()
    /// >>> store.load(io.BytesIO(b'<foo> <p> "1" .'), "text/turtle", base_iri="http://example.com/", to_graph=NamedNode("http://example.com/g"))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[text_signature = "($self, input, /, mime_type, *, base_iri = None, to_graph = None)"]
    #[args(input, mime_type, "*", base_iri = "None", to_graph = "None")]
    fn load(
        &self,
        input: &PyAny,
        mime_type: &str,
        base_iri: Option<&str>,
        to_graph: Option<&PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let to_graph_name = if let Some(graph_name) = to_graph {
            Some(extract_graph_name(graph_name)?)
        } else {
            None
        };
        let input = BufReader::new(PyFileLike::new(input.to_object(py)));
        if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
            self.inner
                .load_graph(
                    input,
                    graph_format,
                    &to_graph_name.unwrap_or(GraphName::DefaultGraph),
                    base_iri,
                )
                .map_err(map_io_err)
        } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
            if to_graph_name.is_some() {
                return Err(ValueError::py_err(
                    "The target graph name parameter is not available for dataset formats",
                ));
            }
            self.inner
                .load_dataset(input, dataset_format, base_iri)
                .map_err(map_io_err)
        } else {
            Err(ValueError::py_err(format!(
                "Not supported MIME type: {}",
                mime_type
            )))
        }
    }

    /// Dumps the store quads or triples into a file
    ///
    /// It currently supports the following formats:
    ///
    /// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples``)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads``)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle``)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig``)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml``)
    ///
    /// It supports also some MIME type aliases.
    /// For example ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
    ///
    /// :param output: The binary I/O object to write to. For example, it could be a file opened in binary mode with ``open('my_file.ttl', 'wb')``.
    /// :type input: io.RawIOBase or io.BufferedIOBase
    /// :param mime_type: the MIME type of the RDF serialization
    /// :type mime_type: str
    /// :param from_graph: if a triple based format is requested, the store graph from which dump the triples. By default, the default graph is used.
    /// :type from_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :raises ValueError: if the MIME type is not supported or the `from_graph` parameter is given with a quad syntax.
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> output = io.BytesIO()
    /// >>> store.dump(output, "text/turtle", from_graph=NamedNode("http://example.com/g"))
    /// >>> output.getvalue()
    /// b'<http://example.com> <http://example.com/p> "1" .\n'
    #[text_signature = "($self, output, /, mime_type, *, from_graph = None)"]
    #[args(output, mime_type, "*", from_graph = "None")]
    fn dump(
        &self,
        output: &PyAny,
        mime_type: &str,
        from_graph: Option<&PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let from_graph_name = if let Some(graph_name) = from_graph {
            Some(extract_graph_name(graph_name)?)
        } else {
            None
        };
        let output = PyFileLike::new(output.to_object(py));
        if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
            self.inner
                .dump_graph(
                    output,
                    graph_format,
                    &from_graph_name.unwrap_or(GraphName::DefaultGraph),
                )
                .map_err(map_io_err)
        } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
            if from_graph_name.is_some() {
                return Err(ValueError::py_err(
                    "The target graph name parameter is not available for dataset formats",
                ));
            }
            self.inner
                .dump_dataset(output, dataset_format)
                .map_err(map_io_err)
        } else {
            Err(ValueError::py_err(format!(
                "Not supported MIME type: {}",
                mime_type
            )))
        }
    }
}

#[pyproto]
impl PyObjectProtocol for PyMemoryStore {
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __richcmp__(&self, other: &PyCell<Self>, op: CompareOp) -> PyResult<bool> {
        let other: &PyMemoryStore = &other.borrow();
        match op {
            CompareOp::Eq => Ok(self == other),
            CompareOp::Ne => Ok(self != other),
            _ => Err(NotImplementedError::py_err("Ordering is not implemented")),
        }
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }
}

#[pyproto]
impl<'p> PySequenceProtocol<'p> for PyMemoryStore {
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __contains__(&self, quad: PyQuad) -> bool {
        self.inner.contains(&quad)
    }
}

#[pyproto]
impl PyIterProtocol for PyMemoryStore {
    fn __iter__(slf: PyRef<Self>) -> QuadIter {
        QuadIter {
            inner: Box::new(slf.inner.quads_for_pattern(None, None, None, None)),
        }
    }
}

#[pyclass(unsendable)]
pub struct QuadIter {
    inner: Box<dyn Iterator<Item = Quad>>,
}

#[pyproto]
impl PyIterProtocol for QuadIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> Option<PyQuad> {
        slf.inner.next().map(|q| q.into())
    }
}
