use crate::error::{error_json, ok_json, ErrorKind};
use crate::model_ffi::{bool_to_response, c_str_to_str, parse_quad_value};
use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};
use oxigraph::model::{GraphName, NamedNode, NamedOrBlankNode, Term};
use oxigraph::sparql::results::{QueryResultsFormat, QueryResultsParser, SliceQueryResultsParserOutput};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use serde_json::{json, Map, Value};
use std::cell::UnsafeCell;
use std::os::raw::c_char;

/// Opaque handle to a Store. Passed from Rust to C# and back.
pub type StoreHandle = *mut UnsafeCell<Store>;

/// Open a file-backed Store on disk.
/// path is the directory path for RocksDB storage.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_open(path: *const c_char) -> *mut c_char {
    let path_str = unsafe { c_str_to_str(path) };
    if path_str.is_empty() {
        return error_json(ErrorKind::InvalidArgument { message: "Path is empty".into() });
    }
    match Store::open(path_str) {
        Ok(s) => store_to_handle(s),
        Err(e) => error_json(ErrorKind::Store { message: e.to_string() }),
    }
}

/// Open a file-backed Store in read-only mode.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_open_read_only(path: *const c_char) -> *mut c_char {
    let path_str = unsafe { c_str_to_str(path) };
    if path_str.is_empty() {
        return error_json(ErrorKind::InvalidArgument { message: "Path is empty".into() });
    }
    match Store::open_read_only(path_str) {
        Ok(s) => store_to_handle(s),
        Err(e) => error_json(ErrorKind::Store { message: e.to_string() }),
    }
}

/// Flush pending writes to disk.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_flush(handle: StoreHandle) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Store handle is null".into() });
    }
    let store = unsafe { &mut *(*handle).get() };
    match store.flush() { Ok(_) => ok_json(&"flushed"), Err(e) => error_json(ErrorKind::Store { message: e.to_string() }) }
}

/// Optimize database.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_optimize(handle: StoreHandle) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Store handle is null".into() });
    }
    let store = unsafe { &mut *(*handle).get() };
    match store.optimize() { Ok(_) => ok_json(&"optimized"), Err(e) => error_json(ErrorKind::Store { message: e.to_string() }) }
}

/// Backup store to target directory.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_backup(handle: StoreHandle, target: *const c_char) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Store handle is null".into() });
    }
    let store = unsafe { &mut *(*handle).get() };
    let path = std::path::Path::new(unsafe { c_str_to_str(target) });
    match store.backup(path) { Ok(_) => ok_json(&"backed up"), Err(e) => error_json(ErrorKind::Store { message: e.to_string() }) }
}

fn store_to_handle(store: Store) -> *mut c_char {
    let boxed = Box::new(UnsafeCell::new(store));
    let ptr = Box::into_raw(boxed);
    let handle_value = ptr as u64;
    match serde_json::to_string(&handle_value) {
        Ok(json) => {
            let full = format!("{{\"ok\":{{\"handle\":{}}}}}", json);
            std::ffi::CString::new(full).unwrap().into_raw()
        }
        Err(e) => {
            unsafe { drop(Box::from_raw(ptr)); }
            error_json(ErrorKind::Store { message: e.to_string() })
        }
    }
}

/// Create a new in-memory Store.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_new() -> *mut c_char {
    match Store::new() {
        Ok(s) => store_to_handle(s),
        Err(e) => error_json(ErrorKind::Store { message: e.to_string() }),
    }
}

