use crate::io::PyFileLike;
use crate::model::*;
use crate::sparql::*;
use crate::store_utils::*;
use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::GraphNameRef;
use oxigraph::store::memory::*;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::prelude::{
    pyclass, pymethods, pyproto, Py, PyAny, PyCell, PyObject, PyRef, PyRefMut, PyResult, Python,
};
use pyo3::{PyIterProtocol, PyObjectProtocol, PySequenceProtocol};
use std::convert::TryFrom;
use std::io::BufReader;

/// In-memory store.
/// It encodes a `RDF dataset <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_ and allows to query it using SPARQL.
///
/// The :py:func:`str` function provides a serialization of the store in NQuads:
///
/// >>> store = MemoryStore()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
/// >>> str(store)
/// '<http://example.com> <http://example.com/p> "1" <http://example.com/g> .\n'
#[pyclass(name = "MemoryStore", module = "oxigraph")]
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
        self.inner.insert(quad)
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
        self.inner.remove(quad)
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
            inner: self.inner.quads_for_pattern(
                subject.as_ref().map(|p| p.into()),
                predicate.as_ref().map(|p| p.into()),
                object.as_ref().map(|p| p.into()),
                graph_name.as_ref().map(|p| p.into()),
            ),
        })
    }

    /// Executes a `SPARQL 1.1 query <https://www.w3.org/TR/sparql11-query/>`_.
    ///
    /// :param query: the query to execute
    /// :type query: str
    /// :param use_default_graph_as_union: if the SPARQL query should look for triples in all the dataset graphs by default (i.e. without `GRAPH` operations). Disabled by default.
    /// :type use_default_graph_as_union: bool, optional
    /// :param default_graph: list of the graphs that should be used as the query default graph. By default, the store default graph is used.
    /// :type default_graph: NamedNode or BlankNode or DefaultGraph or list(NamedNode or BlankNode or DefaultGraph) or None, optional
    /// :param named_graphs: list of the named graphs that could be used in SPARQL `GRAPH` clause. By default, all the store named graphs are available.
    /// :type named_graphs: list(NamedNode or BlankNode) or None, optional
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
    #[text_signature = "($self, query, *, use_default_graph_as_union, default_graph, named_graphs)"]
    #[args(
        query,
        "*",
        use_default_graph_as_union = "false",
        default_graph = "None",
        named_graphs = "None"
    )]
    fn query(
        &self,
        query: &str,
        use_default_graph_as_union: bool,
        default_graph: Option<&PyAny>,
        named_graphs: Option<&PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyObject> {
        let query = parse_query(
            query,
            use_default_graph_as_union,
            default_graph,
            named_graphs,
        )?;
        let results = self.inner.query(query).map_err(map_evaluation_error)?;
        query_results_to_python(py, results)
    }

    /// Executes a `SPARQL 1.1 update <https://www.w3.org/TR/sparql11-update/>`_.
    ///
    /// :param update: the update to execute
    /// :type update: str
    /// :raises SyntaxError: if the provided update is invalid
    ///
    /// The store does not track the existence of empty named graphs.
    /// This method has no ACID guarantees.
    ///
    /// ``INSERT DATA`` update:
    ///
    /// >>> store = MemoryStore()
    /// >>> store.update('INSERT DATA { <http://example.com> <http://example.com/p> "1" }')
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<DefaultGraph>>]
    ///
    /// ``DELETE DATA`` update:
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.update('DELETE DATA { <http://example.com> <http://example.com/p> "1" }')
    /// >>> list(store)
    /// []
    ///
    /// ``DELETE`` update:
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.update('DELETE WHERE { <http://example.com> ?p ?o }')
    /// >>> list(store)
    /// []
    #[text_signature = "($self, update)"]
    fn update(&self, update: &str) -> PyResult<()> {
        self.inner.update(update).map_err(map_evaluation_error)
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
        input: PyObject,
        mime_type: &str,
        base_iri: Option<&str>,
        to_graph: Option<&PyAny>,
    ) -> PyResult<()> {
        let to_graph_name = if let Some(graph_name) = to_graph {
            Some(PyGraphNameRef::try_from(graph_name)?)
        } else {
            None
        };
        let input = BufReader::new(PyFileLike::new(input));
        if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
            self.inner
                .load_graph(
                    input,
                    graph_format,
                    &to_graph_name.unwrap_or(PyGraphNameRef::DefaultGraph),
                    base_iri,
                )
                .map_err(map_io_err)
        } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
            if to_graph_name.is_some() {
                return Err(PyValueError::new_err(
                    "The target graph name parameter is not available for dataset formats",
                ));
            }
            self.inner
                .load_dataset(input, dataset_format, base_iri)
                .map_err(map_io_err)
        } else {
            Err(PyValueError::new_err(format!(
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
    fn dump(&self, output: PyObject, mime_type: &str, from_graph: Option<&PyAny>) -> PyResult<()> {
        let from_graph_name = if let Some(graph_name) = from_graph {
            Some(PyGraphNameRef::try_from(graph_name)?)
        } else {
            None
        };
        let output = PyFileLike::new(output);
        if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
            self.inner
                .dump_graph(
                    output,
                    graph_format,
                    &from_graph_name.unwrap_or(PyGraphNameRef::DefaultGraph),
                )
                .map_err(map_io_err)
        } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
            if from_graph_name.is_some() {
                return Err(PyValueError::new_err(
                    "The target graph name parameter is not available for dataset formats",
                ));
            }
            self.inner
                .dump_dataset(output, dataset_format)
                .map_err(map_io_err)
        } else {
            Err(PyValueError::new_err(format!(
                "Not supported MIME type: {}",
                mime_type
            )))
        }
    }

    /// Returns an iterator over all the store named graphs
    ///
    /// :return: an iterator of the store graph names
    /// :rtype: iter(NamedNode or BlankNode)
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store.named_graphs())
    /// [<NamedNode value=http://example.com/g>]
    #[text_signature = "($self)"]
    fn named_graphs(&self) -> GraphNameIter {
        GraphNameIter {
            inner: self.inner.named_graphs(),
        }
    }

    /// Adds a named graph to the store
    ///
    /// :param graph_name: the name of the name graph to add
    /// :type graph_name: NamedNode or BlankNode
    ///
    /// >>> store = MemoryStore()
    /// >>> store.add_graph(NamedNode('http://example.com/g'))
    /// >>> list(store.named_graphs())
    /// [<NamedNode value=http://example.com/g>]
    #[text_signature = "($self, graph_name)"]
    fn add_graph(&self, graph_name: PyGraphName) {
        match graph_name {
            PyGraphName::DefaultGraph(_) => (),
            PyGraphName::NamedNode(graph_name) => self.inner.insert_named_graph(graph_name),
            PyGraphName::BlankNode(graph_name) => self.inner.insert_named_graph(graph_name),
        }
    }

    /// Removes a graph from the store
    ///
    /// The default graph will not be remove but just cleared.
    ///
    /// :param graph_name: the name of the name graph to remove
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph
    ///
    /// >>> store = MemoryStore()
    /// >>> quad = Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))
    /// >>> store.remove_graph(NamedNode('http://example.com/g'))
    /// >>> list(store)
    /// []
    #[text_signature = "($self, graph_name)"]
    fn remove_graph(&self, graph_name: &PyAny) -> PyResult<()> {
        match PyGraphNameRef::try_from(graph_name)? {
            PyGraphNameRef::DefaultGraph => self.inner.clear_graph(GraphNameRef::DefaultGraph),
            PyGraphNameRef::NamedNode(graph_name) => self
                .inner
                .remove_named_graph(&PyNamedOrBlankNodeRef::NamedNode(graph_name)),
            PyGraphNameRef::BlankNode(graph_name) => self
                .inner
                .remove_named_graph(&PyNamedOrBlankNodeRef::BlankNode(graph_name)),
        }
        Ok(())
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
            _ => Err(PyNotImplementedError::new_err(
                "Ordering is not implemented",
            )),
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
            inner: slf.inner.iter(),
        }
    }
}

#[pyclass(unsendable, module = "oxigraph")]
pub struct QuadIter {
    inner: MemoryQuadIter,
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

#[pyclass(unsendable, module = "oxigraph")]
pub struct GraphNameIter {
    inner: MemoryGraphNameIter,
}

#[pyproto]
impl PyIterProtocol for GraphNameIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> Option<PyNamedOrBlankNode> {
        slf.inner.next().map(|q| q.into())
    }
}
