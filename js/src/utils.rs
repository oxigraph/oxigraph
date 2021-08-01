use js_sys::Error;
use wasm_bindgen::JsValue;

#[macro_export]
macro_rules! format_err {
    ($msg:literal $(,)?) => {
        ::wasm_bindgen::JsValue::from(::js_sys::Error::new($msg))
    };
    ($fmt:literal, $($arg:tt)*) => {
        ::wasm_bindgen::JsValue::from(::js_sys::Error::new(&format!($fmt, $($arg)*)))
    };
}

pub fn to_err(e: impl ToString) -> JsValue {
    JsValue::from(Error::new(&e.to_string()))
}
