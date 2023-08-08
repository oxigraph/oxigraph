#![allow(clippy::needless_option_as_deref)]

use crate::io::{allow_threads_unsafe, map_io_err, map_parse_error, PyReadable, PyWritable};
use crate::model::*;
use crate::sparql::*;
use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::{GraphName, GraphNameRef};
use oxigraph::sparql::Update;
use oxigraph::store::{self, LoaderError, SerializerError, StorageError, Store};
use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use std::path::PathBuf;

/// RDF store.
///
/// It encodes a `RDF dataset <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_ and allows to query it using SPARQL.
/// It is based on the `RocksDB <https://rocksdb.org/>`_ key-value database.
///
/// This store ensures the "repeatable read" isolation level: the store only exposes changes that have
/// been "committed" (i.e. no partial writes) and the exposed state does not change for the complete duration
/// of a read operation (e.g. a SPARQL query) or a read/write operation (e.g. a SPARQL update).
///
/// The :py:class:`Store` constructor opens a read-write instance.
/// To open a static read-only instance use :py:func:`Store.read_only`
/// and to open a read-only instance that tracks a read-write instance use :py:func:`Store.secondary`.
///
/// :param path: the path of the directory in which the store should read and write its data. If the directory does not exist, it is created.
///              If no directory is provided a temporary one is created and removed when the Python garbage collector removes the store.
///              In this case, the store data are kept in memory and never written on disk.
/// :type path: str or pathlib.Path or None, optional
/// :raises IOError: if the target directory contains invalid data or could not be accessed.
///
/// The :py:func:`str` function provides a serialization of the store in NQuads:
///
/// >>> store = Store()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
/// >>> str(store)
/// '<http://example.com> <http://example.com/p> "1" <http://example.com/g> .\n'
#[pyclass(frozen, name = "Store", module = "pyoxigraph")]
#[derive(Clone)]
pub struct PyStore {
    inner: Store,
}

#[pymethods]
impl PyStore {
    #[new]
    #[pyo3(signature = (path = None))]
    fn new(path: Option<PathBuf>, py: Python<'_>) -> PyResult<Self> {
        py.allow_threads(|| {
            Ok(Self {
                inner: if let Some(path) = path {
                    Store::open(path)
                } else {
                    Store::new()
                }
                .map_err(map_storage_error)?,
            })
        })
    }

    /// Opens a read-only store from disk.
    ///
    /// Opening as read-only while having an other process writing the database is undefined behavior.
    /// :py:func:`Store.secondary` should be used in this case.
    ///
    /// :param path: path to the primary read-write instance data.
    /// :type path: str
    /// :return: the opened store.
    /// :rtype: Store
    /// :raises IOError: if the target directory contains invalid data or could not be accessed.
    #[staticmethod]
    fn read_only(path: &str, py: Python<'_>) -> PyResult<Self> {
        py.allow_threads(|| {
            Ok(Self {
                inner: Store::open_read_only(path).map_err(map_storage_error)?,
            })
        })
    }

    /// Opens a read-only clone of a running read-write store.
    ///
    /// Changes done while this process is running will be replicated after a possible lag.
    ///
    /// It should only be used if a primary instance opened with :py:func:`Store` is running at the same time.
    ///
    /// If you want to simple read-only store use :py:func:`Store.read_only`.
    ///
    /// :param primary_path: path to the primary read-write instance data.
    /// :type primary_path: str
    /// :param secondary_path: path to an other directory for the secondary instance cache. If not given a temporary directory will be used.
    /// :type secondary_path: str or None, optional
    /// :return: the opened store.
    /// :rtype: Store
    /// :raises IOError: if the target directories contain invalid data or could not be accessed.
    #[staticmethod]
    #[pyo3(signature = (primary_path, secondary_path = None))]
    fn secondary(
        primary_path: &str,
        secondary_path: Option<&str>,
        py: Python<'_>,
    ) -> PyResult<Self> {
        py.allow_threads(|| {
            Ok(Self {
                inner: if let Some(secondary_path) = secondary_path {
                    Store::open_persistent_secondary(primary_path, secondary_path)
                } else {
                    Store::open_secondary(primary_path)
                }
                .map_err(map_storage_error)?,
            })
        })
    }