/// Add a quad to the store.
/// `quad_json` is a JSON-serialized Quad.
/// Returns JSON: {"ok": "ok"}
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_add(
    handle: StoreHandle,
    quad_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(quad_json) };
    if json_str.is_empty() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Quad JSON is empty".into(),
        });
    }
    let quad = match parse_quad_value(json_str) {
        Ok(q) => q,
        Err(e) => return error_json(e),
    };
    match store.insert(quad) {
        Ok(_) => ok_json(&"ok"),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// Removes a quad from the store.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_remove(
    handle: StoreHandle,
    quad_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(quad_json) };
    if json_str.is_empty() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Quad JSON is empty".into(),
        });
    }
    let quad = match parse_quad_value(json_str) {
        Ok(q) => q,
        Err(e) => return error_json(e),
    };
    match store.remove(&quad) {
        Ok(_) => ok_json(&"ok"),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// Check if the store contains a quad.
/// Returns JSON: {"ok": true} or {"ok": false}
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_contains(
    handle: StoreHandle,
    quad_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(quad_json) };
    if json_str.is_empty() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Quad JSON is empty".into(),
        });
    }
    let quad = match parse_quad_value(json_str) {
        Ok(q) => q,
        Err(e) => return error_json(e),
    };
    match store.contains(&quad) {
        Ok(contains) => bool_to_response(contains),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// Get the number of quads in the store.
/// Returns JSON: {"ok": <count>}
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_count(handle: StoreHandle) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    match store.len() {
        Ok(len) => ok_json(&len),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// Execute a SPARQL query against the store.
/// `query_json` is a JSON object: {"query":"SELECT ...","base_iri":null,"use_default_graph_as_union":false}
/// Returns JSON:
///   SELECT → {"ok":{"type":"solutions","variables":["s","p","o"],"rows":[{"s":{...},"p":{...},"o":{...}},...]}}
///   CONSTRUCT/DESCRIBE → {"ok":{"type":"triples","triples":[{...},...]}}
///   ASK → {"ok":{"type":"boolean","value":true}}
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_query(
    handle: StoreHandle,
    query_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(query_json) };
    let opts: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid query JSON: {e}"),
            });
        }
    };

    let query = opts["query"].as_str().unwrap_or("");
    if query.is_empty() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Query string is empty".into(),
        });
    }

    let mut evaluator = SparqlEvaluator::default();
    if let Some(base_iri) = opts["base_iri"].as_str().filter(|s| !s.is_empty()) {
        evaluator = match evaluator.with_base_iri(base_iri) {
            Ok(e) => e,
            Err(e) => return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid base IRI: {e}"),
            }),
        };
    }
    if let Some(prefixes) = opts["prefixes"].as_object() {
        for (prefix, iri) in prefixes {
            if let Some(iri_str) = iri.as_str() {
                evaluator = match evaluator.with_prefix(prefix, iri_str) {
                    Ok(e) => e,
                    Err(e) => return error_json(ErrorKind::InvalidArgument {
                        message: format!("Invalid prefix {prefix}: {e}"),
                    }),
                };
            }
        }
    }
    if opts["use_default_graph_as_union"].as_bool().unwrap_or(false) {
        let mut prepared = match evaluator.parse_query(query) {
            Ok(p) => p,
            Err(e) => {
                return error_json(ErrorKind::InvalidArgument {
                    message: format!("SPARQL syntax error: {e}"),
                });
            }
        };
        prepared.dataset_mut().set_default_graph_as_union();
        let results = match prepared.on_store(store).execute() {
            Ok(r) => r,
            Err(e) => {
                return error_json(ErrorKind::InvalidArgument {
                    message: format!("SPARQL evaluation error: {e}"),
                });
            }
        };
        return query_results_to_response(results);
    }

    let prepared = match evaluator.parse_query(query) {
        Ok(p) => p,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("SPARQL syntax error: {e}"),
            });
        }
    };
    let results = match prepared.on_store(store).execute() {
        Ok(r) => r,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("SPARQL evaluation error: {e}"),
            });
        }
    };
    query_results_to_response(results)
}

