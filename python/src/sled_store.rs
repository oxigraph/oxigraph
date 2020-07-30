use crate::model::*;
use crate::store_utils::*;
use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::*;
use oxigraph::sparql::QueryOptions;
use oxigraph::SledStore;
use pyo3::exceptions::ValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::{PyIterProtocol, PyObjectProtocol, PySequenceProtocol};
use std::io;
use std::io::Cursor;

#[pyclass(name = SledStore)]
#[derive(Clone)]
pub struct PySledStore {
    inner: SledStore,
}

#[pymethods]
impl PySledStore {
    #[new]
    fn new(path: Option<&str>) -> PyResult<Self> {
        Ok(Self {
            inner: if let Some(path) = path {
                SledStore::open(path).map_err(map_io_err)?
            } else {
                SledStore::new().map_err(map_io_err)?
            },
        })
    }

    fn add(&self, quad: &PyTuple) -> PyResult<()> {
        self.inner.insert(&extract_quad(quad)?).map_err(map_io_err)
    }

    fn remove(&self, quad: &PyTuple) -> PyResult<()> {
        self.inner.remove(&extract_quad(quad)?).map_err(map_io_err)
    }

    fn r#match(
        &self,
        subject: &PyAny,
        predicate: &PyAny,
        object: &PyAny,
        graph_name: Option<&PyAny>,
    ) -> PyResult<QuadIter> {
        let (subject, predicate, object, graph_name) =
            extract_quads_pattern(subject, predicate, object, graph_name)?;
        Ok(QuadIter {
            inner: Box::new(self.inner.quads_for_pattern(
                subject.as_ref(),
                predicate.as_ref(),
                object.as_ref(),
                graph_name.as_ref(),
            )),
        })
    }

    fn query(&self, query: &str, py: Python<'_>) -> PyResult<PyObject> {
        let results = self
            .inner
            .query(query, QueryOptions::default())
            .map_err(map_evaluation_error)?;
        query_results_to_python(py, results)
    }

    #[args(data, mime_type, "*", base_iri = "\"\"", to_graph = "None")]
    fn load(
        &self,
        data: &str,
        mime_type: &str,
        base_iri: &str,
        to_graph: Option<&PyAny>,
    ) -> PyResult<()> {
        let to_graph_name = if let Some(graph_name) = to_graph {
            Some(extract_graph_name(graph_name)?)
        } else {
            None
        };
        let base_iri = if base_iri.is_empty() {
            None
        } else {
            Some(base_iri)
        };

        if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
            self.inner
                .load_graph(
                    Cursor::new(data),
                    graph_format,
                    &to_graph_name.unwrap_or(GraphName::DefaultGraph),
                    base_iri,
                )
                .map_err(map_io_err)
        } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
            if to_graph_name.is_some() {
                return Err(ValueError::py_err(
                    "The target graph name parameter is not available for dataset formats",
                ));
            }
            self.inner
                .load_dataset(Cursor::new(data), dataset_format, base_iri)
                .map_err(map_io_err)
        } else {
            Err(ValueError::py_err(format!(
                "Not supported MIME type: {}",
                mime_type
            )))
        }
    }
}

#[pyproto]
impl PyObjectProtocol for PySledStore {
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }
}

#[pyproto]
impl PySequenceProtocol for PySledStore {
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __contains__(&self, quad: &PyTuple) -> PyResult<bool> {
        self.inner
            .contains(&extract_quad(quad)?)
            .map_err(map_io_err)
    }
}

#[pyproto]
impl PyIterProtocol for PySledStore {
    fn __iter__(slf: PyRef<Self>) -> QuadIter {
        QuadIter {
            inner: Box::new(slf.inner.quads_for_pattern(None, None, None, None)),
        }
    }
}

#[pyclass(unsendable)]
pub struct QuadIter {
    inner: Box<dyn Iterator<Item = Result<Quad, io::Error>>>,
}

#[pyproto]
impl PyIterProtocol for QuadIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(
        mut slf: PyRefMut<Self>,
    ) -> PyResult<Option<(PyObject, PyObject, PyObject, PyObject)>> {
        slf.inner
            .next()
            .map(move |q| Ok(quad_to_python(slf.py(), q.map_err(map_io_err)?)))
            .transpose()
    }
}
