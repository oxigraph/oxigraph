use crate::io::*;
use crate::model::*;
use crate::store::map_storage_error;
use oxigraph::io::RdfSerializer;
use oxigraph::model::Term;
use oxigraph::sparql::results::{
    QueryResultsFormat, QueryResultsParseError, QueryResultsParser, QueryResultsSerializer,
    ReaderQueryResultsParserOutput, ReaderSolutionsParser,
};
use oxigraph::sparql::{
    AggregateFunctionAccumulator, PreparedSparqlQuery, QueryEvaluationError, QueryResults,
    QuerySolution, QuerySolutionIter, QueryTripleIter, SparqlEvaluator, UpdateEvaluationError,
    Variable,
};
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyRuntimeError, PySyntaxError, PyValueError};
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedStr;
use pyo3::types::PyTuple;
#[cfg(feature = "geosparql")]
use spargeo::register_geosparql_functions;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::vec::IntoIter;

pub fn prepare_sparql_query(
    evaluator: SparqlEvaluator,
    query: &str,
    use_default_graph_as_union: bool,
    default_graph: Option<&Bound<'_, PyAny>>,
    named_graphs: Option<&Bound<'_, PyAny>>,
) -> PyResult<PreparedSparqlQuery> {
    let mut prepared = evaluator
        .parse_query(query)
        .map_err(|e| PySyntaxError::new_err(e.to_string()))?;

    if use_default_graph_as_union && default_graph.is_some() {
        return Err(PyValueError::new_err(
            "The query() method use_default_graph_as_union and default_graph arguments should not be set at the same time",
        ));
    }

    if use_default_graph_as_union {
        prepared.dataset_mut().set_default_graph_as_union();
    }

    if let Some(default_graph) = default_graph {
        if let Ok(default_graphs) = default_graph.try_iter() {
            prepared.dataset_mut().set_default_graph(
                default_graphs
                    .map(|graph| Ok(graph?.extract::<PyGraphName>()?.into()))
                    .collect::<PyResult<_>>()?,
            )
        } else if let Ok(default_graph) = default_graph.extract::<PyGraphName>() {
            prepared
                .dataset_mut()
                .set_default_graph(vec![default_graph.into()]);
        } else {
            return Err(PyValueError::new_err(format!(
                "The query() method default_graph argument should be a NamedNode, a BlankNode, the DefaultGraph or a not empty list of them. {} found",
                default_graph.get_type()
            )));
        }
    }

    if let Some(named_graphs) = named_graphs {
        prepared.dataset_mut().set_available_named_graphs(
            named_graphs
                .try_iter()?
                .map(|graph| Ok(graph?.extract::<PyNamedOrBlankNode>()?.into()))
                .collect::<PyResult<_>>()?,
        )
    }

    Ok(prepared)
}

pub fn sparql_evaluator_from_python(
    base_iri: Option<&str>,
    prefixes: Option<HashMap<String, String>>,
    custom_functions: Option<HashMap<PyNamedNode, PyObject>>,
    custom_aggregate_functions: Option<HashMap<PyNamedNode, PyObject>>,
) -> PyResult<SparqlEvaluator> {
    let mut evaluator = SparqlEvaluator::default();
    #[cfg(feature = "geosparql")]
    {
        evaluator = register_geosparql_functions(evaluator);
    }

    if let Some(custom_functions) = custom_functions {
        for (name, function) in custom_functions {
            evaluator = evaluator.with_custom_function(name.into(), move |args| {
                Python::with_gil(|py| {
                    Some(
                        function
                            .call1(
                                py,
                                PyTuple::new(py, args.iter().map(|t| PyTerm::from(t.clone())))
                                    .ok()?,
                            )
                            .ok()?
                            .extract::<Option<PyTerm>>(py)
                            .ok()??
                            .into(),
                    )
                })
            })
        }
    }
    if let Some(custom_aggregate_functions) = custom_aggregate_functions {
        for (name, function) in custom_aggregate_functions {
            evaluator = evaluator.with_custom_aggregate_function(name.into(), move || {
                Python::with_gil(|py| {
                    Box::new(PyAggregateFunctionAccumulator {
                        inner: function.call0(py).ok(),
                    })
                })
            })
        }
    }

    if let Some(base_iri) = base_iri {
        evaluator = evaluator
            .with_base_iri(base_iri)
            .map_err(|e| PyValueError::new_err(format!("Invalid base IRI '{base_iri}': {e}")))?;
    }

    if let Some(prefixes) = prefixes {
        for (prefix_name, prefix_iri) in prefixes {
            evaluator = evaluator
                .with_prefix(&prefix_name, &prefix_iri)
                .map_err(|e| {
                    PyValueError::new_err(format!(
                        "Invalid prefix IRI '{prefix_iri}' for {prefix_name}: {e}"
                    ))
                })?;
        }
    }

    Ok(evaluator)
}

