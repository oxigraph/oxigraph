#![expect(clippy::multiple_inherent_impl)]

use crate::model::{PyTriple, PyVariable};
use crate::sparql::{
    PyQueryBoolean, PyQuerySolution, map_evaluation_error, query_results_to_python,
};
use oxigraph::model::{Triple, Variable};
use oxigraph::sparql::{
    Delta, DeltaKind, IncrementalQueryResults, QueryEvaluationError, QueryResults,
    QueryResultsDelta, QuerySolution, QuerySolutionIter, QueryTripleIter,
    StoreIncrementalQueryDeltasState, StoreIncrementalQueryResultsState,
};
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyNotImplementedError, PyRuntimeError};
use pyo3::prelude::*;
use std::io;
use std::sync::Arc;
use std::vec::IntoIter;

/// The kind of a result change: :py:attr:`ADDITION` or :py:attr:`DELETION`.
#[pyclass(
    frozen,
    name = "DeltaKind",
    module = "pyincroxigraph",
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct PyDeltaKind {
    addition: bool,
}

#[pymethods]
impl PyDeltaKind {
    /// An added occurrence of a solution, or an added graph triple.
    #[classattr]
    const ADDITION: Self = Self { addition: true };

    /// A removed occurrence of a solution, or a removed graph triple.
    #[classattr]
    const DELETION: Self = Self { addition: false };

    fn __repr__(&self) -> &'static str {
        if self.addition {
            "<DeltaKind.ADDITION>"
        } else {
            "<DeltaKind.DELETION>"
        }
    }
}

impl From<DeltaKind> for PyDeltaKind {
    fn from(kind: DeltaKind) -> Self {
        Self {
            addition: matches!(kind, DeltaKind::Addition),
        }
    }
}

#[derive(Clone, Eq, PartialEq, IntoPyObject)]
enum PyDeltaValue {
    Solution(PyQuerySolution),
    Triple(PyTriple),
}

/// An immutable addition or deletion of a :py:class:`QuerySolution` or :py:class:`Triple`.
///
/// SELECT changes preserve duplicate rows: each change adds or removes one occurrence.
/// CONSTRUCT changes use graph set semantics.
#[pyclass(
    frozen,
    name = "Delta",
    module = "pyincroxigraph",
    eq,
    skip_from_py_object
)]
#[derive(Clone, Eq, PartialEq)]
pub struct PyDelta {
    kind: PyDeltaKind,
    value: PyDeltaValue,
}

#[pymethods]
impl PyDelta {
    /// The change kind.
    /// :rtype: DeltaKind
    #[getter]
    fn kind(&self) -> PyDeltaKind {
        self.kind
    }

    /// The added or removed solution or triple.
    /// :rtype: QuerySolution or Triple
    #[getter]
    fn value(&self) -> PyDeltaValue {
        self.value.clone()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "<Delta kind={} value={}>",
            self.kind.__repr__(),
            self.value.clone().into_bound_py_any(py)?.repr()?
        ))
    }
}

impl From<Delta<QuerySolution>> for PyDelta {
    fn from(delta: Delta<QuerySolution>) -> Self {
        let (kind, value) = delta.into_parts();
        Self {
            kind: kind.into(),
            value: PyDeltaValue::Solution(value.into()),
        }
    }
}

impl From<Delta<Triple>> for PyDelta {
    fn from(delta: Delta<Triple>) -> Self {
        let (kind, value) = delta.into_parts();
        Self {
            kind: kind.into(),
            value: PyDeltaValue::Triple(value.into()),
        }
    }
}

/// An owned, single-pass batch of SELECT changes, yielding :py:class:`Delta` objects.
///
/// A batch remains usable after its query handle is read again or closed. Discarding an
/// unread batch discards its changes for this consumer.
#[pyclass(name = "QuerySolutionDeltas", module = "pyincroxigraph")]
pub struct PyQuerySolutionDeltas {
    variables: Arc<[Variable]>,
    inner: IntoIter<Delta<QuerySolution>>,
}

