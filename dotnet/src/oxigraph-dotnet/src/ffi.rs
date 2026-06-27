use crate::error::{error_json, ok_json, ErrorKind};
use crate::model_ffi::{bool_to_response, c_str_to_str, parse_quad_value};
use crate::stream_ffi::{CallbackReader, CallbackWriter, ReadFn, WriteFn};
use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};
use oxigraph::model::{GraphName, NamedNode, NamedOrBlankNode, Term};
use oxigraph::sparql::results::{QueryResultsFormat, QueryResultsParser, SliceQueryResultsParserOutput};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use serde_json::{json, Map, Value};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fs::File;
use std::os::raw::{c_char, c_void};
use std::sync::Mutex;

/// Type for C# callback: receives JSON array of Terms, returns JSON Term or null.
type CustomFnCallback = unsafe extern "C" fn(args_json: *const c_char) -> *mut c_char;

/// Aggregate function callbacks from C#.
type AggregateNewCallback = unsafe extern "C" fn() -> *mut c_void;    // returns context handle
type AggregateAccCallback = unsafe extern "C" fn(ctx: *mut c_void, term_json: *const c_char);
type AggregateFinishCallback = unsafe extern "C" fn(ctx: *mut c_void) -> *mut c_char; // returns term JSON or null
type AggregateFreeCallback = unsafe extern "C" fn(ctx: *mut c_void);

static CUSTOM_FUNCTIONS: std::sync::LazyLock<Mutex<HashMap<String, CustomFnCallback>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Storage for aggregate function callback tuples.
#[derive(Clone)]
struct AggregateCallbacks {
    new_fn: AggregateNewCallback,
    acc_fn: AggregateAccCallback,
    finish_fn: AggregateFinishCallback,
    free_fn: AggregateFreeCallback,
}

// SAFETY: function pointers are Send.
unsafe impl Send for AggregateCallbacks {}

static AGGREGATE_FUNCTIONS: std::sync::LazyLock<Mutex<HashMap<String, AggregateCallbacks>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register a custom SPARQL function from C#.
/// `name` is the function IRI, `callback` is a C# UnmanagedCallersOnly function pointer.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_register_custom_function(
    name: *const c_char,
    callback: CustomFnCallback,
) -> *mut c_char {
    let name_str = unsafe { c_str_to_str(name) }.to_string();
    if name_str.is_empty() || callback as usize == 0 {
        return error_json(ErrorKind::InvalidArgument {
            message: "Name or callback is null".into(),
        });
    }
    CUSTOM_FUNCTIONS.lock().unwrap().insert(name_str, callback);
    ok_json(&"registered")
}

/// Unregister a custom SPARQL function.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_unregister_custom_function(name: *const c_char) -> *mut c_char {
    let name_str = unsafe { c_str_to_str(name) };
    CUSTOM_FUNCTIONS.lock().unwrap().remove(name_str);
    ok_json(&"unregistered")
}

// ─── Custom aggregate functions ─────────────────────

/// Wrapper around C# aggregate callbacks that implements AggregateFunctionAccumulator.
struct CallbackAggregateAccumulator {
    ctx: *mut std::os::raw::c_void,
    acc_fn: AggregateAccCallback,
    finish_fn: AggregateFinishCallback,
    free_fn: AggregateFreeCallback,
}

// SAFETY: The C# side ensures thread-safe access via its own locking.
unsafe impl Send for CallbackAggregateAccumulator {}
unsafe impl Sync for CallbackAggregateAccumulator {}

impl oxigraph::sparql::AggregateFunctionAccumulator for CallbackAggregateAccumulator {
    fn accumulate(&mut self, element: Term) {
        let json = serde_json::to_string(&element).unwrap_or_default();
        let c_str = std::ffi::CString::new(json).unwrap();
        unsafe { (self.acc_fn)(self.ctx, c_str.as_ptr()) };
    }

    fn finish(&mut self) -> Option<Term> {
        let ptr = unsafe { (self.finish_fn)(self.ctx) };
        if ptr.is_null() { return None; }
        let json = unsafe { crate::model_ffi::c_str_to_str(ptr) };
        let term: Option<Term> = serde_json::from_str(json).unwrap_or(None);
        crate::error::oxigraph_free_string(ptr);
        term
    }
}

impl Drop for CallbackAggregateAccumulator {
    fn drop(&mut self) {
        unsafe { (self.free_fn)(self.ctx) };
    }
}

/// Register a custom aggregate SPARQL function.
/// `new_fn` creates a context; `acc_fn` accumulates a term; `finish_fn` returns the result; `free_fn` destroys context.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_register_aggregate_function(
    name: *const c_char,
    new_fn: AggregateNewCallback,
    acc_fn: AggregateAccCallback,
    finish_fn: AggregateFinishCallback,
    free_fn: AggregateFreeCallback,
) -> *mut c_char {
    let name_str = unsafe { c_str_to_str(name) }.to_string();
    if name_str.is_empty() || new_fn as usize == 0 || acc_fn as usize == 0 || finish_fn as usize == 0 || free_fn as usize == 0 {
        return error_json(ErrorKind::InvalidArgument { message: "Name or callbacks are null".into() });
    }
    AGGREGATE_FUNCTIONS.lock().unwrap().insert(name_str, AggregateCallbacks {
        new_fn, acc_fn, finish_fn, free_fn,
    });
    ok_json(&"registered")
}

