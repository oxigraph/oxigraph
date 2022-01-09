#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::Update;
use std::str;

fuzz_target!(|data: &[u8]| {
    if let Ok(data) = str::from_utf8(data) {
        Update::parse(data, None);
    }
});