/// Execute a SPARQL update against the store.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_update(
    handle: StoreHandle,
    update_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(update_json) };
    let opts: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid update JSON: {e}"),
            });
        }
    };

    let update = opts["update"].as_str().unwrap_or("");
    if update.is_empty() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Update string is empty".into(),
        });
    }

    let mut evaluator = SparqlEvaluator::default();
    if let Some(base_iri) = opts["base_iri"].as_str().filter(|s| !s.is_empty()) {
        evaluator = match evaluator.with_base_iri(base_iri) {
            Ok(e) => e,
            Err(e) => return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid base IRI: {e}"),
            }),
        };
    }
    if let Some(prefixes) = opts["prefixes"].as_object() {
        for (prefix, iri) in prefixes {
            if let Some(iri_str) = iri.as_str() {
                evaluator = match evaluator.with_prefix(prefix, iri_str) {
                    Ok(e) => e,
                    Err(e) => return error_json(ErrorKind::InvalidArgument {
                        message: format!("Invalid prefix {prefix}: {e}"),
                    }),
                };
            }
        }
    }

    match evaluator.parse_update(update) {
        Ok(update) => match update.on_store(store).execute() {
            Ok(_) => ok_json(&"update executed"),
            Err(e) => error_json(ErrorKind::InvalidArgument {
                message: format!("SPARQL evaluation error: {e}"),
            }),
        },
        Err(e) => error_json(ErrorKind::InvalidArgument {
            message: format!("SPARQL syntax error: {e}"),
        }),
    }
}

/// Match quads by pattern. null fields are wildcards.
/// `pattern_json` is a JSON object with optional "subject", "predicate", "object", "graph" keys.
/// Returns JSON: {"ok": [{...}, {...}]} — array of matching quads.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_match(
    handle: StoreHandle,
    pattern_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(pattern_json) };
    let pattern: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid pattern JSON: {e}"),
            });
        }
    };

    // Parse optional S/P/O/G filters from pattern JSON
    let subject = parse_named_or_blank(&pattern, "subject");
    let predicate = parse_named(&pattern, "predicate");
    let object = parse_term(&pattern, "object");

    let graph: Option<GraphName> = pattern
        .get("graph")
        .and_then(|g| serde_json::from_value(g.clone()).ok());

    let results = store.quads_for_pattern(
        subject.as_ref(),
        predicate.as_ref(),
        object.as_ref(),
        graph.as_ref(),
    );
    let quads: Result<Vec<_>, _> = results.collect();
    match quads {
        Ok(quads) => ok_json(&quads),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

fn parse_named_or_blank(pattern: &Value, key: &str) -> Option<NamedOrBlankNode> {
    pattern.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn parse_named(pattern: &Value, key: &str) -> Option<NamedNode> {
    pattern.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn parse_term(pattern: &Value, key: &str) -> Option<Term> {
    pattern.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn query_results_to_response(results: QueryResults) -> *mut c_char {
    match results {
        QueryResults::Solutions(solutions) => {
            let variables: Vec<String> = solutions
                .variables()
                .iter()
                .map(|v| v.as_str().to_string())
                .collect();
            let mut rows = Vec::new();
            for solution in solutions {
                if let Ok(s) = solution {
                    let mut row = Map::new();
                    for var in &variables {
                        if let Some(term) = s.get(var.as_str()) {
                            row.insert(
                                var.clone(),
                                serde_json::to_value(term).unwrap_or_default(),
                            );
                        }
                    }
                    rows.push(Value::Object(row));
                }
            }
            let response = json!({
                "type": "solutions",
                "variables": variables,
                "rows": rows,
            });
            ok_json(&response)
        }
        QueryResults::Boolean(value) => {
            let response = json!({
                "type": "boolean",
                "value": value,
            });
            ok_json(&response)
        }
        QueryResults::Graph(triples) => {
            let triples: Vec<_> = triples
                .filter_map(|r| r.ok())
                .collect();
            let response = json!({
                "type": "triples",
                "triples": triples,
            });
            ok_json(&response)
        }
    }
}

/// Clear all quads from the store.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_clear(handle: StoreHandle) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    match store.clear() {
        Ok(_) => ok_json(&"cleared"),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// Insert multiple quads atomically.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_extend(
    handle: StoreHandle,
    quads_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(quads_json) };
    let quads: Vec<oxigraph::model::Quad> = match serde_json::from_str(json_str) {
        Ok(q) => q,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid quads JSON: {e}"),
            });
        }
    };
    match store.extend(quads) {
        Ok(_) => ok_json(&"extended"),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// List all named graphs.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_named_graphs(handle: StoreHandle) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let graphs: Vec<NamedOrBlankNode> = store
        .named_graphs()
        .filter_map(|r| r.ok())
        .collect();
    ok_json(&graphs)
}