    /// Adds a quad to the store.
    ///
    /// :param quad: the quad to add.
    /// :type quad: Quad
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    fn add(&self, quad: &PyQuad, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| {
            self.inner.insert(quad).map_err(map_storage_error)?;
            Ok(())
        })
    }

    /// Adds atomically a set of quads to this store.
    ///
    /// Insertion is done in a transactional manner: either the full operation succeeds or nothing is written to the database.
    /// The :py:func:`bulk_extend` method is also available for much faster loading of a large number of quads but without transactional guarantees.
    ///
    /// :param quads: the quads to add.
    /// :type quads: iterable(Quad)
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.extend([Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))])
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    fn extend(&self, quads: &PyAny, py: Python<'_>) -> PyResult<()> {
        let quads = quads
            .iter()?
            .map(|q| q?.extract())
            .collect::<PyResult<Vec<PyQuad>>>()?;
        py.allow_threads(|| {
            self.inner.extend(quads).map_err(map_storage_error)?;
            Ok(())
        })
    }

    /// Adds a set of quads to this store.
    ///
    /// This function is designed to be as fast as possible **without** transactional guarantees.
    /// Only a part of the data might be written to the store.
    ///
    /// :param quads: the quads to add.
    /// :type quads: iterable(Quad)
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.bulk_extend([Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))])
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    fn bulk_extend(&self, quads: &PyAny) -> PyResult<()> {
        self.inner
            .bulk_loader()
            .load_ok_quads::<PyErr, PythonOrStorageError>(
                quads.iter()?.map(|q| q?.extract::<PyQuad>()),
            )?;
        Ok(())
    }

    /// Removes a quad from the store.
    ///
    /// :param quad: the quad to remove.
    /// :type quad: Quad
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the quad removal.
    ///
    /// >>> store = Store()
    /// >>> quad = Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))
    /// >>> store.add(quad)
    /// >>> store.remove(quad)
    /// >>> list(store)
    /// []
    fn remove(&self, quad: &PyQuad, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| {
            self.inner.remove(quad).map_err(map_storage_error)?;
            Ok(())
        })
    }

    /// Looks for the quads matching a given pattern.
    ///
    /// :param subject: the quad subject or :py:const:`None` to match everything.
    /// :type subject: NamedNode or BlankNode or Triple or None
    /// :param predicate: the quad predicate or :py:const:`None` to match everything.
    /// :type predicate: NamedNode or None
    /// :param object: the quad object or :py:const:`None` to match everything.
    /// :type object: NamedNode or BlankNode or Literal or Triple or None
    /// :param graph_name: the quad graph name. To match only the default graph, use :py:class:`DefaultGraph`. To match everything use :py:const:`None`.
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :return: an iterator of the quads matching the pattern.
    /// :rtype: iterator(Quad)
    /// :raises IOError: if an I/O error happens during the quads lookup.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store.quads_for_pattern(NamedNode('http://example.com'), None, None, None))
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[pyo3(signature = (subject, predicate, object, graph_name = None))]
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
                subject.as_ref().map(Into::into),
                predicate.as_ref().map(Into::into),
                object.as_ref().map(Into::into),
                graph_name.as_ref().map(Into::into),
            ),
        })
    }

    /// Executes a `SPARQL 1.1 query <https://www.w3.org/TR/sparql11-query/>`_.
    ///
    /// :param query: the query to execute.
    /// :type query: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the SPARQL query or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param use_default_graph_as_union: if the SPARQL query should look for triples in all the dataset graphs by default (i.e. without `GRAPH` operations). Disabled by default.
    /// :type use_default_graph_as_union: bool, optional
    /// :param default_graph: list of the graphs that should be used as the query default graph. By default, the store default graph is used.
    /// :type default_graph: NamedNode or BlankNode or DefaultGraph or list(NamedNode or BlankNode or DefaultGraph) or None, optional
    /// :param named_graphs: list of the named graphs that could be used in SPARQL `GRAPH` clause. By default, all the store named graphs are available.
    /// :type named_graphs: list(NamedNode or BlankNode) or None, optional
    /// :return: a :py:class:`bool` for ``ASK`` queries, an iterator of :py:class:`Triple` for ``CONSTRUCT`` and ``DESCRIBE`` queries and an iterator of :py:class:`QuerySolution` for ``SELECT`` queries.
    /// :rtype: QuerySolutions or QueryTriples or bool
    /// :raises SyntaxError: if the provided query is invalid.
    /// :raises IOError: if an I/O error happens while reading the store.
    ///
    /// ``SELECT`` query:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> [solution['s'] for solution in store.query('SELECT ?s WHERE { ?s ?p ?o }')]
    /// [<NamedNode value=http://example.com>]
    ///
    /// ``CONSTRUCT`` query:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> list(store.query('CONSTRUCT WHERE { ?s ?p ?o }'))
    /// [<Triple subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]
    ///
    /// ``ASK`` query:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.query('ASK { ?s ?p ?o }')
    /// True
    #[pyo3(signature = (query, *, base_iri = None, use_default_graph_as_union = false, default_graph = None, named_graphs = None))]
    fn query(
        &self,
        query: &str,
        base_iri: Option<&str>,
        use_default_graph_as_union: bool,
        default_graph: Option<&PyAny>,
        named_graphs: Option<&PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyObject> {
        let query = parse_query(
            query,
            base_iri,
            use_default_graph_as_union,
            default_graph,
            named_graphs,
        )?;
        let results =
            allow_threads_unsafe(|| self.inner.query(query)).map_err(map_evaluation_error)?;
        Ok(query_results_to_python(py, results))
    }

    /// Executes a `SPARQL 1.1 update <https://www.w3.org/TR/sparql11-update/>`_.
    ///
    /// Updates are applied in a transactional manner: either the full operation succeeds or nothing is written to the database.
    ///
    /// :param update: the update to execute.
    /// :type update: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the SPARQL update or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :rtype: None
    /// :raises SyntaxError: if the provided update is invalid.
    /// :raises IOError: if an I/O error happens while reading the store.
    ///
    /// ``INSERT DATA`` update:
    ///
    /// >>> store = Store()
    /// >>> store.update('INSERT DATA { <http://example.com> <http://example.com/p> "1" }')
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<DefaultGraph>>]
    ///
    /// ``DELETE DATA`` update:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.update('DELETE DATA { <http://example.com> <http://example.com/p> "1" }')
    /// >>> list(store)
    /// []
    ///
    /// ``DELETE`` update:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.update('DELETE WHERE { <http://example.com> ?p ?o }')
    /// >>> list(store)
    /// []
    #[pyo3(signature = (update, *, base_iri = None))]
    fn update(&self, update: &str, base_iri: Option<&str>, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| {
            let update =
                Update::parse(update, base_iri).map_err(|e| map_evaluation_error(e.into()))?;
            self.inner.update(update).map_err(map_evaluation_error)
        })
    }

    /// Loads an RDF serialization into the store.
    ///
    /// Loads are applied in a transactional manner: either the full operation succeeds or nothing is written to the database.
    /// The :py:func:`bulk_load` method is also available for much faster loading of big files but without transactional guarantees.
    ///
    /// Beware, the full file is loaded into memory.
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
    /// For example, ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
    ///
    /// :param input: The binary I/O object or file path to read from. For example, it could be a file path as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
    /// :type input: io(bytes) or io(str) or str or pathlib.Path
    /// :param mime_type: the MIME type of the RDF serialization.
    /// :type mime_type: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param to_graph: if it is a file composed of triples, the graph in which the triples should be stored. By default, the default graph is used.
    /// :type to_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :rtype: None
    /// :raises ValueError: if the MIME type is not supported or the `to_graph` parameter is given with a quad file.
    /// :raises SyntaxError: if the provided data is invalid.
    /// :raises IOError: if an I/O error happens during a quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.load(io.BytesIO(b'<foo> <p> "1" .'), "text/turtle", base_iri="http://example.com/", to_graph=NamedNode("http://example.com/g"))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[pyo3(signature = (input, mime_type, *, base_iri = None, to_graph = None))]
    fn load(
        &self,
        input: PyObject,
        mime_type: &str,
        base_iri: Option<&str>,
        to_graph: Option<&PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let to_graph_name = if let Some(graph_name) = to_graph {
            Some(GraphName::from(&PyGraphNameRef::try_from(graph_name)?))
        } else {
            None
        };
        let input = if let Ok(path) = input.extract::<PathBuf>(py) {
            PyReadable::from_file(&path, py).map_err(map_io_err)?
        } else {
            PyReadable::from_data(input, py)
        };
        py.allow_threads(|| {
            if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
                self.inner
                    .load_graph(
                        input,
                        graph_format,
                        to_graph_name.as_ref().unwrap_or(&GraphName::DefaultGraph),
                        base_iri,
                    )
                    .map_err(map_loader_error)
            } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
                if to_graph_name.is_some() {
                    return Err(PyValueError::new_err(
                        "The target graph name parameter is not available for dataset formats",
                    ));
                }
                self.inner
                    .load_dataset(input, dataset_format, base_iri)
                    .map_err(map_loader_error)
            } else {
                Err(PyValueError::new_err(format!(
                    "Not supported MIME type: {mime_type}"
                )))
            }
        })
    }

    /// Loads an RDF serialization into the store.
    ///
    /// This function is designed to be as fast as possible on big files **without** transactional guarantees.
    /// If the file is invalid only a piece of it might be written to the store.
    ///
    /// The :py:func:`load` method is also available for loads with transactional guarantees.
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
    /// For example, ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
    ///
    /// :param input: The binary I/O object or file path to read from. For example, it could be a file path as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
    /// :type input: io(bytes) or io(str) or str or pathlib.Path
    /// :param mime_type: the MIME type of the RDF serialization.
    /// :type mime_type: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param to_graph: if it is a file composed of triples, the graph in which the triples should be stored. By default, the default graph is used.
    /// :type to_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :rtype: None
    /// :raises ValueError: if the MIME type is not supported or the `to_graph` parameter is given with a quad file.
    /// :raises SyntaxError: if the provided data is invalid.
    /// :raises IOError: if an I/O error happens during a quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.bulk_load(io.BytesIO(b'<foo> <p> "1" .'), "text/turtle", base_iri="http://example.com/", to_graph=NamedNode("http://example.com/g"))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[pyo3(signature = (input, mime_type, *, base_iri = None, to_graph = None))]
    fn bulk_load(
        &self,
        input: PyObject,
        mime_type: &str,
        base_iri: Option<&str>,
        to_graph: Option<&PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let to_graph_name = if let Some(graph_name) = to_graph {
            Some(GraphName::from(&PyGraphNameRef::try_from(graph_name)?))
        } else {
            None
        };
        let input = if let Ok(path) = input.extract::<PathBuf>(py) {
            PyReadable::from_file(&path, py).map_err(map_io_err)?
        } else {
            PyReadable::from_data(input, py)
        };
        py.allow_threads(|| {
            if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
                self.inner
                    .bulk_loader()
                    .load_graph(
                        input,
                        graph_format,
                        &to_graph_name.unwrap_or(GraphName::DefaultGraph),
                        base_iri,
                    )
                    .map_err(map_loader_error)
            } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
                if to_graph_name.is_some() {
                    return Err(PyValueError::new_err(
                        "The target graph name parameter is not available for dataset formats",
                    ));
                }
                self.inner
                    .bulk_loader()
                    .load_dataset(input, dataset_format, base_iri)
                    .map_err(map_loader_error)
            } else {
                Err(PyValueError::new_err(format!(
                    "Not supported MIME type: {mime_type}"
                )))
            }
        })
    }

    /// Dumps the store quads or triples into a file.
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
    /// For example, ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
    ///
    /// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``.
    /// :type output: io(bytes) or str or pathlib.Path
    /// :param mime_type: the MIME type of the RDF serialization.
    /// :type mime_type: str
    /// :param from_graph: if a triple based format is requested, the store graph from which dump the triples. By default, the default graph is used.
    /// :type from_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :rtype: None
    /// :raises ValueError: if the MIME type is not supported or the `from_graph` parameter is given with a quad syntax.
    /// :raises IOError: if an I/O error happens during a quad lookup
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> output = io.BytesIO()
    /// >>> store.dump(output, "text/turtle", from_graph=NamedNode("http://example.com/g"))
    /// >>> output.getvalue()
    /// b'<http://example.com> <http://example.com/p> "1" .\n'
    #[pyo3(signature = (output, mime_type, *, from_graph = None))]
    fn dump(
        &self,
        output: PyObject,
        mime_type: &str,
        from_graph: Option<&PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let output = if let Ok(path) = output.extract::<PathBuf>(py) {
            PyWritable::from_file(&path, py).map_err(map_io_err)?
        } else {
            PyWritable::from_data(output)
        };
        let from_graph_name = if let Some(graph_name) = from_graph {
            Some(GraphName::from(&PyGraphNameRef::try_from(graph_name)?))
        } else {
            None
        };
        py.allow_threads(|| {
            if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
                self.inner
                    .dump_graph(
                        output,
                        graph_format,
                        &from_graph_name.unwrap_or(GraphName::DefaultGraph),
                    )
                    .map_err(map_serializer_error)
            } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
                if from_graph_name.is_some() {
                    return Err(PyValueError::new_err(
                        "The target graph name parameter is not available for dataset formats",
                    ));
                }
                self.inner
                    .dump_dataset(output, dataset_format)
                    .map_err(map_serializer_error)
            } else {
                Err(PyValueError::new_err(format!(
                    "Not supported MIME type: {mime_type}"
                )))
            }
        })
    }

    /// Returns an iterator over all the store named graphs.
    ///
    /// :return: an iterator of the store graph names.
    /// :rtype: iterator(NamedNode or BlankNode)
    /// :raises IOError: if an I/O error happens during the named graphs lookup.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store.named_graphs())
    /// [<NamedNode value=http://example.com/g>]
    fn named_graphs(&self) -> GraphNameIter {
        GraphNameIter {
            inner: self.inner.named_graphs(),
        }
    }

    /// Returns if the store contains the given named graph.
    ///
    /// :param graph_name: the name of the named graph.
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph
    /// :rtype: bool
    /// :raises IOError: if an I/O error happens during the named graph lookup.
    ///
    /// >>> store = Store()
    /// >>> store.add_graph(NamedNode('http://example.com/g'))
    /// >>> store.contains_named_graph(NamedNode('http://example.com/g'))
    /// True
    fn contains_named_graph(&self, graph_name: &PyAny) -> PyResult<bool> {
        let graph_name = GraphName::from(&PyGraphNameRef::try_from(graph_name)?);
        match graph_name {
            GraphName::DefaultGraph => Ok(true),
            GraphName::NamedNode(graph_name) => self.inner.contains_named_graph(&graph_name),
            GraphName::BlankNode(graph_name) => self.inner.contains_named_graph(&graph_name),
        }
        .map_err(map_storage_error)
    }

    /// Adds a named graph to the store.
    ///
    /// :param graph_name: the name of the name graph to add.
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the named graph insertion.
    ///
    /// >>> store = Store()
    /// >>> store.add_graph(NamedNode('http://example.com/g'))
    /// >>> list(store.named_graphs())
    /// [<NamedNode value=http://example.com/g>]
    fn add_graph(&self, graph_name: &PyAny, py: Python<'_>) -> PyResult<()> {
        let graph_name = GraphName::from(&PyGraphNameRef::try_from(graph_name)?);
        py.allow_threads(|| {
            match graph_name {
                GraphName::DefaultGraph => Ok(()),
                GraphName::NamedNode(graph_name) => {
                    self.inner.insert_named_graph(&graph_name).map(|_| ())
                }
                GraphName::BlankNode(graph_name) => {
                    self.inner.insert_named_graph(&graph_name).map(|_| ())
                }
            }
            .map_err(map_storage_error)
        })
    }

    /// Clears a graph from the store without removing it.
    ///
    /// :param graph_name: the name of the name graph to clear.
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the operation.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> store.clear_graph(NamedNode('http://example.com/g'))
    /// >>> list(store)
    /// []
    /// >>> list(store.named_graphs())
    /// [<NamedNode value=http://example.com/g>]
    fn clear_graph(&self, graph_name: &PyAny, py: Python<'_>) -> PyResult<()> {
        let graph_name = GraphName::from(&PyGraphNameRef::try_from(graph_name)?);
        py.allow_threads(|| {
            self.inner
                .clear_graph(&graph_name)
                .map_err(map_storage_error)
        })
    }

    /// Removes a graph from the store.
    ///
    /// The default graph will not be removed but just cleared.
    ///
    /// :param graph_name: the name of the name graph to remove.
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the named graph removal.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> store.remove_graph(NamedNode('http://example.com/g'))
    /// >>> list(store.named_graphs())
    /// []
    fn remove_graph(&self, graph_name: &PyAny, py: Python<'_>) -> PyResult<()> {
        let graph_name = GraphName::from(&PyGraphNameRef::try_from(graph_name)?);
        py.allow_threads(|| {
            match graph_name {
                GraphName::DefaultGraph => self.inner.clear_graph(GraphNameRef::DefaultGraph),
                GraphName::NamedNode(graph_name) => {
                    self.inner.remove_named_graph(&graph_name).map(|_| ())
                }
                GraphName::BlankNode(graph_name) => {
                    self.inner.remove_named_graph(&graph_name).map(|_| ())
                }
            }
            .map_err(map_storage_error)
        })
    }

    /// Clears the store by removing all its contents.
    ///
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the operation.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> store.clear()
    /// >>> list(store)
    /// []
    /// >>> list(store.named_graphs())
    /// []
    fn clear(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.inner.clear().map_err(map_storage_error))
    }

    /// Flushes all buffers and ensures that all writes are saved on disk.
    ///
    /// Flushes are automatically done using background threads but might lag a little bit.
    ///
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the flush.
    fn flush(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.inner.flush().map_err(map_storage_error))
    }

    /// Optimizes the database for future workload.
    ///
    /// Useful to call after a batch upload or another similar operation.
    ///
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the optimization.
    fn optimize(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.inner.optimize().map_err(map_storage_error))
    }

    /// Creates database backup into the `target_directory`.
    ///
    /// After its creation, the backup is usable using :py:class:`Store` constructor.
    /// like a regular pyxigraph database and operates independently from the original database.
    ///
    /// Warning: Backups are only possible for on-disk databases created by providing a path to :py:class:`Store` constructor.
    /// Temporary in-memory databases created without path are not compatible with the backup system.
    ///
    /// Warning: An error is raised if the ``target_directory`` already exists.
    ///
    /// If the target directory is in the same file system as the current database,
    /// the database content will not be fully copied
    /// but hard links will be used to point to the original database immutable snapshots.
    /// This allows cheap regular backups.
    ///
    /// If you want to move your data to another RDF storage system, you should have a look at the :py:func:`dump_dataset` function instead.
    ///
    /// :param target_directory: the directory name to save the database to.
    /// :type target_directory: str
    /// :rtype: None
    /// :raises IOError: if an I/O error happens during the backup.
    fn backup(&self, target_directory: &str, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| {
            self.inner
                .backup(target_directory)
                .map_err(map_storage_error)
        })
    }

    fn __str__(&self, py: Python<'_>) -> String {
        py.allow_threads(|| self.inner.to_string())
    }

    fn __bool__(&self) -> PyResult<bool> {
        Ok(!self.inner.is_empty().map_err(map_storage_error)?)
    }

    fn __len__(&self) -> PyResult<usize> {
        self.inner.len().map_err(map_storage_error)
    }

    fn __contains__(&self, quad: &PyQuad) -> PyResult<bool> {
        self.inner.contains(quad).map_err(map_storage_error)
    }

    fn __iter__(&self) -> QuadIter {
        QuadIter {
            inner: self.inner.iter(),
        }
    }
}

