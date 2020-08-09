use crate::model::*;
use oxigraph::model::*;
use oxigraph::sparql::{
    EvaluationError, QueryOptions, QueryResults, QuerySolution, QuerySolutionIter, QueryTripleIter,
};
use pyo3::exceptions::{IOError, RuntimeError, SyntaxError, TypeError, ValueError};
use pyo3::prelude::*;
use pyo3::{PyIterProtocol, PyMappingProtocol, PyNativeType, PyObjectProtocol};
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

pub fn build_query_options(
    use_default_graph_as_union: bool,
    default_graph_uris: Option<Vec<PyNamedNode>>,
    named_graph_uris: Option<Vec<PyNamedNode>>,
) -> PyResult<QueryOptions> {
    let mut options = QueryOptions::default();
    if use_default_graph_as_union {
        options = options.with_default_graph_as_union();
    }
    if let Some(default_graph_uris) = default_graph_uris {
        if default_graph_uris.is_empty() {
            return Err(ValueError::py_err(
                "The list of the default graph URIs could not be empty",
            ));
        }
        for default_graph_uri in default_graph_uris {
            options = options.with_default_graph(default_graph_uri);
        }
    }
    if let Some(named_graph_uris) = named_graph_uris {
        if named_graph_uris.is_empty() {
            return Err(ValueError::py_err(
                "The list of the named graph URIs could not be empty",
            ));
        }
        for named_graph_uri in named_graph_uris {
            options = options.with_named_graph(named_graph_uri);
        }
    }
    Ok(options)
}

pub fn query_results_to_python(py: Python<'_>, results: QueryResults) -> PyResult<PyObject> {
    Ok(match results {
        QueryResults::Solutions(inner) => PyQuerySolutionIter { inner }.into_py(py),
        QueryResults::Graph(inner) => PyQueryTripleIter { inner }.into_py(py),
        QueryResults::Boolean(b) => b.into_py(py),
    })
}

#[pyclass(unsendable, name = QuerySolution)]
pub struct PyQuerySolution {
    inner: QuerySolution,
}

#[pyproto]
impl PyObjectProtocol for PyQuerySolution {
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

#[pyclass(unsendable, name = QuerySolutionIter)]
pub struct PyQuerySolutionIter {
    inner: QuerySolutionIter,
}

#[pyproto]
impl PyIterProtocol for PyQuerySolutionIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyQuerySolution>> {
        Ok(slf
            .inner
            .next()
            .transpose()
            .map_err(map_evaluation_error)?
            .map(move |inner| PyQuerySolution { inner }))
    }
}

#[pyclass(unsendable, name = QueryTripleIter)]
pub struct PyQueryTripleIter {
    inner: QueryTripleIter,
}

#[pyproto]
impl PyIterProtocol for PyQueryTripleIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyTriple>> {
        Ok(slf
            .inner
            .next()
            .transpose()
            .map_err(map_evaluation_error)?
            .map(|t| t.into()))
    }
}

pub fn map_io_err(error: io::Error) -> PyErr {
    match error.kind() {
        io::ErrorKind::InvalidInput => ValueError::py_err(error.to_string()),
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => {
            SyntaxError::py_err(error.to_string())
        }
        _ => IOError::py_err(error.to_string()),
    }
}

pub fn map_evaluation_error(error: EvaluationError) -> PyErr {
    match error {
        EvaluationError::Parsing(error) => SyntaxError::py_err(error.to_string()),
        EvaluationError::Io(error) => map_io_err(error),
        EvaluationError::Query(error) => ValueError::py_err(error.to_string()),
        _ => RuntimeError::py_err(error.to_string()),
    }
}