/// Check if a named graph exists.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_contains_named_graph(
    handle: StoreHandle,
    graph_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(graph_json) };
    let graph: GraphName = match serde_json::from_str(json_str) {
        Ok(g) => g,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid graph JSON: {e}"),
            });
        }
    };
    let result = match &graph {
        GraphName::NamedNode(n) => {
            let nb: NamedOrBlankNode = n.clone().into();
            store.contains_named_graph(&nb)
        }
        GraphName::BlankNode(b) => {
            let nb: NamedOrBlankNode = b.clone().into();
            store.contains_named_graph(&nb)
        }
        GraphName::DefaultGraph => return ok_json(&true), // default graph always exists
    };
    match result {
        Ok(contains) => bool_to_response(contains),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// Add a named graph (empty).
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_insert_named_graph(
    handle: StoreHandle,
    graph_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(graph_json) };
    let graph: NamedOrBlankNode = match serde_json::from_str(json_str) {
        Ok(g) => g,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid graph JSON: {e}"),
            });
        }
    };
    match store.insert_named_graph(graph.clone()) {
        Ok(_) => ok_json(&"graph added"),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// Clear a specific named graph.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_clear_graph(
    handle: StoreHandle,
    graph_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(graph_json) };
    let graph: GraphName = match serde_json::from_str(json_str) {
        Ok(g) => g,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid graph JSON: {e}"),
            });
        }
    };
    match store.clear_graph(&graph) {
        Ok(_) => ok_json(&"graph cleared"),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// Remove a named graph entirely.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_remove_named_graph(
    handle: StoreHandle,
    graph_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(graph_json) };
    let graph: NamedOrBlankNode = match serde_json::from_str(json_str) {
        Ok(g) => g,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid graph JSON: {e}"),
            });
        }
    };
    match store.remove_named_graph(&graph) {
        Ok(_) => ok_json(&"graph removed"),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
}

/// Serialize quads to RDF text (standalone, no store needed).
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_serialize(input_json: *const c_char) -> *mut c_char {
    let json_str = unsafe { c_str_to_str(input_json) };
    let opts: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid serialize options JSON: {e}"),
            });
        }
    };

    let quads: Vec<oxigraph::model::Quad> = match serde_json::from_value(
        opts["quads"].clone(),
    ) {
        Ok(q) => q,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid quads JSON: {e}"),
            });
        }
    };

    let format_str = opts["format"].as_str().unwrap_or("nquads");
    let format = match parse_format(format_str) {
        Some(f) => f,
        None => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Unknown RDF format: {format_str}"),
            });
        }
    };

    let mut serializer = RdfSerializer::from_format(format);
    if let Some(base_iri) = opts["base_iri"].as_str().filter(|s| !s.is_empty()) {
        serializer = match serializer.with_base_iri(base_iri) {
            Ok(s) => s,
            Err(e) => {
                return error_json(ErrorKind::InvalidArgument {
                    message: format!("Invalid base IRI: {e}"),
                });
            }
        };
    }

    let mut buf = Vec::new();
    let mut writer = serializer.for_writer(&mut buf);
    for quad in &quads {
        if let Err(e) = writer.serialize_quad(quad) {
            return error_json(ErrorKind::Parse {
                message: e.to_string(),
                file: None,
                line: None,
            });
        }
    }
    let output = String::from_utf8_lossy(&buf).to_string();
    ok_json(&output)
}

