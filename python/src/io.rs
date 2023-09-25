#![allow(clippy::needless_option_as_deref)]

use crate::model::{PyQuad, PyTriple};
use oxigraph::io::{FromReadQuadReader, ParseError, RdfFormat, RdfParser, RdfSerializer};
use oxigraph::model::QuadRef;
use oxigraph::sparql::results::QueryResultsFormat;
use pyo3::exceptions::{PySyntaxError, PyValueError};
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::cmp::max;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Parses RDF graph and dataset serialization formats.
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
/// :param input: The I/O object or file path to read from. For example, it could be a file path as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
/// :type input: io(bytes) or io(str) or str or pathlib.Path
/// :param format: the format of the RDF serialization using a media type like ``text/turtle`` or an extension like `ttl`. If :py:const:`None`, the format is guessed from the file name extension.
/// :type format: str or None, optional
/// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
/// :type base_iri: str or None, optional
/// :param without_named_graphs: Sets that the parser must fail when parsing a named graph.
/// :type without_named_graphs: bool, optional
/// :param rename_blank_nodes: Renames the blank nodes identifiers from the ones set in the serialization to random ids. This allows to avoid identifier conflicts when merging graphs together.
/// :type rename_blank_nodes: bool, optional
/// :return: an iterator of RDF triples or quads depending on the format.
/// :rtype: iterator(Quad)
/// :raises ValueError: if the format is not supported.
/// :raises SyntaxError: if the provided data is invalid.
/// :raises OSError: if a system error happens while reading the file.
///
/// >>> input = io.BytesIO(b'<foo> <p> "1" .')
/// >>> list(parse(input, "text/turtle", base_iri="http://example.com/"))
/// [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<DefaultGraph>>]
#[pyfunction]
#[pyo3(signature = (input, /, format = None, *, base_iri = None, without_named_graphs = false, rename_blank_nodes = false))]
pub fn parse(
    input: &PyAny,
    format: Option<&str>,
    base_iri: Option<&str>,
    without_named_graphs: bool,
    rename_blank_nodes: bool,
    py: Python<'_>,
) -> PyResult<PyObject> {
    let file_path = input.extract::<PathBuf>().ok();
    let format = parse_format(format, file_path.as_deref())?;
    let input = if let Some(file_path) = &file_path {
        PyReadable::from_file(file_path, py).map_err(map_io_err)?
    } else {
        PyReadable::from_data(input)
    };
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
        inner: parser.parse_read(input),
        file_path,
    }
    .into_py(py))
}

/// Serializes an RDF graph or dataset.
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
/// :param input: the RDF triples and quads to serialize.
/// :type input: iterable(Triple) or iterable(Quad)
/// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``. If :py:const:`None`, a :py:class:`bytes` buffer is returned with the serialized content.
/// :type output: io(bytes) or str or pathlib.Path or None, optional
/// :param format: the format of the RDF serialization using a media type like ``text/turtle`` or an extension like `ttl`. If :py:const:`None`, the format is guessed from the file name extension.
/// :type format: str or None, optional
/// :return: py:class:`bytes` with the serialization if the ``output`` parameter is :py:const:`None`, :py:const:`None` if ``output`` is set.
/// :rtype: bytes or None
/// :raises ValueError: if the format is not supported.
/// :raises TypeError: if a triple is given during a quad format serialization or reverse.
/// :raises OSError: if a system error happens while writing the file.
///
/// >>> serialize([Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))], format="ttl")
/// b'<http://example.com> <http://example.com/p> "1" .\n'
///
/// >>> output = io.BytesIO()
/// >>> serialize([Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))], output, "text/turtle")
/// >>> output.getvalue()
/// b'<http://example.com> <http://example.com/p> "1" .\n'
#[pyfunction]
#[pyo3(signature = (input, output = None, /, format = None))]
pub fn serialize<'a>(
    input: &PyAny,
    output: Option<&PyAny>,
    format: Option<&str>,
    py: Python<'a>,
) -> PyResult<Option<&'a PyBytes>> {
    PyWritable::do_write(
        |output, format| {
            let mut writer = RdfSerializer::from_format(format).serialize_to_write(output);
            for i in input.iter()? {
                let i = i?;
                if let Ok(triple) = i.extract::<PyRef<PyTriple>>() {
                    writer.write_triple(&*triple)
                } else {
                    let quad = i.extract::<PyRef<PyQuad>>()?;
                    let quad = QuadRef::from(&*quad);
                    if !quad.graph_name.is_default_graph() && !format.supports_datasets() {
                        return Err(PyValueError::new_err(
                            "The {format} format does not support named graphs",
                        ));
                    }
                    writer.write_quad(quad)
                }
                .map_err(map_io_err)?;
            }
            writer.finish().map_err(map_io_err)
        },
        output,
        format,
        py,
    )
}