#[pyclass(unsendable, module = "pyoxigraph")]
pub struct QuadIter {
    inner: store::QuadIter,
}

#[pymethods]
impl QuadIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<Self> {
        slf
    }

    fn __next__(&mut self) -> PyResult<Option<PyQuad>> {
        self.inner
            .next()
            .map(|q| Ok(q.map_err(map_storage_error)?.into()))
            .transpose()
    }
}

#[pyclass(unsendable, module = "pyoxigraph")]
pub struct GraphNameIter {
    inner: store::GraphNameIter,
}

#[pymethods]
impl GraphNameIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<Self> {
        slf
    }

    fn __next__(&mut self) -> PyResult<Option<PyNamedOrBlankNode>> {
        self.inner
            .next()
            .map(|q| Ok(q.map_err(map_storage_error)?.into()))
            .transpose()
    }
}

pub fn extract_quads_pattern<'a>(
    subject: &'a PyAny,
    predicate: &'a PyAny,
    object: &'a PyAny,
    graph_name: Option<&'a PyAny>,
) -> PyResult<(
    Option<PySubjectRef<'a>>,
    Option<PyNamedNodeRef<'a>>,
    Option<PyTermRef<'a>>,
    Option<PyGraphNameRef<'a>>,
)> {
    Ok((
        if subject.is_none() {
            None
        } else {
            Some(TryFrom::try_from(subject)?)
        },
        if predicate.is_none() {
            None
        } else {
            Some(TryFrom::try_from(predicate)?)
        },
        if object.is_none() {
            None
        } else {
            Some(TryFrom::try_from(object)?)
        },
        if let Some(graph_name) = graph_name {
            if graph_name.is_none() {
                None
            } else {
                Some(TryFrom::try_from(graph_name)?)
            }
        } else {
            None
        },
    ))
}

pub fn map_storage_error(error: StorageError) -> PyErr {
    match error {
        StorageError::Io(error) => PyIOError::new_err(error.to_string()),
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}

pub fn map_loader_error(error: LoaderError) -> PyErr {
    match error {
        LoaderError::Storage(error) => map_storage_error(error),
        LoaderError::Parsing(error) => map_parse_error(error),
    }
}

pub fn map_serializer_error(error: SerializerError) -> PyErr {
    match error {
        SerializerError::Storage(error) => map_storage_error(error),
        SerializerError::Io(error) => PyIOError::new_err(error.to_string()),
    }
}

enum PythonOrStorageError {
    Python(PyErr),
    Storage(StorageError),
}

impl From<PyErr> for PythonOrStorageError {
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

impl From<StorageError> for PythonOrStorageError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}
impl From<PythonOrStorageError> for PyErr {
    fn from(error: PythonOrStorageError) -> Self {
        match error {
            PythonOrStorageError::Python(error) => error,
            PythonOrStorageError::Storage(error) => map_storage_error(error),
        }
    }
}
