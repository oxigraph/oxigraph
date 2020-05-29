use crate::format_err;
use crate::model::*;
use crate::utils::to_err;
use js_sys::{Array, Map};
use oxigraph::model::NamedOrBlankNode;
use oxigraph::sparql::{QueryOptions, QueryResult};
use oxigraph::{DatasetSyntax, Error, FileSyntax, GraphSyntax, MemoryStore};
use std::convert::TryInto;
use std::io::Cursor;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = MemoryStore)]
#[derive(Default)]
pub struct JsMemoryStore {
    store: MemoryStore,
    from_js: FromJsConverter,
}

#[wasm_bindgen(js_class = MemoryStore)]
impl JsMemoryStore {
    #[wasm_bindgen(constructor)]
    pub fn new(quads: Option<Box<[JsValue]>>) -> Result<JsMemoryStore, JsValue> {
        console_error_panic_hook::set_once();

        let store = Self::default();
        if let Some(quads) = quads {
            for quad in quads.iter() {
                store.add(quad)?;
            }
        }
        Ok(store)
    }

    #[wasm_bindgen(js_name = dataFactory, getter)]
    pub fn data_factory(&self) -> JsDataFactory {
        JsDataFactory::default()
    }

    pub fn add(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .insert(&self.from_js.to_quad(quad)?.try_into()?)
            .map_err(to_err)
    }

    pub fn delete(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .remove(&self.from_js.to_quad(quad)?.try_into()?)
            .map_err(to_err)
    }

    pub fn has(&self, quad: &JsValue) -> Result<bool, JsValue> {
        Ok(self
            .store
            .contains(&self.from_js.to_quad(quad)?.try_into()?))
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
            .prepare_query(query, QueryOptions::default())
            .map_err(to_err)?;
        let results = query.exec().map_err(to_err)?;
        let output = match results {
            QueryResult::Bindings(solutions) => {
                let results = Array::new();
                for solution in solutions {
                    let solution = solution.map_err(to_err)?;
                    let result = Map::new();
                    for (variable, value) in solution.iter() {
                        result.set(
                            &variable.as_str().into(),
                            &JsTerm::from(value.clone()).into(),
                        );
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

    pub fn load(
        &self,
        data: &str,
        mime_type: &str,
        base_iri: &JsValue,
        to_graph_name: &JsValue,
    ) -> Result<(), JsValue> {
        let base_iri = if base_iri.is_null() || base_iri.is_undefined() {
            None
        } else if base_iri.is_string() {
            base_iri.as_string()
        } else if let JsTerm::NamedNode(base_iri) = self.from_js.to_term(&base_iri)? {
            Some(base_iri.value())
        } else {
            return Err(format_err!(
                "If provided, the base IRI should be a NamedNode or a string"
            ));
        };

        let to_graph_name: Option<NamedOrBlankNode> =
            match self.from_js.to_optional_term(to_graph_name)? {
                Some(JsTerm::NamedNode(node)) => Some(node.into()),
                Some(JsTerm::BlankNode(node)) => Some(node.into()),
                Some(JsTerm::DefaultGraph(_)) => None,
                Some(_) => {
                    return Err(format_err!(
                        "If provided, the target graph name should be a NamedNode or a BlankNode"
                    ))
                }
                None => None,
            };

        if let Some(graph_syntax) = GraphSyntax::from_mime_type(mime_type) {
            self.store
                .load_graph(
                    Cursor::new(data),
                    graph_syntax,
                    to_graph_name.as_ref(),
                    base_iri.as_deref(),
                )
                .map_err(to_err)
        } else if let Some(dataset_syntax) = DatasetSyntax::from_mime_type(mime_type) {
            if to_graph_name.is_some() {
                return Err(format_err!(
                    "The target graph name parameter is not available for dataset formats"
                ));
            }
            self.store
                .load_dataset(Cursor::new(data), dataset_syntax, base_iri.as_deref())
                .map_err(to_err)
        } else {
            Err(format_err!("Not supported MIME type: {}", mime_type))
        }
    }
}