#[pyclass(name = "QuadReader", module = "pyoxigraph")]
pub struct PyQuadReader {
    inner: FromReadQuadReader<PyReadable>,
    file_path: Option<PathBuf>,
}

#[pymethods]
impl PyQuadReader {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyQuad>> {
        py.allow_threads(|| {
            self.inner
                .next()
                .map(|q| {
                    Ok(q.map_err(|e| map_parse_error(e, self.file_path.clone()))?
                        .into())
                })
                .transpose()
        })
    }
}

pub enum PyReadable {
    Bytes(Cursor<Vec<u8>>),
    Io(PyIo),
    File(File),
}

impl PyReadable {
    pub fn from_file(file: &Path, py: Python<'_>) -> io::Result<Self> {
        Ok(Self::File(py.allow_threads(|| File::open(file))?))
    }

    pub fn from_data(data: &PyAny) -> Self {
        if let Ok(bytes) = data.extract::<Vec<u8>>() {
            Self::Bytes(Cursor::new(bytes))
        } else if let Ok(string) = data.extract::<String>() {
            Self::Bytes(Cursor::new(string.into_bytes()))
        } else {
            Self::Io(PyIo(data.into()))
        }
    }
}

impl Read for PyReadable {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Bytes(bytes) => bytes.read(buf),
            Self::Io(io) => io.read(buf),
            Self::File(file) => file.read(buf),
        }
    }
}

pub enum PyWritable {
    Bytes(Vec<u8>),
    Io(PyIo),
    File(File),
}

impl PyWritable {
    pub fn do_write<'a, F: Format>(
        write: impl FnOnce(BufWriter<Self>, F) -> PyResult<BufWriter<Self>>,
        output: Option<&PyAny>,
        format: Option<&str>,
        py: Python<'a>,
    ) -> PyResult<Option<&'a PyBytes>> {
        let file_path = output.and_then(|output| output.extract::<PathBuf>().ok());
        let format = parse_format::<F>(format, file_path.as_deref())?;
        let output = if let Some(output) = output {
            if let Some(file_path) = &file_path {
                Self::File(
                    py.allow_threads(|| File::create(file_path))
                        .map_err(map_io_err)?,
                )
            } else {
                Self::Io(PyIo(output.into()))
            }
        } else {
            PyWritable::Bytes(Vec::new())
        };
        let writer = write(BufWriter::new(output), format)?;
        py.allow_threads(|| writer.into_inner())
            .map_err(|e| map_io_err(e.into_error()))?
            .close(py)
    }

    fn close(self, py: Python<'_>) -> PyResult<Option<&PyBytes>> {
        match self {
            Self::Bytes(bytes) => Ok(Some(PyBytes::new(py, &bytes))),
            Self::File(mut file) => {
                py.allow_threads(|| {
                    file.flush()?;
                    file.sync_all()
                })
                .map_err(map_io_err)?;
                Ok(None)
            }
            Self::Io(mut io) => {
                py.allow_threads(|| io.flush()).map_err(map_io_err)?;
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
                .as_ref(py)
                .call_method1(intern!(py, "read"), (to_read,))
                .map_err(to_io_err)?;
            let bytes = read
                .extract::<&[u8]>()
                .or_else(|e| read.extract::<&str>().map(str::as_bytes).map_err(|_| e))
                .map_err(to_io_err)?;
            buf[..bytes.len()].copy_from_slice(bytes);
            Ok(bytes.len())
        })
    }
}

