use crate::model::*;
use oxigraph::model::*;
use oxigraph::sparql::{QueryResult, QuerySolution, QuerySolutionsIterator};
use oxigraph::Result;
use pyo3::exceptions::{IOError, TypeError, ValueError};
use pyo3::prelude::*;
use pyo3::{PyIterProtocol, PyMappingProtocol, PyNativeType, PyObjectProtocol};
use std::fmt::Write;
use std::io;

pub fn extract_quads_pattern(
    subject: &PyAny,
    predicate: &PyAny,
    object: &PyAny,
    graph_name: Option<&PyAny>,
) -> PyResult<(
    Option<NamedOrBlankNode>,
    Option<NamedNode>,
    Option<Term>,
    Option<GraphName>,
)> {
    Ok((
        if subject.is_none() {
            None
        } else {
            Some(extract_named_or_blank_node(subject)?)
        },
        if predicate.is_none() {
            None
        } else {
            Some(extract_named_node(predicate)?)
        },
        if object.is_none() {
            None
        } else {
            Some(extract_term(object)?)
        },
        if let Some(graph_name) = graph_name {
            if graph_name.is_none() {
                None
            } else {
                Some(extract_graph_name(graph_name)?)
            }
        } else {
            None
        },
    ))
}

pub fn query_results_to_python(py: Python<'_>, results: QueryResult) -> PyResult<PyObject> {
    Ok(match results {
        QueryResult::Solutions(inner) => QuerySolutionIter { inner }.into_py(py),
        QueryResult::Graph(inner) => TripleResultIter { inner }.into_py(py),
        QueryResult::Boolean(b) => b.into_py(py),
    })
}

#[pyclass(unsendable)]
pub struct PyQuerySolution {
    inner: QuerySolution,
}

#[pyproto]
impl PyObjectProtocol for PyQuerySolution {
    fn __repr__(&self) -> String {
        let mut buffer = "<QuerySolution".to_owned();
        for (k, v) in self.inner.iter() {
            write!(&mut buffer, " {}={}", k.as_str(), v).unwrap();
        }
        buffer.push('>');
        buffer
    }
}

#[pyproto]
impl PyMappingProtocol for PyQuerySolution {
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(&self, input: &PyAny) -> PyResult<Option<PyObject>> {
        if let Ok(key) = usize::extract(input) {
            Ok(self
                .inner
                .get(key)
                .map(|term| term_to_python(input.py(), term.clone())))
        } else if let Ok(key) = <&str>::extract(input) {
            Ok(self
                .inner
                .get(key)
                .map(|term| term_to_python(input.py(), term.clone())))
        } else {
            Err(TypeError::py_err(format!(
                "{} is not an integer of a string",
                input.get_type().name(),
            )))
        }
    }
}

#[pyclass(unsendable)]
pub struct QuerySolutionIter {
    inner: QuerySolutionsIterator,
}

#[pyproto]
impl PyIterProtocol for QuerySolutionIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyQuerySolution>> {
        Ok(slf
            .inner
            .next()
            .transpose()
            .map_err(|e| IOError::py_err(e.to_string()))? //TODO: improve
            .map(move |inner| PyQuerySolution { inner }))
    }
}

#[pyclass(unsendable)]
pub struct TripleResultIter {
    inner: Box<dyn Iterator<Item = Result<Triple>>>,
}

#[pyproto]
impl PyIterProtocol for TripleResultIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<(PyObject, PyObject, PyObject)>> {
        Ok(slf
            .inner
            .next()
            .transpose()
            .map_err(|e| IOError::py_err(e.to_string()))? //TODO: improve
            .map(move |t| triple_to_python(slf.py(), t)))
    }
}

pub fn map_io_err(error: io::Error) -> PyErr {
    match error.kind() {
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => {
            ValueError::py_err(error.to_string())
        }
        _ => IOError::py_err(error.to_string()),
    }
}
