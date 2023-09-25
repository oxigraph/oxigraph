use crate::io::*;
use crate::model::*;
use crate::store::map_storage_error;
use oxigraph::io::RdfSerializer;
use oxigraph::model::Term;
use oxigraph::sparql::results::{
    FromReadQueryResultsReader, FromReadSolutionsReader, ParseError, QueryResultsParser,
    QueryResultsSerializer,
};
use oxigraph::sparql::{
    EvaluationError, Query, QueryResults, QuerySolution, QuerySolutionIter, QueryTripleIter,
    Variable,
};
use pyo3::basic::CompareOp;
use pyo3::exceptions::{
    PyNotImplementedError, PyRuntimeError, PySyntaxError, PyTypeError, PyValueError,
};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::io::BufReader;
use std::path::PathBuf;
use std::vec::IntoIter;

pub fn parse_query(
    query: &str,
    base_iri: Option<&str>,
    use_default_graph_as_union: bool,
    default_graph: Option<&PyAny>,
    named_graphs: Option<&PyAny>,
    py: Python<'_>,
) -> PyResult<Query> {
    let mut query = allow_threads_unsafe(py, || Query::parse(query, base_iri))
        .map_err(|e| map_evaluation_error(e.into()))?;

    if use_default_graph_as_union && default_graph.is_some() {
        return Err(PyValueError::new_err(
            "The query() method use_default_graph_as_union and default_graph arguments should not be set at the same time",
        ));
    }

    if use_default_graph_as_union {
        query.dataset_mut().set_default_graph_as_union();
    }

    if let Some(default_graph) = default_graph {
        if let Ok(default_graphs) = default_graph.iter() {
            query.dataset_mut().set_default_graph(
                default_graphs
                    .map(|graph| Ok(graph?.extract::<PyGraphName>()?.into()))
                    .collect::<PyResult<_>>()?,
            )
        } else if let Ok(default_graph) = default_graph.extract::<PyGraphName>() {
            query
                .dataset_mut()
                .set_default_graph(vec![default_graph.into()]);
        } else {
            return Err(PyValueError::new_err(
                format!("The query() method default_graph argument should be a NamedNode, a BlankNode, the DefaultGraph or a not empty list of them. {} found", default_graph.get_type()
                )));
        }
    }

    if let Some(named_graphs) = named_graphs {
        query.dataset_mut().set_available_named_graphs(
            named_graphs
                .iter()?
                .map(|graph| Ok(graph?.extract::<PyNamedOrBlankNode>()?.into()))
                .collect::<PyResult<_>>()?,
        )
    }

    Ok(query)
}

