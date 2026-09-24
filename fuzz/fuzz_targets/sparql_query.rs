#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::Query;
use std::fmt::Write;
use std::str::FromStr;

fuzz_target!(|data: &str| {
    if let Ok(query) = Query::from_str(data) {
        let mut serialization = String::new();
        if write!(&mut serialization, "{query}").is_err() {
            return; // TODO: fix these
        }
        let roundtrip = Query::from_str(&serialization).unwrap();
        assert_eq!(serialization, roundtrip.to_string());
    }
});