impl Write for PyIo {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Python::with_gil(|py| {
            self.0
                .as_ref(py)
                .call_method1(intern!(py, "write"), (PyBytes::new(py, buf),))
                .map_err(to_io_err)?
                .extract::<usize>()
                .map_err(to_io_err)
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Python::with_gil(|py| {
            self.0
                .as_ref(py)
                .call_method0(intern!(py, "flush"))
                .map_err(to_io_err)?;
            Ok(())
        })
    }
}

pub trait Format: Sized {
    fn from_media_type(media_type: &str) -> Option<Self>;
    fn from_extension(extension: &str) -> Option<Self>;
}

impl Format for RdfFormat {
    fn from_media_type(media_type: &str) -> Option<Self> {
        Self::from_media_type(media_type)
    }

    fn from_extension(extension: &str) -> Option<Self> {
        Self::from_extension(extension)
    }
}

impl Format for QueryResultsFormat {
    fn from_media_type(media_type: &str) -> Option<Self> {
        Self::from_media_type(media_type)
    }

    fn from_extension(extension: &str) -> Option<Self> {
        Self::from_extension(extension)
    }
}

pub fn parse_format<F: Format>(format: Option<&str>, path: Option<&Path>) -> PyResult<F> {
    let format = if let Some(format) = format {
        format
    } else if let Some(path) = path {
        if let Some(ext) = path.extension().and_then(OsStr::to_str) {
            ext
        } else {
            return Err(PyValueError::new_err(format!(
                "The file name {} has no extension to guess a file format from",
                path.display()
            )));
        }
    } else {
        return Err(PyValueError::new_err(
            "The format parameter is required when a file path is not given",
        ));
    };
    if format.contains('/') {
        F::from_media_type(format).ok_or_else(|| {
            PyValueError::new_err(format!("Not supported RDF format media type: {format}"))
        })
    } else {
        F::from_extension(format).ok_or_else(|| {
            PyValueError::new_err(format!("Not supported RDF format extension: {format}"))
        })
    }
}

fn to_io_err(error: PyErr) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error)
}

pub fn map_io_err(error: io::Error) -> PyErr {
    if error
        .get_ref()
        .map_or(false, <(dyn Error + Send + Sync + 'static)>::is::<PyErr>)
    {
        *error.into_inner().unwrap().downcast().unwrap()
    } else {
        error.into()
    }
}

pub fn map_parse_error(error: ParseError, file_path: Option<PathBuf>) -> PyErr {
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

/// Release the GIL
/// There should not be ANY use of pyo3 code inside of this method!!!
///
/// Code from pyo3: https://github.com/PyO3/pyo3/blob/a67180c8a42a0bc0fdc45b651b62c0644130cf47/src/python.rs#L366
#[allow(unsafe_code)]
pub fn allow_threads_unsafe<T>(_py: Python<'_>, f: impl FnOnce() -> T) -> T {
    struct RestoreGuard {
        tstate: *mut pyo3::ffi::PyThreadState,
    }

    impl Drop for RestoreGuard {
        fn drop(&mut self) {
            // SAFETY: not cloned so called once
            unsafe {
                pyo3::ffi::PyEval_RestoreThread(self.tstate);
            }
        }
    }

    // SAFETY: we have the restore part in Drop to make sure it's properly executed
    let tstate = unsafe { pyo3::ffi::PyEval_SaveThread() };
    let _guard = RestoreGuard { tstate };
    f()
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