#[pymethods]
impl PyQuerySolutionDeltas {
    /// The projected variables in projection order, including for an empty batch.
    /// :rtype: list[Variable]
    #[getter]
    fn variables(&self) -> Vec<PyVariable> {
        self.variables.iter().cloned().map(Into::into).collect()
    }

    /// :rtype: QuerySolutionDeltas
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :rtype: Delta
    fn __next__(&mut self) -> Option<PyDelta> {
        self.inner.next().map(Into::into)
    }
}

/// An owned, single-pass batch of CONSTRUCT changes, yielding :py:class:`Delta` objects.
#[pyclass(name = "QueryTripleDeltas", module = "pyincroxigraph")]
pub struct PyQueryTripleDeltas {
    inner: IntoIter<Delta<Triple>>,
}

#[pymethods]
impl PyQueryTripleDeltas {
    /// :rtype: QueryTripleDeltas
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :rtype: Delta
    fn __next__(&mut self) -> Option<PyDelta> {
        self.inner.next().map(Into::into)
    }
}

/// An owned, single-pass batch yielding updated :py:class:`QueryBoolean` values.
///
/// The first batch contains the initial ASK value, including false. Subsequent batches
/// contain a value only if it changed, so each batch contains zero or one values.
#[pyclass(name = "QueryBooleanDeltas", module = "pyincroxigraph")]
pub struct PyQueryBooleanDeltas {
    inner: IntoIter<bool>,
}

#[pymethods]
impl PyQueryBooleanDeltas {
    /// :rtype: QueryBooleanDeltas
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :rtype: QueryBoolean
    fn __next__(&mut self) -> Option<PyQueryBoolean> {
        self.inner.next().map(Into::into)
    }
}

enum OwnedSnapshot {
    Solutions {
        variables: Arc<[Variable]>,
        rows: Vec<QuerySolution>,
    },
    Boolean(bool),
    Graph(Vec<Triple>),
}

impl OwnedSnapshot {
    fn into_results(self) -> QueryResults<'static> {
        match self {
            Self::Solutions { variables, rows } => {
                QueryResults::Solutions(QuerySolutionIter::new(variables, rows.into_iter().map(Ok)))
            }
            Self::Boolean(value) => QueryResults::Boolean(value),
            Self::Graph(triples) => {
                QueryResults::Graph(QueryTripleIter::new(triples.into_iter().map(Ok)))
            }
        }
    }
}

// These private wrappers are used only by Python::detach, which runs its closure
// synchronously on the calling thread. The Python handles remain unsendable.
struct UngilResultsState(StoreIncrementalQueryResultsState);

#[expect(unsafe_code)]
// SAFETY: Access stays on the creating thread, including while Python is detached.
unsafe impl Send for UngilResultsState {}

impl UngilResultsState {
    fn read(&mut self) -> Result<OwnedSnapshot, QueryEvaluationError> {
        let variables = self.0.variables().map(Arc::<[Variable]>::from);
        Ok(match self.0.results()? {
            IncrementalQueryResults::Solutions(rows) => {
                let variables = variables.unwrap_or_default();
                let rows = rows
                    .map(|row| (Arc::clone(&variables), row.values().to_vec()).into())
                    .collect();
                OwnedSnapshot::Solutions { variables, rows }
            }
            IncrementalQueryResults::Boolean(value) => OwnedSnapshot::Boolean(value),
            IncrementalQueryResults::Graph(triples) => {
                OwnedSnapshot::Graph(triples.cloned().collect())
            }
        })
    }
}

/// A live query handle returning complete, owned snapshots.
///
/// Create it using :py:meth:`Store.query_incremental_results`. Use it on its creating
/// thread. Each read processes all currently pending committed changes; several commits
/// may be combined. Result order is unspecified and SELECT duplicates are preserved.
/// Use a context manager or :py:meth:`close` to release its subscriptions promptly.
#[pyclass(
    unsendable,
    name = "IncrementalQueryResults",
    module = "pyincroxigraph"
)]
pub struct PyIncrementalQueryResults {
    inner: Option<UngilResultsState>,
    variables: Option<Vec<Variable>>,
}

