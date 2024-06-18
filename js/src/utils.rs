use js_sys::Error;
use wasm_bindgen::prelude::*;

#[macro_export]
macro_rules! format_err {
    ($msg:literal $(,)?) => {
        ::wasm_bindgen::JsValue::from(::js_sys::Error::new($msg))
    };
    ($fmt:literal, $($arg:tt)*) => {
        ::wasm_bindgen::JsValue::from(::js_sys::Error::new(&format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! console_warn {
    ($($t:tt)*) => ($crate::utils::warn(&format_args!($($t)*).to_string()))
}

#[allow(clippy::needless_pass_by_value)]
pub fn to_err(e: impl ToString) -> JsValue {
    JsValue::from(Error::new(&e.to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[allow(unsafe_code)]
    #[wasm_bindgen(js_namespace = console)]
    pub(crate) fn warn(s: &str);
}
