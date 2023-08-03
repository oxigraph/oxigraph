#![no_main]

use libfuzzer_sys::fuzz_target;
use oxttl::{TriGParser, TriGSerializer};

fuzz_target!(|data: &[u8]| {
    // We parse
    let mut quads = Vec::new();
    let mut parser = TriGParser::new()
        .with_quoted_triples()
        .with_base_iri("http://example.com/")
        .unwrap()
        .parse();
    for chunk in data.split(|c| *c == 0xFF) {
        parser.extend_from_slice(chunk);
        while let Some(result) = parser.read_next() {
            if let Ok(quad) = result {
                quads.push(quad);
            }
        }
    }
    parser.end();
    while let Some(result) = parser.read_next() {
        if let Ok(quad) = result {
            quads.push(quad);
        }
    }
    assert!(parser.is_end());

    // We serialize
    let mut writer = TriGSerializer::new().serialize_to_write(Vec::new());
    for quad in &quads {
        writer.write_quad(quad).unwrap();
    }
    let new_serialization = writer.finish().unwrap();

    // We parse the serialization
    let new_quads = TriGParser::new()
        .with_quoted_triples()
        .parse_read(new_serialization.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            format!(
                "Error on {:?} from {quads:?} based on {:?}: {e}",
                String::from_utf8_lossy(&new_serialization),
                String::from_utf8_lossy(data)
            )
        })
        .unwrap();

    // We check the roundtrip has not changed anything
    assert_eq!(new_quads, quads);
});