struct PyAggregateFunctionAccumulator {
    inner: Option<PyObject>,
}

impl AggregateFunctionAccumulator for PyAggregateFunctionAccumulator {
    fn accumulate(&mut self, element: Term) {
        Python::with_gil(|py| {
            self.inner = self.inner.take().and_then(|inner| {
                inner
                    .call_method1(py, "accumulate", (PyTerm::from(element),))
                    .ok()?;
                Some(inner)
            })
        })
    }

    fn finish(&mut self) -> Option<Term> {
        Python::with_gil(|py| {
            Some(
                self.inner
                    .take()?
                    .call_method0(py, "finish")
                    .ok()?
                    .extract::<PyTerm>(py)
                    .ok()?
                    .into(),
            )
        })
    }
}

pub fn query_results_to_python<'py>(
    py: Python<'py>,
    results: QueryResults<'static>,
) -> PyResult<Bound<'py, PyAny>> {
    match results {
        QueryResults::Solutions(inner) => PyQuerySolutions {
            inner: PyQuerySolutionsVariant::Query(UngilQuerySolutionIter(inner)),
        }
        .into_bound_py_any(py),
        QueryResults::Graph(inner) => PyQueryTriples {
            inner: UngilQueryTripleIter(inner),
        }
        .into_bound_py_any(py),
        QueryResults::Boolean(inner) => PyQueryBoolean { inner }.into_bound_py_any(py),
    }
}

/// Tuple associating variables and terms that are the result of a SPARQL ``SELECT`` query.
///
/// It is the equivalent of a row in SQL.
///
/// It could be indexes by variable name (:py:class:`Variable` or :py:class:`str`) or position in the tuple (:py:class:`int`).
/// Unpacking also works.
///
/// >>> store = Store()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// >>> solution = next(store.query('SELECT ?s ?p ?o WHERE { ?s ?p ?o }'))
/// >>> solution[Variable('s')]
/// <NamedNode value=http://example.com>
/// >>> solution['s']
/// <NamedNode value=http://example.com>
/// >>> solution[0]
/// <NamedNode value=http://example.com>
/// >>> s, p, o = solution
/// >>> s
/// <NamedNode value=http://example.com>
#[pyclass(frozen, name = "QuerySolution", module = "pyoxigraph", eq)]
#[derive(Eq, PartialEq)]
pub struct PyQuerySolution {
    inner: QuerySolution,
}

#[pymethods]
impl PyQuerySolution {
    fn __repr__(&self) -> String {
        let mut buffer = String::new();
        buffer.push_str("<QuerySolution");
        for (k, v) in self.inner.iter() {
            buffer.push(' ');
            buffer.push_str(k.as_str());
            buffer.push('=');
            term_repr(v.as_ref(), &mut buffer)
        }
        buffer.push('>');
        buffer
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(&self, key: PySolutionKey<'_>) -> Option<PyTerm> {
        match key {
            PySolutionKey::Usize(key) => self.inner.get(key),
            PySolutionKey::Str(key) => {
                let k: &str = &key;
                self.inner.get(k)
            }
            PySolutionKey::Variable(key) => self.inner.get(<&Variable>::from(&*key)),
        }
        .map(|term| PyTerm::from(term.clone()))
    }

    fn __iter__(&self) -> SolutionValueIter {
        SolutionValueIter {
            inner: self.inner.values().to_vec().into_iter(),
        }
    }

    /// :rtype: QuerySolution
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: QuerySolution
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }
}

#[derive(FromPyObject)]
pub enum PySolutionKey<'a> {
    Usize(usize),
    Str(PyBackedStr),
    Variable(PyRef<'a, PyVariable>),
}

