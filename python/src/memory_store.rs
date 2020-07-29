use crate::model::*;
use crate::store_utils::*;
use oxigraph::model::*;
use oxigraph::sparql::QueryOptions;
use oxigraph::{DatasetSyntax, GraphSyntax, MemoryStore};
use pyo3::basic::CompareOp;
use pyo3::exceptions::{NotImplementedError, ValueError};
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::{PyIterProtocol, PyObjectProtocol, PySequenceProtocol};
use std::io::Cursor;

#[pyclass(name = MemoryStore)]
#[derive(Eq, PartialEq, Clone)]
pub struct PyMemoryStore {
    inner: MemoryStore,
}

#[pymethods]
impl PyMemoryStore {
    #[new]
    fn new() -> Self {
        Self {
            inner: MemoryStore::new(),
        }
    }

    fn add(&self, quad: &PyTuple) -> PyResult<()> {
        self.inner.insert(extract_quad(quad)?);
        Ok(())
    }

    fn remove(&self, quad: &PyTuple) -> PyResult<()> {
        self.inner.remove(&extract_quad(quad)?);
        Ok(())
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
            .map_err(|e| ValueError::py_err(e.to_string()))?;
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

        if let Some(graph_syntax) = GraphSyntax::from_media_type(mime_type) {
            self.inner
                .load_graph(
                    Cursor::new(data),
                    graph_syntax,
                    &to_graph_name.unwrap_or(GraphName::DefaultGraph),
                    base_iri,
                )
                .map_err(map_io_err)
        } else if let Some(dataset_syntax) = DatasetSyntax::from_media_type(mime_type) {
            if to_graph_name.is_some() {
                return Err(ValueError::py_err(
                    "The target graph name parameter is not available for dataset formats",
                ));
            }
            self.inner
                .load_dataset(Cursor::new(data), dataset_syntax, base_iri)
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
impl PyObjectProtocol for PyMemoryStore {
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __richcmp__(&self, other: &PyCell<Self>, op: CompareOp) -> PyResult<bool> {
        let other: &PyMemoryStore = &other.borrow();
        match op {
            CompareOp::Eq => Ok(self == other),
            CompareOp::Ne => Ok(self != other),
            _ => Err(NotImplementedError::py_err("Ordering is not implemented")),
        }
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }
}

#[pyproto]
impl PySequenceProtocol for PyMemoryStore {
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __contains__(&self, quad: &PyTuple) -> PyResult<bool> {
        Ok(self.inner.contains(&extract_quad(quad)?))
    }
}

#[pyproto]
impl PyIterProtocol for PyMemoryStore {
    fn __iter__(slf: PyRef<Self>) -> QuadIter {
        QuadIter {
            inner: Box::new(slf.inner.quads_for_pattern(None, None, None, None)),
        }
    }
}

#[pyclass(unsendable)]
pub struct QuadIter {
    inner: Box<dyn Iterator<Item = Quad>>,
}

#[pyproto]
impl PyIterProtocol for QuadIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> Option<(PyObject, PyObject, PyObject, PyObject)> {
        slf.inner.next().map(move |q| quad_to_python(slf.py(), q))
    }
}
