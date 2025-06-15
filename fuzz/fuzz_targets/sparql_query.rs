#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::SparqlParser;

fuzz_target!(|data: &str| {
    let _ = SparqlParser::new().parse_query(data);
});