/// Parse RDF text into quads.
/// `input_json`: {"data":"<turtle data>","format":"turtle","base_iri":null}
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_parse(input_json: *const c_char) -> *mut c_char {
    let json_str = unsafe { c_str_to_str(input_json) };
    let opts: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid parse options JSON: {e}"),
            });
        }
    };

    let data = opts["data"].as_str().unwrap_or("");
    let format_str = opts["format"].as_str().unwrap_or("turtle");
    let format = match parse_format(format_str) {
        Some(f) => f,
        None => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Unknown RDF format: {format_str}"),
            });
        }
    };

    let mut parser = RdfParser::from_format(format);
    if let Some(base_iri) = opts["base_iri"].as_str() {
        parser = match parser.with_base_iri(base_iri) {
            Ok(p) => p,
            Err(e) => {
                return error_json(ErrorKind::InvalidArgument {
                    message: format!("Invalid base IRI: {e}"),
                });
            }
        };
    }

    let quads: Vec<oxigraph::model::Quad> = parser
        .for_slice(data.as_bytes())
        .filter_map(|q| q.ok())
        .collect();
    ok_json(&quads)
}

/// Load RDF text into the store.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_load(
    handle: StoreHandle,
    load_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(load_json) };
    let opts: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid load options JSON: {e}"),
            });
        }
    };

    let data = opts["data"].as_str().unwrap_or("");
    let format_str = opts["format"].as_str().unwrap_or("turtle");
    let format = match parse_format(format_str) {
        Some(f) => f,
        None => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Unknown RDF format: {format_str}"),
            });
        }
    };

    let mut parser = RdfParser::from_format(format);
    if let Some(base_iri) = opts["base_iri"].as_str() {
        parser = match parser.with_base_iri(base_iri) {
            Ok(p) => p,
            Err(e) => {
                return error_json(ErrorKind::InvalidArgument {
                    message: format!("Invalid base IRI: {e}"),
                });
            }
        };
    }
    if let Some(to_graph_json) = opts.get("to_graph") {
        if let Ok(graph) = serde_json::from_value::<GraphName>(to_graph_json.clone()) {
            parser = parser.with_default_graph(graph);
        }
    }

    match store.load_from_slice(parser, data.as_bytes()) {
        Ok(_) => ok_json(&"loaded"),
        Err(e) => error_json(ErrorKind::Parse {
            message: e.to_string(),
            file: None,
            line: None,
        }),
    }
}

/// Dump store contents as RDF text.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_dump(
    handle: StoreHandle,
    dump_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let json_str = unsafe { c_str_to_str(dump_json) };
    let opts: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid dump options JSON: {e}"),
            });
        }
    };

    let format_str = opts["format"].as_str().unwrap_or("nquads");
    let format = match parse_format(format_str) {
        Some(f) => f,
        None => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Unknown RDF format: {format_str}"),
            });
        }
    };

    let mut serializer = RdfSerializer::from_format(format);
    if let Some(base_iri) = opts["base_iri"].as_str() {
        serializer = match serializer.with_base_iri(base_iri) {
            Ok(s) => s,
            Err(e) => {
                return error_json(ErrorKind::InvalidArgument {
                    message: format!("Invalid base IRI: {e}"),
                });
            }
        };
    }

    let mut buf = Vec::new();
    match if let Some(from_graph_json) = opts.get("from_graph") {
        if let Ok(graph) = serde_json::from_value::<GraphName>(from_graph_json.clone()) {
            store.dump_graph_to_writer(&graph, serializer, &mut buf)
        } else {
            store.dump_to_writer(serializer, &mut buf)
        }
    } else {
        store.dump_to_writer(serializer, &mut buf)
    } {
        Ok(_) => {
            let output = String::from_utf8_lossy(&buf).to_string();
            ok_json(&output)
        }
        Err(e) => error_json(ErrorKind::Parse {
            message: e.to_string(),
            file: None,
            line: None,
        }),
    }
}