/// Unregister an aggregate function.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_unregister_aggregate_function(name: *const c_char) -> *mut c_char {
    let name_str = unsafe { c_str_to_str(name) };
    AGGREGATE_FUNCTIONS.lock().unwrap().remove(name_str);
    ok_json(&"unregistered")
}

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
    // Inject registered custom functions (snapshot to avoid lock lifetime issues)
    let custom_fns: Vec<(String, CustomFnCallback)> = {
        CUSTOM_FUNCTIONS
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    };
    for (name, callback) in custom_fns {
        let name_for_closure = name.clone();
        let func_name = NamedNode::new(name).unwrap();
        evaluator = evaluator.with_custom_function(
            func_name,
            move |args: &[Term]| -> Option<Term> {
                // Build JSON array: [function_name, term1, term2, ...]
                let mut arr: Vec<Value> = vec![Value::String(name_for_closure.clone())];
                arr.extend(args.iter().map(|t| serde_json::to_value(t).unwrap_or_default()));
                let args_json = serde_json::to_string(&arr).unwrap_or_default();
                let args_c = std::ffi::CString::new(args_json).unwrap();
                let result_ptr = unsafe { callback(args_c.as_ptr()) };
                if result_ptr.is_null() {
                    return None;
                }
                let result_json = unsafe { crate::model_ffi::c_str_to_str(result_ptr) };
                let term: Option<Term> = serde_json::from_str(result_json).unwrap_or(None);
                crate::error::oxigraph_free_string(result_ptr);
                term
            },
        );
    }

    // Inject registered aggregate functions (snapshot to avoid lock lifetime issues)
    let agg_fns: Vec<(String, AggregateCallbacks)> = {
        AGGREGATE_FUNCTIONS
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    for (name, callbacks) in agg_fns {
        let func_name = NamedNode::new(name).unwrap();
        let new_fn = callbacks.new_fn;
        let acc_fn = callbacks.acc_fn;
        let finish_fn = callbacks.finish_fn;
        let free_fn = callbacks.free_fn;
        evaluator = evaluator.with_custom_aggregate_function(
            func_name,
            move || {
                Box::new(CallbackAggregateAccumulator {
                    ctx: unsafe { new_fn() },
                    acc_fn,
                    finish_fn,
                    free_fn,
                })
            },
        );
    }

    // Helper to apply variable substitutions to a prepared query
    fn apply_substitutions(
        mut prepared: oxigraph::sparql::PreparedSparqlQuery,
        substitutions: &Value,
    ) -> Result<oxigraph::sparql::PreparedSparqlQuery, *mut c_char> {
        if let Some(subs) = substitutions.as_object() {
            for (var_name, term_json) in subs {
                let var_name_owned: String = var_name.clone();
                let var = match oxigraph::sparql::Variable::new(var_name_owned) {
                    Ok(v) => v,
                    Err(e) => return Err(error_json(ErrorKind::InvalidArgument {
                        message: format!("Invalid variable name '{var_name}': {e}"),
                    })),
                };
                let term: Term = match serde_json::from_value(term_json.clone()) {
                    Ok(t) => t,
                    Err(e) => return Err(error_json(ErrorKind::InvalidArgument {
                        message: format!("Invalid term for '{var_name}': {e}"),
                    })),
                };
                prepared = prepared.substitute_variable(var, term);
            }
        }
        Ok(prepared)
    }

    // Helper to apply dataset restrictions (default_graph, named_graphs, union)
    fn configure_dataset(
        mut prepared: oxigraph::sparql::PreparedSparqlQuery,
        opts: &Value,
    ) -> Result<oxigraph::sparql::PreparedSparqlQuery, *mut c_char> {
        // Apply default_graph_as_union
        if opts["use_default_graph_as_union"].as_bool().unwrap_or(false) {
            prepared.dataset_mut().set_default_graph_as_union();
        }

        // Apply default_graph restriction
        if let Some(default_graphs) = opts.get("default_graph") {
            if let Some(arr) = default_graphs.as_array() {
                let graphs: Vec<GraphName> = arr
                    .iter()
                    .filter_map(|v| serde_json::from_value::<GraphName>(v.clone()).ok())
                    .collect();
                if !graphs.is_empty() {
                    prepared.dataset_mut().set_default_graph(graphs);
                }
            } else if let Ok(graph) = serde_json::from_value::<GraphName>(default_graphs.clone()) {
                prepared.dataset_mut().set_default_graph(vec![graph]);
            }
        }

        // Apply named_graphs restriction
        if let Some(named_graphs) = opts.get("named_graphs") {
            if let Some(arr) = named_graphs.as_array() {
                let graphs: Vec<NamedOrBlankNode> = arr
                    .iter()
                    .filter_map(|v| serde_json::from_value::<NamedOrBlankNode>(v.clone()).ok())
                    .collect();
                if !graphs.is_empty() {
                    prepared.dataset_mut().set_available_named_graphs(graphs);
                }
            }
        }

        Ok(prepared)
    }

    let mut prepared = match evaluator.parse_query(query) {
        Ok(p) => p,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("SPARQL syntax error: {e}"),
            });
        }
    };
    prepared = match apply_substitutions(prepared, &opts["substitutions"]) {
        Ok(p) => p,
        Err(e) => return e,
    };
    prepared = match configure_dataset(prepared, &opts) {
        Ok(p) => p,
        Err(e) => return e,
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

    // Inject custom functions (same as query path)
    let custom_fns: Vec<(String, CustomFnCallback)> = {
        CUSTOM_FUNCTIONS.lock().unwrap().iter().map(|(k, v)| (k.clone(), *v)).collect()
    };
    for (name, callback) in custom_fns {
        let name_for_closure = name.clone();
        evaluator = evaluator.with_custom_function(
            NamedNode::new(name).unwrap(),
            move |args: &[Term]| -> Option<Term> {
                let mut arr: Vec<Value> = vec![Value::String(name_for_closure.clone())];
                arr.extend(args.iter().map(|t| serde_json::to_value(t).unwrap_or_default()));
                let args_json = serde_json::to_string(&arr).unwrap_or_default();
                let args_c = std::ffi::CString::new(args_json).unwrap();
                let result_ptr = unsafe { callback(args_c.as_ptr()) };
                if result_ptr.is_null() { return None; }
                let result_json = unsafe { crate::model_ffi::c_str_to_str(result_ptr) };
                let term: Option<Term> = serde_json::from_str(result_json).unwrap_or(None);
                crate::error::oxigraph_free_string(result_ptr);
                term
            },
        );
    }

    // Inject custom aggregate functions (same as query path)
    let agg_fns: Vec<(String, AggregateCallbacks)> = {
        AGGREGATE_FUNCTIONS.lock().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    };
    for (name, callbacks) in agg_fns {
        let new_fn = callbacks.new_fn;
        let acc_fn = callbacks.acc_fn;
        let finish_fn = callbacks.finish_fn;
        let free_fn = callbacks.free_fn;
        evaluator = evaluator.with_custom_aggregate_function(
            NamedNode::new(name).unwrap(),
            move || {
                Box::new(CallbackAggregateAccumulator {
                    ctx: unsafe { new_fn() },
                    acc_fn,
                    finish_fn,
                    free_fn,
                })
            },
        );
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

/// Bulk-extend using RocksDB bulk loader (writes new SST files).
/// More efficient than extend for very large datasets.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_bulk_extend(
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
    let mut loader = store.bulk_loader();
    if let Err(e) = loader.load_ok_quads::<oxigraph::store::StorageError, oxigraph::store::StorageError>(
        quads.into_iter().map(Ok),
    ) {
        return error_json(ErrorKind::Store { message: e.to_string() });
    }
    match loader.commit() {
        Ok(_) => ok_json(&"bulk extended"),
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
    if let Some(prefixes) = opts.get("prefixes").and_then(|p| p.as_object()) {
        for (prefix, iri_val) in prefixes {
            if let Some(iri) = iri_val.as_str() {
                serializer = match serializer.with_prefix(prefix, iri) {
                    Ok(s) => s,
                    Err(e) => {
                        return error_json(ErrorKind::InvalidArgument {
                            message: format!("Invalid prefix '{prefix}' IRI '{iri}': {e}"),
                        });
                    }
                };
            }
        }
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
/// `input_json`: {"data":"...","format":"turtle","base_iri":null,"without_named_graphs":false,"rename_blank_nodes":false,"lenient":false}
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
    let base_iri = opts["base_iri"].as_str();
    let without_named_graphs = opts["without_named_graphs"].as_bool().unwrap_or(false);
    let rename_blank_nodes = opts["rename_blank_nodes"].as_bool().unwrap_or(false);
    let lenient = opts["lenient"].as_bool().unwrap_or(false);

    let parser = match build_parser_with_options(format_str, base_iri, None, without_named_graphs, rename_blank_nodes, lenient) {
        Ok(p) => p,
        Err(e) => return e,
    };

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
    if opts["lenient"].as_bool().unwrap_or(false) {
        parser = parser.lenient();
    }
    if opts["rename_blank_nodes"].as_bool().unwrap_or(false) {
        parser = parser.rename_blank_nodes();
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

// ─── Shared helpers ───────────────────────────────

/// Build an RdfParser from format string and options.
fn build_parser(
    format_str: &str,
    base_iri: Option<&str>,
    to_graph_json: Option<&Value>,
) -> Result<RdfParser, *mut c_char> {
    let format = match parse_format(format_str) {
        Some(f) => f,
        None => {
            return Err(error_json(ErrorKind::InvalidArgument {
                message: format!("Unknown RDF format: {format_str}"),
            }));
        }
    };
    let mut parser = RdfParser::from_format(format);
    if let Some(iri) = base_iri.filter(|s| !s.is_empty()) {
        parser = match parser.with_base_iri(iri) {
            Ok(p) => p,
            Err(e) => {
                return Err(error_json(ErrorKind::InvalidArgument {
                    message: format!("Invalid base IRI: {e}"),
                }));
            }
        };
    }
    if let Some(graph) = to_graph_json.and_then(|v| serde_json::from_value::<GraphName>(v.clone()).ok())
    {
        parser = parser.with_default_graph(graph);
    }
    Ok(parser)
}

/// Build parser with additional parse options.
fn build_parser_with_options(
    format_str: &str,
    base_iri: Option<&str>,
    to_graph_json: Option<&Value>,
    without_named_graphs: bool,
    rename_blank_nodes: bool,
    lenient: bool,
) -> Result<RdfParser, *mut c_char> {
    let mut parser = build_parser(format_str, base_iri, to_graph_json)?;
    if without_named_graphs {
        parser = parser.without_named_graphs();
    }
    if rename_blank_nodes {
        parser = parser.rename_blank_nodes();
    }
    if lenient {
        parser = parser.lenient();
    }
    Ok(parser)
}

/// Build an RdfSerializer from format string and options.
fn build_serializer(
    format_str: &str,
    base_iri: Option<&str>,
) -> Result<RdfSerializer, *mut c_char> {
    let format = match parse_format(format_str) {
        Some(f) => f,
        None => {
            return Err(error_json(ErrorKind::InvalidArgument {
                message: format!("Unknown RDF format: {format_str}"),
            }));
        }
    };
    let mut serializer = RdfSerializer::from_format(format);
    if let Some(iri) = base_iri.filter(|s| !s.is_empty()) {
        serializer = match serializer.with_base_iri(iri) {
            Ok(s) => s,
            Err(e) => {
                return Err(error_json(ErrorKind::InvalidArgument {
                    message: format!("Invalid base IRI: {e}"),
                }));
            }
        };
    }
    Ok(serializer)
}

// ─── File-based I/O ───────────────────────────────

/// Load RDF from a file path into the store.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_load_from_file(
    handle: StoreHandle,
    path: *const c_char,
    format: *const c_char,
    base_iri: *const c_char,
    to_graph_json: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let path_str = unsafe { c_str_to_str(path) };
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };
    let to_graph_str = unsafe { c_str_to_str(to_graph_json) };

    let to_graph_val: Option<Value> = if to_graph_str.is_empty() {
        None
    } else {
        match serde_json::from_str(to_graph_str) {
            Ok(v) => Some(v),
            Err(e) => {
                return error_json(ErrorKind::InvalidArgument {
                    message: format!("Invalid to_graph JSON: {e}"),
                });
            }
        }
    };

    // Parse lenient/rename_blank_nodes from options_json
    let opts_str = unsafe { c_str_to_str(options_json) };
    let (lenient, rename_blank_nodes) = if opts_str.is_empty() {
        (false, false)
    } else {
        let opts: Value = match serde_json::from_str(opts_str) {
            Ok(v) => v,
            Err(_) => Value::Null,
        };
        (
            opts["lenient"].as_bool().unwrap_or(false),
            opts["rename_blank_nodes"].as_bool().unwrap_or(false),
        )
    };

    let parser = match build_parser_with_options(
        format_str, Some(base_iri_str), to_graph_val.as_ref(),
        false, rename_blank_nodes, lenient,
    ) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let file = match File::open(path_str) {
        Ok(f) => f,
        Err(e) => {
            return error_json(ErrorKind::Parse {
                message: format!("Cannot open file '{path_str}': {e}"),
                file: Some(path_str.to_string()),
                line: None,
            });
        }
    };

    match store.load_from_reader(parser, file) {
        Ok(_) => ok_json(&"loaded"),
        Err(e) => error_json(ErrorKind::Parse {
            message: e.to_string(),
            file: Some(path_str.to_string()),
            line: None,
        }),
    }
}

/// Bulk-load RDF from a file path into the store (parallel, optimized for large files).
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_bulk_load_from_file(
    handle: StoreHandle,
    path: *const c_char,
    format: *const c_char,
    base_iri: *const c_char,
    to_graph_json: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let path_str = unsafe { c_str_to_str(path) };
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };
    let to_graph_str = unsafe { c_str_to_str(to_graph_json) };

    let to_graph_val: Option<Value> = if to_graph_str.is_empty() {
        None
    } else {
        match serde_json::from_str(to_graph_str) {
            Ok(v) => Some(v),
            Err(e) => {
                return error_json(ErrorKind::InvalidArgument {
                    message: format!("Invalid to_graph JSON: {e}"),
                });
            }
        }
    };

    // Parse lenient/rename_blank_nodes from options_json
    let opts_str = unsafe { c_str_to_str(options_json) };
    let (lenient, rename_blank_nodes) = if opts_str.is_empty() {
        (false, false)
    } else {
        let opts: Value = match serde_json::from_str(opts_str) {
            Ok(v) => v,
            Err(_) => Value::Null,
        };
        (
            opts["lenient"].as_bool().unwrap_or(false),
            opts["rename_blank_nodes"].as_bool().unwrap_or(false),
        )
    };

    let parser = match build_parser_with_options(
        format_str, Some(base_iri_str), to_graph_val.as_ref(),
        false, rename_blank_nodes, lenient,
    ) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let mut loader = store.bulk_loader();
    let result = loader.parallel_load_from_file(parser, path_str);
    match result {
        Ok(_) => match loader.commit() {
            Ok(_) => ok_json(&"bulk loaded"),
            Err(e) => error_json(ErrorKind::Store {
                message: e.to_string(),
            }),
        },
        Err(e) => error_json(ErrorKind::Parse {
            message: e.to_string(),
            file: Some(path_str.to_string()),
            line: None,
        }),
    }
}

/// Dump store contents to a file.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_dump_to_file(
    handle: StoreHandle,
    path: *const c_char,
    format: *const c_char,
    base_iri: *const c_char,
    from_graph_json: *const c_char,
    prefixes_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument {
            message: "Store handle is null".into(),
        });
    }
    let store = unsafe { &mut *(*handle).get() };
    let path_str = unsafe { c_str_to_str(path) };
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };
    let from_graph_str = unsafe { c_str_to_str(from_graph_json) };

    let serializer = match build_serializer(format_str, Some(base_iri_str)) {
        Ok(s) => s,
        Err(e) => return e,
    };

    // Apply prefixes if provided
    let prefixes_str = unsafe { c_str_to_str(prefixes_json) };
    let serializer = if prefixes_str.is_empty() {
        serializer
    } else {
        match serde_json::from_str::<Value>(prefixes_str) {
            Ok(prefixes_val) => {
                let mut s = serializer;
                if let Some(prefixes_obj) = prefixes_val.as_object() {
                    for (prefix, iri_val) in prefixes_obj {
                        if let Some(iri) = iri_val.as_str() {
                            s = match s.with_prefix(prefix, iri) {
                                Ok(s) => s,
                                Err(e) => {
                                    return error_json(ErrorKind::InvalidArgument {
                                        message: format!("Invalid prefix '{prefix}' IRI '{iri}': {e}"),
                                    });
                                }
                            };
                        }
                    }
                }
                s
            }
            Err(_) => serializer,
        }
    };

    let file = match File::create(path_str) {
        Ok(f) => f,
        Err(e) => {
            return error_json(ErrorKind::Parse {
                message: format!("Cannot create file '{path_str}': {e}"),
                file: Some(path_str.to_string()),
                line: None,
            });
        }
    };

    let result = if from_graph_str.is_empty() {
        store.dump_to_writer(serializer, file)
    } else {
        match serde_json::from_str::<GraphName>(from_graph_str) {
            Ok(graph) => store.dump_graph_to_writer(&graph, serializer, file),
            Err(e) => {
                return error_json(ErrorKind::InvalidArgument {
                    message: format!("Invalid from_graph JSON: {e}"),
                });
            }
        }
    };

    match result {
        Ok(_) => ok_json(&"dumped"),
        Err(e) => error_json(ErrorKind::Parse {
            message: e.to_string(),
            file: Some(path_str.to_string()),
            line: None,
        }),
    }
}

