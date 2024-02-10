use crate::io::*;
use crate::model::*;
use crate::store::map_storage_error;
use oxigraph::io::RdfSerializer;
use oxigraph::model::Term;
use oxigraph::sparql::results::{
    FromReadQueryResultsReader, FromReadSolutionsReader, QueryResultsFormat,
    QueryResultsParseError, QueryResultsParser, QueryResultsSerializer,
};
use oxigraph::sparql::{
    EvaluationError, Query, QueryResults, QuerySolution, QuerySolutionIter, QueryTripleIter,
    Variable,
};
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyRuntimeError, PySyntaxError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
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
#[pyclass(frozen, name = "QuerySolution", module = "pyoxigraph")]
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

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(&self, key: PySolutionKey<'_>) -> Option<PyTerm> {
        match key {
            PySolutionKey::Usize(key) => self.inner.get(key),
            PySolutionKey::Str(key) => self.inner.get(key),
            PySolutionKey::Variable(key) => self.inner.get(<&Variable>::from(&*key)),
        }
        .map(|term| PyTerm::from(term.clone()))
    }

    #[allow(clippy::unnecessary_to_owned)]
    fn __iter__(&self) -> SolutionValueIter {
        SolutionValueIter {
            inner: self.inner.values().to_vec().into_iter(),
        }
    }
}

#[derive(FromPyObject)]
pub enum PySolutionKey<'a> {
    Usize(usize),
    Str(&'a str),
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
enum PyQuerySolutionsVariant {
    Query(QuerySolutionIter),
    Reader {
        iter: FromReadSolutionsReader<PyReadable>,
        file_path: Option<PathBuf>,
    },
}

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
    /// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (:py:attr:`QueryResultsFormat.XML`)
    /// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (:py:attr:`QueryResultsFormat.JSON`)
    /// * `CSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.CSV`)
    /// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.TSV`)
    ///
    /// It supports also some media type and extension aliases.
    /// For example, ``application/json`` could also be used for `JSON <https://www.w3.org/TR/sparql11-results-json/>`_.
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
    fn serialize<'a>(
        &mut self,
        output: Option<PyWritableOutput>,
        format: Option<PyQueryResultsFormatInput>,
        py: Python<'a>,
    ) -> PyResult<Option<&'a PyBytes>> {
        PyWritable::do_write(
            |output, file_path| {
                let format = lookup_query_results_format(format, file_path.as_deref())?;
                let mut writer = QueryResultsSerializer::from_format(format)
                    .serialize_solutions_to_write(
                        output,
                        match &self.inner {
                            PyQuerySolutionsVariant::Query(inner) => inner.variables().to_vec(),
                            PyQuerySolutionsVariant::Reader { iter, .. } => {
                                iter.variables().to_vec()
                            }
                        },
                    )?;
                match &mut self.inner {
                    PyQuerySolutionsVariant::Query(inner) => {
                        for solution in inner {
                            writer.write(&solution.map_err(map_evaluation_error)?)?;
                        }
                    }
                    PyQuerySolutionsVariant::Reader { iter, file_path } => {
                        for solution in iter {
                            writer.write(&solution.map_err(|e| {
                                map_query_results_parse_error(e, file_path.clone())
                            })?)?;
                        }
                    }
                }

                Ok(writer.finish()?)
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
    /// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (:py:attr:`QueryResultsFormat.XML`)
    /// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (:py:attr:`QueryResultsFormat.JSON`)
    /// * `CSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.CSV`)
    /// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.TSV`)
    ///
    /// It supports also some media type and extension aliases.
    /// For example, ``application/json`` could also be used for `JSON <https://www.w3.org/TR/sparql11-results-json/>`_.
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
    fn serialize<'a>(
        &mut self,
        output: Option<PyWritableOutput>,
        format: Option<PyQueryResultsFormatInput>,
        py: Python<'a>,
    ) -> PyResult<Option<&'a PyBytes>> {
        PyWritable::do_write(
            |output, file_path| {
                let format = lookup_query_results_format(format, file_path.as_deref())?;
                py.allow_threads(|| {
                    Ok(QueryResultsSerializer::from_format(format)
                        .serialize_boolean_to_write(output, self.inner)?)
                })
            },
            output,
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
    /// * `canonical <https://www.w3.org/TR/n-triples/#canonical-ntriples>`_ `N-Triples <https://www.w3.org/TR/n-triples/>`_ (:py:attr:`RdfFormat.N_TRIPLES`)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (:py:attr:`RdfFormat.N_QUADS`)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (:py:attr:`RdfFormat.TURTLE`)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (:py:attr:`RdfFormat.TRIG`)
    /// * `N3 <https://w3c.github.io/N3/spec/>`_ (:py:attr:`RdfFormat.N3`)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (:py:attr:`RdfFormat.RDF_XML`)
    ///
    /// It supports also some media type and extension aliases.
    /// For example, ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` or ``xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
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
    fn serialize<'a>(
        &mut self,
        output: Option<PyWritableOutput>,
        format: Option<PyRdfFormatInput>,
        py: Python<'a>,
    ) -> PyResult<Option<&'a PyBytes>> {
        PyWritable::do_write(
            |output, file_path| {
                let format = lookup_rdf_format(format, file_path.as_deref())?;
                let mut writer = RdfSerializer::from_format(format).serialize_to_write(output);
                for triple in &mut self.inner {
                    writer.write_triple(&triple.map_err(map_evaluation_error)?)?;
                }
                Ok(writer.finish()?)
            },
            output,
            py,
        )
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
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
/// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (:py:attr:`QueryResultsFormat.XML`)
/// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (:py:attr:`QueryResultsFormat.JSON`)
/// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.TSV`)
///
/// It supports also some media type and extension aliases.
/// For example, ``application/json`` could also be used for `JSON <https://www.w3.org/TR/sparql11-results-json/>`_.
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
) -> PyResult<PyObject> {
    let input = PyReadable::from_args(&path, input, py)?;
    let format = lookup_query_results_format(format, path.as_deref())?;
    let results = QueryResultsParser::from_format(format)
        .parse_read(input)
        .map_err(|e| map_query_results_parse_error(e, path.clone()))?;
    Ok(match results {
        FromReadQueryResultsReader::Solutions(iter) => PyQuerySolutions {
            inner: PyQuerySolutionsVariant::Reader {
                iter,
                file_path: path,
            },
        }
        .into_py(py),
        FromReadQueryResultsReader::Boolean(inner) => PyQueryBoolean { inner }.into_py(py),
    })
}

