#![allow(clippy::needless_option_as_deref)]

use crate::model::{PyQuad, PyTriple};
use oxigraph::io::read::{ParseError, QuadReader, TripleReader};
use oxigraph::io::{
    DatasetFormat, DatasetParser, DatasetSerializer, GraphFormat, GraphParser, GraphSerializer,
};
use pyo3::exceptions::{PyIOError, PySyntaxError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::wrap_pyfunction;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Cursor, Read, Write};

pub fn add_to_module(module: &PyModule) -> PyResult<()> {
    module.add_wrapped(wrap_pyfunction!(parse))?;
    module.add_wrapped(wrap_pyfunction!(serialize))
}

/// Parses RDF graph and dataset serialization formats.
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
/// :type input: io(bytes) or io(str) or str
/// :param mime_type: the MIME type of the RDF serialization.
/// :type mime_type: str
/// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
/// :type base_iri: str or None, optional
/// :return: an iterator of RDF triples or quads depending on the format.
/// :rtype: iterator(Triple) or iterator(Quad)
/// :raises ValueError: if the MIME type is not supported.
/// :raises SyntaxError: if the provided data is invalid.
///
/// >>> input = io.BytesIO(b'<foo> <p> "1" .')
/// >>> list(parse(input, "text/turtle", base_iri="http://example.com/"))
/// [<Triple subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]
#[pyfunction]
#[pyo3(text_signature = "(input, mime_type, *, base_iri = None)")]
pub fn parse(
    input: PyObject,
    mime_type: &str,
    base_iri: Option<&str>,
    py: Python<'_>,
) -> PyResult<PyObject> {
    let input = if let Ok(path) = input.extract::<&str>(py) {
        PyReadable::from_file(path, py)
    } else {
        PyReadable::from_data(input, py)
    }
    .map_err(map_io_err)?;
    if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
        let mut parser = GraphParser::from_format(graph_format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
        }
        Ok(PyTripleReader {
            inner: py.allow_threads(|| parser.read_triples(input).map_err(map_parse_error))?,
        }
        .into_py(py))
    } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
        let mut parser = DatasetParser::from_format(dataset_format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
        }
        Ok(PyQuadReader {
            inner: py.allow_threads(|| parser.read_quads(input).map_err(map_parse_error))?,
        }
        .into_py(py))
    } else {
        Err(PyValueError::new_err(format!(
            "Not supported MIME type: {mime_type}"
        )))
    }
}

/// Serializes an RDF graph or dataset.
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
/// :param input: the RDF triples and quads to serialize.
/// :type input: iterable(Triple) or iterable(Quad)
/// :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``.
/// :type output: io(bytes) or str
/// :param mime_type: the MIME type of the RDF serialization.
/// :type mime_type: str
/// :rtype: None
/// :raises ValueError: if the MIME type is not supported.
/// :raises TypeError: if a triple is given during a quad format serialization or reverse.
///
/// >>> output = io.BytesIO()
/// >>> serialize([Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))], output, "text/turtle")
/// >>> output.getvalue()
/// b'<http://example.com> <http://example.com/p> "1" .\n'
#[pyfunction]
pub fn serialize(input: &PyAny, output: PyObject, mime_type: &str, py: Python<'_>) -> PyResult<()> {
    let output = if let Ok(path) = output.extract::<&str>(py) {
        PyWritable::from_file(path, py)
    } else {
        PyWritable::from_data(output)
    }
    .map_err(map_io_err)?;
    if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
        let mut writer = GraphSerializer::from_format(graph_format)
            .triple_writer(output)
            .map_err(map_io_err)?;
        for i in input.iter()? {
            writer
                .write(&*i?.downcast::<PyCell<PyTriple>>()?.borrow())
                .map_err(map_io_err)?;
        }
        writer.finish().map_err(map_io_err)?;
        Ok(())
    } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
        let mut writer = DatasetSerializer::from_format(dataset_format)
            .quad_writer(output)
            .map_err(map_io_err)?;
        for i in input.iter()? {
            writer
                .write(&*i?.downcast::<PyCell<PyQuad>>()?.borrow())
                .map_err(map_io_err)?;
        }
        writer.finish().map_err(map_io_err)?;
        Ok(())
    } else {
        Err(PyValueError::new_err(format!(
            "Not supported MIME type: {mime_type}"
        )))
    }
}

#[pyclass(name = "TripleReader")]
pub struct PyTripleReader {
    inner: TripleReader<PyReadable>,
}

#[pymethods]
impl PyTripleReader {
    fn __iter__(slf: PyRef<'_, Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyTriple>> {
        py.allow_threads(|| {
            self.inner
                .next()
                .map(|q| Ok(q.map_err(map_parse_error)?.into()))
                .transpose()
        })
    }
}

