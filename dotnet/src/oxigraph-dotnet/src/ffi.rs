use crate::error::{error_json, ok_json, ErrorKind};
use crate::model_ffi::{bool_to_response, c_str_to_str, parse_quad_value};
use oxigraph::store::Store;
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
