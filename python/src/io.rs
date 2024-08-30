#![allow(clippy::needless_option_as_deref)]

use crate::model::{PyQuad, PyTriple};
use oxigraph::io::{RdfFormat, RdfParseError, RdfParser, RdfSerializer, ReaderQuadParser};
use oxigraph::model::QuadRef;
use pyo3::exceptions::{PyDeprecationWarning, PySyntaxError, PyValueError};
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::pybacked::{PyBackedBytes, PyBackedStr};
use pyo3::types::PyBytes;
use std::cmp::max;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Parses RDF graph and dataset serialization formats.
///
/// It currently supports the following formats:
///
/// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (:py:attr:`RdfFormat.N_TRIPLES`)
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
/// :param input: The :py:class:`str`, :py:class:`bytes` or I/O object to read from. For example, it could be the file content as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
/// :type input: bytes or str or typing.IO[bytes] or typing.IO[str] or None, optional
/// :param format: the format of the RDF serialization. If :py:const:`None`, the format is guessed from the file name extension.
/// :type format: RdfFormat or None, optional
/// :param path: The file path to read from. Replaces the ``input`` parameter.
/// :type path: str or os.PathLike[str] or None, optional
/// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
/// :type base_iri: str or None, optional
/// :param without_named_graphs: Sets that the parser must fail when parsing a named graph.
/// :type without_named_graphs: bool, optional
/// :param rename_blank_nodes: Renames the blank nodes identifiers from the ones set in the serialization to random ids. This allows to avoid identifier conflicts when merging graphs together.
/// :type rename_blank_nodes: bool, optional
/// :return: an iterator of RDF triples or quads depending on the format.
/// :rtype: collections.abc.Iterator[Quad]
/// :raises ValueError: if the format is not supported.
/// :raises SyntaxError: if the provided data is invalid.
/// :raises OSError: if a system error happens while reading the file.
///
/// >>> list(parse(input=b'<foo> <p> "1" .', format=RdfFormat.TURTLE, base_iri="http://example.com/"))
/// [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<DefaultGraph>>]
#[pyfunction]
#[pyo3(signature = (input = None, format = None, *, path = None, base_iri = None, without_named_graphs = false, rename_blank_nodes = false))]
pub fn parse(
    input: Option<PyReadableInput>,
    format: Option<PyRdfFormatInput>,
    path: Option<PathBuf>,
    base_iri: Option<&str>,
    without_named_graphs: bool,
    rename_blank_nodes: bool,
    py: Python<'_>,
) -> PyResult<PyObject> {
    let input = PyReadable::from_args(&path, input, py)?;
    let format = lookup_rdf_format(format, path.as_deref())?;
    let mut parser = RdfParser::from_format(format);
    if let Some(base_iri) = base_iri {
        parser = parser
            .with_base_iri(base_iri)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
    }
    if without_named_graphs {
        parser = parser.without_named_graphs();
    }
    if rename_blank_nodes {
        parser = parser.rename_blank_nodes();
    }
    Ok(PyQuadReader {
        inner: parser.for_reader(input),
        file_path: path,
    }
    .into_py(py))
}

/// Serializes an RDF graph or dataset.
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
/// :param input: the RDF triples and quads to serialize.
/// :type input: collections.abc.Iterable[Triple] or collections.abc.Iterable[Quad]
/// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``. If :py:const:`None`, a :py:class:`bytes` buffer is returned with the serialized content.
/// :type output: typing.IO[bytes] or str or os.PathLike[str] or None, optional
/// :param format: the format of the RDF serialization. If :py:const:`None`, the format is guessed from the file name extension.
/// :type format: RdfFormat or None, optional
/// :return: :py:class:`bytes` with the serialization if the ``output`` parameter is :py:const:`None`, :py:const:`None` if ``output`` is set.
/// :rtype: bytes or None
/// :raises ValueError: if the format is not supported.
/// :raises TypeError: if a triple is given during a quad format serialization or reverse.
/// :raises OSError: if a system error happens while writing the file.
///
/// >>> serialize([Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))], format=RdfFormat.TURTLE)
/// b'<http://example.com> <http://example.com/p> "1" .\n'
///
/// >>> import io
/// >>> output = io.BytesIO()
/// >>> serialize([Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))], output, RdfFormat.TURTLE)
/// >>> output.getvalue()
/// b'<http://example.com> <http://example.com/p> "1" .\n'
#[pyfunction]
#[pyo3(signature = (input, output = None, format = None))]
pub fn serialize<'py>(
    input: &Bound<'py, PyAny>,
    output: Option<PyWritableOutput>,
    format: Option<PyRdfFormatInput>,
    py: Python<'py>,
) -> PyResult<Option<Bound<'py, PyBytes>>> {
    PyWritable::do_write(
        |output, file_path| {
            let format = lookup_rdf_format(format, file_path.as_deref())?;
            let mut serializer = RdfSerializer::from_format(format).for_writer(output);
            for i in input.iter()? {
                let i = i?;
                if let Ok(triple) = i.extract::<PyRef<'_, PyTriple>>() {
                    serializer.serialize_triple(&*triple)
                } else {
                    let quad = i.extract::<PyRef<'_, PyQuad>>()?;
                    let quad = QuadRef::from(&*quad);
                    if !quad.graph_name.is_default_graph() && !format.supports_datasets() {
                        return Err(PyValueError::new_err(
                            "The {format} format does not support named graphs",
                        ));
                    }
                    serializer.serialize_quad(quad)
                }?;
            }
            Ok(serializer.finish()?)
        },
        output,
        py,
    )
}