/// Parse SPARQL query results from XML/JSON/CSV/TSV.
/// `input_json`: {"data":"...","format":"xml"|"json"|"csv"|"tsv"}
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_parse_query_results(input_json: *const c_char) -> *mut c_char {
    let json_str = unsafe { c_str_to_str(input_json) };
    let opts: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => return error_json(ErrorKind::InvalidArgument { message: format!("Invalid JSON: {e}") }),
    };
    let data = opts["data"].as_str().unwrap_or("");
    let format_str = opts["format"].as_str().unwrap_or("xml");
    let format = match format_str.to_lowercase().as_str() {
        "xml" => QueryResultsFormat::Xml,
        "json" => QueryResultsFormat::Json,
        "csv" => QueryResultsFormat::Csv,
        "tsv" => QueryResultsFormat::Tsv,
        _ => return error_json(ErrorKind::InvalidArgument { message: format!("Unknown format: {format_str}") }),
    };

    let parser = QueryResultsParser::from_format(format);
    match parser.for_slice(data.as_bytes()) {
        Ok(output) => match output {
            SliceQueryResultsParserOutput::Solutions(solutions) => {
                let variables: Vec<String> = solutions.variables().iter().map(|v| v.as_str().to_string()).collect();
                let mut rows = Vec::new();
                for sol in solutions {
                    if let Ok(s) = sol {
                        let mut row = Map::new();
                        for var in &variables {
                            if let Some(term) = s.get(var.as_str()) {
                                row.insert(var.clone(), serde_json::to_value(term).unwrap_or_default());
                            }
                        }
                        rows.push(Value::Object(row));
                    }
                }
                let response = json!({"type": "solutions", "variables": variables, "rows": rows});
                ok_json(&response)
            }
            SliceQueryResultsParserOutput::Boolean(b) => {
                let response = json!({"type": "boolean", "value": b});
                ok_json(&response)
            }
        },
        Err(e) => error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None }),
    }
}

/// Map C# format string to RdfFormat.
fn parse_format(s: &str) -> Option<RdfFormat> {
    match s.to_lowercase().as_str() {
        "n3" => Some(RdfFormat::N3),
        "nquads" | "n-quads" => Some(RdfFormat::NQuads),
        "ntriples" | "n-triples" => Some(RdfFormat::NTriples),
        "rdfxml" | "rdf-xml" | "rdf/xml" => Some(RdfFormat::RdfXml),
        "trig" => Some(RdfFormat::TriG),
        "turtle" => Some(RdfFormat::Turtle),
        "jsonld" | "json-ld" => Some(RdfFormat::JsonLd {
            profile: oxigraph::io::JsonLdProfileSet::empty(),
        }),
        _ => None,
    }
}

/// Destroy a Store and free its memory.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_destroy(handle: StoreHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        // Drop the Store, then the UnsafeCell, then the Box allocation.
        drop(Box::from_raw(handle));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_lifecycle() {
        // Create
        let json_ptr = oxigraph_store_new();
        let json_str = unsafe { c_str_to_str(json_ptr) };
        let v: serde_json::Value = serde_json::from_str(json_str).unwrap();
        let handle_val = v["ok"]["handle"].as_u64().unwrap();
        let handle = handle_val as *mut UnsafeCell<Store>;
        oxigraph_free_string(json_ptr);

        // Verify count starts at 0
        let count_ptr = oxigraph_store_count(handle);
        let count_str = unsafe { c_str_to_str(count_ptr) };
        let count_v: serde_json::Value = serde_json::from_str(count_str).unwrap();
        assert_eq!(count_v["ok"].as_u64().unwrap(), 0);
        oxigraph_free_string(count_ptr);

        // Destroy
        oxigraph_store_destroy(handle);
    }
}
