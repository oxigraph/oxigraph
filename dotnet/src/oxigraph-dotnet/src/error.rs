use serde::Serialize;
use serde_json::json;
use std::ffi::CString;
use std::os::raw::c_char;

/// Error returned across FFI boundary as JSON.
#[derive(Serialize)]
#[serde(tag = "kind")]
pub enum ErrorKind {
    #[serde(rename = "store")]
    Store { message: String },
    #[serde(rename = "parse")]
    Parse {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        file: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
    },
    #[serde(rename = "invalid_argument")]
    InvalidArgument { message: String },
}

/// Convert a serializable value to a JSON `{"ok": value}` response string as `*mut c_char`.
/// The caller must free the returned pointer with `oxigraph_free_string`.
pub fn ok_json<T: Serialize>(value: &T) -> *mut c_char {
    let body = serde_json::to_value(value).unwrap_or_default();
    let json = serde_json::to_string(&json!({"ok": body})).unwrap_or_else(|e| {
        serde_json::to_string(&json!({"error": {"kind": "internal", "message": e.to_string()}}))
            .unwrap()
    });
    CString::new(json).unwrap().into_raw()
}

/// Build a JSON `{"ok": value}` response string from a JSON `Value`.
#[allow(dead_code)]
pub fn ok_value(value: &serde_json::Value) -> *mut c_char {
    let json = serde_json::to_string(&json!({"ok": value})).unwrap();
    CString::new(json).unwrap().into_raw()
}

/// Build a JSON `{"error": ...}` response string.
pub fn error_json(error: ErrorKind) -> *mut c_char {
    let json = serde_json::to_string(&json!({"error": error})).unwrap();
    CString::new(json).unwrap().into_raw()
}

/// Free a string previously returned by an FFI function.
#[unsafe(no_mangle)]
pub extern "C" fn oxigraph_free_string(ptr: *mut std::os::raw::c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
