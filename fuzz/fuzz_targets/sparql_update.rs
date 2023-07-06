#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::Update;
use std::str;

fuzz_target!(|data: &str| {
    let _ = Update::parse(data, None);
});