#[pyclass(module = "pyoxigraph")]
pub struct SolutionValueIter {
    inner: IntoIter<Option<Term>>,
}

#[pymethods]
impl SolutionValueIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<Option<PyTerm>> {
        self.inner.next().map(|v| v.map(PyTerm::from))
    }
}

/// An iterator of :py:class:`QuerySolution` returned by a SPARQL ``SELECT`` query
///
/// >>> store = Store()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// >>> list(store.query('SELECT ?s WHERE { ?s ?p ?o }'))
/// [<QuerySolution s=<NamedNode value=http://example.com>>]
#[pyclass(unsendable, name = "QuerySolutions", module = "pyoxigraph")]
pub struct PyQuerySolutions {
    inner: PyQuerySolutionsVariant,
}

#[allow(clippy::large_enum_variant, clippy::allow_attributes)]
enum PyQuerySolutionsVariant {
    Query(UngilQuerySolutionIter),
    Reader {
        iter: ReaderSolutionsParser<PyReadable>,
        file_path: Option<PathBuf>,
    },
}

struct UngilQuerySolutionIter(QuerySolutionIter<'static>);

#[expect(unsafe_code)]
// SAFETY: To derive Ungil
unsafe impl Send for UngilQuerySolutionIter {}

#[pymethods]
impl PyQuerySolutions {
    /// :return: the ordered list of all variables that could appear in the query results
    /// :rtype: list[Variable]
    ///
    /// >>> store = Store()
    /// >>> store.query('SELECT ?s WHERE { ?s ?p ?o }').variables
    /// [<Variable value=s>]
    #[getter]
    fn variables(&self) -> Vec<PyVariable> {
        match &self.inner {
            PyQuerySolutionsVariant::Query(inner) => inner
                .0
                .variables()
                .iter()
                .map(|v| v.clone().into())
                .collect(),
            PyQuerySolutionsVariant::Reader { iter, .. } => {
                iter.variables().iter().map(|v| v.clone().into()).collect()
            }
        }
    }

    /// Writes the query results into a file.
    ///
    /// It currently supports the following formats:
    ///
    /// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (:py:attr:`QueryResultsFormat.XML`)
    /// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (:py:attr:`QueryResultsFormat.JSON`)
    /// * `CSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.CSV`)
    /// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.TSV`)
    ///
    /// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``. If :py:const:`None`, a :py:class:`bytes` buffer is returned with the serialized content.
    /// :type output: typing.IO[bytes] or str or os.PathLike[str] or None, optional
    /// :param format: the format of the query results serialization. If :py:const:`None`, the format is guessed from the file name extension.
    /// :type format: QueryResultsFormat or None, optional
    /// :rtype: bytes or None
    /// :raises ValueError: if the format is not supported.
    /// :raises OSError: if a system error happens while writing the file.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> results = store.query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }")
    /// >>> results.serialize(format=QueryResultsFormat.JSON)
    /// b'{"head":{"vars":["s","p","o"]},"results":{"bindings":[{"s":{"type":"uri","value":"http://example.com"},"p":{"type":"uri","value":"http://example.com/p"},"o":{"type":"literal","value":"1"}}]}}'
    #[pyo3(signature = (output = None, format = None))]
    fn serialize(
        &mut self,
        output: Option<PyWritableOutput>,
        format: Option<PyQueryResultsFormatInput>,
        py: Python<'_>,
    ) -> PyResult<Option<Vec<u8>>> {
        PyWritable::do_write(
            |output, file_path| {
                let format = lookup_query_results_format(format, file_path.as_deref())?;
                py.allow_threads(|| {
                    let mut serializer = QueryResultsSerializer::from_format(format)
                        .serialize_solutions_to_writer(
                            output,
                            match &self.inner {
                                PyQuerySolutionsVariant::Query(inner) => {
                                    inner.0.variables().to_vec()
                                }
                                PyQuerySolutionsVariant::Reader { iter, .. } => {
                                    iter.variables().to_vec()
                                }
                            },
                        )?;
                    match &mut self.inner {
                        PyQuerySolutionsVariant::Query(inner) => {
                            for solution in &mut inner.0 {
                                serializer.serialize(&solution.map_err(map_evaluation_error)?)?;
                            }
                        }
                        PyQuerySolutionsVariant::Reader { iter, file_path } => {
                            for solution in iter {
                                serializer.serialize(&solution.map_err(|e| {
                                    map_query_results_parse_error(e, file_path.clone())
                                })?)?;
                            }
                        }
                    }

                    Ok(serializer.finish()?)
                })
            },
            output,
            py,
        )
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyQuerySolution>> {
        Ok(match &mut self.inner {
            PyQuerySolutionsVariant::Query(inner) => py
                .allow_threads(move || inner.0.next())
                .transpose()
                .map_err(map_evaluation_error),
            PyQuerySolutionsVariant::Reader { iter, file_path } => py
                .allow_threads(|| iter.next())
                .transpose()
                .map_err(|e| map_query_results_parse_error(e, file_path.clone())),
        }?
        .map(move |inner| PyQuerySolution { inner }))
    }
}

/// A boolean returned by a SPARQL ``ASK`` query.
///
/// It can be easily casted to a regular boolean using the :py:func:`bool` function.
///
/// >>> store = Store()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// >>> bool(store.query('ASK { ?s ?p ?o }'))
/// True
#[pyclass(frozen, name = "QueryBoolean", module = "pyoxigraph", eq, ord, hash)]
#[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Clone, Copy)]
pub struct PyQueryBoolean {
    inner: bool,
}

#[pymethods]
impl PyQueryBoolean {
    /// Writes the query results into a file.
    ///
    /// It currently supports the following formats:
    ///
    /// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (:py:attr:`QueryResultsFormat.XML`)
    /// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (:py:attr:`QueryResultsFormat.JSON`)
    /// * `CSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.CSV`)
    /// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.TSV`)
    ///
    /// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``. If :py:const:`None`, a :py:class:`bytes` buffer is returned with the serialized content.
    /// :type output: typing.IO[bytes] or str or os.PathLike[str] or None, optional
    /// :param format: the format of the query results serialization. If :py:const:`None`, the format is guessed from the file name extension.
    /// :type format: QueryResultsFormat or None, optional
    /// :rtype: bytes or None
    /// :raises ValueError: if the format is not supported.
    /// :raises OSError: if a system error happens while writing the file.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> results = store.query("ASK { ?s ?p ?o }")
    /// >>> results.serialize(format=QueryResultsFormat.JSON)
    /// b'{"head":{},"boolean":true}'
    #[pyo3(signature = (output = None, format = None))]
    fn serialize(
        &self,
        output: Option<PyWritableOutput>,
        format: Option<PyQueryResultsFormatInput>,
        py: Python<'_>,
    ) -> PyResult<Option<Vec<u8>>> {
        PyWritable::do_write(
            |output, file_path| {
                let format = lookup_query_results_format(format, file_path.as_deref())?;
                py.allow_threads(|| {
                    Ok(QueryResultsSerializer::from_format(format)
                        .serialize_boolean_to_writer(output, self.inner)?)
                })
            },
            output,
            py,
        )
    }

    fn __bool__(&self) -> bool {
        self.inner
    }

    fn __repr__(&self) -> String {
        format!("<QueryBoolean {}>", self.inner)
    }
}

/// An iterator of :py:class:`Triple` returned by a SPARQL ``CONSTRUCT`` or ``DESCRIBE`` query
///
/// >>> store = Store()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// >>> list(store.query('CONSTRUCT WHERE { ?s ?p ?o }'))
/// [<Triple subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]
#[pyclass(unsendable, name = "QueryTriples", module = "pyoxigraph")]
pub struct PyQueryTriples {
    inner: UngilQueryTripleIter,
}

struct UngilQueryTripleIter(QueryTripleIter<'static>);

#[expect(unsafe_code)]
// SAFETY: To derive Ungil
unsafe impl Send for UngilQueryTripleIter {}

#[pymethods]
impl PyQueryTriples {
    /// Writes the query results into a file.
    ///
    /// It currently supports the following formats:
    ///
    /// * `JSON-LD 1.0 <https://www.w3.org/TR/json-ld/>`_ (:py:attr:`RdfFormat.JSON_LD`)
    /// * `canonical <https://www.w3.org/TR/n-triples/#canonical-ntriples>`_ `N-Triples <https://www.w3.org/TR/n-triples/>`_ (:py:attr:`RdfFormat.N_TRIPLES`)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (:py:attr:`RdfFormat.N_QUADS`)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (:py:attr:`RdfFormat.TURTLE`)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (:py:attr:`RdfFormat.TRIG`)
    /// * `N3 <https://w3c.github.io/N3/spec/>`_ (:py:attr:`RdfFormat.N3`)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (:py:attr:`RdfFormat.RDF_XML`)
    ///
    /// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``. If :py:const:`None`, a :py:class:`bytes` buffer is returned with the serialized content.
    /// :type output: typing.IO[bytes] or str or os.PathLike[str] or None, optional
    /// :param format: the format of the RDF serialization. If :py:const:`None`, the format is guessed from the file name extension.
    /// :type format: RdfFormat or None, optional
    /// :rtype: bytes or None
    /// :raises ValueError: if the format is not supported.
    /// :raises OSError: if a system error happens while writing the file.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> results = store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }")
    /// >>> results.serialize(format=RdfFormat.N_TRIPLES)
    /// b'<http://example.com> <http://example.com/p> "1" .\n'
    #[pyo3(signature = (output = None, format = None))]
    fn serialize(
        &mut self,
        output: Option<PyWritableOutput>,
        format: Option<PyRdfFormatInput>,
        py: Python<'_>,
    ) -> PyResult<Option<Vec<u8>>> {
        PyWritable::do_write(
            |output, file_path| {
                let format = lookup_rdf_format(format, file_path.as_deref())?;
                py.allow_threads(move || {
                    let mut serializer = RdfSerializer::from_format(format).for_writer(output);
                    for triple in &mut self.inner.0 {
                        serializer.serialize_triple(&triple.map_err(map_evaluation_error)?)?;
                    }
                    Ok(serializer.finish()?)
                })
            },
            output,
            py,
        )
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyTriple>> {
        Ok(py
            .allow_threads(move || self.inner.0.next())
            .transpose()
            .map_err(map_evaluation_error)?
            .map(Into::into))
    }
}

/// Parses SPARQL query results.
///
/// It currently supports the following formats:
///
/// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (:py:attr:`QueryResultsFormat.XML`)
/// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (:py:attr:`QueryResultsFormat.JSON`)
/// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.TSV`)
///
/// :param input: The :py:class:`str`, :py:class:`bytes` or I/O object to read from. For example, it could be the file content as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
/// :type input: bytes or str or typing.IO[bytes] or typing.IO[str] or None, optional
/// :param format: the format of the query results serialization. If :py:const:`None`, the format is guessed from the file name extension.
/// :type format: QueryResultsFormat or None, optional
/// :param path: The file path to read from. Replaces the ``input`` parameter.
/// :type path: str or os.PathLike[str] or None, optional
/// :return: an iterator of :py:class:`QuerySolution` or a :py:class:`bool`.
/// :rtype: QuerySolutions or QueryBoolean
/// :raises ValueError: if the format is not supported.
/// :raises SyntaxError: if the provided data is invalid.
/// :raises OSError: if a system error happens while reading the file.
///
/// >>> list(parse_query_results('?s\t?p\t?o\n<http://example.com/s>\t<http://example.com/s>\t1\n', QueryResultsFormat.TSV))
/// [<QuerySolution s=<NamedNode value=http://example.com/s> p=<NamedNode value=http://example.com/s> o=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#integer>>>]
///
/// >>> parse_query_results('{"head":{},"boolean":true}', QueryResultsFormat.JSON)
/// <QueryBoolean true>
#[pyfunction]
#[pyo3(signature = (input = None, format = None, *, path = None))]
pub fn parse_query_results(
    input: Option<PyReadableInput>,
    format: Option<PyQueryResultsFormatInput>,
    path: Option<PathBuf>,
    py: Python<'_>,
) -> PyResult<Bound<'_, PyAny>> {
    let input = PyReadable::from_args(&path, input, py)?;
    let format = lookup_query_results_format(format, path.as_deref())?;
    let results = QueryResultsParser::from_format(format)
        .for_reader(input)
        .map_err(|e| map_query_results_parse_error(e, path.clone()))?;
    match results {
        ReaderQueryResultsParserOutput::Solutions(iter) => PyQuerySolutions {
            inner: PyQuerySolutionsVariant::Reader {
                iter,
                file_path: path,
            },
        }
        .into_bound_py_any(py),
        ReaderQueryResultsParserOutput::Boolean(inner) => {
            PyQueryBoolean { inner }.into_bound_py_any(py)
        }
    }
}

/// `SPARQL query <https://www.w3.org/TR/sparql11-query/>`_ results serialization formats.
///
/// The following formats are supported:
///
/// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (:py:attr:`QueryResultsFormat.XML`)
/// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (:py:attr:`QueryResultsFormat.JSON`)
/// * `CSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.CSV`)
/// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.TSV`)
#[pyclass(frozen, name = "QueryResultsFormat", module = "pyoxigraph", eq, hash)]
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct PyQueryResultsFormat {
    inner: QueryResultsFormat,
}

#[pymethods]
impl PyQueryResultsFormat {
    /// `SPARQL Query Results CSV Format <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_
    #[classattr]
    const CSV: Self = Self {
        inner: QueryResultsFormat::Csv,
    };
    /// `SPARQL Query Results JSON Format <https://www.w3.org/TR/sparql11-results-json/>`_
    #[classattr]
    const JSON: Self = Self {
        inner: QueryResultsFormat::Json,
    };
    /// `SPARQL Query Results TSV Format <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_
    #[classattr]
    const TSV: Self = Self {
        inner: QueryResultsFormat::Tsv,
    };
    /// `SPARQL Query Results XML Format <https://www.w3.org/TR/rdf-sparql-XMLres/>`_
    #[classattr]
    const XML: Self = Self {
        inner: QueryResultsFormat::Xml,
    };

    /// :return: the format canonical IRI according to the `Unique URIs for file formats registry <https://www.w3.org/ns/formats/>`_.
    /// :rtype: str
    ///
    /// >>> QueryResultsFormat.JSON.iri
    /// 'http://www.w3.org/ns/formats/SPARQL_Results_JSON'
    #[getter]
    fn iri(&self) -> &'static str {
        self.inner.iri()
    }

    /// :return: the format `IANA media type <https://tools.ietf.org/html/rfc2046>`_.
    /// :rtype: str
    ///
    /// >>> QueryResultsFormat.JSON.media_type
    /// 'application/sparql-results+json'
    #[getter]
    fn media_type(&self) -> &'static str {
        self.inner.media_type()
    }

    /// :return: the format `IANA-registered <https://tools.ietf.org/html/rfc2046>`_ file extension.
    /// :rtype: str
    ///
    /// >>> QueryResultsFormat.JSON.file_extension
    /// 'srj'
    #[getter]
    fn file_extension(&self) -> &'static str {
        self.inner.file_extension()
    }

    /// :return: the format name.
    /// :rtype: str
    ///
    /// >>> QueryResultsFormat.JSON.name
    /// 'SPARQL Results in JSON'
    #[getter]
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    /// Looks for a known format from a media type.
    ///
    /// It supports some media type aliases.
    /// For example, "application/xml" is going to return :py:const:`QueryResultsFormat.XML` even if it is not its canonical media type.
    ///
    /// :param media_type: the media type.
    /// :type media_type: str
    /// :return: :py:class:`QueryResultsFormat` if the media type is known or :py:const:`None` if not.
    /// :rtype: QueryResultsFormat or None
    ///
    /// >>> QueryResultsFormat.from_media_type("application/sparql-results+json; charset=utf-8")
    /// <QueryResultsFormat SPARQL Results in JSON>
    #[staticmethod]
    fn from_media_type(media_type: &str) -> Option<Self> {
        Some(Self {
            inner: QueryResultsFormat::from_media_type(media_type)?,
        })
    }

    /// Looks for a known format from an extension.
    ///
    /// It supports some aliases.
    ///
    /// :param extension: the extension.
    /// :type extension: str
    /// :return: :py:class:`QueryResultsFormat` if the extension is known or :py:const:`None` if not.
    /// :rtype: QueryResultsFormat or None
    ///
    /// >>> QueryResultsFormat.from_extension("json")
    /// <QueryResultsFormat SPARQL Results in JSON>
    #[staticmethod]
    fn from_extension(extension: &str) -> Option<Self> {
        Some(Self {
            inner: QueryResultsFormat::from_extension(extension)?,
        })
    }

    fn __str__(&self) -> &'static str {
        self.inner.name()
    }

    fn __repr__(&self) -> String {
        format!("<QueryResultsFormat {}>", self.inner.name())
    }

    /// :rtype: QueryResultsFormat
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: QueryResultsFormat
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }
}