#[pyclass(name = "QuadReader", module = "pyoxigraph")]
pub struct PyQuadReader {
    inner: ReaderQuadParser<PyReadable>,
    file_path: Option<PathBuf>,
}

#[pymethods]
impl PyQuadReader {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyQuad>> {
        py.allow_threads(|| {
            Ok(self
                .inner
                .next()
                .transpose()
                .map_err(|e| map_parse_error(e, self.file_path.clone()))?
                .map(PyQuad::from))
        })
    }
}

/// RDF serialization formats.
///
/// The following formats are supported:
///
/// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (:py:attr:`RdfFormat.N_TRIPLES`)
/// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (:py:attr:`RdfFormat.N_QUADS`)
/// * `Turtle <https://www.w3.org/TR/turtle/>`_ (:py:attr:`RdfFormat.TURTLE`)
/// * `TriG <https://www.w3.org/TR/trig/>`_ (:py:attr:`RdfFormat.TRIG`)
/// * `N3 <https://w3c.github.io/N3/spec/>`_ (:py:attr:`RdfFormat.N3`)
/// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (:py:attr:`RdfFormat.RDF_XML`)
///
/// >>> RdfFormat.N3.media_type
/// 'text/n3'
#[pyclass(frozen, name = "RdfFormat", module = "pyoxigraph", eq, hash)]
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct PyRdfFormat {
    inner: RdfFormat,
}

#[pymethods]
impl PyRdfFormat {
    /// `N3 <https://w3c.github.io/N3/spec/>`_
    #[classattr]
    const N3: Self = Self {
        inner: RdfFormat::N3,
    };
    /// `N-Quads <https://www.w3.org/TR/n-quads/>`_
    #[classattr]
    const N_QUADS: Self = Self {
        inner: RdfFormat::NQuads,
    };
    /// `N-Triples <https://www.w3.org/TR/n-triples/>`_
    #[classattr]
    const N_TRIPLES: Self = Self {
        inner: RdfFormat::NTriples,
    };
    /// `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_
    #[classattr]
    const RDF_XML: Self = Self {
        inner: RdfFormat::RdfXml,
    };
    /// `TriG <https://www.w3.org/TR/trig/>`_
    #[classattr]
    const TRIG: Self = Self {
        inner: RdfFormat::TriG,
    };
    /// `Turtle <https://www.w3.org/TR/turtle/>`_
    #[classattr]
    const TURTLE: Self = Self {
        inner: RdfFormat::Turtle,
    };

