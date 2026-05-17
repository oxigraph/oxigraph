#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::Query;
use std::str::FromStr;

fuzz_target!(|data: &str| {
    if let Ok(query) = Query::from_str(data) {
        let roundtrip = Query::from_str(&query.to_string()).unwrap();
        assert_eq!(query.to_string(), roundtrip.to_string());
    }
});