#[pyclass(name = "QuadReader")]
pub struct PyQuadReader {
    inner: QuadReader<PyReadable>,
}

#[pymethods]
impl PyQuadReader {
    fn __iter__(slf: PyRef<'_, Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyQuad>> {
        py.allow_threads(|| {
            self.inner
                .next()
                .map(|q| Ok(q.map_err(map_parse_error)?.into()))
                .transpose()
        })
    }
}

pub(crate) enum PyReadable {
    Bytes(Cursor<Vec<u8>>),
    Io(BufReader<PyIo>),
    File(BufReader<File>),
}

impl PyReadable {
    pub fn from_file(file: &str, py: Python<'_>) -> io::Result<Self> {
        Ok(Self::File(BufReader::new(
            py.allow_threads(|| File::open(file))?,
        )))
    }

    pub fn from_data(data: PyObject, py: Python<'_>) -> io::Result<Self> {
        Ok(if let Ok(bytes) = data.extract::<Vec<u8>>(py) {
            Self::Bytes(Cursor::new(bytes))
        } else if let Ok(string) = data.extract::<String>(py) {
            Self::Bytes(Cursor::new(string.into_bytes()))
        } else {
            Self::Io(BufReader::new(PyIo(data)))
        })
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

impl BufRead for PyReadable {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        match self {
            Self::Bytes(bytes) => bytes.fill_buf(),
            Self::Io(io) => io.fill_buf(),
            Self::File(file) => file.fill_buf(),
        }
    }

    fn consume(&mut self, amt: usize) {
        match self {
            Self::Bytes(bytes) => bytes.consume(amt),
            Self::Io(io) => io.consume(amt),
            Self::File(file) => file.consume(amt),
        }
    }
}

pub(crate) enum PyWritable {
    Io(BufWriter<PyIo>),
    File(BufWriter<File>),
}

impl PyWritable {
    pub fn from_file(file: &str, py: Python<'_>) -> io::Result<Self> {
        Ok(Self::File(BufWriter::new(
            py.allow_threads(|| File::create(file))?,
        )))
    }

    pub fn from_data(data: PyObject) -> io::Result<Self> {
        Ok(Self::Io(BufWriter::new(PyIo(data))))
    }
}

impl Write for PyWritable {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Io(io) => io.write(buf),
            Self::File(file) => file.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Io(io) => io.flush(),
            Self::File(file) => file.flush(),
        }
    }
}

pub(crate) struct PyIo(PyObject);

impl Read for PyIo {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        Python::with_gil(|py| {
            let read = self
                .0
                .call_method(py, "read", (buf.len(),), None)
                .map_err(to_io_err)?;
            let bytes = read
                .extract::<&[u8]>(py)
                .or_else(|e| {
                    read.extract::<&str>(py)
                        .map(|s| s.as_bytes())
                        .map_err(|_| e)
                })
                .map_err(to_io_err)?;
            buf.write_all(bytes)?;
            Ok(bytes.len())
        })
    }
}

impl Write for PyIo {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Python::with_gil(|py| {
            self.0
                .call_method(py, "write", (PyBytes::new(py, buf),), None)
                .map_err(to_io_err)?
                .extract::<usize>(py)
                .map_err(to_io_err)
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Python::with_gil(|py| {
            self.0.call_method(py, "flush", (), None)?;
            Ok(())
        })
    }
}

fn to_io_err(error: impl Into<PyErr>) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error.into())
}

pub(crate) fn map_io_err(error: io::Error) -> PyErr {
    if error.get_ref().map_or(false, |s| s.is::<PyErr>()) {
        *error.into_inner().unwrap().downcast().unwrap()
    } else {
        PyIOError::new_err(error.to_string())
    }
}

pub(crate) fn map_parse_error(error: ParseError) -> PyErr {
    match error {
        ParseError::Syntax(error) => PySyntaxError::new_err(error.to_string()),
        ParseError::Io(error) => map_io_err(error),
    }
}

/// Release the GIL
/// There should not be ANY use of pyo3 code inside of this method!!!
///
/// Code from pyo3: https://github.com/PyO3/pyo3/blob/a67180c8a42a0bc0fdc45b651b62c0644130cf47/src/python.rs#L366
#[allow(unsafe_code)]
pub(crate) fn allow_threads_unsafe<T>(f: impl FnOnce() -> T) -> T {
    struct RestoreGuard {
        tstate: *mut pyo3::ffi::PyThreadState,
    }

    impl Drop for RestoreGuard {
        fn drop(&mut self) {
            unsafe {
                pyo3::ffi::PyEval_RestoreThread(self.tstate);
            }
        }
    }

    let _guard = RestoreGuard {
        tstate: unsafe { pyo3::ffi::PyEval_SaveThread() },
    };
    f()
}
