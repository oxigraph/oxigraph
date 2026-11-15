#![expect(clippy::multiple_inherent_impl)]

use crate::model::PyQuad;
use crate::store::map_storage_error;
use oxigraph::model::Quad;
use oxigraph::sparql::Delta;
use oxigraph::store::Store;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

/// An atomic, write-only batch of quad additions and removals.
///
/// Create it using :py:meth:`Store.transaction`. Operations are buffered in memory and
/// applied in order at commit; they are invisible to queries until then. This transaction
/// provides no read snapshot. Use it as a context manager to commit on success and roll
/// back on exceptions. Dropping an uncommitted transaction discards its buffered writes.
#[pyclass(name = "Transaction", module = "pyincroxigraph")]
pub struct PyTransaction {
    store: Store,
    changes: Option<Vec<Delta<Quad>>>,
    entered: bool,
}

impl PyTransaction {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            changes: Some(Vec::new()),
            entered: false,
        }
    }

    fn ensure_open(&self) -> PyResult<()> {
        if self.changes.is_none() {
            return Err(PyRuntimeError::new_err("The transaction is closed"));
        }
        Ok(())
    }
}

#[pymethods]
impl PyTransaction {
    /// Buffers a quad addition.
    /// :param quad: the quad to add.
    /// :type quad: Quad
    /// :rtype: None
    /// :raises RuntimeError: if the transaction is closed.
    fn add(&mut self, quad: PyQuad) -> PyResult<()> {
        self.changes
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("The transaction is closed"))?
            .push(Delta::addition(quad.into()));
        Ok(())
    }

    /// Buffers additions of all provided quads.
    /// :param quads: the quads to add.
    /// :type quads: collections.abc.Iterable[Quad]
    /// :rtype: None
    /// :raises RuntimeError: if the transaction is closed.
    fn extend(&mut self, quads: &Bound<'_, PyAny>) -> PyResult<()> {
        self.ensure_open()?;
        let quads = quads
            .try_iter()?
            .map(|quad| Ok(Delta::addition(quad?.extract::<PyQuad>()?.into())))
            .collect::<PyResult<Vec<_>>>()?;
        if let Some(changes) = &mut self.changes {
            changes.extend(quads);
        }
        Ok(())
    }

    /// Buffers a quad removal.
    /// :param quad: the quad to remove.
    /// :type quad: Quad
    /// :rtype: None
    /// :raises RuntimeError: if the transaction is closed.
    fn remove(&mut self, quad: PyQuad) -> PyResult<()> {
        self.changes
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("The transaction is closed"))?
            .push(Delta::deletion(quad.into()));
        Ok(())
    }

    /// Applies the entire batch atomically and closes the transaction, including on failure.
    /// :rtype: None
    /// :raises RuntimeError: if the transaction is closed.
    /// :raises OSError: if committing the batch fails.
    fn commit(&mut self, py: Python<'_>) -> PyResult<()> {
        let changes = self
            .changes
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("The transaction is closed"))?;
        py.detach(|| {
            let mut transaction = self.store.start_transaction().map_err(map_storage_error)?;
            for change in changes {
                match change {
                    Delta::Addition(quad) => transaction.insert(quad),
                    Delta::Deletion(quad) => transaction.remove(&quad),
                }
            }
            transaction.commit().map_err(map_storage_error)
        })
    }

    /// Discards pending writes and closes the transaction. Repeated rollback is safe.
    /// :rtype: None
    fn rollback(&mut self) {
        self.changes = None;
    }

    /// Whether the transaction has been committed or rolled back.
    /// :rtype: bool
    #[getter]
    fn closed(&self) -> bool {
        self.changes.is_none()
    }

    /// :rtype: Transaction
    fn __enter__(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        slf.ensure_open()?;
        if slf.entered {
            return Err(PyRuntimeError::new_err(
                "The transaction is already entered",
            ));
        }
        slf.entered = true;
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
        py: Python<'_>,
    ) -> PyResult<()> {
        if !exc_type.is_none() {
            self.rollback();
        } else if !self.closed() {
            self.commit(py)?;
        }
        Ok(())
    }
}