fn lookup_query_results_format(
    format: Option<PyQueryResultsFormatInput>,
    path: Option<&Path>,
) -> PyResult<QueryResultsFormat> {
    if let Some(format) = format {
        return match format {
            PyQueryResultsFormatInput::Object(format) => Ok(format.inner),
            PyQueryResultsFormatInput::MediaType(media_type) => {
                deprecation_warning(
                    "Using a string to specify a query results format is deprecated, please use a QueryResultsFormat object instead.",
                )?;
                QueryResultsFormat::from_media_type(&media_type).ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "The media type {media_type} is not supported by pyoxigraph"
                    ))
                })
            }
        };
    }
    let Some(path) = path else {
        return Err(PyValueError::new_err(
            "The format parameter is required when a file path is not given",
        ));
    };
    let Some(ext) = path.extension().and_then(OsStr::to_str) else {
        return Err(PyValueError::new_err(format!(
            "The file name {} has no extension to guess a file format from",
            path.display()
        )));
    };
    QueryResultsFormat::from_extension(ext)
        .ok_or_else(|| PyValueError::new_err(format!("Not supported RDF format extension: {ext}")))
}

#[derive(FromPyObject)]
pub enum PyQueryResultsFormatInput {
    Object(PyQueryResultsFormat),
    MediaType(String),
}