/// Parse RDF from a file path (standalone, no store needed).
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_parse_from_file(
    path: *const c_char,
    format: *const c_char,
    base_iri: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    let path_str = unsafe { c_str_to_str(path) };
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };
    let opts_str = unsafe { c_str_to_str(options_json) };

    let (without_named_graphs, rename_blank_nodes, lenient) = if opts_str.is_empty() {
        (false, false, false)
    } else {
        let opts: Value = match serde_json::from_str(opts_str) {
            Ok(v) => v,
            Err(_) => Value::Null,
        };
        (
            opts["without_named_graphs"].as_bool().unwrap_or(false),
            opts["rename_blank_nodes"].as_bool().unwrap_or(false),
            opts["lenient"].as_bool().unwrap_or(false),
        )
    };

    let parser = match build_parser_with_options(
        format_str, Some(base_iri_str), None,
        without_named_graphs, rename_blank_nodes, lenient,
    ) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let file = match File::open(path_str) {
        Ok(f) => f,
        Err(e) => {
            return error_json(ErrorKind::Parse {
                message: format!("Cannot open file '{path_str}': {e}"),
                file: Some(path_str.to_string()),
                line: None,
            });
        }
    };

    let quads: Vec<oxigraph::model::Quad> = parser.for_reader(file).filter_map(|q| q.ok()).collect();
    ok_json(&quads)
}

