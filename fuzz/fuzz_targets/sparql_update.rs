#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::SparqlParser;
use std::str;

fuzz_target!(|data: &str| {
    let _ = SparqlParser::new().parse_update(data);
});
