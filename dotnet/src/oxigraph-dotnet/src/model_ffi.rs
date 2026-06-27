use crate::error::{ok_json, ErrorKind};
use oxigraph::model::Quad;
use serde_json::Value;
use std::os::raw::c_char;

/// Convert a C string to a Rust &str.
pub unsafe fn c_str_to_str<'a>(ptr: *const c_char) -> &'a str {
    if ptr.is_null() {
        return "";
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(ptr) };
    c_str.to_str().unwrap_or("")
}

/// Deserialize a Quad from JSON.
pub fn quad_from_json(json: &str) -> Result<Quad, ErrorKind> {
    serde_json::from_str::<Quad>(json).map_err(|e| ErrorKind::InvalidArgument {
        message: format!("Invalid Quad JSON: {e}"),
    })
}

/// Serialize a Quad to a JSON response pointer.
pub fn quad_to_response(quad: &Quad) -> *mut c_char {
    // Serialize the quad with the standard serde format used by oxrdf.
    // The oxrdf types produce JSON like:
    //   {"subject":{"type":"uri","value":"http://..."},"predicate":{"type":"uri","value":"http://..."},
    //    "object":{"type":"literal","value":"..."},"graph":{"type":"default"}}
    //
    // For contains: we just need {"ok": true} or {"ok": false}
    // For add/remove: we just need {"ok": "ok"}
    ok_json(&serde_json::to_value(quad).unwrap_or_default())
}

/// Serialize a boolean result to JSON response pointer.
pub fn bool_to_response(value: bool) -> *mut c_char {
    ok_json(&value)
}

/// Serialize a Quad to a JSON string (used for the C# side to send).
#[allow(dead_code)]
pub fn quad_to_json(quad: &Quad) -> String {
    serde_json::to_string(quad).unwrap_or_default()
}

/// Parse a Quad from a JSON value (used when the C# side sends a Quad).
pub fn parse_quad_value(json: &str) -> Result<Quad, ErrorKind> {
    let v: Value =
        serde_json::from_str(json).map_err(|e| ErrorKind::InvalidArgument {
            message: format!("Invalid JSON: {e}"),
        })?;
    serde_json::from_value(v).map_err(|e| ErrorKind::InvalidArgument {
        message: format!("Invalid Quad: {e}"),
    })
}
