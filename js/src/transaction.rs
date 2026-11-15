#![expect(clippy::multiple_inherent_impl)]

use crate::format_err;
use crate::model::to_quad;
use js_sys::try_iter;
use oxigraph::model::Quad;
use oxigraph::sparql::Delta;
use oxigraph::store::Store;
use wasm_bindgen::prelude::*;

/// A buffered, atomic, write-only batch. Dropping it discards uncommitted writes.
#[wasm_bindgen(js_name = Transaction, skip_typescript)]
pub struct JsTransaction {
    store: Store,
    changes: Option<Vec<Delta<Quad>>>,
}

impl JsTransaction {
    pub(crate) fn new(store: Store) -> Self {
        Self {
            store,
            changes: Some(Vec::new()),
        }
    }

    fn changes(&mut self) -> Result<&mut Vec<Delta<Quad>>, JsValue> {
        self.changes
            .as_mut()
            .ok_or_else(|| format_err!("The transaction is closed"))
    }
}

#[wasm_bindgen(js_class = Transaction)]
impl JsTransaction {
    pub fn add(&mut self, quad: &JsValue) -> Result<(), JsValue> {
        self.changes()?.push(Delta::addition(to_quad(quad)?));
        Ok(())
    }

    pub fn delete(&mut self, quad: &JsValue) -> Result<(), JsValue> {
        self.changes()?.push(Delta::deletion(to_quad(quad)?));
        Ok(())
    }

    pub fn extend(&mut self, quads: &JsValue) -> Result<(), JsValue> {
        self.changes()?;
        let quads = collect_quads(quads)?;
        self.changes()?
            .extend(quads.into_iter().map(Delta::addition));
        Ok(())
    }

    /// Applies all operations atomically and closes the transaction, including on failure.
    pub fn commit(&mut self) -> Result<(), JsValue> {
        let changes = self
            .changes
            .take()
            .ok_or_else(|| format_err!("The transaction is closed"))?;
        let mut transaction = self.store.start_transaction().map_err(JsError::from)?;
        for change in changes {
            match change {
                Delta::Addition(quad) => transaction.insert(quad),
                Delta::Deletion(quad) => transaction.remove(&quad),
            }
        }
        transaction.commit().map_err(JsError::from)?;
        Ok(())
    }

    pub fn rollback(&mut self) {
        self.changes = None;
    }

    #[wasm_bindgen(getter)]
    pub fn closed(&self) -> bool {
        self.changes.is_none()
    }
}

pub(crate) fn collect_quads(quads: &JsValue) -> Result<Vec<Quad>, JsValue> {
    try_iter(quads)?
        .ok_or_else(|| js_sys::TypeError::new("quads must be iterable"))?
        .map(|quad| to_quad(&quad?))
        .collect()
}