impl PyIncrementalQueryResults {
    pub fn new(inner: StoreIncrementalQueryResultsState) -> Self {
        let variables = inner.variables().map(<[_]>::to_vec);
        Self {
            inner: Some(UngilResultsState(inner)),
            variables,
        }
    }
}

#[pymethods]
impl PyIncrementalQueryResults {
    /// The SELECT projection variables, or None for ASK and CONSTRUCT.
    /// :rtype: list[Variable] or None
    #[getter]
    fn variables(&self) -> Option<Vec<PyVariable>> {
        self.variables
            .as_ref()
            .map(|variables| variables.iter().cloned().map(Into::into).collect())
    }

    /// Processes pending changes and captures the complete current answer.
    ///
    /// The returned iterator owns its data and remains usable after another read or close.
    /// Capturing a snapshot copies its rows or triples. An evaluation error closes the handle.
    /// :rtype: QuerySolutions or QueryBoolean or QueryTriples
    /// :raises RuntimeError: if the handle is closed or evaluation fails.
    /// :raises OSError: if reading the store fails.
    /// :raises NotImplementedError: if an incremental operator is unsupported.
    fn results<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.as_mut().ok_or_else(closed_query)?;
        match py.detach(|| inner.read()) {
            Ok(snapshot) => query_results_to_python(py, snapshot.into_results()),
            Err(error) => {
                self.close();
                Err(map_incremental_error(error))
            }
        }
    }

    /// Whether the handle has been closed, including following an evaluation error.
    /// :rtype: bool
    #[getter]
    fn closed(&self) -> bool {
        self.inner.is_none()
    }

    /// Releases the query state and subscriptions. Calling close repeatedly is safe.
    /// :rtype: None
    fn close(&mut self) {
        self.inner = None;
    }

    /// :rtype: IncrementalQueryResults
    fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        if slf.closed() {
            return Err(closed_query());
        }
        Ok(slf)
    }

    /// :type exc_type: typing.Any
    /// :type exc_value: typing.Any
    /// :type traceback: typing.Any
    /// :rtype: None
    #[expect(unused_variables)]
    fn __exit__(
        &mut self,
        exc_type: &Bound<'_, PyAny>,
        exc_value: &Bound<'_, PyAny>,
        traceback: &Bound<'_, PyAny>,
    ) {
        self.close();
    }
}

enum OwnedDeltas {
    Solutions {
        variables: Arc<[Variable]>,
        changes: Vec<Delta<QuerySolution>>,
    },
    Boolean(Vec<bool>),
    Graph(Vec<Delta<Triple>>),
}

impl OwnedDeltas {
    fn into_python(self, py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        match self {
            Self::Solutions { variables, changes } => PyQuerySolutionDeltas {
                variables,
                inner: changes.into_iter(),
            }
            .into_bound_py_any(py),
            Self::Boolean(values) => PyQueryBooleanDeltas {
                inner: values.into_iter(),
            }
            .into_bound_py_any(py),
            Self::Graph(changes) => PyQueryTripleDeltas {
                inner: changes.into_iter(),
            }
            .into_bound_py_any(py),
        }
    }
}

struct UngilDeltasState(StoreIncrementalQueryDeltasState);

#[expect(unsafe_code)]
// SAFETY: Access stays on the creating thread, including while Python is detached.
unsafe impl Send for UngilDeltasState {}

impl UngilDeltasState {
    fn read(&mut self) -> Result<OwnedDeltas, QueryEvaluationError> {
        Ok(match self.0.deltas()? {
            QueryResultsDelta::Solutions(changes) => OwnedDeltas::Solutions {
                variables: changes.variables().into(),
                changes: changes.collect::<Result<_, _>>()?,
            },
            QueryResultsDelta::Boolean(values) => {
                OwnedDeltas::Boolean(values.collect::<Result<_, _>>()?)
            }
            QueryResultsDelta::Graph(changes) => {
                OwnedDeltas::Graph(changes.collect::<Result<_, _>>()?)
            }
        })
    }
}

