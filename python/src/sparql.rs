use crate::model::*;
use crate::store_utils::*;
use oxigraph::model::Term;
use oxigraph::sparql::*;
use pyo3::exceptions::{PyRuntimeError, PySyntaxError, PyTypeError, PyValueError};
use pyo3::prelude::{
    pyclass, pymethods, pyproto, FromPyObject, IntoPy, Py, PyAny, PyCell, PyErr, PyObject, PyRef,
    PyRefMut, PyResult, Python,
};
use pyo3::{PyIterProtocol, PyMappingProtocol, PyObjectProtocol};
use std::vec::IntoIter;

pub fn parse_query(
    query: &str,
    use_default_graph_as_union: bool,
    default_graph: Option<&PyAny>,
    named_graphs: Option<&PyAny>,
) -> PyResult<Query> {
    let mut query = Query::parse(query, None).map_err(|e| map_evaluation_error(e.into()))?;

    if use_default_graph_as_union && default_graph.is_some() {
        return Err(PyValueError::new_err(
            "The query() method use_default_graph_as_union and default_graph arguments should not be set at the same time",
        ));
    }

    if use_default_graph_as_union {
        query.dataset_mut().set_default_graph_as_union();
    }

    if let Some(default_graph) = default_graph {
        if let Ok(default_graphs) = default_graph.iter() {
            query.dataset_mut().set_default_graph(
                default_graphs
                    .map(|graph| Ok(graph?.extract::<PyGraphName>()?.into()))
                    .collect::<PyResult<_>>()?,
            )
        } else if let Ok(default_graph) = default_graph.extract::<PyGraphName>() {
            query
                .dataset_mut()
                .set_default_graph(vec![default_graph.into()]);
        } else {
            return Err(PyValueError::new_err(
                format!("The query() method default_graph argument should be a NamedNode, a BlankNode, the DefaultGraph or a not empty list of them. {} found", default_graph.get_type()
                )));
        }
    }

    if let Some(named_graphs) = named_graphs {
        query.dataset_mut().set_available_named_graphs(
            named_graphs
                .iter()?
                .map(|graph| Ok(graph?.extract::<PyNamedOrBlankNode>()?.into()))
                .collect::<PyResult<_>>()?,
        )
    }

    Ok(query)
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
/// Unpacking also works.
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
/// >>> s, p, o = solution
/// >>> s
/// <NamedNode value=http://example.com>
#[pyclass(unsendable, name = "QuerySolution", module = "oxigraph")]
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

    fn __getitem__(&self, input: &PyAny) -> PyResult<Option<PyTerm>> {
        if let Ok(key) = usize::extract(input) {
            Ok(self.inner.get(key).map(|term| PyTerm::from(term.clone())))
        } else if let Ok(key) = <&str>::extract(input) {
            Ok(self.inner.get(key).map(|term| PyTerm::from(term.clone())))
        } else if let Ok(key) = input.downcast::<PyCell<PyVariable>>() {
            let key = &*key.borrow();
            Ok(self
                .inner
                .get(<&Variable>::from(key))
                .map(|term| PyTerm::from(term.clone())))
        } else {
            Err(PyTypeError::new_err(format!(
                "{} is not an integer of a string",
                input.get_type().name()?,
            )))
        }
    }
}

#[pyproto]
impl PyIterProtocol for PyQuerySolution {
    fn __iter__(slf: PyRef<Self>) -> SolutionValueIter {
        SolutionValueIter {
            inner: slf
                .inner
                .values()
                .map(|v| v.cloned())
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }
}

#[pyclass(module = "oxigraph")]
pub struct SolutionValueIter {
    inner: IntoIter<Option<Term>>,
}

#[pyproto]
impl PyIterProtocol for SolutionValueIter {
    fn __iter__(slf: PyRefMut<Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(mut slf: PyRefMut<Self>) -> Option<Option<PyTerm>> {
        slf.inner.next().map(|v| v.map(PyTerm::from))
    }
}

/// An iterator of :py:class:`QuerySolution` returned by a SPARQL ``SELECT`` query
///
/// >>> store = SledStore()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
/// >>> list(store.query('SELECT ?s WHERE { ?s ?p ?o }'))
/// [<QuerySolution s=<NamedNode value=http://example.com>>]
#[pyclass(unsendable, name = "QuerySolutions", module = "oxigraph")]
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
#[pyclass(unsendable, name = "QueryTriples", module = "oxigraph")]
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
        EvaluationError::Parsing(error) => PySyntaxError::new_err(error.to_string()),
        EvaluationError::Io(error) => map_io_err(error),
        EvaluationError::Query(error) => PyValueError::new_err(error.to_string()),
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}
