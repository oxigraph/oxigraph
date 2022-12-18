#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::Update;
use std::str;

fuzz_target!(|data: &str| {
    Update::parse(data, None);
});
