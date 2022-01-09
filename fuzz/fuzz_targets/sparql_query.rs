#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::Query;
use std::str;

fuzz_target!(|data: &[u8]| {
    if let Ok(data) = str::from_utf8(data) {
        Query::parse(data, None);
    }
});