/// `SPARQL query <https://www.w3.org/TR/sparql11-query/>`_ results serialization formats.
///
/// The following formats are supported:
/// * `XML <https://www.w3.org/TR/rdf-sparql-XMLres/>`_ (:py:attr:`QueryResultsFormat.XML`)
/// * `JSON <https://www.w3.org/TR/sparql11-results-json/>`_ (:py:attr:`QueryResultsFormat.JSON`)
/// * `CSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.CSV`)
/// * `TSV <https://www.w3.org/TR/sparql11-results-csv-tsv/>`_ (:py:attr:`QueryResultsFormat.TSV`)
#[pyclass(name = "QueryResultsFormat", module = "pyoxigraph")]
#[derive(Clone)]
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
    pub const fn name(&self) -> &'static str {
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

    fn __hash__(&self) -> u64 {
        hash(&self.inner)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    /// :rtype: QueryResultsFormat
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: QueryResultsFormat
    #[allow(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ PyAny) -> PyRef<'a, Self> {
        slf
    }
}

pub fn lookup_query_results_format(
    format: Option<PyQueryResultsFormatInput>,
    path: Option<&Path>,
) -> PyResult<QueryResultsFormat> {
    if let Some(format) = format {
        return match format {
            PyQueryResultsFormatInput::Object(format) => Ok(format.inner),
            PyQueryResultsFormatInput::MediaType(media_type) => {
                deprecation_warning("Using a string to specify a query results format is deprecated, please use a QueryResultsFormat object instead.")?;
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

pub fn map_evaluation_error(error: EvaluationError) -> PyErr {
    match error {
        EvaluationError::Parsing(error) => PySyntaxError::new_err(error.to_string()),
        EvaluationError::Storage(error) => map_storage_error(error),
        EvaluationError::GraphParsing(error) => map_parse_error(error, None),
        EvaluationError::ResultsParsing(error) => map_query_results_parse_error(error, None),
        EvaluationError::ResultsSerialization(error) => error.into(),
        EvaluationError::Service(error) => match error.downcast::<io::Error>() {
            Ok(error) => (*error).into(),
            Err(error) => PyRuntimeError::new_err(error.to_string()),
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
        QueryResultsParseError::Io(error) => error.into(),
    }
}