/// Serialize quads to a file (standalone).
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_serialize_to_file(
    path: *const c_char,
    quads_json: *const c_char,
    format: *const c_char,
    base_iri: *const c_char,
    prefixes_json: *const c_char,
) -> *mut c_char {
    let path_str = unsafe { c_str_to_str(path) };
    let quads_str = unsafe { c_str_to_str(quads_json) };
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };

    let quads: Vec<oxigraph::model::Quad> = match serde_json::from_str(quads_str) {
        Ok(q) => q,
        Err(e) => {
            return error_json(ErrorKind::InvalidArgument {
                message: format!("Invalid quads JSON: {e}"),
            });
        }
    };

    let serializer = match build_serializer(format_str, Some(base_iri_str)) {
        Ok(s) => s,
        Err(e) => return e,
    };

    // Apply prefixes if provided
    let prefixes_str = unsafe { c_str_to_str(prefixes_json) };
    let serializer = if prefixes_str.is_empty() {
        serializer
    } else {
        match serde_json::from_str::<Value>(prefixes_str) {
            Ok(prefixes_val) => {
                let mut s = serializer;
                if let Some(prefixes_obj) = prefixes_val.as_object() {
                    for (prefix, iri_val) in prefixes_obj {
                        if let Some(iri) = iri_val.as_str() {
                            s = match s.with_prefix(prefix, iri) {
                                Ok(s) => s,
                                Err(e) => {
                                    return error_json(ErrorKind::InvalidArgument {
                                        message: format!("Invalid prefix '{prefix}' IRI '{iri}': {e}"),
                                    });
                                }
                            };
                        }
                    }
                }
                s
            }
            Err(_) => serializer,
        }
    };

    let file = match File::create(path_str) {
        Ok(f) => f,
        Err(e) => {
            return error_json(ErrorKind::Parse {
                message: format!("Cannot create file '{path_str}': {e}"),
                file: Some(path_str.to_string()),
                line: None,
            });
        }
    };

    let mut writer = serializer.for_writer(file);
    for quad in &quads {
        if let Err(e) = writer.serialize_quad(quad) {
            return error_json(ErrorKind::Parse {
                message: e.to_string(),
                file: Some(path_str.to_string()),
                line: None,
            });
        }
    }
    match writer.finish() {
        Ok(_) => ok_json(&"serialized"),
        Err(e) => error_json(ErrorKind::Parse {
            message: e.to_string(),
            file: Some(path_str.to_string()),
            line: None,
        }),
    }
}