pub fn map_evaluation_error(error: QueryEvaluationError) -> PyErr {
    match error {
        QueryEvaluationError::Dataset(error) => match error.downcast() {
            Ok(error) => map_storage_error(*error),
            Err(error) => io_or_runtime_error(error),
        },
        QueryEvaluationError::Service(error) => io_or_runtime_error(error),
        QueryEvaluationError::Unexpected(error) => match error.downcast() {
            Ok(error) => map_parse_error(*error, None),
            Err(error) => match error.downcast() {
                Ok(error) => map_query_results_parse_error(*error, None),
                Err(error) => io_or_runtime_error(error),
            },
        },
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}

fn io_or_runtime_error(error: Box<dyn Error>) -> PyErr {
    match error.downcast::<io::Error>() {
        Ok(error) => (*error).into(),
        Err(error) => PyRuntimeError::new_err(error.to_string()),
    }
}

pub fn map_update_evaluation_error(error: UpdateEvaluationError) -> PyErr {
    match error {
        UpdateEvaluationError::Storage(error) => map_storage_error(error),
        UpdateEvaluationError::GraphParsing(error) => map_parse_error(error, None),
        UpdateEvaluationError::Service(error) => match error.downcast::<io::Error>() {
            Ok(error) => (*error).into(),
            Err(error) => PyRuntimeError::new_err(error.to_string()),
        },
        UpdateEvaluationError::Unexpected(error) => match error.downcast() {
            Ok(error) => map_parse_error(*error, None),
            Err(error) => match error.downcast() {
                Ok(error) => map_query_results_parse_error(*error, None),
                Err(error) => io_or_runtime_error(error),
            },
        },
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}

pub fn map_query_results_parse_error(
    error: QueryResultsParseError,
    file_path: Option<PathBuf>,
) -> PyErr {
    match error {
        QueryResultsParseError::Syntax(error) => {
            // Python 3.9 does not support end line and end column
            if python_version() >= (3, 10) {
                let params = if let Some(location) = error.location() {
                    (
                        file_path.map(PathBuf::into_os_string),
                        Some(location.start.line + 1),
                        Some(location.start.column + 1),
                        None::<Vec<u8>>,
                        Some(location.end.line + 1),
                        Some(location.end.column + 1),
                    )
                } else {
                    (None, None, None, None, None, None)
                };
                PySyntaxError::new_err((error.to_string(), params))
            } else {
                let params = if let Some(location) = error.location() {
                    (
                        file_path.map(PathBuf::into_os_string),
                        Some(location.start.line + 1),
                        Some(location.start.column + 1),
                        None::<Vec<u8>>,
                    )
                } else {
                    (None, None, None, None)
                };
                PySyntaxError::new_err((error.to_string(), params))
            }
        }
        QueryResultsParseError::Io(error) => error.into(),
    }
}
