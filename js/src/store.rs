use crate::model::*;
use crate::{console_warn, format_err};
use js_sys::{Array, Function, Map, Promise, Reflect, try_iter};
use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};
use oxigraph::model::*;
use oxigraph::sparql::results::{QueryResultsFormat, QueryResultsSerializer};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
#[cfg(feature = "geosparql")]
use spargeo::GEOSPARQL_EXTENSION_FUNCTIONS;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

// We skip_typescript on specific wasm_bindgen macros and provide custom TypeScript types for parts of this module in order to have narrower types
// instead of any and improve compatibility with RDF/JS Dataset interfaces (https://rdf.js.org/dataset-spec/).
//
// The Store type overlay hides deprecated parameters on methods like dump.
#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
export class Store {
    readonly size: number;
    readonly length: number;

    constructor(quads?: Iterable<Quad>);

    add(quad: Quad): void;

    delete(quad: Quad): void;

    dump(
        options: {
            format: string;
            fromGraphName?: BlankNode | DefaultGraph | NamedNode;
            prefixes?: Record<string, string>;
            baseIri?: NamedNode | string;
        }
    ): string;

    has(quad: Quad): boolean;

    load(
        data: string,
        options: {
            baseIri?: NamedNode | string;
            format: string;
            noTransaction?: boolean;
            toGraphName?: BlankNode | DefaultGraph | NamedNode;
            unchecked?: boolean;
            lenient?: boolean;
        }
    ): void;

    match(subject?: Term | null, predicate?: Term | null, object?: Term | null, graph?: Term | null): Quad[];

    query(
        query: string,
        options?: {
            baseIri?: NamedNode | string;
            prefixes?: Record<string, string>;
            resultsFormat?: string;
            defaultGraph?: BlankNode | DefaultGraph | NamedNode | Iterable<BlankNode | DefaultGraph | NamedNode>;
            namedGraphs?: Iterable<BlankNode | NamedNode>;
            useDefaultGraphAsUnion?: boolean;
            substitutions?: Record<string, Term>;
        }
    ): boolean | Map<string, Term>[] | Quad[] | string;

    queryAsync(
        query: string,
        options?: {
            baseIri?: NamedNode | string;
            prefixes?: Record<string, string>;
            resultsFormat?: string;
            defaultGraph?: BlankNode | DefaultGraph | NamedNode | Iterable<BlankNode | DefaultGraph | NamedNode>;
            namedGraphs?: Iterable<BlankNode | NamedNode>;
            useDefaultGraphAsUnion?: boolean;
            substitutions?: Record<string, Term>;
        }
    ): Promise<boolean | Map<string, Term>[] | Quad[] | string>;

    update(
        update: string,
        options?: {
            baseIri?: NamedNode | string;
            prefixes?: Record<string, string>;
        }
    ): void;

    updateAsync(
        update: string,
        options?: {
            baseIri?: NamedNode | string;
            prefixes?: Record<string, string>;
        }
    ): Promise<void>;

    extend(quads: Iterable<Quad>): void;

    bulkLoad(
        data: string,
        options: {
            baseIri?: NamedNode | string;
            format: string;
            toGraphName?: BlankNode | DefaultGraph | NamedNode;
            lenient?: boolean;
        }
    ): void;

    namedGraphs(): (BlankNode | NamedNode)[];

    containsNamedGraph(graph_name: BlankNode | DefaultGraph | NamedNode): boolean;

    addGraph(graph_name: BlankNode | DefaultGraph | NamedNode): void;

    clearGraph(graph_name: BlankNode | DefaultGraph | NamedNode): void;

    removeGraph(graph_name: BlankNode | DefaultGraph | NamedNode): void;

    clear(): void;

    forEach(callback: (quad: Quad) => void): void;

    filter(predicate: (quad: Quad) => boolean): Quad[];

    some(predicate: (quad: Quad) => boolean): boolean;

    every(predicate: (quad: Quad) => boolean): boolean;

    find(predicate: (quad: Quad) => boolean): Quad | undefined;

    [Symbol.iterator](): Iterator<Quad>;
}
"###;

#[wasm_bindgen(js_name = Store, skip_typescript)]
pub struct JsStore {
    store: Store,
}

#[wasm_bindgen(js_class = Store)]
impl JsStore {
    #[wasm_bindgen(constructor)]
    pub fn new(quads: &JsValue) -> Result<JsStore, JsValue> {
        console_error_panic_hook::set_once();

        let store = Self {
            store: Store::new().map_err(JsError::from)?,
        };
        if !quads.is_undefined() && !quads.is_null() {
            if let Some(quads) = try_iter(quads)? {
                for quad in quads {
                    store.add(&quad?)?;
                }
            }
        }
        Ok(store)
    }