// ─── Stream/callback-based I/O ─────────────────────

/// Load RDF from a .NET Stream (via read callback) into the store.
/// `context` is passed as-is to every callback invocation.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_load_from_callback(
    handle: StoreHandle,
    callback: ReadFn,
    context: *mut std::os::raw::c_void,
    format: *const c_char,
    base_iri: *const c_char,
    to_graph_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Store handle is null".into() });
    }
    if callback as usize == 0 || context.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Callback or context is null".into() });
    }
    let store = unsafe { &mut *(*handle).get() };
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };
    let to_graph_str = unsafe { c_str_to_str(to_graph_json) };

    let to_graph_val: Option<Value> = if to_graph_str.is_empty() {
        None
    } else {
        match serde_json::from_str(to_graph_str) {
            Ok(v) => Some(v),
            Err(e) => return error_json(ErrorKind::InvalidArgument { message: format!("Invalid to_graph JSON: {e}") }),
        }
    };

    let parser = match build_parser(format_str, Some(base_iri_str), to_graph_val.as_ref()) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let reader = CallbackReader::new(context, callback);
    match store.load_from_reader(parser, reader) {
        Ok(_) => ok_json(&"loaded"),
        Err(e) => error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None }),
    }
}

/// Dump store contents to a .NET Stream (via write callback).
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_store_dump_to_callback(
    handle: StoreHandle,
    callback: WriteFn,
    context: *mut std::os::raw::c_void,
    format: *const c_char,
    base_iri: *const c_char,
    from_graph_json: *const c_char,
    prefixes_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Store handle is null".into() });
    }
    if callback as usize == 0 || context.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Callback or context is null".into() });
    }
    let store = unsafe { &mut *(*handle).get() };
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };
    let from_graph_str = unsafe { c_str_to_str(from_graph_json) };

    let serializer = match build_serializer(format_str, Some(base_iri_str)) {
        Ok(s) => s,
        Err(e) => return e,
    };

    // Apply prefixes if provided
    let prefixes_str = unsafe { c_str_to_str(prefixes_json) };
    let serializer = if prefixes_str.is_empty() {
        serializer
    } else {
        match serde_json::from_str::<Value>(prefixes_str) {
            Ok(prefixes_val) => {
                let mut s = serializer;
                if let Some(prefixes_obj) = prefixes_val.as_object() {
                    for (prefix, iri_val) in prefixes_obj {
                        if let Some(iri) = iri_val.as_str() {
                            s = match s.with_prefix(prefix, iri) {
                                Ok(s) => s,
                                Err(e) => {
                                    return error_json(ErrorKind::InvalidArgument {
                                        message: format!("Invalid prefix '{prefix}' IRI '{iri}': {e}"),
                                    });
                                }
                            };
                        }
                    }
                }
                s
            }
            Err(_) => serializer,
        }
    };

    let writer = CallbackWriter::new(context, callback);
    let result = if from_graph_str.is_empty() {
        store.dump_to_writer(serializer, writer)
    } else {
        match serde_json::from_str::<GraphName>(from_graph_str) {
            Ok(graph) => store.dump_graph_to_writer(&graph, serializer, writer),
            Err(e) => return error_json(ErrorKind::InvalidArgument { message: format!("Invalid from_graph JSON: {e}") }),
        }
    };

    match result {
        Ok(_) => ok_json(&"dumped"),
        Err(e) => error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None }),
    }
}

/// Parse RDF from a .NET Stream (via read callback) into quads JSON.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_parse_from_callback(
    callback: ReadFn,
    context: *mut std::os::raw::c_void,
    format: *const c_char,
    base_iri: *const c_char,
) -> *mut c_char {
    if callback as usize == 0 || context.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Callback or context is null".into() });
    }
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };

    let parser = match build_parser(format_str, Some(base_iri_str), None) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let reader = CallbackReader::new(context, callback);
    let quads: Vec<oxigraph::model::Quad> = parser.for_reader(reader).filter_map(|q| q.ok()).collect();
    ok_json(&quads)
}