    /// :return: the format canonical IRI according to the `Unique URIs for file formats registry <https://www.w3.org/ns/formats/>`_.
    /// :rtype: str
    ///
    /// >>> RdfFormat.N_TRIPLES.iri
    /// 'http://www.w3.org/ns/formats/N-Triples'
    #[getter]
    fn iri(&self) -> &'static str {
        self.inner.iri()
    }

    /// :return: the format `IANA media type <https://tools.ietf.org/html/rfc2046>`_.
    /// :rtype: str
    ///
    /// >>> RdfFormat.N_TRIPLES.media_type
    /// 'application/n-triples'
    #[getter]
    fn media_type(&self) -> &'static str {
        self.inner.media_type()
    }

    /// :return: the format `IANA-registered <https://tools.ietf.org/html/rfc2046>`_ file extension.
    /// :rtype: str
    ///
    /// >>> RdfFormat.N_TRIPLES.file_extension
    /// 'nt'
    #[getter]
    fn file_extension(&self) -> &'static str {
        self.inner.file_extension()
    }

    /// :return: the format name.
    /// :rtype: str
    ///
    /// >>> RdfFormat.N_TRIPLES.name
    /// 'N-Triples'
    #[getter]
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    /// :return: if the formats supports `RDF datasets <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_ and not only `RDF graphs <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph>`_.
    /// :rtype: bool
    ///
    /// >>> RdfFormat.N_TRIPLES.supports_datasets
    /// False
    /// >>> RdfFormat.N_QUADS.supports_datasets
    /// True
    #[getter]
    fn supports_datasets(&self) -> bool {
        self.inner.supports_datasets()
    }

    /// :return: if the formats supports `RDF-star quoted triples <https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#dfn-quoted>`_.
    /// :rtype: bool
    ///
    /// >>> RdfFormat.N_TRIPLES.supports_rdf_star
    /// True
    /// >>> RdfFormat.RDF_XML.supports_rdf_star
    /// False
    #[getter]
    fn supports_rdf_star(&self) -> bool {
        self.inner.supports_rdf_star()
    }

    /// Looks for a known format from a media type.
    ///
    /// It supports some media type aliases.
    /// For example, "application/xml" is going to return RDF/XML even if it is not its canonical media type.
    ///
    /// :param media_type: the media type.
    /// :type media_type: str
    /// :return: :py:class:`RdfFormat` if the media type is known or :py:const:`None` if not.
    /// :rtype: RdfFormat or None
    ///
    /// >>> RdfFormat.from_media_type("text/turtle; charset=utf-8")
    /// <RdfFormat Turtle>
    #[staticmethod]
    fn from_media_type(media_type: &str) -> Option<Self> {
        Some(Self {
            inner: RdfFormat::from_media_type(media_type)?,
        })
    }

    /// Looks for a known format from an extension.
    ///
    /// It supports some aliases.
    ///
    /// :param extension: the extension.
    /// :type extension: str
    /// :return: :py:class:`RdfFormat` if the extension is known or :py:const:`None` if not.
    /// :rtype: RdfFormat or None
    ///
    /// >>> RdfFormat.from_extension("nt")
    /// <RdfFormat N-Triples>
    #[staticmethod]
    fn from_extension(extension: &str) -> Option<Self> {
        Some(Self {
            inner: RdfFormat::from_extension(extension)?,
        })
    }

    fn __str__(&self) -> &'static str {
        self.inner.name()
    }

    fn __repr__(&self) -> String {
        format!("<RdfFormat {}>", self.inner.name())
    }

    /// :rtype: RdfFormat
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: RdfFormat
    #[allow(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }
}

pub enum PyReadable {
    String(Cursor<PyBackedStr>),
    Bytes(Cursor<PyBackedBytes>),
    Io(PyIo),
    File(File),
}

impl PyReadable {
    pub fn from_args(
        path: &Option<PathBuf>,
        input: Option<PyReadableInput>,
        py: Python<'_>,
    ) -> PyResult<Self> {
        match (path, input) {
            (Some(_), Some(_)) => Err(PyValueError::new_err(
                "input and file_path can't be both set at the same time",
            )),
            (Some(path), None) => Ok(Self::File(py.allow_threads(|| File::open(path))?)),
            (None, Some(input)) => Ok(input.into()),
            (None, None) => Err(PyValueError::new_err(
                "Either input or file_path must be set",
            )),
        }
    }
}

impl Read for PyReadable {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::String(str) => str.read(buf),
            Self::Bytes(bytes) => bytes.read(buf),
            Self::Io(io) => io.read(buf),
            Self::File(file) => file.read(buf),
        }
    }
}

#[derive(FromPyObject)]
pub enum PyReadableInput {
    String(PyBackedStr),
    Bytes(PyBackedBytes),
    Io(PyObject),
}

impl From<PyReadableInput> for PyReadable {
    fn from(input: PyReadableInput) -> Self {
        match input {
            PyReadableInput::String(string) => Self::String(Cursor::new(string)),
            PyReadableInput::Bytes(bytes) => Self::Bytes(Cursor::new(bytes)),
            PyReadableInput::Io(io) => Self::Io(PyIo(io)),
        }
    }
}

pub enum PyWritable {
    Bytes(Vec<u8>),
    Io(PyIo),
    File(File),
}

