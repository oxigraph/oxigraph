#![expect(clippy::multiple_inherent_impl)]

use crate::format_err;
use crate::model::{DataFactory, from_term, from_triple};
use crate::store::results_to_js;
use js_sys::{Array, Map, Object, Reflect};
use oxigraph::model::Variable;
use oxigraph::sparql::{
    DeltaKind, IncrementalQueryResults, QueryEvaluationError, QueryResults, QueryResultsDelta,
    QuerySolutionIter, QueryTripleIter, StoreIncrementalQueryDeltasState,
    StoreIncrementalQueryResultsState,
};
use std::io;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// One occurrence added to or removed from a query result.
#[wasm_bindgen(js_name = DeltaKind)]
pub enum JsDeltaKind {
    Addition,
    Deletion,
}

/// A synchronous live query returning complete, owned JavaScript snapshots.
#[wasm_bindgen(js_name = IncrementalQueryResults, skip_typescript)]
pub struct JsIncrementalQueryResults {
    inner: Option<StoreIncrementalQueryResultsState>,
    variables: Option<Vec<String>>,
    data_factory: DataFactory,
    results_format: Option<String>,
}

impl JsIncrementalQueryResults {
    pub(crate) fn new(
        inner: StoreIncrementalQueryResultsState,
        data_factory: DataFactory,
        results_format: Option<String>,
    ) -> Self {
        let variables = inner.variables().map(variable_names);
        Self {
            inner: Some(inner),
            variables,
            data_factory,
            results_format,
        }
    }
}

#[wasm_bindgen(js_class = IncrementalQueryResults)]
impl JsIncrementalQueryResults {
    #[wasm_bindgen(getter)]
    pub fn variables(&self) -> JsValue {
        variables_to_js(self.variables.as_deref())
    }

    /// Processes pending committed changes. The returned value owns its data.
    pub fn results(&mut self) -> Result<JsValue, JsValue> {
        let result = (|| {
            let state = self
                .inner
                .as_mut()
                .ok_or_else(|| format_err!("The query handle is closed"))?;
            let variables = state.variables().map(Arc::<[Variable]>::from);
            let snapshot = match state
                .results()
                .map_err(|error| map_incremental_error(&error))?
            {
                IncrementalQueryResults::Solutions(rows) => {
                    let variables = variables.unwrap_or_default();
                    let rows: Vec<_> = rows
                        .map(|row| (Arc::clone(&variables), row.values().to_vec()).into())
                        .collect();
                    QueryResults::Solutions(QuerySolutionIter::new(
                        variables,
                        rows.into_iter().map(Ok),
                    ))
                }
                IncrementalQueryResults::Boolean(value) => QueryResults::Boolean(value),
                IncrementalQueryResults::Graph(triples) => {
                    let triples: Vec<_> = triples.cloned().collect();
                    QueryResults::Graph(QueryTripleIter::new(triples.into_iter().map(Ok)))
                }
            };
            results_to_js(snapshot, &self.data_factory, self.results_format.as_deref())
        })();
        if result.is_err() {
            self.close();
        }
        result
    }

    #[wasm_bindgen(getter)]
    pub fn closed(&self) -> bool {
        self.inner.is_none()
    }

    /// Releases subscriptions. Repeated calls are safe.
    pub fn close(&mut self) {
        self.inner = None;
    }
}

/// A synchronous live query returning consumed, owned batches of changes.
#[wasm_bindgen(js_name = IncrementalQueryDeltas, skip_typescript)]
pub struct JsIncrementalQueryDeltas {
    inner: Option<StoreIncrementalQueryDeltasState>,
    variables: Option<Vec<String>>,
    data_factory: DataFactory,
}

impl JsIncrementalQueryDeltas {
    pub(crate) fn new(inner: StoreIncrementalQueryDeltasState, data_factory: DataFactory) -> Self {
        let variables = inner.variables().map(variable_names);
        Self {
            inner: Some(inner),
            variables,
            data_factory,
        }
    }
}

#[wasm_bindgen(js_class = IncrementalQueryDeltas)]
impl JsIncrementalQueryDeltas {
    #[wasm_bindgen(getter)]
    pub fn variables(&self) -> JsValue {
        variables_to_js(self.variables.as_deref())
    }

    /// SELECT/CONSTRUCT return Delta records; ASK returns zero or one booleans.
    pub fn deltas(&mut self) -> Result<JsValue, JsValue> {
        let result = (|| {
            let state = self
                .inner
                .as_mut()
                .ok_or_else(|| format_err!("The query handle is closed"))?;
            let batch = Array::new();
            match state
                .deltas()
                .map_err(|error| map_incremental_error(&error))?
            {
                QueryResultsDelta::Solutions(changes) => {
                    for change in changes {
                        let change = change.map_err(|error| map_incremental_error(&error))?;
                        let row = Map::new();
                        for (variable, value) in change.value().iter() {
                            row.set(
                                &variable.as_str().into(),
                                &from_term(&self.data_factory, value),
                            );
                        }
                        batch.push(&delta_record(change.kind(), row.as_ref())?);
                    }
                }
                QueryResultsDelta::Boolean(values) => {
                    for value in values {
                        batch.push(&value.map_err(|error| map_incremental_error(&error))?.into());
                    }
                }
                QueryResultsDelta::Graph(changes) => {
                    for change in changes {
                        let change = change.map_err(|error| map_incremental_error(&error))?;
                        batch.push(&delta_record(
                            change.kind(),
                            &from_triple(&self.data_factory, change.value()),
                        )?);
                    }
                }
            }
            Ok(batch.into())
        })();
        if result.is_err() {
            self.close();
        }
        result
    }

    #[wasm_bindgen(getter)]
    pub fn closed(&self) -> bool {
        self.inner.is_none()
    }

    pub fn close(&mut self) {
        self.inner = None;
    }
}

fn variable_names(variables: &[Variable]) -> Vec<String> {
    variables
        .iter()
        .map(|variable| variable.as_str().to_owned())
        .collect()
}

fn variables_to_js(variables: Option<&[String]>) -> JsValue {
    variables.map_or(JsValue::NULL, |names| {
        names.iter().map(JsValue::from).collect::<Array>().into()
    })
}

fn delta_record(kind: DeltaKind, value: &JsValue) -> Result<JsValue, JsValue> {
    let record = Object::new();
    let kind = match kind {
        DeltaKind::Addition => JsDeltaKind::Addition as u32,
        DeltaKind::Deletion => JsDeltaKind::Deletion as u32,
    };
    Reflect::set(&record, &"kind".into(), &kind.into())?;
    Reflect::set(&record, &"value".into(), value)?;
    Ok(Object::freeze(&record).into())
}

pub(crate) fn map_incremental_error(error: &QueryEvaluationError) -> JsValue {
    let unsupported = matches!(error, QueryEvaluationError::Unexpected(inner)
        if inner.downcast_ref::<io::Error>().is_some_and(|error| error.kind() == io::ErrorKind::Unsupported));
    let result = js_sys::Error::new(&error.to_string());
    if unsupported {
        result.set_name("NotImplementedError");
    }
    result.into()
}