/// Serialize quads to a .NET Stream (via write callback).
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_serialize_to_callback(
    callback: WriteFn,
    context: *mut std::os::raw::c_void,
    quads_json: *const c_char,
    format: *const c_char,
    base_iri: *const c_char,
) -> *mut c_char {
    if callback as usize == 0 || context.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Callback or context is null".into() });
    }
    let quads_str = unsafe { c_str_to_str(quads_json) };
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };

    let quads: Vec<oxigraph::model::Quad> = match serde_json::from_str(quads_str) {
        Ok(q) => q,
        Err(e) => return error_json(ErrorKind::InvalidArgument { message: format!("Invalid quads JSON: {e}") }),
    };

    let serializer = match build_serializer(format_str, Some(base_iri_str)) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let mut writer = serializer.for_writer(CallbackWriter::new(context, callback));
    for quad in &quads {
        if let Err(e) = writer.serialize_quad(quad) {
            return error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None });
        }
    }
    match writer.finish() {
        Ok(_) => ok_json(&"serialized"),
        Err(e) => error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None }),
    }
}

// ─── Iterator FFI ──────────────────────────────────

/// Opaque handle to a lazy quad iterator (from parse).
pub type QuadIterHandle = *mut UnsafeCell<Box<dyn Iterator<Item = Result<oxigraph::model::Quad, oxigraph::io::RdfParseError>> + 'static>>;

#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_parse_iter_from_file(
    path: *const c_char,
    format: *const c_char,
    base_iri: *const c_char,
) -> *mut c_char {
    let path_str = unsafe { c_str_to_str(path) };
    let format_str = unsafe { c_str_to_str(format) };
    let base_iri_str = unsafe { c_str_to_str(base_iri) };

    let parser = match build_parser(format_str, Some(base_iri_str), None) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let file = match File::open(path_str) {
        Ok(f) => f,
        Err(e) => return error_json(ErrorKind::Parse {
            message: format!("Cannot open file '{path_str}': {e}"),
            file: Some(path_str.to_string()), line: None,
        }),
    };

    let iter: Box<dyn Iterator<Item = Result<oxigraph::model::Quad, oxigraph::io::RdfParseError>> + 'static> =
        Box::new(parser.for_reader(file));
    let boxed = Box::new(UnsafeCell::new(iter));
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

#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_parse_iter_next(handle: QuadIterHandle) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Iterator handle is null".into() });
    }
    let iter = unsafe { &mut *(*handle).get() };
    match iter.next() {
        Some(Ok(quad)) => ok_json(&quad),
        Some(Err(e)) => error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None }),
        None => ok_json(&serde_json::Value::Null), // end of iteration
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_parse_iter_destroy(handle: QuadIterHandle) {
    if handle.is_null() { return; }
    unsafe { drop(Box::from_raw(handle)); }
}