    pub fn add(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .insert(&FROM_JS.with(|c| c.to_quad(quad))?)
            .map_err(JsError::from)?;
        Ok(())
    }

    pub fn delete(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .remove(&FROM_JS.with(|c| c.to_quad(quad))?)
            .map_err(JsError::from)?;
        Ok(())
    }

    pub fn has(&self, quad: &JsValue) -> Result<bool, JsValue> {
        Ok(self
            .store
            .contains(&FROM_JS.with(|c| c.to_quad(quad))?)
            .map_err(JsError::from)?)
    }

    #[wasm_bindgen(getter=size)]
    pub fn size(&self) -> Result<usize, JsError> {
        Ok(self.store.len()?)
    }

    #[wasm_bindgen(getter)]
    pub fn length(&self) -> Result<usize, JsError> {
        Ok(self.store.len()?)
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
                .map(<&NamedOrBlankNode>::into),
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
            .map_err(JsError::from)?
            .into_boxed_slice())
    }

    pub fn query(&self, query: &str, options: &JsValue) -> Result<JsValue, JsValue> {
        // Parsing options
        let mut base_iri = None;
        let mut prefixes = None;
        let mut use_default_graph_as_union = false;
        let mut results_format = None;
        let mut default_graph = None;
        let mut named_graphs = None;
        let mut substitutions = None;
        if !options.is_undefined() {
            base_iri = convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;

            let js_prefixes = Reflect::get(options, &JsValue::from_str("prefixes"))?;
            if !js_prefixes.is_undefined() && !js_prefixes.is_null() {
                prefixes = Some(extract_prefixes(&js_prefixes)?);
            }

            let js_default_graph = Reflect::get(options, &JsValue::from_str("defaultGraph"))?;
            default_graph = if js_default_graph.is_undefined() || js_default_graph.is_null() {
                None
            } else if let Some(iter) = try_iter(&js_default_graph)? {
                Some(
                    iter.map(|term| FROM_JS.with(|c| c.to_term(&term?))?.try_into())
                        .collect::<Result<Vec<GraphName>, _>>()?,
                )
            } else {
                Some(vec![
                    FROM_JS.with(|c| c.to_term(&js_default_graph))?.try_into()?,
                ])
            };

            let js_named_graphs = Reflect::get(options, &JsValue::from_str("namedGraphs"))?;
            named_graphs = if js_named_graphs.is_null() || js_named_graphs.is_undefined() {
                None
            } else {
                Some(
                    try_iter(&Reflect::get(options, &JsValue::from_str("namedGraphs"))?)?
                        .ok_or_else(|| format_err!("namedGraphs option must be iterable"))?
                        .map(|term| FROM_JS.with(|c| c.to_term(&term?))?.try_into())
                        .collect::<Result<Vec<NamedOrBlankNode>, _>>()?,
                )
            };

            use_default_graph_as_union =
                Reflect::get(options, &JsValue::from_str("useDefaultGraphAsUnion"))?
                    .is_truthy();

            let js_results_format = Reflect::get(options, &JsValue::from_str("resultsFormat"))?;
            if !js_results_format.is_undefined() && !js_results_format.is_null() {
                results_format = Some(
                    js_results_format
                        .as_string()
                        .ok_or_else(|| format_err!("resultsFormat option must be a string"))?,
                );
            }

            let js_substitutions = Reflect::get(options, &JsValue::from_str("substitutions"))?;
            if !js_substitutions.is_undefined() && !js_substitutions.is_null() {
                substitutions = Some(extract_substitutions(&js_substitutions)?);
            }
        }

        let mut evaluator = SparqlEvaluator::new();
        #[cfg(feature = "geosparql")]
        for (name, implementation) in GEOSPARQL_EXTENSION_FUNCTIONS {
            evaluator = evaluator.with_custom_function(name.into(), implementation)
        }
        if let Some(base_iri) = base_iri {
            evaluator = evaluator.with_base_iri(base_iri).map_err(JsError::from)?;
        }
        if let Some(prefixes) = prefixes {
            for (prefix_name, prefix_iri) in prefixes {
                evaluator = evaluator
                    .with_prefix(&prefix_name, &prefix_iri)
                    .map_err(JsError::from)?;
            }
        }

        let mut prepared_query = evaluator.parse_query(&query).map_err(JsError::from)?;
        if use_default_graph_as_union {
            prepared_query.dataset_mut().set_default_graph_as_union();
        }
        if let Some(default_graph) = default_graph {
            prepared_query
                .dataset_mut()
                .set_default_graph(default_graph);
        }
        if let Some(named_graphs) = named_graphs {
            prepared_query
                .dataset_mut()
                .set_available_named_graphs(named_graphs);
        }
        if let Some(substitutions) = substitutions {
            for (variable, term) in substitutions {
                prepared_query = prepared_query.substitute_variable(variable, term);
            }
        }

        let results = prepared_query
            .on_store(&self.store)
            .execute()
            .map_err(JsError::from)?;
        Ok(match results {
            QueryResults::Solutions(solutions) => {
                if let Some(results_format) = results_format {
                    let mut serializer =
                        QueryResultsSerializer::from_format(query_results_format(&results_format)?)
                            .serialize_solutions_to_writer(Vec::new(), solutions.variables().into())
                            .map_err(JsError::from)?;
                    for solution in solutions {
                        serializer
                            .serialize(&solution.map_err(JsError::from)?)
                            .map_err(JsError::from)?;
                    }
                    JsValue::from_str(
                        &String::from_utf8(serializer.finish().map_err(JsError::from)?)
                            .map_err(JsError::from)?,
                    )
                } else {
                    let results = Array::new();
                    for solution in solutions {
                        let solution = solution.map_err(JsError::from)?;
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
            QueryResults::Graph(triples) => {
                if let Some(results_format) = results_format {
                    let mut serializer = RdfSerializer::from_format(rdf_format(&results_format)?)
                        .for_writer(Vec::new());
                    for triple in triples {
                        serializer
                            .serialize_triple(&triple.map_err(JsError::from)?)
                            .map_err(JsError::from)?;
                    }
                    JsValue::from_str(
                        &String::from_utf8(serializer.finish().map_err(JsError::from)?)
                            .map_err(JsError::from)?,
                    )
                } else {
                    let results = Array::new();
                    for triple in triples {
                        results.push(
                            &JsQuad::from(
                                triple
                                    .map_err(JsError::from)?
                                    .in_graph(GraphName::DefaultGraph),
                            )
                            .into(),
                        );
                    }
                    results.into()
                }
            }
            QueryResults::Boolean(b) => {
                if let Some(results_format) = results_format {
                    JsValue::from_str(
                        &String::from_utf8(
                            QueryResultsSerializer::from_format(query_results_format(
                                &results_format,
                            )?)
                            .serialize_boolean_to_writer(Vec::new(), b)
                            .map_err(JsError::from)?,
                        )
                        .map_err(JsError::from)?,
                    )
                } else {
                    b.into()
                }
            }
        })
    }

    #[wasm_bindgen(js_name = queryAsync)]
    pub fn query_async(&self, query: String, options: JsValue) -> Promise {
        let store = self.store.clone();
        future_to_promise(async move {
            // Parsing options
            let mut base_iri = None;
            let mut prefixes = None;
            let mut use_default_graph_as_union = false;
            let mut results_format = None;
            let mut default_graph = None;
            let mut named_graphs = None;
            let mut substitutions = None;
            if !options.is_undefined() {
                base_iri = convert_base_iri(&Reflect::get(&options, &JsValue::from_str("baseIri"))?)?;

                let js_prefixes = Reflect::get(&options, &JsValue::from_str("prefixes"))?;
                if !js_prefixes.is_undefined() && !js_prefixes.is_null() {
                    prefixes = Some(extract_prefixes(&js_prefixes)?);
                }

                let js_default_graph = Reflect::get(&options, &JsValue::from_str("defaultGraph"))?;
                default_graph = if js_default_graph.is_undefined() || js_default_graph.is_null() {
                    None
                } else if let Some(iter) = try_iter(&js_default_graph)? {
                    Some(
                        iter.map(|term| FROM_JS.with(|c| c.to_term(&term?))?.try_into())
                            .collect::<Result<Vec<GraphName>, _>>()?,
                    )
                } else {
                    Some(vec![
                        FROM_JS.with(|c| c.to_term(&js_default_graph))?.try_into()?,
                    ])
                };

                let js_named_graphs = Reflect::get(&options, &JsValue::from_str("namedGraphs"))?;
                named_graphs = if js_named_graphs.is_null() || js_named_graphs.is_undefined() {
                    None
                } else {
                    Some(
                        try_iter(&Reflect::get(&options, &JsValue::from_str("namedGraphs"))?)?
                            .ok_or_else(|| format_err!("namedGraphs option must be iterable"))?
                            .map(|term| FROM_JS.with(|c| c.to_term(&term?))?.try_into())
                            .collect::<Result<Vec<NamedOrBlankNode>, _>>()?,
                    )
                };

                use_default_graph_as_union =
                    Reflect::get(&options, &JsValue::from_str("useDefaultGraphAsUnion"))?
                        .is_truthy();

                let js_results_format = Reflect::get(&options, &JsValue::from_str("resultsFormat"))?;
                if !js_results_format.is_undefined() && !js_results_format.is_null() {
                    results_format = Some(
                        js_results_format
                            .as_string()
                            .ok_or_else(|| format_err!("resultsFormat option must be a string"))?,
                    );
                }

                let js_substitutions = Reflect::get(&options, &JsValue::from_str("substitutions"))?;
                if !js_substitutions.is_undefined() && !js_substitutions.is_null() {
                    substitutions = Some(extract_substitutions(&js_substitutions)?);
                }
            }

            let mut evaluator = SparqlEvaluator::new();
            #[cfg(feature = "geosparql")]
            for (name, implementation) in GEOSPARQL_EXTENSION_FUNCTIONS {
                evaluator = evaluator.with_custom_function(name.into(), implementation)
            }
            if let Some(base_iri) = base_iri {
                evaluator = evaluator.with_base_iri(base_iri).map_err(JsError::from)?;
            }
            if let Some(prefixes) = prefixes {
                for (prefix_name, prefix_iri) in prefixes {
                    evaluator = evaluator
                        .with_prefix(&prefix_name, &prefix_iri)
                        .map_err(JsError::from)?;
                }
            }

            let mut prepared_query = evaluator.parse_query(&query).map_err(JsError::from)?;
            if use_default_graph_as_union {
                prepared_query.dataset_mut().set_default_graph_as_union();
            }
            if let Some(default_graph) = default_graph {
                prepared_query
                    .dataset_mut()
                    .set_default_graph(default_graph);
            }
            if let Some(named_graphs) = named_graphs {
                prepared_query
                    .dataset_mut()
                    .set_available_named_graphs(named_graphs);
            }
            if let Some(substitutions) = substitutions {
                for (variable, term) in substitutions {
                    prepared_query = prepared_query.substitute_variable(variable, term);
                }
            }

            let results = prepared_query
                .on_store(&store)
                .execute()
                .map_err(JsError::from)?;
            Ok(match results {
                QueryResults::Solutions(solutions) => {
                    if let Some(results_format) = results_format {
                        let mut serializer =
                            QueryResultsSerializer::from_format(query_results_format(&results_format)?)
                                .serialize_solutions_to_writer(Vec::new(), solutions.variables().into())
                                .map_err(JsError::from)?;
                        for solution in solutions {
                            serializer
                                .serialize(&solution.map_err(JsError::from)?)
                                .map_err(JsError::from)?;
                        }
                        JsValue::from_str(
                            &String::from_utf8(serializer.finish().map_err(JsError::from)?)
                                .map_err(JsError::from)?,
                        )
                    } else {
                        let results = Array::new();
                        let mut count = 0;
                        for solution in solutions {
                            let solution = solution.map_err(JsError::from)?;
                            let result = Map::new();
                            for (variable, value) in solution.iter() {
                                result.set(
                                    &variable.as_str().into(),
                                    &JsTerm::from(value.clone()).into(),
                                );
                            }
                            results.push(&result.into());

                            // Yield to the event loop every 1000 solutions to keep the UI responsive
                            count += 1;
                            if count % 1000 == 0 {
                                let promise = Promise::resolve(&JsValue::undefined());
                                wasm_bindgen_futures::JsFuture::from(promise).await?;
                            }
                        }
                        results.into()
                    }
                }
                QueryResults::Graph(triples) => {
                    if let Some(results_format) = results_format {
                        let mut serializer = RdfSerializer::from_format(rdf_format(&results_format)?)
                            .for_writer(Vec::new());
                        for triple in triples {
                            serializer
                                .serialize_triple(&triple.map_err(JsError::from)?)
                                .map_err(JsError::from)?;
                        }
                        JsValue::from_str(
                            &String::from_utf8(serializer.finish().map_err(JsError::from)?)
                                .map_err(JsError::from)?,
                        )
                    } else {
                        let results = Array::new();
                        let mut count = 0;
                        for triple in triples {
                            results.push(
                                &JsQuad::from(
                                    triple
                                        .map_err(JsError::from)?
                                        .in_graph(GraphName::DefaultGraph),
                                )
                                .into(),
                            );

                            // Yield to the event loop every 1000 triples to keep the UI responsive
                            count += 1;
                            if count % 1000 == 0 {
                                let promise = Promise::resolve(&JsValue::undefined());
                                wasm_bindgen_futures::JsFuture::from(promise).await?;
                            }
                        }
                        results.into()
                    }
                }
                QueryResults::Boolean(b) => {
                    if let Some(results_format) = results_format {
                        JsValue::from_str(
                            &String::from_utf8(
                                QueryResultsSerializer::from_format(query_results_format(
                                    &results_format,
                                )?)
                                .serialize_boolean_to_writer(Vec::new(), b)
                                .map_err(JsError::from)?,
                            )
                            .map_err(JsError::from)?,
                        )
                    } else {
                        b.into()
                    }
                }
            })
        })
    }

    pub fn update(&self, update: &str, options: &JsValue) -> Result<(), JsValue> {
        // Parsing options
        let mut base_iri = None;
        let mut prefixes = None;
        if !options.is_undefined() {
            base_iri = convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;

            let js_prefixes = Reflect::get(options, &JsValue::from_str("prefixes"))?;
            if !js_prefixes.is_undefined() && !js_prefixes.is_null() {
                prefixes = Some(extract_prefixes(&js_prefixes)?);
            }
        }

        let mut evaluator = SparqlEvaluator::new();
        #[cfg(feature = "geosparql")]
        for (name, implementation) in GEOSPARQL_EXTENSION_FUNCTIONS {
            evaluator = evaluator.with_custom_function(name.into(), implementation)
        }
        if let Some(base_iri) = base_iri {
            evaluator = evaluator.with_base_iri(base_iri).map_err(JsError::from)?;
        }
        if let Some(prefixes) = prefixes {
            for (prefix_name, prefix_iri) in prefixes {
                evaluator = evaluator
                    .with_prefix(&prefix_name, &prefix_iri)
                    .map_err(JsError::from)?;
            }
        }

        Ok(evaluator
            .parse_update(update)
            .map_err(JsError::from)?
            .on_store(&self.store)
            .execute()
            .map_err(JsError::from)?)
    }

    #[wasm_bindgen(js_name = updateAsync)]
    pub fn update_async(&self, update: String, options: JsValue) -> Promise {
        let store = self.store.clone();
        future_to_promise(async move {
            // Parsing options
            let mut base_iri = None;
            let mut prefixes = None;
            if !options.is_undefined() {
                base_iri = convert_base_iri(&Reflect::get(&options, &JsValue::from_str("baseIri"))?)?;

                let js_prefixes = Reflect::get(&options, &JsValue::from_str("prefixes"))?;
                if !js_prefixes.is_undefined() && !js_prefixes.is_null() {
                    prefixes = Some(extract_prefixes(&js_prefixes)?);
                }
            }

            let mut evaluator = SparqlEvaluator::new();
            #[cfg(feature = "geosparql")]
            for (name, implementation) in GEOSPARQL_EXTENSION_FUNCTIONS {
                evaluator = evaluator.with_custom_function(name.into(), implementation)
            }
            if let Some(base_iri) = base_iri {
                evaluator = evaluator.with_base_iri(base_iri).map_err(JsError::from)?;
            }
            if let Some(prefixes) = prefixes {
                for (prefix_name, prefix_iri) in prefixes {
                    evaluator = evaluator
                        .with_prefix(&prefix_name, &prefix_iri)
                        .map_err(JsError::from)?;
                }
            }

            evaluator
                .parse_update(&update)
                .map_err(JsError::from)?
                .on_store(&store)
                .execute()
                .map_err(JsError::from)?;

            Ok(JsValue::undefined())
        })
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
        let mut lenient = false;
        let mut no_transaction = false;
        if let Some(format_str) = options.as_string() {
            // Backward compatibility with format as a string
            console_warn!(
                "The format should be passed to Store.load in an option dictionary like store.load(my_content, {{format: 'nt'}})"
            );
            format = Some(rdf_format(&format_str)?);
        } else if !options.is_undefined() && !options.is_null() {
            if let Some(format_str) =
                Reflect::get(options, &JsValue::from_str("format"))?.as_string()
            {
                format = Some(rdf_format(&format_str)?);
            }
            parsed_base_iri =
                convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;
            let to_graph_name_js = Reflect::get(options, &JsValue::from_str("toGraphName"))?;
            parsed_to_graph_name = FROM_JS.with(|c| c.to_optional_term(&to_graph_name_js))?;
            unchecked = Reflect::get(options, &JsValue::from_str("unchecked"))?.is_truthy();
            lenient = Reflect::get(options, &JsValue::from_str("lenient"))?.is_truthy();
            no_transaction =
                Reflect::get(options, &JsValue::from_str("noTransaction"))?.is_truthy();
        }
        let format = format
            .ok_or_else(|| format_err!("The format option should be provided as a second argument of Store.load like store.load(my_content, {{format: 'nt'}}"))?;
        if let Some(base_iri) = convert_base_iri(base_iri)? {
            console_warn!(
                "The baseIri should be passed to Store.load in an option dictionary like store.load(my_content, {{format: 'nt', baseIri: 'http//example.com'}})"
            );
            parsed_base_iri = Some(base_iri);
        }
        if let Some(to_graph_name) = FROM_JS.with(|c| c.to_optional_term(to_graph_name))? {
            console_warn!(
                "The target graph name should be passed to Store.load in an option dictionary like store.load(my_content, {{format: 'nt', toGraphName: 'http//example.com'}})"
            );
            parsed_to_graph_name = Some(to_graph_name);
        }

        let mut parser = RdfParser::from_format(format);
        if let Some(to_graph_name) = parsed_to_graph_name {
            parser = parser.with_default_graph(GraphName::try_from(to_graph_name)?);
        }
        if let Some(base_iri) = parsed_base_iri {
            parser = parser.with_base_iri(base_iri).map_err(JsError::from)?;
        }
        if unchecked {
            console_warn!(
                "The `unchecked` option in Store.load is deprecated, please use `lenient` instead"
            );
            parser = parser.lenient();
        } else if lenient {
            parser = parser.lenient();
        }
        if no_transaction {
            let mut loader = self.store.bulk_loader();
            loader
                .load_from_slice(parser, data.as_bytes())
                .map_err(JsError::from)?;
            loader.commit().map_err(JsError::from)?;
        } else {
            self.store
                .load_from_slice(parser, data)
                .map_err(JsError::from)?;
        }
        Ok(())
    }

    pub fn dump(&self, options: &JsValue, from_graph_name: &JsValue) -> Result<String, JsValue> {
        // Serialization options
        let mut format = None;
        let mut parsed_from_graph_name = None;
        let mut prefixes = None;
        let mut base_iri = None;
        if let Some(format_str) = options.as_string() {
            // Backward compatibility with format as a string
            console_warn!(
                "The format should be passed to Store.dump in an option dictionary like store.dump({{format: 'nt'}})"
            );
            format = Some(rdf_format(&format_str)?);
        } else if !options.is_undefined() && !options.is_null() {
            if let Some(format_str) =
                Reflect::get(options, &JsValue::from_str("format"))?.as_string()
            {
                format = Some(rdf_format(&format_str)?);
            }
            let from_graph_name_js = Reflect::get(options, &JsValue::from_str("fromGraphName"))?;
            parsed_from_graph_name = FROM_JS.with(|c| c.to_optional_term(&from_graph_name_js))?;

            // Parse prefixes option
            let prefixes_js = Reflect::get(options, &JsValue::from_str("prefixes"))?;
            if !prefixes_js.is_undefined() && !prefixes_js.is_null() {
                let mut prefixes_map = Vec::new();

                // Handle both plain objects and Maps
                if let Some(entries) = try_iter(&prefixes_js)? {
                    // It's a Map or other iterable
                    for entry in entries {
                        let entry = entry?;
                        let key = Reflect::get(&entry, &0.into())?
                            .as_string()
                            .ok_or_else(|| format_err!("Prefix key must be a string"))?;
                        let value = Reflect::get(&entry, &1.into())?
                            .as_string()
                            .ok_or_else(|| format_err!("Prefix IRI must be a string"))?;
                        prefixes_map.push((key, value));
                    }
                } else {
                    // It's a plain object
                    let keys = js_sys::Object::keys(&js_sys::Object::from(prefixes_js.clone()));
                    for i in 0..keys.length() {
                        let key = keys
                            .get(i)
                            .as_string()
                            .ok_or_else(|| format_err!("Prefix key must be a string"))?;
                        let value = Reflect::get(&prefixes_js, &JsValue::from_str(&key))?
                            .as_string()
                            .ok_or_else(|| format_err!("Prefix IRI must be a string"))?;
                        prefixes_map.push((key, value));
                    }
                }
                prefixes = Some(prefixes_map);
            }

            // Parse base_iri option
            base_iri = convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;
        }
        let format = format
            .ok_or_else(|| format_err!("The format option should be provided as a second argument of Store.load like store.dump({{format: 'nt'}}"))?;
        if let Some(from_graph_name) = FROM_JS.with(|c| c.to_optional_term(from_graph_name))? {
            console_warn!(
                "The source graph name should be passed to Store.dump in an option dictionary like store.dump({{format: 'nt', fromGraphName: 'http//example.com'}})"
            );
            parsed_from_graph_name = Some(from_graph_name);
        }

        // Create serializer with prefixes and base_iri
        let mut serializer = RdfSerializer::from_format(format);
        if let Some(prefixes) = prefixes {
            for (prefix_name, prefix_iri) in prefixes {
                serializer = serializer
                    .with_prefix(&prefix_name, &prefix_iri)
                    .map_err(JsError::from)?;
            }
        }
        if let Some(base_iri) = base_iri {
            serializer = serializer.with_base_iri(base_iri).map_err(JsError::from)?;
        }

        let buffer = if let Some(from_graph_name) = parsed_from_graph_name {
            self.store.dump_graph_to_writer(
                &GraphName::try_from(from_graph_name)?,
                serializer,
                Vec::new(),
            )
        } else {
            self.store.dump_to_writer(serializer, Vec::new())
        }
        .map_err(JsError::from)?;
        Ok(String::from_utf8(buffer).map_err(JsError::from)?)
    }

    pub fn extend(&self, quads: &JsValue) -> Result<(), JsValue> {
        let quads = if let Some(quads) = try_iter(quads)? {
            quads
                .map(|q| FROM_JS.with(|c| c.to_quad(&q?)))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            return Err(format_err!("quads argument must be iterable"));
        };
        self.store.extend(quads).map_err(JsError::from)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = bulkLoad)]
    pub fn bulk_load(&self, data: &str, options: &JsValue) -> Result<(), JsValue> {
        // Parsing options
        let mut format = None;
        let mut parsed_base_iri = None;
        let mut parsed_to_graph_name = None;
        let mut lenient = false;
        if !options.is_undefined() && !options.is_null() {
            if let Some(format_str) =
                Reflect::get(options, &JsValue::from_str("format"))?.as_string()
            {
                format = Some(rdf_format(&format_str)?);
            }
            parsed_base_iri =
                convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;
            let to_graph_name_js = Reflect::get(options, &JsValue::from_str("toGraphName"))?;
            parsed_to_graph_name = FROM_JS.with(|c| c.to_optional_term(&to_graph_name_js))?;
            lenient = Reflect::get(options, &JsValue::from_str("lenient"))?.is_truthy();
        }
        let format = format
            .ok_or_else(|| format_err!("The format option should be provided like store.bulk_load(my_content, {{format: 'nt'}}"))?;

        let mut parser = RdfParser::from_format(format);
        if let Some(to_graph_name) = parsed_to_graph_name {
            parser = parser.with_default_graph(GraphName::try_from(to_graph_name)?);
        }
        if let Some(base_iri) = parsed_base_iri {
            parser = parser.with_base_iri(base_iri).map_err(JsError::from)?;
        }
        if lenient {
            parser = parser.lenient();
        }

        let mut loader = self.store.bulk_loader();
        loader
            .load_from_slice(parser, data.as_bytes())
            .map_err(JsError::from)?;
        loader.commit().map_err(JsError::from)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = namedGraphs)]
    pub fn named_graphs(&self) -> Result<Box<[JsValue]>, JsValue> {
        Ok(self
            .store
            .named_graphs()
            .map(|g| {
                g.map(|g| {
                    JsTerm::from(match g {
                        NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n),
                        NamedOrBlankNode::BlankNode(n) => Term::BlankNode(n),
                    })
                    .into()
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
            .into_boxed_slice())
    }

    #[wasm_bindgen(js_name = containsNamedGraph)]
    pub fn contains_named_graph(&self, graph_name: &JsValue) -> Result<bool, JsValue> {
        let graph_name = FROM_JS.with(|c| c.to_term(graph_name))?;
        let graph_name_ref = GraphName::try_from(graph_name)?;
        let result = match &graph_name_ref {
            GraphName::DefaultGraph => Ok(true),
            GraphName::NamedNode(g) => self.store.contains_named_graph(g),
            GraphName::BlankNode(g) => self.store.contains_named_graph(g),
        };
        Ok(result.map_err(JsError::from)?)
    }

    #[wasm_bindgen(js_name = addGraph)]
    pub fn add_graph(&self, graph_name: &JsValue) -> Result<(), JsValue> {
        let graph_name = FROM_JS.with(|c| c.to_term(graph_name))?;
        let graph_name_ref = GraphName::try_from(graph_name)?;
        match &graph_name_ref {
            GraphName::DefaultGraph => Ok(()),
            GraphName::NamedNode(g) => self.store.insert_named_graph(g),
            GraphName::BlankNode(g) => self.store.insert_named_graph(g),
        }
        .map_err(JsError::from)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = clearGraph)]
    pub fn clear_graph(&self, graph_name: &JsValue) -> Result<(), JsValue> {
        let graph_name = FROM_JS.with(|c| c.to_term(graph_name))?;
        let graph_name_ref = GraphName::try_from(graph_name)?;
        self.store
            .clear_graph(&graph_name_ref)
            .map_err(JsError::from)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = removeGraph)]
    pub fn remove_graph(&self, graph_name: &JsValue) -> Result<(), JsValue> {
        let graph_name = FROM_JS.with(|c| c.to_term(graph_name))?;
        let graph_name_ref = GraphName::try_from(graph_name)?;
        match &graph_name_ref {
            GraphName::DefaultGraph => self.store.clear_graph(GraphNameRef::DefaultGraph),
            GraphName::NamedNode(g) => self.store.remove_named_graph(g),
            GraphName::BlankNode(g) => self.store.remove_named_graph(g),
        }
        .map_err(JsError::from)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), JsValue> {
        self.store.clear().map_err(JsError::from)?;
        Ok(())
    }

    /// JavaScript-idiomatic collection methods
    /// Note: These methods iterate over all quads and call JavaScript callbacks,
    /// which crosses the JS/WASM boundary for each quad. For large datasets,
    /// consider using SPARQL queries or the match() method for better performance.

    #[wasm_bindgen(js_name = forEach)]
    pub fn for_each(&self, callback: &Function) -> Result<(), JsValue> {
        let this = JsValue::NULL;
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            callback.call1(&this, &js_quad)?;
        }
        Ok(())
    }

    pub fn filter(&self, predicate: &Function) -> Result<Box<[JsValue]>, JsValue> {
        let this = JsValue::NULL;
        let mut results = Vec::new();
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            let matches = predicate.call1(&this, &js_quad)?;
            if matches.is_truthy() {
                results.push(js_quad);
            }
        }
        Ok(results.into_boxed_slice())
    }

    pub fn some(&self, predicate: &Function) -> Result<bool, JsValue> {
        let this = JsValue::NULL;
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            let matches = predicate.call1(&this, &js_quad)?;
            if matches.is_truthy() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn every(&self, predicate: &Function) -> Result<bool, JsValue> {
        let this = JsValue::NULL;
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            let matches = predicate.call1(&this, &js_quad)?;
            if !matches.is_truthy() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn find(&self, predicate: &Function) -> Result<JsValue, JsValue> {
        let this = JsValue::NULL;
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            let matches = predicate.call1(&this, &js_quad)?;
            if matches.is_truthy() {
                return Ok(js_quad);
            }
        }
        Ok(JsValue::UNDEFINED)
    }

    // Symbol.iterator implementation - must be manually wired up in JavaScript
    // as wasm-bindgen doesn't support computed property names
    #[wasm_bindgen(skip_typescript)]
    pub fn __iterator(&self) -> Result<JsValue, JsValue> {
        let quads = self
            .store
            .quads_for_pattern(None, None, None, None)
            .map(|v| v.map(|v| JsQuad::from(v).into()))
            .collect::<Result<Vec<JsValue>, _>>()
            .map_err(JsError::from)?;
        Ok(Array::from_iter(quads).values().into())
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

fn extract_prefixes(prefixes_obj: &JsValue) -> Result<Vec<(String, String)>, JsValue> {
    let mut prefixes = Vec::new();
    let obj = js_sys::Object::try_from(prefixes_obj)
        .ok_or_else(|| format_err!("prefixes option must be an object"))?;
    let entries = js_sys::Object::entries(&obj);
    for i in 0..entries.length() {
        let entry = entries.get(i);
        let pair = Array::from(&entry);
        let prefix_name = pair
            .get(0)
            .as_string()
            .ok_or_else(|| format_err!("prefix name must be a string"))?;
        let prefix_iri = pair
            .get(1)
            .as_string()
            .ok_or_else(|| format_err!("prefix IRI must be a string"))?;
        prefixes.push((prefix_name, prefix_iri));
    }
    Ok(prefixes)
}

fn extract_substitutions(substitutions_obj: &JsValue) -> Result<Vec<(Variable, Term)>, JsValue> {
    let mut substitutions = Vec::new();
    let obj = js_sys::Object::try_from(substitutions_obj)
        .ok_or_else(|| format_err!("substitutions option must be an object"))?;
    let entries = js_sys::Object::entries(&obj);
    for i in 0..entries.length() {
        let entry = entries.get(i);
        let pair = Array::from(&entry);
        let variable_name = pair
            .get(0)
            .as_string()
            .ok_or_else(|| format_err!("variable name must be a string"))?;
        let variable = Variable::new(variable_name).map_err(JsError::from)?;
        let term = Term::try_from(FROM_JS.with(|c| c.to_term(&pair.get(1)))?)?;
        substitutions.push((variable, term));
    }
    Ok(substitutions)
}
