use crate::model::*;
use oxigraph::model::*;
use oxigraph::sparql::{QueryResult, QuerySolution};
use oxigraph::Result;
use pyo3::exceptions::TypeError;
use pyo3::prelude::*;
use pyo3::{create_exception, PyIterProtocol, PyMappingProtocol, PyNativeType};
use std::vec::IntoIter;

create_exception!(oxigraph, ParseError, pyo3::exceptions::Exception);

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

pub fn query_results_to_python(
    py: Python<'_>,
    results: QueryResult<'_>,
    error: impl Fn(String) -> PyErr,
) -> PyResult<PyObject> {
    Ok(match results {
        QueryResult::Solutions(solutions) => QuerySolutionIter {
            inner: solutions
                .collect::<Result<Vec<_>>>()
                .map_err(|e| error(e.to_string()))?
                .into_iter(),
        }
        .into_py(py),
        QueryResult::Graph(triples) => TripleResultIter {
            inner: triples
                .collect::<Result<Vec<_>>>()
                .map_err(|e| error(e.to_string()))?
                .into_iter(),
        }
        .into_py(py),
        QueryResult::Boolean(b) => b.into_py(py),
    })
}

#[pyclass(unsendable)]
pub struct PyQuerySolution {
    inner: QuerySolution,
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
    inner: IntoIter<QuerySolution>,
}

#[pyproto]
impl PyIterProtocol for QuerySolutionIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> Option<PyQuerySolution> {
        slf.inner.next().map(move |inner| PyQuerySolution { inner })
    }
}

#[pyclass(unsendable)]
pub struct TripleResultIter {
    inner: IntoIter<Triple>,
}

#[pyproto]
impl PyIterProtocol for TripleResultIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> Option<(PyObject, PyObject, PyObject)> {
        slf.inner.next().map(move |t| triple_to_python(slf.py(), t))
    }
}
