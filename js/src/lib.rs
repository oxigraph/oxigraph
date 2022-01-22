#![allow(clippy::unused_unit)]
use wasm_bindgen::prelude::*;

mod model;
mod store;
mod utils;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}
