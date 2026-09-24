#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::Update;
use std::fmt::Write;
use std::str::FromStr;

fuzz_target!(|data: &str| {
    if let Ok(update) = Update::from_str(data) {
        let mut serialization = String::new();
        if write!(&mut serialization, "{update}").is_err() {
            return; // TODO: fix these
        }
        let roundtrip = Update::from_str(&serialization).unwrap();
        assert_eq!(serialization, roundtrip.to_string());
    }
});
