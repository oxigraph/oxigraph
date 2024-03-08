#![no_main]

use libfuzzer_sys::fuzz_target;
use oxrdfxml::{RdfXmlParser, RdfXmlSerializer};

fuzz_target!(|data: &[u8]| {
    // We parse
    let triples = RdfXmlParser::new()
        .parse_read(data)
        .flatten()
        .collect::<Vec<_>>();

    // We serialize
    let mut writer = RdfXmlSerializer::new().serialize_to_write(Vec::new());
    for triple in &triples {
        writer.write_triple(triple).unwrap();
    }
    let new_serialization = writer.finish().unwrap();

    // We parse the serialization
    let new_triples = RdfXmlParser::new()
        .parse_read(new_serialization.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            format!(
                "Error on {:?} from {triples:?} based on {:?}: {e}",
                String::from_utf8_lossy(&new_serialization),
                String::from_utf8_lossy(data)
            )
        })
        .unwrap();

    // We check the roundtrip has not changed anything
    assert_eq!(new_triples, triples);
});
