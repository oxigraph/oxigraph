use crate::model::{PyQuad, PyTriple};
use crate::store_utils::map_io_err;
use oxigraph::io::read::{QuadReader, TripleReader};
use oxigraph::io::{
    DatasetFormat, DatasetParser, DatasetSerializer, GraphFormat, GraphParser, GraphSerializer,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::wrap_pyfunction;
use pyo3::PyIterProtocol;
use std::io;
use std::io::{BufReader, Read, Write};

pub fn add_to_module(module: &PyModule) -> PyResult<()> {
    module.add_wrapped(wrap_pyfunction!(parse))?;
    module.add_wrapped(wrap_pyfunction!(serialize))
}

/// Parses RDF graph and dataset serialization formats
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
/// :return: an iterator of RDF triples or quads depending on the format
/// :rtype: iter(Triple) or iter(Quad)
/// :raises ValueError: if the MIME type is not supported
/// :raises SyntaxError: if the provided data is invalid
///
/// >>> input = io.BytesIO(b'<foo> <p> "1" .')
/// >>> list(parse(input, "text/turtle", base_iri="http://example.com/"))
/// [<Triple subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]
#[pyfunction]
#[text_signature = "(input, /, mime_type, *, base_iri = None)"]
pub fn parse(
    input: PyObject,
    mime_type: &str,
    base_iri: Option<&str>,
    py: Python<'_>,
) -> PyResult<PyObject> {
    let input = BufReader::new(PyFileLike::new(input));
    if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
        let mut parser = GraphParser::from_format(graph_format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
        }
        Ok(PyTripleReader {
            inner: parser.read_triples(input).map_err(map_io_err)?,
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
            inner: parser.read_quads(input).map_err(map_io_err)?,
        }
        .into_py(py))
    } else {
        Err(PyValueError::new_err(format!(
            "Not supported MIME type: {}",
            mime_type
        )))
    }
}

/// Serializes an RDF graph or dataset
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
/// :param input: the RDF triples and quads to serialize
/// :type input: iter(Triple) or iter(Quad)
/// :param output: The binary I/O object to write to. For example, it could be a file opened in binary mode with ``open('my_file.ttl', 'wb')``.
/// :type output: io.RawIOBase or io.BufferedIOBase
/// :param mime_type: the MIME type of the RDF serialization
/// :type mime_type: str
/// :raises ValueError: if the MIME type is not supported
/// :raises TypeError: if a triple is given during a quad format serialization or reverse
///
/// >>> output = io.BytesIO()
/// >>> serialize([Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))], output, "text/turtle")
/// >>> output.getvalue()
/// b'<http://example.com> <http://example.com/p> "1" .\n'
#[pyfunction]
#[text_signature = "(input, output, /, mime_type, *, base_iri = None)"]
pub fn serialize(input: &PyAny, output: PyObject, mime_type: &str) -> PyResult<()> {
    let output = PyFileLike::new(output);
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
            "Not supported MIME type: {}",
            mime_type
        )))
    }
}

#[pyclass(name = "TripleReader", module = "oxigraph")]
pub struct PyTripleReader {
    inner: TripleReader<BufReader<PyFileLike>>,
}

#[pyproto]
impl PyIterProtocol for PyTripleReader {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyTriple>> {
        slf.inner
            .next()
            .map(|q| Ok(q.map_err(map_io_err)?.into()))
            .transpose()
    }
}

#[pyclass(name = "QuadReader", module = "oxigraph")]
pub struct PyQuadReader {
    inner: QuadReader<BufReader<PyFileLike>>,
}

#[pyproto]
impl PyIterProtocol for PyQuadReader {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyQuad>> {
        slf.inner
            .next()
            .map(|q| Ok(q.map_err(map_io_err)?.into()))
            .transpose()
    }
}

pub struct PyFileLike {
    inner: PyObject,
}

impl PyFileLike {
    pub fn new(inner: PyObject) -> Self {
        Self { inner }
    }
}

impl Read for PyFileLike {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let read = self
            .inner
            .call_method(py, "read", (buf.len(),), None)
            .map_err(|e| to_io_err(e, py))?;
        let bytes: &PyBytes = read.cast_as(py).map_err(|e| to_io_err(e, py))?;
        buf.write_all(bytes.as_bytes())?;
        Ok(bytes.len()?)
    }
}

impl Write for PyFileLike {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        Ok(usize::extract(
            self.inner
                .call_method(py, "write", (PyBytes::new(py, buf),), None)
                .map_err(|e| to_io_err(e, py))?
                .as_ref(py),
        )
        .map_err(|e| to_io_err(e, py))?)
    }

    fn flush(&mut self) -> io::Result<()> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        self.inner.call_method(py, "flush", (), None)?;
        Ok(())
    }
}

fn to_io_err(error: impl Into<PyErr>, py: Python<'_>) -> io::Error {
    if let Ok(message) = error
        .into()
        .to_object(py)
        .call_method(py, "__str__", (), None)
    {
        if let Ok(message) = message.extract::<String>(py) {
            io::Error::new(io::ErrorKind::Other, message)
        } else {
            io::Error::new(io::ErrorKind::Other, "An unknown error has occurred")
        }
    } else {
        io::Error::new(io::ErrorKind::Other, "An unknown error has occurred")
    }
}
