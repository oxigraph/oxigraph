use crate::format_err;
use crate::model::*;
use crate::utils::to_err;
use js_sys::{Array, Map};
use oxigraph::sparql::{PreparedQuery, QueryOptions, QueryResult};
use oxigraph::{Error, MemoryRepository, Repository, RepositoryConnection};
use std::convert::TryInto;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = MemoryStore)]
#[derive(Default)]
pub struct JsMemoryStore {
    store: MemoryRepository,
    from_js: FromJsConverter,
}

#[wasm_bindgen(js_class = MemoryStore)]
impl JsMemoryStore {
    #[wasm_bindgen(constructor)]
    pub fn new(quads: Option<Box<[JsValue]>>) -> Result<JsMemoryStore, JsValue> {
        console_error_panic_hook::set_once();

        let this = Self::default();
        if let Some(quads) = quads {
            for quad in quads.iter() {
                this.add(quad)?;
            }
        }
        Ok(this)
    }

    #[wasm_bindgen(js_name = dataFactory, getter)]
    pub fn data_factory(&self) -> JsDataFactory {
        JsDataFactory::default()
    }

    pub fn add(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .connection()
            .map_err(to_err)?
            .insert(&self.from_js.to_quad(quad)?.try_into()?)
            .map_err(to_err)
    }

    pub fn delete(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .connection()
            .map_err(to_err)?
            .remove(&self.from_js.to_quad(quad)?.try_into()?)
            .map_err(to_err)
    }

    pub fn has(&self, quad: &JsValue) -> Result<bool, JsValue> {
        self.store
            .connection()
            .map_err(to_err)?
            .contains(&self.from_js.to_quad(quad)?.try_into()?)
            .map_err(to_err)
    }

    #[wasm_bindgen(js_name = match)]
    pub fn match_quads(
        &self,
        subject: &JsValue,
        predicate: &JsValue,
        object: &JsValue,
        graph: &JsValue,
    ) -> Result<Box<[JsValue]>, JsValue> {
        Ok(self
            .store
            .connection()
            .map_err(to_err)?
            .quads_for_pattern(
                match self.from_js.to_optional_term(subject)? {
                    Some(JsTerm::NamedNode(node)) => Some(node.into()),
                    Some(JsTerm::BlankNode(node)) => Some(node.into()),
                    Some(_) => {
                        return Err(format_err!(
                            "The match subject parameter should be a named or a blank node",
                        ))
                    }
                    None => None,
                }.as_ref(),
                match self.from_js.to_optional_term(predicate)? {
                    Some(JsTerm::NamedNode(node)) => Some(node.into()),
                    Some(_) => {
                        return Err(format_err!(
                            "The match predicate parameter should be a named node",
                        ))
                    }
                    None => None,
                }.as_ref(),
                match self.from_js.to_optional_term(object)? {
                    Some(JsTerm::NamedNode(node)) => Some(node.into()),
                    Some(JsTerm::BlankNode(node)) => Some(node.into()),
                    Some(JsTerm::Literal(literal)) => Some(literal.into()),
                    Some(_) => {
                        return Err(format_err!(
                            "The match object parameter should be a named or a blank node or a literal",
                        ))
                    }
                    None => None,
                }.as_ref(),
                match self.from_js.to_optional_term(graph)? {
                    Some(JsTerm::NamedNode(node)) => Some(Some(node.into())),
                    Some(JsTerm::BlankNode(node)) => Some(Some(node.into())),
                    Some(JsTerm::DefaultGraph(_)) => Some(None),
                    Some(_) => {
                        return Err(format_err!(
                            "The match subject parameter should be a named or a blank node or the default graph",
                        ))
                    }
                    None => None,
                }.as_ref().map(|v| v.as_ref()),
            ).map(|v| v.map(|v| JsQuad::from(v).into())).collect::<Result<Vec<_>,Error>>().map_err(to_err)?.into_boxed_slice())
    }

    pub fn query(&self, query: &str) -> Result<JsValue, JsValue> {
        let query = self
            .store
            .connection()
            .map_err(to_err)?
            .prepare_query(query, QueryOptions::default())
            .map_err(to_err)?;
        let results = query.exec().map_err(to_err)?;
        let output = match results {
            QueryResult::Bindings(bindings) => {
                let (variables, iter) = bindings.destruct();
                let variables: Vec<JsValue> = variables
                    .into_iter()
                    .map(|v| v.name().unwrap().into())
                    .collect();
                let results = Array::new();
                for values in iter {
                    let values = values.map_err(to_err)?;
                    let result = Map::new();
                    for (variable, value) in variables.iter().zip(values) {
                        if let Some(value) = value {
                            result.set(variable, &JsTerm::from(value).into());
                        }
                    }
                    results.push(&result.into());
                }
                results.into()
            }
            QueryResult::Graph(quads) => {
                let results = Array::new();
                for quad in quads {
                    results.push(&JsQuad::from(quad.map_err(to_err)?.in_graph(None)).into());
                }
                results.into()
            }
            QueryResult::Boolean(b) => b.into(),
        };
        Ok(output)
    }
}
