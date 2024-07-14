use crate::model::*;
use crate::utils::to_err;
use crate::{console_warn, format_err};
use js_sys::{Array, Map, Reflect};
use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::*;
use oxigraph::sparql::results::QueryResultsFormat;
use oxigraph::sparql::{Query, QueryResults, Update};
use oxigraph::store::Store;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
export class Store {
    readonly size: number;

    constructor(quads?: Quad[]);

    add(quad: Quad): void;

    delete(quad: Quad): void;

    dump(
        options: {
            format: string;
            from_graph_name?: BlankNode | DefaultGraph | NamedNode;
        }
    ): string;

    has(quad: Quad): boolean;

    load(
        data: string,
        options: {
            base_iri?: NamedNode | string;
            format: string;
            no_transaction?: boolean;
            to_graph_name?: BlankNode | DefaultGraph | NamedNode;
        }
    ): void;

    match(subject?: Term | null, predicate?: Term | null, object?: Term | null, graph?: Term | null): Quad[];

    query(
        query: string,
        options?: {
            base_iri?: NamedNode | string;
            results_format?: string;
            use_default_graph_as_union?: boolean;
        }
    ): boolean | Map<string, Term>[] | Quad[] | string;

    update(
        update: string,
        options?: {
            base_iri?: NamedNode | string;
        }
    ): void;
}
"###;


