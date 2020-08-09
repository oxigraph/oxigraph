use crate::model::*;
use crate::store_utils::*;
use oxigraph::sparql::*;
use pyo3::exceptions::{RuntimeError, SyntaxError, TypeError, ValueError};
use pyo3::prelude::*;
use pyo3::{PyIterProtocol, PyMappingProtocol, PyNativeType, PyObjectProtocol};

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
        QueryResults::Solutions(inner) => PyQuerySolutions { inner }.into_py(py),
        QueryResults::Graph(inner) => PyQueryTriples { inner }.into_py(py),
        QueryResults::Boolean(b) => b.into_py(py),
    })
}

/// Tuple associating variables and terms that are the result of a SPARQL ``SELECT`` query.
///
/// It is the equivalent of a row in SQL.
///
/// It could be indexes by variable name (:py:class:`Variable` or :py:class:`str`) or position in the tuple (:py:class:`int`).
///
/// >>> store = SledStore()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// >>> solution = next(store.query('SELECT ?s ?p ?o WHERE { ?s ?p ?o }'))
/// >>> solution[Variable('s')]
/// <NamedNode value=http://example.com>
/// >>> solution['s']
/// <NamedNode value=http://example.com>
/// >>> solution[0]
/// <NamedNode value=http://example.com>
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
        } else if let Ok(key) = input.downcast::<PyCell<PyVariable>>() {
            let key = &*key.borrow();
            Ok(self
                .inner
                .get(<&Variable>::from(key))
                .map(|term| term_to_python(input.py(), term.clone())))
        } else {
            Err(TypeError::py_err(format!(
                "{} is not an integer of a string",
                input.get_type().name(),
            )))
        }
    }
}

/// An iterator of :py:class:`QuerySolution` returned by a SPARQL ``SELECT`` query
///
/// >>> store = SledStore()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// >>> list(store.query('SELECT ?s WHERE { ?s ?p ?o }'))
/// [<QuerySolution s=<NamedNode value=http://example.com>>]
#[pyclass(unsendable, name = QuerySolutions)]
pub struct PyQuerySolutions {
    inner: QuerySolutionIter,
}

#[pymethods]
impl PyQuerySolutions {
    /// :return: the ordered list of all variables that could appear in the query results
    /// :rtype: list(Variable)
    ///
    /// >>> store = SledStore()
    /// >>> store.query('SELECT ?s WHERE { ?s ?p ?o }').variables
    /// [<Variable value=s>]
    #[getter]
    fn variables(&self) -> Vec<PyVariable> {
        self.inner
            .variables()
            .iter()
            .map(|v| v.clone().into())
            .collect()
    }
}

#[pyproto]
impl PyIterProtocol for PyQuerySolutions {
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

/// An iterator of :py:class:`Triple` returned by a SPARQL ``CONSTRUCT`` or ``DESCRIBE`` query
///
/// >>> store = MemoryStore()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// >>> list(store.query('CONSTRUCT WHERE { ?s ?p ?o }'))
/// [<Triple subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]
#[pyclass(unsendable, name = QueryTriples)]
pub struct PyQueryTriples {
    inner: QueryTripleIter,
}

#[pyproto]
impl PyIterProtocol for PyQueryTriples {
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

pub fn map_evaluation_error(error: EvaluationError) -> PyErr {
    match error {
        EvaluationError::Parsing(error) => SyntaxError::py_err(error.to_string()),
        EvaluationError::Io(error) => map_io_err(error),
        EvaluationError::Query(error) => ValueError::py_err(error.to_string()),
        _ => RuntimeError::py_err(error.to_string()),
    }
}
