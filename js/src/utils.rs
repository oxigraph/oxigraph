use wasm_bindgen::prelude::*;

#[macro_export]
macro_rules! format_err {
    ($msg:literal $(,)?) => {
        ::wasm_bindgen::JsValue::from(::js_sys::Error::new(&format!($msg)))
    };
    ($fmt:literal, $($arg:tt)*) => {
        ::wasm_bindgen::JsValue::from(::js_sys::Error::new(&format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! console_warn {
    ($($t:tt)*) => ($crate::utils::warn(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub(crate) fn warn(s: &str);
}
