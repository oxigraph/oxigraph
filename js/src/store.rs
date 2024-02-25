use crate::format_err;
use crate::model::*;
use crate::utils::to_err;
use js_sys::{Array, Map, Reflect};
use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::*;
use oxigraph::sparql::{Query, QueryResults, Update};
use oxigraph::store::Store;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = Store)]
pub struct JsStore {
    store: Store,
}

#[wasm_bindgen(js_class = Store)]
impl JsStore {
    #[wasm_bindgen(constructor)]
    #[allow(clippy::use_self)]
    pub fn new(quads: Option<Box<[JsValue]>>) -> Result<JsStore, JsValue> {
        console_error_panic_hook::set_once();

        let store = Self {
            store: Store::new().map_err(to_err)?,
        };
        if let Some(quads) = quads {
            for quad in &*quads {
                store.add(quad)?;
            }
        }
        Ok(store)
    }

    pub fn add(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .insert(&FROM_JS.with(|c| c.to_quad(quad))?)
            .map_err(to_err)?;
        Ok(())
    }

    pub fn delete(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .remove(&FROM_JS.with(|c| c.to_quad(quad))?)
            .map_err(to_err)?;
        Ok(())
    }

    pub fn has(&self, quad: &JsValue) -> Result<bool, JsValue> {
        self.store
            .contains(&FROM_JS.with(|c| c.to_quad(quad))?)
            .map_err(to_err)
    }

    #[wasm_bindgen(getter=size)]
    pub fn size(&self) -> Result<usize, JsValue> {
        self.store.len().map_err(to_err)
    }

    #[wasm_bindgen(js_name = match)]
    pub fn match_quads(
        &self,
        subject: &JsValue,
        predicate: &JsValue,
        object: &JsValue,
        graph_name: &JsValue,
    ) -> Result<Box<[JsValue]>, JsValue> {
        Ok(self
            .store
            .quads_for_pattern(
                if let Some(subject) = FROM_JS.with(|c| c.to_optional_term(subject))? {
                    Some(subject.try_into()?)
                } else {
                    None
                }
                .as_ref()
                .map(<&Subject>::into),
                if let Some(predicate) = FROM_JS.with(|c| c.to_optional_term(predicate))? {
                    Some(NamedNode::try_from(predicate)?)
                } else {
                    None
                }
                .as_ref()
                .map(<&NamedNode>::into),
                if let Some(object) = FROM_JS.with(|c| c.to_optional_term(object))? {
                    Some(object.try_into()?)
                } else {
                    None
                }
                .as_ref()
                .map(<&Term>::into),
                if let Some(graph_name) = FROM_JS.with(|c| c.to_optional_term(graph_name))? {
                    Some(graph_name.try_into()?)
                } else {
                    None
                }
                .as_ref()
                .map(<&GraphName>::into),
            )
            .map(|v| v.map(|v| JsQuad::from(v).into()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(to_err)?
            .into_boxed_slice())
    }

    pub fn query(&self, query: &str, options: &JsValue) -> Result<JsValue, JsValue> {
        // Parsing options
        let mut base_iri = None;
        let mut use_default_graph_as_union = false;
        if !options.is_undefined() {
            base_iri = Reflect::get(options, &JsValue::from_str("base_iri"))?.as_string();
            use_default_graph_as_union =
                Reflect::get(options, &JsValue::from_str("use_default_graph_as_union"))?
                    .is_truthy();
        }

        let mut query = Query::parse(query, base_iri.as_deref()).map_err(to_err)?;
        if use_default_graph_as_union {
            query.dataset_mut().set_default_graph_as_union();
        }
        let results = self.store.query(query).map_err(to_err)?;
        let output = match results {
            QueryResults::Solutions(solutions) => {
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
            QueryResults::Graph(quads) => {
                let results = Array::new();
                for quad in quads {
                    results.push(
                        &JsQuad::from(quad.map_err(to_err)?.in_graph(GraphName::DefaultGraph))
                            .into(),
                    );
                }
                results.into()
            }
            QueryResults::Boolean(b) => b.into(),
        };
        Ok(output)
    }

    pub fn update(&self, update: &str, options: &JsValue) -> Result<(), JsValue> {
        // Parsing options
        let mut base_iri = None;
        if !options.is_undefined() {
            base_iri = Reflect::get(options, &JsValue::from_str("base_iri"))?.as_string();
        }

        let update = Update::parse(update, base_iri.as_deref()).map_err(to_err)?;
        self.store.update(update).map_err(to_err)
    }

    pub fn load(
        &self,
        data: &str,
        format: &str,
        base_iri: &JsValue,
        to_graph_name: &JsValue,
    ) -> Result<(), JsValue> {
        let format = rdf_format(format)?;
        let base_iri = if base_iri.is_null() || base_iri.is_undefined() {
            None
        } else if base_iri.is_string() {
            base_iri.as_string()
        } else if let JsTerm::NamedNode(base_iri) = FROM_JS.with(|c| c.to_term(base_iri))? {
            Some(base_iri.value())
        } else {
            return Err(format_err!(
                "If provided, the base IRI should be a NamedNode or a string"
            ));
        };

        let mut parser = RdfParser::from_format(format);
        if let Some(to_graph_name) = FROM_JS.with(|c| c.to_optional_term(to_graph_name))? {
            parser = parser.with_default_graph(GraphName::try_from(to_graph_name)?);
        }
        if let Some(base_iri) = base_iri {
            parser = parser.with_base_iri(base_iri).map_err(to_err)?;
        }
        self.store
            .load_from_read(parser, data.as_bytes())
            .map_err(to_err)
    }

    pub fn dump(&self, format: &str, from_graph_name: &JsValue) -> Result<String, JsValue> {
        let format = rdf_format(format)?;
        let buffer =
            if let Some(from_graph_name) = FROM_JS.with(|c| c.to_optional_term(from_graph_name))? {
                self.store.dump_graph_to_write(
                    &GraphName::try_from(from_graph_name)?,
                    format,
                    Vec::new(),
                )
            } else {
                self.store.dump_to_write(format, Vec::new())
            }
            .map_err(to_err)?;
        String::from_utf8(buffer).map_err(to_err)
    }
}

fn rdf_format(format: &str) -> Result<RdfFormat, JsValue> {
    if format.contains('/') {
        RdfFormat::from_media_type(format)
            .ok_or_else(|| format_err!("Not supported RDF format media type: {format}"))
    } else {
        RdfFormat::from_extension(format)
            .ok_or_else(|| format_err!("Not supported RDF format extension: {format}"))
    }
}