#[wasm_bindgen(js_name = Store, skip_typescript)]
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
        let mut results_format = None;
        if !options.is_undefined() {
            base_iri = convert_base_iri(&Reflect::get(options, &JsValue::from_str("base_iri"))?)?;
            use_default_graph_as_union =
                Reflect::get(options, &JsValue::from_str("use_default_graph_as_union"))?
                    .is_truthy();
            let js_results_format = Reflect::get(options, &JsValue::from_str("results_format"))?;
            if !js_results_format.is_undefined() && !js_results_format.is_null() {
                results_format = Some(
                    js_results_format
                        .as_string()
                        .ok_or_else(|| to_err("results_format option must be a string"))?,
                );
            }
        }

        let mut query = Query::parse(query, base_iri.as_deref()).map_err(to_err)?;
        if use_default_graph_as_union {
            query.dataset_mut().set_default_graph_as_union();
        }
        let results = self.store.query(query).map_err(to_err)?;

        Ok(match results {
            QueryResults::Solutions(solutions) => {
                if let Some(results_format) = results_format {
                    let results_format = query_results_format(&results_format)?;
                    JsValue::from_str(
                        &String::from_utf8(
                            QueryResults::Solutions(solutions)
                                .write(Vec::new(), results_format)
                                .map_err(to_err)?,
                        )
                        .map_err(to_err)?,
                    )
                } else {
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
            }
            QueryResults::Graph(quads) => {
                if let Some(results_format) = results_format {
                    let rdf_format = rdf_format(&results_format)?;
                    JsValue::from_str(
                        &String::from_utf8(
                            QueryResults::Graph(quads)
                                .write_graph(Vec::new(), rdf_format)
                                .map_err(to_err)?,
                        )
                        .map_err(to_err)?,
                    )
                } else {
                    let results = Array::new();
                    for quad in quads {
                        results.push(
                            &JsQuad::from(quad.map_err(to_err)?.in_graph(GraphName::DefaultGraph))
                                .into(),
                        );
                    }
                    results.into()
                }
            }
            QueryResults::Boolean(b) => {
                if let Some(results_format) = results_format {
                    let results_format = query_results_format(&results_format)?;
                    JsValue::from_str(
                        &String::from_utf8(
                            QueryResults::Boolean(b)
                                .write(Vec::new(), results_format)
                                .map_err(to_err)?,
                        )
                        .map_err(to_err)?,
                    )
                } else {
                    b.into()
                }
            }
        })
    }

    pub fn update(&self, update: &str, options: &JsValue) -> Result<(), JsValue> {
        // Parsing options
        let mut base_iri = None;
        if !options.is_undefined() {
            base_iri = convert_base_iri(&Reflect::get(options, &JsValue::from_str("base_iri"))?)?;
        }

        let update = Update::parse(update, base_iri.as_deref()).map_err(to_err)?;
        self.store.update(update).map_err(to_err)
    }

    pub fn load(
        &self,
        data: &str,
        options: &JsValue,
        base_iri: &JsValue,
        to_graph_name: &JsValue,
    ) -> Result<(), JsValue> {
        // Parsing options
        let mut format = None;
        let mut parsed_base_iri = None;
        let mut parsed_to_graph_name = None;
        let mut unchecked = false;
        let mut no_transaction = false;
        if let Some(format_str) = options.as_string() {
            // Backward compatibility with format as a string
            console_warn!("The format should be passed to Store.load in an option dictionary like store.load(my_content, {{format: 'nt'}})");
            format = Some(rdf_format(&format_str)?);
        } else if !options.is_undefined() && !options.is_null() {
            if let Some(format_str) =
                Reflect::get(options, &JsValue::from_str("format"))?.as_string()
            {
                format = Some(rdf_format(&format_str)?);
            }
            parsed_base_iri =
                convert_base_iri(&Reflect::get(options, &JsValue::from_str("base_iri"))?)?;
            let to_graph_name_js = Reflect::get(options, &JsValue::from_str("to_graph_name"))?;
            parsed_to_graph_name = FROM_JS.with(|c| c.to_optional_term(&to_graph_name_js))?;
            unchecked = Reflect::get(options, &JsValue::from_str("unchecked"))?.is_truthy();
            no_transaction =
                Reflect::get(options, &JsValue::from_str("no_transaction"))?.is_truthy();
        }
        let format = format
            .ok_or_else(|| format_err!("The format option should be provided as a second argument of Store.load like store.load(my_content, {format: 'nt'}"))?;
        if let Some(base_iri) = convert_base_iri(base_iri)? {
            console_warn!("The base_iri should be passed to Store.load in an option dictionary like store.load(my_content, {{format: 'nt', base_iri: 'http//example.com'}})");
            parsed_base_iri = Some(base_iri);
        }
        if let Some(to_graph_name) = FROM_JS.with(|c| c.to_optional_term(to_graph_name))? {
            console_warn!("The target graph name should be passed to Store.load in an option dictionary like store.load(my_content, {{format: 'nt', to_graph_name: 'http//example.com'}})");
            parsed_to_graph_name = Some(to_graph_name);
        }

        let mut parser = RdfParser::from_format(format);
        if let Some(to_graph_name) = parsed_to_graph_name {
            parser = parser.with_default_graph(GraphName::try_from(to_graph_name)?);
        }
        if let Some(base_iri) = parsed_base_iri {
            parser = parser.with_base_iri(base_iri).map_err(to_err)?;
        }
        if unchecked {
            parser = parser.unchecked();
        }
        if no_transaction {
            self.store
                .bulk_loader()
                .load_from_read(parser, data.as_bytes())
        } else {
            self.store.load_from_read(parser, data.as_bytes())
        }
        .map_err(to_err)
    }

    pub fn dump(&self, options: &JsValue, from_graph_name: &JsValue) -> Result<String, JsValue> {
        // Serialization options
        let mut format = None;
        let mut parsed_from_graph_name = None;
        if let Some(format_str) = options.as_string() {
            // Backward compatibility with format as a string
            console_warn!("The format should be passed to Store.dump in an option dictionary like store.dump({{format: 'nt'}})");
            format = Some(rdf_format(&format_str)?);
        } else if !options.is_undefined() && !options.is_null() {
            if let Some(format_str) =
                Reflect::get(options, &JsValue::from_str("format"))?.as_string()
            {
                format = Some(rdf_format(&format_str)?);
            }
            let from_graph_name_js = Reflect::get(options, &JsValue::from_str("from_graph_name"))?;
            parsed_from_graph_name = FROM_JS.with(|c| c.to_optional_term(&from_graph_name_js))?;
        }
        let format = format
            .ok_or_else(|| format_err!("The format option should be provided as a second argument of Store.load like store.dump({format: 'nt'}"))?;
        if let Some(from_graph_name) = FROM_JS.with(|c| c.to_optional_term(from_graph_name))? {
            console_warn!("The source graph name should be passed to Store.dump in an option dictionary like store.dump({{format: 'nt', from_graph_name: 'http//example.com'}})");
            parsed_from_graph_name = Some(from_graph_name);
        }

        let buffer = if let Some(from_graph_name) = parsed_from_graph_name {
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
            .ok_or_else(|| format_err!("Not supported RDF format media type: {}", format))
    } else {
        RdfFormat::from_extension(format)
            .ok_or_else(|| format_err!("Not supported RDF format extension: {}", format))
    }
}

fn query_results_format(format: &str) -> Result<QueryResultsFormat, JsValue> {
    if format.contains('/') {
        QueryResultsFormat::from_media_type(format).ok_or_else(|| {
            format_err!(
                "Not supported SPARQL query results format media type: {}",
                format
            )
        })
    } else {
        QueryResultsFormat::from_extension(format).ok_or_else(|| {
            format_err!(
                "Not supported SPARQL query results format extension: {}",
                format
            )
        })
    }
}

fn convert_base_iri(value: &JsValue) -> Result<Option<String>, JsValue> {
    if value.is_null() || value.is_undefined() {
        Ok(None)
    } else if let Some(value) = value.as_string() {
        Ok(Some(value))
    } else if let JsTerm::NamedNode(value) = FROM_JS.with(|c| c.to_term(value))? {
        Ok(Some(value.value()))
    } else {
        Err(format_err!(
            "If provided, the base IRI must be a NamedNode or a string"
        ))
    }
}
