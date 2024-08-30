#![no_main]

use libfuzzer_sys::fuzz_target;
use oxttl::N3Parser;

fuzz_target!(|data: &[u8]| {
    let mut quads = Vec::new();
    let mut parser = N3Parser::new()
        .with_base_iri("http://example.com/")
        .unwrap()
        .low_level();
    for chunk in data.split(|c| *c == 0xFF) {
        parser.extend_from_slice(chunk);
        while let Some(result) = parser.parse_next() {
            if let Ok(quad) = result {
                quads.push(quad);
            }
        }
    }
    parser.end();
    while let Some(result) = parser.parse_next() {
        if let Ok(quad) = result {
            quads.push(quad);
        }
    }
    assert!(parser.is_end());
    //TODO: serialize
});