pub fn query_results_to_python(py: Python<'_>, results: QueryResults) -> PyObject {
    match results {
        QueryResults::Solutions(inner) => PyQuerySolutions {
            inner: PyQuerySolutionsVariant::Query(inner),
        }
        .into_py(py),
        QueryResults::Graph(inner) => PyQueryTriples { inner }.into_py(py),
        QueryResults::Boolean(inner) => PyQueryBoolean { inner }.into_py(py),
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
#[pyclass(frozen, unsendable, name = "QuerySolution", module = "pyoxigraph")]
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

    fn __richcmp__(&self, other: &Self, op: CompareOp) -> PyResult<bool> {
        match op {
            CompareOp::Eq => Ok(self.inner == other.inner),
            CompareOp::Ne => Ok(self.inner != other.inner),
            _ => Err(PyNotImplementedError::new_err(
                "Ordering is not implemented",
            )),
        }
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(&self, input: &PyAny) -> PyResult<Option<PyTerm>> {
        if let Ok(key) = usize::extract(input) {
            Ok(self.inner.get(key).map(|term| PyTerm::from(term.clone())))
        } else if let Ok(key) = <&str>::extract(input) {
            Ok(self.inner.get(key).map(|term| PyTerm::from(term.clone())))
        } else if let Ok(key) = input.extract::<PyRef<PyVariable>>() {
            Ok(self
                .inner
                .get(<&Variable>::from(&*key))
                .map(|term| PyTerm::from(term.clone())))
        } else {
            Err(PyTypeError::new_err(format!(
                "{} is not an integer of a string",
                input.get_type().name()?,
            )))
        }
    }

    #[allow(clippy::unnecessary_to_owned)]
    fn __iter__(&self) -> SolutionValueIter {
        SolutionValueIter {
            inner: self.inner.values().to_vec().into_iter(),
        }
    }
}

#[pyclass(module = "pyoxigraph")]
pub struct SolutionValueIter {
    inner: IntoIter<Option<Term>>,
}

#[pymethods]
impl SolutionValueIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<Self> {
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
enum PyQuerySolutionsVariant {
    Query(QuerySolutionIter),
    Reader {
        iter: FromReadSolutionsReader<BufReader<PyReadable>>,
        file_path: Option<PathBuf>,
    },
}

#[pymethods]
impl PyQuerySolutions {
    /// :return: the ordered list of all variables that could appear in the query results
    /// :rtype: list(Variable)
    ///
    /// >>> store = Store()
    /// >>> store.query('SELECT ?s WHERE { ?s ?p ?o }').variables
    /// [<Variable value=s>]
    #[getter]
    fn variables(&self) -> Vec<PyVariable> {
        match &self.inner {
            PyQuerySolutionsVariant::Query(inner) => {
                inner.variables().iter().map(|v| v.clone().into()).collect()
            }
            PyQuerySolutionsVariant::Reader { iter, .. } => {
                iter.variables().iter().map(|v| v.clone().into()).collect()
            }
        }
    }

    /// Writes the query results into a file.
    ///
    /// It currently supports the following formats:
    ///
    /// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (``application/sparql-results+xml`` or ``srx``)
    /// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (``application/sparql-results+json`` or ``srj``)
    /// * `CSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (``text/csv`` or ``csv``)
    /// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (``text/tab-separated-values`` or ``tsv``)
    ///
    /// It supports also some media type and extension aliases.
    /// For example, ``application/json`` could also be used for `JSON <https://www.w3.org/TR/sparql11-results-json/>`_.
    ///
    /// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``. If :py:const:`None`, a :py:class:`bytes` buffer is returned with the serialized content.
    /// :type output: io(bytes) or str or pathlib.Path or None, optional
    /// :param format: the format of the query results serialization using a media type like ``text/csv`` or an extension like `csv`. If :py:const:`None`, the format is guessed from the file name extension.
    /// :type format: str or None, optional
    /// :rtype: bytes or None
    /// :raises ValueError: if the format is not supported.
    /// :raises OSError: if a system error happens while writing the file.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> results = store.query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }")
    /// >>> results.serialize(format="json")
    /// b'{"head":{"vars":["s","p","o"]},"results":{"bindings":[{"s":{"type":"uri","value":"http://example.com"},"p":{"type":"uri","value":"http://example.com/p"},"o":{"type":"literal","value":"1"}}]}}'
    #[pyo3(signature = (output = None, /, format = None))]
    fn serialize<'a>(
        &mut self,
        output: Option<&PyAny>,
        format: Option<&str>,
        py: Python<'a>,
    ) -> PyResult<Option<&'a PyBytes>> {
        PyWritable::do_write(
            |output, format| {
                let mut writer = QueryResultsSerializer::from_format(format)
                    .serialize_solutions_to_write(
                        output,
                        match &self.inner {
                            PyQuerySolutionsVariant::Query(inner) => inner.variables().to_vec(),
                            PyQuerySolutionsVariant::Reader { iter, .. } => {
                                iter.variables().to_vec()
                            }
                        },
                    )
                    .map_err(map_io_err)?;
                match &mut self.inner {
                    PyQuerySolutionsVariant::Query(inner) => {
                        for solution in inner {
                            writer
                                .write(&solution.map_err(map_evaluation_error)?)
                                .map_err(map_io_err)?;
                        }
                    }
                    PyQuerySolutionsVariant::Reader { iter, file_path } => {
                        for solution in iter {
                            writer
                                .write(&solution.map_err(|e| {
                                    map_query_results_parse_error(e, file_path.clone())
                                })?)
                                .map_err(map_io_err)?;
                        }
                    }
                }

                writer.finish().map_err(map_io_err)
            },
            output,
            format,
            py,
        )
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyQuerySolution>> {
        Ok(match &mut self.inner {
            PyQuerySolutionsVariant::Query(inner) => allow_threads_unsafe(py, || {
                inner.next().transpose().map_err(map_evaluation_error)
            }),
            PyQuerySolutionsVariant::Reader { iter, file_path } => iter
                .next()
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
#[pyclass(unsendable, name = "QueryBoolean", module = "pyoxigraph")]
pub struct PyQueryBoolean {
    inner: bool,
}

#[pymethods]
impl PyQueryBoolean {
    /// Writes the query results into a file.
    ///
    /// It currently supports the following formats:
    ///
    /// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (``application/sparql-results+xml`` or ``srx``)
    /// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (``application/sparql-results+json`` or ``srj``)
    /// * `CSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (``text/csv`` or ``csv``)
    /// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (``text/tab-separated-values`` or ``tsv``)
    ///
    /// It supports also some media type and extension aliases.
    /// For example, ``application/json`` could also be used for `JSON <https://www.w3.org/TR/sparql11-results-json/>`_.
    ///
    /// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``. If :py:const:`None`, a :py:class:`bytes` buffer is returned with the serialized content.
    /// :type output: io(bytes) or str or pathlib.Path or None, optional
    /// :param format: the format of the query results serialization using a media type like ``text/csv`` or an extension like `csv`. If :py:const:`None`, the format is guessed from the file name extension.
    /// :type format: str or None, optional
    /// :rtype: bytes or None
    /// :raises ValueError: if the format is not supported.
    /// :raises OSError: if a system error happens while writing the file.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> results = store.query("ASK { ?s ?p ?o }")
    /// >>> results.serialize(format="json")
    /// b'{"head":{},"boolean":true}'
    #[pyo3(signature = (output = None, /, format = None))]
    fn serialize<'a>(
        &mut self,
        output: Option<&PyAny>,
        format: Option<&str>,
        py: Python<'a>,
    ) -> PyResult<Option<&'a PyBytes>> {
        PyWritable::do_write(
            |output, format| {
                py.allow_threads(|| {
                    QueryResultsSerializer::from_format(format)
                        .serialize_boolean_to_write(output, self.inner)
                        .map_err(map_io_err)
                })
            },
            output,
            format,
            py,
        )
    }

    fn __bool__(&self) -> bool {
        self.inner
    }

    fn __richcmp__(&self, other: &Self, op: CompareOp) -> bool {
        op.matches(self.inner.cmp(&other.inner))
    }

    fn __hash__(&self) -> u64 {
        self.inner.into()
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
    inner: QueryTripleIter,
}

#[pymethods]
impl PyQueryTriples {
    /// Writes the query results into a file.
    ///
    /// It currently supports the following formats:
    ///
    /// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples`` or ``nt``)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads`` or ``nq``)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle`` or ``ttl``)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig`` or ``trig``)
    /// * `N3 <https://w3c.github.io/N3/spec/>`_ (``text/n3`` or ``n3``)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml`` or ``rdf``)
    ///
    /// It supports also some media type and extension aliases.
    /// For example, ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` or ``xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
    ///
    /// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``. If :py:const:`None`, a :py:class:`bytes` buffer is returned with the serialized content.
    /// :type output: io(bytes) or str or pathlib.Path or None, optional
    /// :param format: the format of the RDF serialization using a media type like ``text/turtle`` or an extension like `ttl`. If :py:const:`None`, the format is guessed from the file name extension.
    /// :type format: str or None, optional
    /// :rtype: bytes or None
    /// :raises ValueError: if the format is not supported.
    /// :raises OSError: if a system error happens while writing the file.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> results = store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }")
    /// >>> results.serialize(format="nt")
    /// b'<http://example.com> <http://example.com/p> "1" .\n'
    #[pyo3(signature = (output = None, /, format = None))]
    fn serialize<'a>(
        &mut self,
        output: Option<&PyAny>,
        format: Option<&str>,
        py: Python<'a>,
    ) -> PyResult<Option<&'a PyBytes>> {
        PyWritable::do_write(
            |output, format| {
                let mut writer = RdfSerializer::from_format(format).serialize_to_write(output);
                for triple in &mut self.inner {
                    writer
                        .write_triple(&triple.map_err(map_evaluation_error)?)
                        .map_err(map_io_err)?;
                }
                writer.finish().map_err(map_io_err)
            },
            output,
            format,
            py,
        )
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyTriple>> {
        Ok(allow_threads_unsafe(py, || self.inner.next())
            .transpose()
            .map_err(map_evaluation_error)?
            .map(Into::into))
    }
}

/// Parses SPARQL query results.
///
/// It currently supports the following formats:
///
/// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (``application/sparql-results+xml`` or ``srx``)
/// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (``application/sparql-results+json`` or ``srj``)
/// * `CSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (``text/csv`` or ``csv``)
/// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (``text/tab-separated-values`` or ``tsv``)
///
/// It supports also some media type and extension aliases.
/// For example, ``application/json`` could also be used for `JSON <https://www.w3.org/TR/sparql11-results-json/>`_.
///
/// :param input: The I/O object or file path to read from. For example, it could be a file path as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
/// :type input: io(bytes) or io(str) or str or pathlib.Path
/// :param format: the format of the RDF serialization using a media type like ``text/turtle`` or an extension like `ttl`. If :py:const:`None`, the format is guessed from the file name extension.
/// :type format: str or None, optional
/// :return: an iterator of :py:class:`QuerySolution` or a :py:class:`bool`.
/// :rtype: QuerySolutions or QueryBoolean
/// :raises ValueError: if the format is not supported.
/// :raises SyntaxError: if the provided data is invalid.
/// :raises OSError: if a system error happens while reading the file.
///
/// >>> input = io.BytesIO(b'?s\t?p\t?o\n<http://example.com/s>\t<http://example.com/s>\t1\n')
/// >>> list(parse_query_results(input, "text/tsv"))
/// [<QuerySolution s=<NamedNode value=http://example.com/s> p=<NamedNode value=http://example.com/s> o=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#integer>>>]
///
/// >>> input = io.BytesIO(b'{"head":{},"boolean":true}')
/// >>> parse_query_results(input, "application/sparql-results+json")
/// <QueryBoolean true>
#[pyfunction]
#[pyo3(signature = (input, /, format = None))]
pub fn parse_query_results(
    input: &PyAny,
    format: Option<&str>,
    py: Python<'_>,
) -> PyResult<PyObject> {
    let file_path = input.extract::<PathBuf>().ok();
    let format = parse_format(format, file_path.as_deref())?;
    let input = if let Some(file_path) = &file_path {
        PyReadable::from_file(file_path, py).map_err(map_io_err)?
    } else {
        PyReadable::from_data(input)
    };
    let results = QueryResultsParser::from_format(format)
        .parse_read(BufReader::new(input))
        .map_err(|e| map_query_results_parse_error(e, file_path.clone()))?;
    Ok(match results {
        FromReadQueryResultsReader::Solutions(iter) => PyQuerySolutions {
            inner: PyQuerySolutionsVariant::Reader { iter, file_path },
        }
        .into_py(py),
        FromReadQueryResultsReader::Boolean(inner) => PyQueryBoolean { inner }.into_py(py),
    })
}

pub fn map_evaluation_error(error: EvaluationError) -> PyErr {
    match error {
        EvaluationError::Parsing(error) => PySyntaxError::new_err(error.to_string()),
        EvaluationError::Storage(error) => map_storage_error(error),
        EvaluationError::GraphParsing(error) => map_parse_error(error, None),
        EvaluationError::ResultsParsing(error) => map_query_results_parse_error(error, None),
        EvaluationError::ResultsSerialization(error) => map_io_err(error),
        EvaluationError::Service(error) => match error.downcast() {
            Ok(error) => map_io_err(*error),
            Err(error) => PyRuntimeError::new_err(error.to_string()),
        },
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}

pub fn map_query_results_parse_error(error: ParseError, file_path: Option<PathBuf>) -> PyErr {
    match error {
        ParseError::Syntax(error) => {
            // Python 3.9 does not support end line and end column
            if python_version() >= (3, 10) {
                let params = if let Some(location) = error.location() {
                    (
                        file_path,
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
                        file_path,
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
        ParseError::Io(error) => map_io_err(error),
    }
}