// ─── Query Results Serialization ────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_query_solutions_serialize_to_file(
    path: *const c_char,
    format: *const c_char,
    variables_json: *const c_char,
    solutions_json: *const c_char,
) -> *mut c_char {
    let path_str = unsafe { c_str_to_str(path) };
    let format_str = unsafe { c_str_to_str(format) };
    let vars_str = unsafe { c_str_to_str(variables_json) };
    let sols_str = unsafe { c_str_to_str(solutions_json) };

    let qformat = match format_str.to_lowercase().as_str() {
        "xml" => QueryResultsFormat::Xml,
        "json" => QueryResultsFormat::Json,
        "csv" => QueryResultsFormat::Csv,
        "tsv" => QueryResultsFormat::Tsv,
        _ => return error_json(ErrorKind::InvalidArgument { message: format!("Unknown format: {format_str}") }),
    };

    let variables: Vec<String> = match serde_json::from_str(vars_str) {
        Ok(v) => v,
        Err(e) => return error_json(ErrorKind::InvalidArgument { message: format!("Invalid variables JSON: {e}") }),
    };
    let solutions: Vec<Value> = match serde_json::from_str(sols_str) {
        Ok(s) => s,
        Err(e) => return error_json(ErrorKind::InvalidArgument { message: format!("Invalid solutions JSON: {e}") }),
    };

    let file = match File::create(path_str) {
        Ok(f) => f,
        Err(e) => return error_json(ErrorKind::Parse {
            message: format!("Cannot create file '{path_str}': {e}"),
            file: Some(path_str.to_string()), line: None,
        }),
    };

    use oxigraph::sparql::results::QueryResultsSerializer;
    // Build Variable list from owned Strings
    let vars: Vec<oxigraph::sparql::Variable> = variables
        .iter()
        .map(|v| {
            let s: String = v.clone();
            oxigraph::sparql::Variable::new(s)
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default();
    let mut serializer = match QueryResultsSerializer::from_format(qformat)
        .serialize_solutions_to_writer(file, vars) {
            Ok(s) => s,
            Err(e) => return error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None }),
        };
    for row in &solutions {
        let mut pairs: Vec<(oxigraph::sparql::Variable, oxigraph::model::Term)> = Vec::new();
        if let Some(obj) = row.as_object() {
            for (k, v) in obj {
                let key_owned: String = k.clone();
                if let (Ok(var), Ok(term)) = (
                    oxigraph::sparql::Variable::new(key_owned),
                    serde_json::from_value::<oxigraph::model::Term>(v.clone()),
                ) {
                    pairs.push((var, term));
                }
            }
        }
        if let Err(e) = serializer.serialize(pairs.iter().map(|(v, t)| (v, t))) {
            return error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None });
        }
    }
    match serializer.finish() {
        Ok(_) => ok_json(&"serialized"),
        Err(e) => error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None }),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_query_boolean_serialize_to_file(
    path: *const c_char,
    format: *const c_char,
    value: bool,
) -> *mut c_char {
    let path_str = unsafe { c_str_to_str(path) };
    let format_str = unsafe { c_str_to_str(format) };

    let qformat = match format_str.to_lowercase().as_str() {
        "xml" => QueryResultsFormat::Xml,
        "json" => QueryResultsFormat::Json,
        "csv" => QueryResultsFormat::Csv,
        "tsv" => QueryResultsFormat::Tsv,
        _ => return error_json(ErrorKind::InvalidArgument { message: format!("Unknown format: {format_str}") }),
    };

    let file = match File::create(path_str) {
        Ok(f) => f,
        Err(e) => return error_json(ErrorKind::Parse {
            message: format!("Cannot create file '{path_str}': {e}"),
            file: Some(path_str.to_string()), line: None,
        }),
    };

    use oxigraph::sparql::results::QueryResultsSerializer;
    match QueryResultsSerializer::from_format(qformat).serialize_boolean_to_writer(file, value) {
        Ok(_) => ok_json(&"serialized"),
        Err(e) => error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None }),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_query_triples_serialize_to_file(
    path: *const c_char,
    rdf_format: *const c_char,
    triples_json: *const c_char,
) -> *mut c_char {
    let path_str = unsafe { c_str_to_str(path) };
    let fmt_str = unsafe { c_str_to_str(rdf_format) };
    let triples_str = unsafe { c_str_to_str(triples_json) };

    let triples: Vec<oxigraph::model::Triple> = match serde_json::from_str(triples_str) {
        Ok(t) => t,
        Err(e) => return error_json(ErrorKind::InvalidArgument { message: format!("Invalid triples JSON: {e}") }),
    };

    let serializer = match build_serializer(fmt_str, None) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let file = match File::create(path_str) {
        Ok(f) => f,
        Err(e) => return error_json(ErrorKind::Parse {
            message: format!("Cannot create file '{path_str}': {e}"),
            file: Some(path_str.to_string()), line: None,
        }),
    };

    let mut writer = serializer.for_writer(file);
    for triple in &triples {
        if let Err(e) = writer.serialize_triple(triple) {
            return error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None });
        }
    }
    match writer.finish() {
        Ok(_) => ok_json(&"serialized"),
        Err(e) => error_json(ErrorKind::Parse { message: e.to_string(), file: None, line: None }),
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
    if let Some(prefixes) = opts.get("prefixes").and_then(|p| p.as_object()) {
        for (prefix, iri_val) in prefixes {
            if let Some(iri) = iri_val.as_str() {
                serializer = match serializer.with_prefix(prefix, iri) {
                    Ok(s) => s,
                    Err(e) => {
                        return error_json(ErrorKind::InvalidArgument {
                            message: format!("Invalid prefix '{prefix}' IRI '{iri}': {e}"),
                        });
                    }
                };
            }
        }
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

/// Canonicalize a list of quads (renames blank nodes consistently).
/// `algorithm`: "unstable", "rdfc10_sha256", "rdfc10_sha384"
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_canonicalize(
    quads_json: *const c_char,
    algorithm: *const c_char,
) -> *mut c_char {
    let quads_str = unsafe { c_str_to_str(quads_json) };
    let algo_str = unsafe { c_str_to_str(algorithm) };

    let quads: Vec<oxigraph::model::Quad> = match serde_json::from_str(quads_str) {
        Ok(q) => q,
        Err(e) => return error_json(ErrorKind::InvalidArgument { message: format!("Invalid quads: {e}") }),
    };

    let mut dataset = oxigraph::model::Dataset::new();
    for quad in quads {
        dataset.insert(quad);
    }

    let algo = match algo_str {
        "unstable" => oxigraph::model::dataset::CanonicalizationAlgorithm::Unstable,
        "rdfc10_sha256" => oxigraph::model::dataset::CanonicalizationAlgorithm::Rdfc10 {
            hash_algorithm: oxigraph::model::dataset::CanonicalizationHashAlgorithm::Sha256,
        },
        "rdfc10_sha384" => oxigraph::model::dataset::CanonicalizationAlgorithm::Rdfc10 {
            hash_algorithm: oxigraph::model::dataset::CanonicalizationHashAlgorithm::Sha384,
        },
        _ => return error_json(ErrorKind::InvalidArgument { message: format!("Unknown canonicalization algorithm: {algo_str}") }),
    };

    dataset.canonicalize(algo);

    let result: Vec<oxigraph::model::Quad> = dataset.iter().collect();
    ok_json(&result)
}

// ─── In-Memory Dataset FFI ──────────────────────────

/// Opaque handle to an in-memory oxigraph::model::Dataset.
pub type DatasetHandle = *mut UnsafeCell<oxigraph::model::Dataset>;

/// Create a new empty Dataset.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_new() -> *mut c_char {
    let boxed = Box::new(UnsafeCell::new(oxigraph::model::Dataset::new()));
    let ptr = Box::into_raw(boxed);
    let handle_value = ptr as u64;
    match serde_json::to_string(&handle_value) {
        Ok(json) => {
            let full = format!("{{\"ok\":{{\"handle\":{}}}}}", json);
            std::ffi::CString::new(full).unwrap().into_raw()
        }
        Err(e) => error_json(ErrorKind::Store { message: e.to_string() }),
    }
}

/// Create a Dataset with initial quads.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_from_quads(quads_json: *const c_char) -> *mut c_char {
    let json_str = unsafe { c_str_to_str(quads_json) };
    let quads: Vec<oxigraph::model::Quad> = match serde_json::from_str(json_str) {
        Ok(q) => q,
        Err(e) => return error_json(ErrorKind::InvalidArgument { message: format!("Invalid quads: {e}") }),
    };
    let mut dataset = oxigraph::model::Dataset::new();
    for quad in quads {
        dataset.insert(quad);
    }
    let boxed = Box::new(UnsafeCell::new(dataset));
    let ptr = Box::into_raw(boxed);
    let handle_value = ptr as u64;
    match serde_json::to_string(&handle_value) {
        Ok(json) => {
            let full = format!("{{\"ok\":{{\"handle\":{}}}}}", json);
            std::ffi::CString::new(full).unwrap().into_raw()
        }
        Err(e) => error_json(ErrorKind::Store { message: e.to_string() }),
    }
}

