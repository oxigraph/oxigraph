use crate::io::{
    PyRdfFormatInput, PyReadable, PyReadableInput, PyWritable, PyWritableOutput, lookup_rdf_format,
    map_parse_error,
};
use crate::model::*;
use crate::sparql::*;
use oxigraph::io::{RdfParser, RdfSerializer};
use oxigraph::model::GraphNameRef;
use oxigraph::sparql::QueryResults;
use oxigraph::store::{self, LoaderError, SerializerError, StorageError, Store};
use pyo3::exceptions::{PyRuntimeError, PySyntaxError, PyValueError};
use pyo3::prelude::*;
use std::collections::{BTreeMap, HashMap};
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
/// To open a static read-only instance use :py:func:`Store.read_only`.
///
/// :param path: the path of the directory in which the store should read and write its data. If the directory does not exist, it is created.
///              If no directory is provided a temporary one is created and removed when the Python garbage collector removes the store.
///              In this case, the store data are kept in memory and never written on disk.
/// :type path: str or os.PathLike[str] or None, optional
/// :raises OSError: if the target directory contains invalid data or could not be accessed.
///
/// The :py:class:`str` function provides a serialization of the store in NQuads:
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
    #[cfg(not(target_family = "wasm"))]
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

    #[cfg(target_family = "wasm")]
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        py.allow_threads(|| {
            Ok(Self {
                inner: Store::new().map_err(map_storage_error)?,
            })
        })
    }

    /// Opens a read-only store from disk.
    ///
    /// Opening as read-only while having an other process writing the database is undefined behavior.
    ///
    /// :param path: path to the primary read-write instance data.
    /// :type path: str
    /// :return: the opened store.
    /// :rtype: Store
    /// :raises OSError: if the target directory contains invalid data or could not be accessed.
    #[cfg(not(target_family = "wasm"))]
    #[staticmethod]
    fn read_only(path: &str, py: Python<'_>) -> PyResult<Self> {
        py.allow_threads(|| {
            Ok(Self {
                inner: Store::open_read_only(path).map_err(map_storage_error)?,
            })
        })
    }

    /// Adds a quad to the store.
    ///
    /// :param quad: the quad to add.
    /// :type quad: Quad
    /// :rtype: None
    /// :raises OSError: if an error happens during the quad insertion.
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

    /// Adds a set of quads to this store.
    ///
    /// Insertion is done in a transactional manner: either the full operation succeeds, or nothing is written to the database.
    /// The :py:func:`bulk_extend` method is also available for loading of a very large number of quads without having them all into memory.
    ///
    /// :param quads: the quads to add.
    /// :type quads: collections.abc.Iterable[Quad]
    /// :rtype: None
    /// :raises OSError: if an error happens during the quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.extend([Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))])
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    fn extend(&self, quads: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<()> {
        let quads = quads
            .try_iter()?
            .map(|q| q?.extract())
            .collect::<PyResult<Vec<PyQuad>>>()?;
        py.allow_threads(|| {
            self.inner.extend(quads).map_err(map_storage_error)?;
            Ok(())
        })
    }

    /// Adds a set of quads to this store.
    ///
    /// :param quads: the quads to add.
    /// :type quads: collections.abc.Iterable[Quad]
    /// :rtype: None
    /// :raises OSError: if an error happens during the quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.bulk_extend([Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))])
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[cfg(not(target_family = "wasm"))]
    fn bulk_extend(&self, quads: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner
            .bulk_loader()
            .load_ok_quads::<PyErr, PythonOrStorageError>(
                quads.try_iter()?.map(|q| q?.extract::<PyQuad>()),
            )?;
        Ok(())
    }

    /// Removes a quad from the store.
    ///
    /// :param quad: the quad to remove.
    /// :type quad: Quad
    /// :rtype: None
    /// :raises OSError: if an error happens during the quad removal.
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
    /// :rtype: collections.abc.Iterator[Quad]
    /// :raises OSError: if an error happens during the quads lookup.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store.quads_for_pattern(NamedNode('http://example.com'), None, None, None))
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[expect(clippy::needless_pass_by_value)]
    #[pyo3(signature = (subject, predicate, object, graph_name = None))]
    fn quads_for_pattern(
        &self,
        subject: Option<PyNamedOrBlankNodeRef<'_>>,
        predicate: Option<PyNamedNodeRef<'_>>,
        object: Option<PyTermRef<'_>>,
        graph_name: Option<PyGraphNameRef<'_>>,
    ) -> QuadIter {
        QuadIter {
            inner: self.inner.quads_for_pattern(
                subject.as_ref().map(Into::into),
                predicate.as_ref().map(Into::into),
                object.as_ref().map(Into::into),
                graph_name.as_ref().map(Into::into),
            ),
        }
    }

    /// Executes a `SPARQL 1.1 query <https://www.w3.org/TR/sparql11-query/>`_.
    ///
    /// :param query: the query to execute.
    /// :type query: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the SPARQL query or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param prefixes: a set of default prefixes to use during the SPARQL query parsing as a prefix name -> prefix IRI dictionary.
    /// :type prefixes: dict[str, str] or None, optional
    /// :param use_default_graph_as_union: if the SPARQL query should look for triples in all the dataset graphs by default (i.e. without `GRAPH` operations). Disabled by default.
    /// :type use_default_graph_as_union: bool, optional
    /// :param default_graph: list of the graphs that should be used as the query default graph. By default, the store default graph is used.
    /// :type default_graph: NamedNode or BlankNode or DefaultGraph or list[NamedNode or BlankNode or DefaultGraph] or None, optional
    /// :param named_graphs: list of the named graphs that could be used in SPARQL `GRAPH` clause. By default, all the store named graphs are available.
    /// :type named_graphs: list[NamedNode or BlankNode] or None, optional
    /// :param substitutions: dictionary of values variables should be substituted with. Substitution follows `RDF-dev SEP-0007 <https://github.com/w3c/sparql-dev/blob/main/SEP/SEP-0007/sep-0007.md>`_.
    /// :type substitutions: dict[Variable, NamedNode or BlankNode or Literal or Triple] or None, optional
    /// :param custom_functions: dictionary of custom functions mapping function names to their definition. Custom functions takes for input some RDF term and returns a RDF term or :py:const:`None`.
    /// :type custom_functions: dict[NamedNode, typing.Callable[[NamedNode or BlankNode or Literal or Triple, ...], NamedNode or BlankNode or Literal or Triple or None]] or None, optional
    /// :param custom_aggregate_functions: dictionary of custom aggregate functions mapping function names to their definition. Custom aggregate functions take no input and return an object with two methods, `accumulate(self, term: Term)` to add a new term to the accumulator and `finish(self) -> Term` to return the accumulated result.
    /// :type custom_aggregate_functions: dict[NamedNode, typing.Callable[[], AggregateFunctionAccumulator]] or None, optional
    /// :return: a :py:class:`bool` for ``ASK`` queries, an iterator of :py:class:`Triple` for ``CONSTRUCT`` and ``DESCRIBE`` queries and an iterator of :py:class:`QuerySolution` for ``SELECT`` queries.
    /// :rtype: QuerySolutions or QueryBoolean or QueryTriples
    /// :raises SyntaxError: if the provided query is invalid.
    /// :raises OSError: if an error happens while reading the store.
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
    /// >>> bool(store.query('ASK { ?s ?p ?o }'))
    /// True
    #[expect(clippy::too_many_arguments)]
    #[pyo3(signature = (query, *, base_iri = None, prefixes = None, use_default_graph_as_union = false, default_graph = None, named_graphs = None, substitutions = None, custom_functions = None, custom_aggregate_functions = None))]
    fn query<'py>(
        &self,
        query: &str,
        base_iri: Option<&str>,
        prefixes: Option<HashMap<String, String>>,
        use_default_graph_as_union: bool,
        default_graph: Option<&Bound<'_, PyAny>>,
        named_graphs: Option<&Bound<'_, PyAny>>,
        substitutions: Option<HashMap<PyVariable, PyTerm>>,
        custom_functions: Option<HashMap<PyNamedNode, PyObject>>,
        custom_aggregate_functions: Option<HashMap<PyNamedNode, PyObject>>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        pub struct UngilQueryResults(QueryResults<'static>);

        #[expect(unsafe_code)]
        // SAFETY: To derive Ungil
        unsafe impl Send for UngilQueryResults {}

        let mut evaluator = prepare_sparql_query(
            sparql_evaluator_from_python(
                base_iri,
                prefixes,
                custom_functions,
                custom_aggregate_functions,
            )?,
            query,
            use_default_graph_as_union,
            default_graph,
            named_graphs,
        )?
        .on_store(&self.inner);
        if let Some(substitutions) = substitutions {
            for (k, v) in substitutions {
                evaluator = evaluator.substitute_variable(k, v);
            }
        }
        let results = py
            .allow_threads(|| Ok(UngilQueryResults(evaluator.execute()?)))
            .map_err(map_evaluation_error)?
            .0;
        query_results_to_python(py, results)
    }

    /// Executes a `SPARQL 1.1 update <https://www.w3.org/TR/sparql11-update/>`_.
    ///
    /// Updates are applied in a transactional manner: either the full operation succeeds, or nothing is written to the database.
    ///
    /// :param update: the update to execute.
    /// :type update: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the SPARQL update or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param prefixes: a set of default prefixes to use during the SPARQL query parsing as a prefix name -> prefix IRI dictionary.
    /// :type prefixes: dict[str, str] or None, optional
    /// :param custom_functions: dictionary of custom functions mapping function names to their definition. Custom functions take for input some RDF terms and returns a RDF term or :py:const:`None`.
    /// :type custom_functions: dict[NamedNode, typing.Callable[[NamedNode or BlankNode or Literal or Triple, ...], NamedNode or BlankNode or Literal or Triple or None]] or None, optional
    /// :param custom_aggregate_functions: dictionary of custom aggregate functions mapping function names to their definition. Custom aggregate functions take no input and return an object with two methods, `accumulate(self, term: Term)` to add a new term to the accumulator and `finish(self) -> Term` to return the accumulated result.
    /// :type custom_aggregate_functions: dict[NamedNode, typing.Callable[[], AggregateFunctionAccumulator]] or None, optional
    /// :rtype: None
    /// :raises SyntaxError: if the provided update is invalid.
    /// :raises OSError: if an error happens while reading the store.
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
    #[pyo3(signature = (update, *, base_iri = None, prefixes = None, custom_functions = None, custom_aggregate_functions = None))]
    fn update(
        &self,
        update: &str,
        base_iri: Option<&str>,
        prefixes: Option<HashMap<String, String>>,
        custom_functions: Option<HashMap<PyNamedNode, PyObject>>,
        custom_aggregate_functions: Option<HashMap<PyNamedNode, PyObject>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        py.allow_threads(|| {
            sparql_evaluator_from_python(
                base_iri,
                prefixes,
                custom_functions,
                custom_aggregate_functions,
            )?
            .parse_update(update)
            .map_err(|e| PySyntaxError::new_err(e.to_string()))?
            .on_store(&self.inner)
            .execute()
            .map_err(map_update_evaluation_error)
        })
    }

    /// Loads RDF serialization into the store.
    ///
    /// Loads are applied in a transactional manner: either the full operation succeeds, or nothing is written to the database.
    /// The :py:func:`bulk_load` method is also available for loading big files without loading all its content into memory.
    ///
    /// Beware, the full file is loaded into memory.
    ///
    /// It currently supports the following formats:
    ///
    /// * `JSON-LD 1.0 <https://www.w3.org/TR/json-ld/>`_ (:py:attr:`RdfFormat.JSON_LD`)
    /// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (:py:attr:`RdfFormat.N_TRIPLES`)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (:py:attr:`RdfFormat.N_QUADS`)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (:py:attr:`RdfFormat.TURTLE`)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (:py:attr:`RdfFormat.TRIG`)
    /// * `N3 <https://w3c.github.io/N3/spec/>`_ (:py:attr:`RdfFormat.N3`)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (:py:attr:`RdfFormat.RDF_XML`)
    ///
    /// :param input: The :py:class:`str`, :py:class:`bytes` or I/O object to read from. For example, it could be the file content as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
    /// :type input: bytes or str or typing.IO[bytes] or typing.IO[str] or None, optional
    /// :param format: the format of the RDF serialization. If :py:const:`None`, the format is guessed from the file name extension.
    /// :type format: RdfFormat or None, optional
    /// :param path: The file path to read from. Replace the ``input`` parameter.
    /// :type path: str or os.PathLike[str] or None, optional
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param to_graph: if it is a file composed of triples, the graph in which the triples should be stored. By default, the default graph is used.
    /// :type to_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :param lenient: Skip some data validation during loading, like validating IRIs. This makes parsing faster at the cost of maybe ingesting invalid data.
    /// :type lenient: bool, optional
    /// :rtype: None
    /// :raises ValueError: if the format is not supported.
    /// :raises SyntaxError: if the provided data is invalid.
    /// :raises OSError: if an error happens during a quad insertion or if a system error happens while reading the file.
    ///
    /// >>> store = Store()
    /// >>> store.load(input='<foo> <p> "1" .', format=RdfFormat.TURTLE, base_iri="http://example.com/", to_graph=NamedNode("http://example.com/g"))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[expect(clippy::needless_pass_by_value)]
    #[pyo3(signature = (input = None, format = None, *, path = None, base_iri = None, to_graph = None, lenient=false))]
    fn load(
        &self,
        input: Option<PyReadableInput>,
        format: Option<PyRdfFormatInput>,
        path: Option<PathBuf>,
        base_iri: Option<&str>,
        to_graph: Option<PyGraphNameRef<'_>>,
        lenient: bool,
        py: Python<'_>,
    ) -> PyResult<()> {
        let to_graph_name = to_graph.as_ref().map(GraphNameRef::from);
        let input = PyReadable::from_args(&path, input, py)?;
        let format = lookup_rdf_format(format, path.as_deref())?;
        py.allow_threads(|| {
            let mut parser = RdfParser::from_format(format);
            if let Some(base_iri) = base_iri {
                parser = parser
                    .with_base_iri(base_iri)
                    .map_err(|e| PyValueError::new_err(e.to_string()))?;
            }
            if let Some(to_graph_name) = to_graph_name {
                parser = parser.with_default_graph(to_graph_name);
            }
            if lenient {
                parser = parser.lenient();
            }
            self.inner
                .load_from_reader(parser, input)
                .map_err(|e| map_loader_error(e, path))
        })
    }

    /// Loads some RDF serialization into the store.
    ///
    /// This function is designed to be as fast as possible on big files **without** transactional guarantees.
    /// If the file is invalid, only a piece of it might be written to the store.
    ///
    /// The :py:func:`load` method is also available for loads with transactional guarantees.
    ///
    /// It currently supports the following formats:
    ///
    /// * `JSON-LD 1.0 <https://www.w3.org/TR/json-ld/>`_ (:py:attr:`RdfFormat.JSON_LD`)
    /// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (:py:attr:`RdfFormat.N_TRIPLES`)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (:py:attr:`RdfFormat.N_QUADS`)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (:py:attr:`RdfFormat.TURTLE`)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (:py:attr:`RdfFormat.TRIG`)
    /// * `N3 <https://w3c.github.io/N3/spec/>`_ (:py:attr:`RdfFormat.N3`)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (:py:attr:`RdfFormat.RDF_XML`)
    ///
    /// :param input: The :py:class:`str`, :py:class:`bytes` or I/O object to read from. For example, it could be the file content as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
    /// :type input: bytes or str or typing.IO[bytes] or typing.IO[str] or None, optional
    /// :param format: the format of the RDF serialization. If :py:const:`None`, the format is guessed from the file name extension.
    /// :type format: RdfFormat or None, optional
    /// :param path: The file path to read from. Replace the ``input`` parameter.
    /// :type path: str or os.PathLike[str] or None, optional
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param to_graph: if it is a file composed of triples, the graph in which the triples should be stored. By default, the default graph is used.
    /// :type to_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :param lenient: Skip some data validation during loading, like validating IRIs. This makes parsing faster at the cost of maybe ingesting invalid data.
    /// :type lenient: bool, optional
    /// :rtype: None
    /// :raises ValueError: if the format is not supported.
    /// :raises SyntaxError: if the provided data is invalid.
    /// :raises OSError: if an error happens during a quad insertion or if a system error happens while reading the file.
    ///
    /// >>> store = Store()
    /// >>> store.bulk_load(input=b'<foo> <p> "1" .', format=RdfFormat.TURTLE, base_iri="http://example.com/", to_graph=NamedNode("http://example.com/g"))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[expect(clippy::needless_pass_by_value)]
    #[pyo3(signature = (input = None, format = None, *, path = None, base_iri = None, to_graph = None, lenient = false))]
    fn bulk_load(
        &self,
        input: Option<PyReadableInput>,
        format: Option<PyRdfFormatInput>,
        path: Option<PathBuf>,
        base_iri: Option<&str>,
        to_graph: Option<PyGraphNameRef<'_>>,
        lenient: bool,
        py: Python<'_>,
    ) -> PyResult<()> {
        let to_graph_name = to_graph.as_ref().map(GraphNameRef::from);
        let input = PyReadable::from_args(&path, input, py)?;
        let format = lookup_rdf_format(format, path.as_deref())?;
        py.allow_threads(|| {
            let mut parser = RdfParser::from_format(format);
            if let Some(base_iri) = base_iri {
                parser = parser.with_base_iri(base_iri).map_err(|e| {
                    PyValueError::new_err(format!("Invalid base IRI '{base_iri}', {e}"))
                })?;
            }
            if let Some(to_graph_name) = to_graph_name {
                parser = parser.with_default_graph(to_graph_name);
            }
            if lenient {
                parser = parser.lenient();
            }
            self.inner
                .bulk_loader()
                .load_from_reader(parser, input)
                .map_err(|e| map_loader_error(e, path))
        })
    }

    /// Dumps the store quads or triples into a file.
    ///
    /// It currently supports the following formats:
    ///
    /// * `JSON-LD 1.0 <https://www.w3.org/TR/json-ld/>`_ (:py:attr:`RdfFormat.JSON_LD`)
    /// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (:py:attr:`RdfFormat.N_TRIPLES`)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (:py:attr:`RdfFormat.N_QUADS`)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (:py:attr:`RdfFormat.TURTLE`)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (:py:attr:`RdfFormat.TRIG`)
    /// * `N3 <https://w3c.github.io/N3/spec/>`_ (:py:attr:`RdfFormat.N3`)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (:py:attr:`RdfFormat.RDF_XML`)
    ///
    /// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``. If :py:const:`None`, a :py:class:`bytes` buffer is returned with the serialized content.
    /// :type output: typing.IO[bytes] or str or os.PathLike[str] or None, optional
    /// :param format: the format of the RDF serialization.  If :py:const:`None`, the format is guessed from the file name extension.
    /// :type format: RdfFormat or None, optional
    /// :param from_graph: the store graph from which dump the triples. Required if the serialization format does not support named graphs. If it does supports named graphs the full dataset is written.
    /// :type from_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :param prefixes: the prefixes used in the serialization if the format supports it.
    /// :type prefixes: dict[str, str] or None, optional
    /// :param base_iri: the base IRI used in the serialization if the format supports it.
    /// :type base_iri: str or None, optional
    /// :return: :py:class:`bytes` with the serialization if the ``output`` parameter is :py:const:`None`, :py:const:`None` if ``output`` is set.
    /// :rtype: bytes or None
    /// :raises ValueError: if the format is not supported or the `from_graph` parameter is not given with a syntax not supporting named graphs.
    /// :raises OSError: if an error happens during a quad lookup or file writing.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.dump(format=RdfFormat.TRIG)
    /// b'<http://example.com> <http://example.com/p> "1" .\n'
    ///
    /// >>> import io
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> output = io.BytesIO()
    /// >>> store.dump(output, RdfFormat.TURTLE, from_graph=NamedNode("http://example.com/g"), prefixes={"ex": "http://example.com/"}, base_iri="http://example.com")
    /// >>> output.getvalue()
    /// b'@base <http://example.com> .\n@prefix ex: </> .\n<> ex:p "1" .\n'
    #[expect(clippy::needless_pass_by_value)]
    #[pyo3(signature = (output = None, format = None, *, from_graph = None, prefixes = None, base_iri = None))]
    fn dump(
        &self,
        output: Option<PyWritableOutput>,
        format: Option<PyRdfFormatInput>,
        from_graph: Option<PyGraphNameRef<'_>>,
        prefixes: Option<BTreeMap<String, String>>,
        base_iri: Option<&str>,
        py: Python<'_>,
    ) -> PyResult<Option<Vec<u8>>> {
        let from_graph_name = from_graph.as_ref().map(GraphNameRef::from);
        PyWritable::do_write(
            |output, file_path| {
                py.allow_threads(|| {
                    let format = lookup_rdf_format(format, file_path.as_deref())?;
                    let mut serializer = RdfSerializer::from_format(format);
                    if let Some(prefixes) = prefixes {
                        for (prefix_name, prefix_iri) in &prefixes {
                            serializer =
                                serializer
                                    .with_prefix(prefix_name, prefix_iri)
                                    .map_err(|e| {
                                        PyValueError::new_err(format!(
                                            "Invalid prefix {prefix_name} IRI '{prefix_iri}', {e}"
                                        ))
                                    })?;
                        }
                    }
                    if let Some(base_iri) = base_iri {
                        serializer = serializer.with_base_iri(base_iri).map_err(|e| {
                            PyValueError::new_err(format!("Invalid base IRI '{base_iri}', {e}"))
                        })?;
                    }
                    if let Some(from_graph_name) = from_graph_name {
                        self.inner
                            .dump_graph_to_writer(from_graph_name, serializer, output)
                    } else {
                        self.inner.dump_to_writer(serializer, output)
                    }
                    .map_err(map_serializer_error)
                })
            },
            output,
            py,
        )
    }

    /// Returns an iterator over all the store named graphs.
    ///
    /// :return: an iterator of the store graph names.
    /// :rtype: collections.abc.Iterator[NamedNode or BlankNode]
    /// :raises OSError: if an error happens during the named graphs lookup.
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
    /// :raises OSError: if an error happens during the named graph lookup.
    ///
    /// >>> store = Store()
    /// >>> store.add_graph(NamedNode('http://example.com/g'))
    /// >>> store.contains_named_graph(NamedNode('http://example.com/g'))
    /// True
    #[expect(clippy::needless_pass_by_value)]
    fn contains_named_graph(
        &self,
        graph_name: PyGraphNameRef<'_>,
        py: Python<'_>,
    ) -> PyResult<bool> {
        let graph_name = GraphNameRef::from(&graph_name);
        py.allow_threads(|| {
            match graph_name {
                GraphNameRef::DefaultGraph => Ok(true),
                GraphNameRef::NamedNode(graph_name) => self.inner.contains_named_graph(graph_name),
                GraphNameRef::BlankNode(graph_name) => self.inner.contains_named_graph(graph_name),
            }
            .map_err(map_storage_error)
        })
    }

    /// Adds a named graph to the store.
    ///
    /// :param graph_name: the name of the name graph to add.
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph
    /// :rtype: None
    /// :raises OSError: if an error happens during the named graph insertion.
    ///
    /// >>> store = Store()
    /// >>> store.add_graph(NamedNode('http://example.com/g'))
    /// >>> list(store.named_graphs())
    /// [<NamedNode value=http://example.com/g>]
    #[expect(clippy::needless_pass_by_value)]
    fn add_graph(&self, graph_name: PyGraphNameRef<'_>, py: Python<'_>) -> PyResult<()> {
        let graph_name = GraphNameRef::from(&graph_name);
        py.allow_threads(|| {
            match graph_name {
                GraphNameRef::DefaultGraph => Ok(()),
                GraphNameRef::NamedNode(graph_name) => self.inner.insert_named_graph(graph_name),
                GraphNameRef::BlankNode(graph_name) => self.inner.insert_named_graph(graph_name),
            }
            .map_err(map_storage_error)
        })
    }

    /// Clears a graph from the store without removing it.
    ///
    /// :param graph_name: the name of the name graph to clear.
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph
    /// :rtype: None
    /// :raises OSError: if an error happens during the operation.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> store.clear_graph(NamedNode('http://example.com/g'))
    /// >>> list(store)
    /// []
    /// >>> list(store.named_graphs())
    /// [<NamedNode value=http://example.com/g>]
    #[expect(clippy::needless_pass_by_value)]
    fn clear_graph(&self, graph_name: PyGraphNameRef<'_>, py: Python<'_>) -> PyResult<()> {
        let graph_name = GraphNameRef::from(&graph_name);
        py.allow_threads(|| {
            self.inner
                .clear_graph(graph_name)
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
    /// :raises OSError: if an error happens during the named graph removal.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> store.remove_graph(NamedNode('http://example.com/g'))
    /// >>> list(store.named_graphs())
    /// []
    #[expect(clippy::needless_pass_by_value)]
    fn remove_graph(&self, graph_name: PyGraphNameRef<'_>, py: Python<'_>) -> PyResult<()> {
        let graph_name = GraphNameRef::from(&graph_name);
        py.allow_threads(|| {
            match graph_name {
                GraphNameRef::DefaultGraph => self.inner.clear_graph(GraphNameRef::DefaultGraph),
                GraphNameRef::NamedNode(graph_name) => self.inner.remove_named_graph(graph_name),
                GraphNameRef::BlankNode(graph_name) => self.inner.remove_named_graph(graph_name),
            }
            .map_err(map_storage_error)
        })
    }

    /// Clears the store by removing all its contents.
    ///
    /// :rtype: None
    /// :raises OSError: if an error happens during the operation.
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
    /// :raises OSError: if an error happens during the flush.
    #[cfg(not(target_family = "wasm"))]
    fn flush(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.inner.flush().map_err(map_storage_error))
    }

    /// Optimizes the database for future workload.
    ///
    /// Useful to call after a batch upload or another similar operation.
    ///
    /// :rtype: None
    /// :raises OSError: if an error happens during the optimization.
    #[cfg(not(target_family = "wasm"))]
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
    /// :type target_directory: str or os.PathLike[str]
    /// :rtype: None
    /// :raises OSError: if an error happens during the backup.
    #[cfg(not(target_family = "wasm"))]
    fn backup(&self, target_directory: PathBuf, py: Python<'_>) -> PyResult<()> {
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
    inner: store::QuadIter<'static>,
}

#[pymethods]
impl QuadIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
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
    inner: store::GraphNameIter<'static>,
}

#[pymethods]
impl GraphNameIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> PyResult<Option<PyNamedOrBlankNode>> {
        self.inner
            .next()
            .map(|q| Ok(q.map_err(map_storage_error)?.into()))
            .transpose()
    }
}

pub fn map_storage_error(error: StorageError) -> PyErr {
    match error {
        StorageError::Io(error) => error.into(),
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}

pub fn map_loader_error(error: LoaderError, file_path: Option<PathBuf>) -> PyErr {
    match error {
        LoaderError::Storage(error) => map_storage_error(error),
        LoaderError::Parsing(error) => map_parse_error(error, file_path),
        LoaderError::InvalidBaseIri { .. } => PyValueError::new_err(error.to_string()),
    }
}

pub fn map_serializer_error(error: SerializerError) -> PyErr {
    match error {
        SerializerError::Storage(error) => map_storage_error(error),
        SerializerError::Io(error) => error.into(),
        SerializerError::DatasetFormatExpected(_) => PyValueError::new_err(error.to_string()),
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