/// A live query handle returning owned batches of result changes.
///
/// Create it using :py:meth:`Store.query_incremental_deltas`. Initial SELECT rows and
/// CONSTRUCT triples are additions; ASK initially yields its boolean value. Each read
/// consumes pending changes, possibly combining several commits without reporting all
/// intermediate results. Handles have independent cursors and must be used on their
/// creating thread. Subscriptions are in-process state and do not survive reopening a store.
#[pyclass(unsendable, name = "IncrementalQueryDeltas", module = "pyincroxigraph")]
pub struct PyIncrementalQueryDeltas {
    inner: Option<UngilDeltasState>,
    variables: Option<Vec<Variable>>,
}

impl PyIncrementalQueryDeltas {
    pub fn new(inner: StoreIncrementalQueryDeltasState) -> Self {
        let variables = inner.variables().map(<[_]>::to_vec);
        Self {
            inner: Some(UngilDeltasState(inner)),
            variables,
        }
    }
}

#[pymethods]
impl PyIncrementalQueryDeltas {
    /// The SELECT projection variables, or None for ASK and CONSTRUCT.
    /// :rtype: list[Variable] or None
    #[getter]
    fn variables(&self) -> Option<Vec<PyVariable>> {
        self.variables
            .as_ref()
            .map(|variables| variables.iter().cloned().map(Into::into).collect())
    }

    /// Consumes all currently ready changes and returns an owned, single-pass batch.
    ///
    /// Empty batches are returned immediately. An evaluation error closes the handle.
    /// :rtype: QuerySolutionDeltas or QueryBooleanDeltas or QueryTripleDeltas
    /// :raises RuntimeError: if the handle is closed or evaluation fails.
    /// :raises OSError: if reading the store fails.
    /// :raises NotImplementedError: if an incremental operator is unsupported.
    fn deltas<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.as_mut().ok_or_else(closed_query)?;
        match py.detach(|| inner.read()) {
            Ok(changes) => changes.into_python(py),
            Err(error) => {
                self.close();
                Err(map_incremental_error(error))
            }
        }
    }

    /// Whether the handle has been closed, including following an evaluation error.
    /// :rtype: bool
    #[getter]
    fn closed(&self) -> bool {
        self.inner.is_none()
    }

    /// Releases the query state and subscriptions. Calling close repeatedly is safe.
    /// :rtype: None
    fn close(&mut self) {
        self.inner = None;
    }

    /// :rtype: IncrementalQueryDeltas
    fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        if slf.closed() {
            return Err(closed_query());
        }
        Ok(slf)
    }

    /// :type exc_type: typing.Any
    /// :type exc_value: typing.Any
    /// :type traceback: typing.Any
    /// :rtype: None
    #[expect(unused_variables)]
    fn __exit__(
        &mut self,
        exc_type: &Bound<'_, PyAny>,
        exc_value: &Bound<'_, PyAny>,
        traceback: &Bound<'_, PyAny>,
    ) {
        self.close();
    }
}

fn closed_query() -> PyErr {
    PyRuntimeError::new_err("The incremental query handle is closed")
}

pub fn map_incremental_error(error: QueryEvaluationError) -> PyErr {
    match error {
        QueryEvaluationError::Unexpected(error) => match error.downcast::<io::Error>() {
            Ok(error) if error.kind() == io::ErrorKind::Unsupported => {
                PyNotImplementedError::new_err(error.to_string())
            }
            Ok(error) => map_evaluation_error(QueryEvaluationError::Unexpected(error)),
            Err(error) => map_evaluation_error(QueryEvaluationError::Unexpected(error)),
        },
        other => map_evaluation_error(other),
    }
}
