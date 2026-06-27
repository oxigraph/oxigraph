use crate::error::{error_json, ok_json, ErrorKind};
use crate::model_ffi::{bool_to_response, c_str_to_str, parse_quad_value};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use serde_json::{json, Map, Value};
use std::cell::UnsafeCell;
use std::os::raw::c_char;

/// Opaque handle to a Store. Passed from Rust to C# and back.
pub type StoreHandle = *mut UnsafeCell<Store>;

/// Create a new in-memory Store.
/// Returns JSON: {"ok": "ok"}
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_new() -> *mut c_char {
    match Store::new() {
        Ok(_store) => {
            // Store is allocated on the heap; C# receives a pointer to it.
            let boxed = Box::new(UnsafeCell::new(_store));
            let ptr = Box::into_raw(boxed);
            let handle_value = ptr as u64;
            match serde_json::to_string(&handle_value) {
                Ok(json) => {
                    let full = format!("{{\"ok\":{{\"handle\":{}}}}}", json);
                    std::ffi::CString::new(full).unwrap().into_raw()
                }
                Err(e) => {
                    // Clean up the store we allocated
                    unsafe { drop(Box::from_raw(ptr)); }
                    error_json(ErrorKind::Store {
                        message: e.to_string(),
                    })
                }
            }
        }
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
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
    if let Some(base_iri) = opts["base_iri"].as_str() {
        evaluator = match evaluator.with_base_iri(base_iri) {
            Ok(e) => e,
            Err(e) => return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid base IRI: {e}"),
            }),
        };
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
    if let Some(base_iri) = opts["base_iri"].as_str() {
        evaluator = match evaluator.with_base_iri(base_iri) {
            Ok(e) => e,
            Err(e) => return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid base IRI: {e}"),
            }),
        };
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

    let _pattern = pattern; // TODO: parse pattern fields for SPOG filtering
    // For now, if no filters specified, return all quads (simplified PoC)
    let quads: Result<Vec<_>, _> = store.iter().collect();
    match quads {
        Ok(quads) => ok_json(&quads),
        Err(e) => error_json(ErrorKind::Store {
            message: e.to_string(),
        }),
    }
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
