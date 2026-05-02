use js_sys::{JsString, Reflect};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsThreadLocal, JsValue};

pub fn reflect_get(
    target: &JsValue,
    key: &'static JsThreadLocal<JsString>,
) -> Result<JsValue, JsValue> {
    key.with(|key| Reflect::get(target, key))
}

#[cfg(feature = "rdf-12")]
pub fn reflect_set(
    target: &JsValue,
    key: &'static JsThreadLocal<JsString>,
    value: &JsValue,
) -> Result<bool, JsValue> {
    key.with(|key| Reflect::set(target, key, value))
}

#[rustfmt::skip]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static BASE_IRI: JsString = "base_iri";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static DATA_FACTORY: JsString = "data_factory";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static DATATYPE: JsString = "datatype";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static DEFAULT_GRAPH: JsString = "default_graph";

    #[cfg(feature = "rdf-12")]
    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static DIRECTION: JsString = "direction";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static FROM_GRAPH_NAME: JsString = "from_graph_name";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static FORMAT: JsString = "format";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static GRAPH: JsString = "graph";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static SUBJECT: JsString = "subject";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static LANGUAGE: JsString = "language";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static LENIENT: JsString = "lenient";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static LTR: JsString = "ltr";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static NAMED_GRAPHS: JsString = "named_graphs";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static NO_TRANSACTION: JsString = "no_transaction";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static OBJECT: JsString = "object";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static PREDICATE: JsString = "predicate";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static RESULTS_FORMAT: JsString = "results_format";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static RTL: JsString = "rtl";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static TERM_TYPE: JsString = "termType";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static TO_GRAPH_NAME: JsString = "to_graph_name";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static USED_DEFAULT_GRAPH_AS_UNION: JsString = "use_default_graph_as_union";

    #[wasm_bindgen(thread_local_v2, static_string)]
    pub static VALUE: JsString = "value";
}