/// Insert a quad into the Dataset.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_insert(handle: DatasetHandle, quad_json: *const c_char) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Dataset handle is null".into() });
    }
    let dataset = unsafe { &mut *(*handle).get() };
    let quad = match parse_quad_value(unsafe { c_str_to_str(quad_json) }) {
        Ok(q) => q,
        Err(e) => return error_json(e),
    };
    dataset.insert(quad);
    ok_json(&"inserted")
}

/// Remove a quad from the Dataset. Returns error if not found.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_remove(handle: DatasetHandle, quad_json: *const c_char) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Dataset handle is null".into() });
    }
    let dataset = unsafe { &mut *(*handle).get() };
    let quad = match parse_quad_value(unsafe { c_str_to_str(quad_json) }) {
        Ok(q) => q,
        Err(e) => return error_json(e),
    };
    if dataset.remove(&quad) {
        ok_json(&"removed")
    } else {
        error_json(ErrorKind::InvalidArgument { message: "Quad not found in dataset".into() })
    }
}

/// Check if Dataset contains a quad.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_contains(handle: DatasetHandle, quad_json: *const c_char) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Dataset handle is null".into() });
    }
    let dataset = unsafe { &mut *(*handle).get() };
    let quad = match parse_quad_value(unsafe { c_str_to_str(quad_json) }) {
        Ok(q) => q,
        Err(e) => return error_json(e),
    };
    bool_to_response(dataset.contains(&quad))
}

/// Get Dataset quad count.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_count(handle: DatasetHandle) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Dataset handle is null".into() });
    }
    let dataset = unsafe { &mut *(*handle).get() };
    ok_json(&dataset.len())
}

/// Clear all quads from the Dataset.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_clear(handle: DatasetHandle) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Dataset handle is null".into() });
    }
    let dataset = unsafe { &mut *(*handle).get() };
    dataset.clear();
    ok_json(&"cleared")
}

/// Get all quads from the Dataset as JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_iter(handle: DatasetHandle) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Dataset handle is null".into() });
    }
    let dataset = unsafe { &mut *(*handle).get() };
    let quads: Vec<oxigraph::model::Quad> = dataset.iter().collect();
    ok_json(&quads)
}

/// Canonicalize Dataset with the given algorithm.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_canonicalize(handle: DatasetHandle, algorithm: *const c_char) -> *mut c_char {
    if handle.is_null() {
        return error_json(ErrorKind::InvalidArgument { message: "Dataset handle is null".into() });
    }
    let dataset = unsafe { &mut *(*handle).get() };
    let algo_str = unsafe { c_str_to_str(algorithm) };
    let algo = match algo_str {
        "unstable" => oxigraph::model::dataset::CanonicalizationAlgorithm::Unstable,
        "rdfc10_sha256" => oxigraph::model::dataset::CanonicalizationAlgorithm::Rdfc10 {
            hash_algorithm: oxigraph::model::dataset::CanonicalizationHashAlgorithm::Sha256,
        },
        "rdfc10_sha384" => oxigraph::model::dataset::CanonicalizationAlgorithm::Rdfc10 {
            hash_algorithm: oxigraph::model::dataset::CanonicalizationHashAlgorithm::Sha384,
        },
        _ => return error_json(ErrorKind::InvalidArgument { message: format!("Unknown canonicalization algorithm: {algo_str}") }),
    };
    dataset.canonicalize(algo);
    ok_json(&"canonicalized")
}

/// Destroy a Dataset and free memory.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_dataset_destroy(handle: DatasetHandle) {
    if handle.is_null() { return; }
    unsafe { drop(Box::from_raw(handle)); }
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
    use crate::error::oxigraph_free_string;
    use std::ffi::CString;

    unsafe extern "C" fn test_callback(args_json: *const c_char) -> *mut c_char {
        let json = unsafe { crate::model_ffi::c_str_to_str(args_json) };
        let parsed: Vec<Value> = serde_json::from_str(json).unwrap();
        if parsed.len() >= 2 {
            if let Some(v) = parsed[1].get("value") {
                let mut result = v.as_str().unwrap_or("").to_string();
                result.push_str("_test");
                let term = serde_json::json!({"type":"literal","value":result});
                let s = CString::new(serde_json::to_string(&term).unwrap()).unwrap();
                return s.into_raw();
            }
        }
        std::ptr::null_mut()
    }

    #[test]
    fn test_custom_function() {
        let name = CString::new("http://example.com/testfn").unwrap();
        let result = oxigraph_register_custom_function(name.as_ptr(), test_callback as CustomFnCallback);
        let json = unsafe { crate::model_ffi::c_str_to_str(result) };
        assert!(json.contains("\"ok\""));

        let handle = {
            let ptr = oxigraph_store_new();
            let json = unsafe { crate::model_ffi::c_str_to_str(ptr) };
            let v: Value = serde_json::from_str(json).unwrap();
            let h = v["ok"]["handle"].as_u64().unwrap() as StoreHandle;
            crate::error::oxigraph_free_string(ptr);
            h
        };

        let quad_json = r#"{"subject":{"type":"uri","value":"http://example.com/s"},"predicate":{"type":"uri","value":"http://example.com/p"},"object":{"type":"literal","value":"hello"},"graph":{"type":"default"}}"#;
        let quad_c = CString::new(quad_json).unwrap();
        let add_result = oxigraph_store_add(handle, quad_c.as_ptr());
        let add_json = unsafe { crate::model_ffi::c_str_to_str(add_result) };
        assert!(add_json.contains("\"ok\""));
        crate::error::oxigraph_free_string(add_result);

        let query_json = r#"{"query":"SELECT ?result WHERE { ?s ?p ?o . BIND(<http://example.com/testfn>(?o) AS ?result) }","base_iri":"","prefixes":{},"use_default_graph_as_union":false,"default_graph":null,"named_graphs":null}"#;
        let query_c = CString::new(query_json).unwrap();
        let query_result = oxigraph_store_query(handle, query_c.as_ptr());
        let query_str = unsafe { crate::model_ffi::c_str_to_str(query_result) };
        assert!(query_str.contains("hello_test"), "Expected hello_test in: {query_str}");
        crate::error::oxigraph_free_string(query_result);

        oxigraph_store_destroy(handle);
        let unreg = oxigraph_unregister_custom_function(name.as_ptr());
        crate::error::oxigraph_free_string(unreg);
    }

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