impl PyWritable {
    pub fn do_write(
        write: impl FnOnce(BufWriter<Self>, Option<PathBuf>) -> PyResult<BufWriter<Self>>,
        output: Option<PyWritableOutput>,
        py: Python<'_>,
    ) -> PyResult<Option<Bound<'_, PyBytes>>> {
        let (output, file_path) = match output {
            Some(PyWritableOutput::Path(file_path)) => (
                Self::File(py.allow_threads(|| File::create(&file_path))?),
                Some(file_path),
            ),
            Some(PyWritableOutput::Io(object)) => (Self::Io(PyIo(object)), None),
            None => (Self::Bytes(Vec::new()), None),
        };
        let serializer = write(BufWriter::new(output), file_path)?;
        py.allow_threads(|| serializer.into_inner())?.close(py)
    }

    fn close(self, py: Python<'_>) -> PyResult<Option<Bound<'_, PyBytes>>> {
        match self {
            Self::Bytes(bytes) => Ok(Some(PyBytes::new_bound(py, &bytes))),
            Self::File(mut file) => {
                py.allow_threads(|| {
                    file.flush()?;
                    file.sync_all()
                })?;
                Ok(None)
            }
            Self::Io(mut io) => {
                py.allow_threads(|| io.flush())?;
                Ok(None)
            }
        }
    }
}

impl Write for PyWritable {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Bytes(bytes) => bytes.write(buf),
            Self::Io(io) => io.write(buf),
            Self::File(file) => file.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Bytes(_) => Ok(()),
            Self::Io(io) => io.flush(),
            Self::File(file) => file.flush(),
        }
    }
}

#[derive(FromPyObject)]
pub enum PyWritableOutput {
    Path(PathBuf),
    Io(PyObject),
}

pub struct PyIo(PyObject);

impl Read for PyIo {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Python::with_gil(|py| {
            if buf.is_empty() {
                return Ok(0);
            }
            let to_read = max(1, buf.len() / 4); // We divide by 4 because TextIO works with number of characters and not with number of bytes
            let read = self
                .0
                .bind(py)
                .call_method1(intern!(py, "read"), (to_read,))?;
            Ok(if let Ok(bytes) = read.extract::<&[u8]>() {
                buf[..bytes.len()].copy_from_slice(bytes);
                bytes.len()
            } else {
                let str = read.extract::<PyBackedStr>()?;
                buf[..str.len()].copy_from_slice(str.as_bytes());
                str.len()
            })
        })
    }
}

impl Write for PyIo {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Python::with_gil(|py| {
            Ok(self
                .0
                .bind(py)
                .call_method1(intern!(py, "write"), (PyBytes::new_bound(py, buf),))?
                .extract::<usize>()?)
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Python::with_gil(|py| {
            self.0.bind(py).call_method0(intern!(py, "flush"))?;
            Ok(())
        })
    }
}

pub fn lookup_rdf_format(
    format: Option<PyRdfFormatInput>,
    path: Option<&Path>,
) -> PyResult<RdfFormat> {
    if let Some(format) = format {
        return match format {
            PyRdfFormatInput::Object(format) => Ok(format.inner),
            PyRdfFormatInput::MediaType(media_type) => {
                deprecation_warning("Using string to specify a RDF format is deprecated, please use a RdfFormat object instead.")?;
                RdfFormat::from_media_type(&media_type).ok_or_else(|| {
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
    RdfFormat::from_extension(ext)
        .ok_or_else(|| PyValueError::new_err(format!("Not supported RDF format extension: {ext}")))
}

#[derive(FromPyObject)]
pub enum PyRdfFormatInput {
    Object(PyRdfFormat),
    MediaType(String),
}

pub fn map_parse_error(error: RdfParseError, file_path: Option<PathBuf>) -> PyErr {
    match error {
        RdfParseError::Syntax(error) => {
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
        RdfParseError::Io(error) => error.into(),
    }
}

pub fn python_version() -> (u8, u8) {
    static VERSION: OnceLock<(u8, u8)> = OnceLock::new();
    *VERSION.get_or_init(|| {
        Python::with_gil(|py| {
            let v = py.version_info();
            (v.major, v.minor)
        })
    })
}

pub fn deprecation_warning(message: &str) -> PyResult<()> {
    Python::with_gil(|py| {
        PyErr::warn_bound(py, &py.get_type_bound::<PyDeprecationWarning>(), message, 0)
    })
}
