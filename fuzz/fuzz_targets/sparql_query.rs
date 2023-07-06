#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::Query;

fuzz_target!(|data: &str| {
    let _ = Query::parse(data, None);
});
