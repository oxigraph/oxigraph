#![no_main]
use libfuzzer_sys::fuzz_target;
use spargebra::Update;
use std::str;
use std::str::FromStr;

fuzz_target!(|data: &str| {
    if let Ok(update) = Update::from_str(data) {
        let roundtrip = Update::from_str(&update.to_string()).unwrap();
        assert_eq!(update.to_string(), roundtrip.to_string());
    }
});
