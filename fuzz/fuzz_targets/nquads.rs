#![no_main]

use libfuzzer_sys::fuzz_target;
use oxrdf::Quad;
use oxttl::{NQuadsParser, NQuadsSerializer, SyntaxError};

fn parse<'a>(chunks: impl IntoIterator<Item = &'a [u8]>) -> (Vec<Quad>, Vec<SyntaxError>) {
    let mut quads = Vec::new();
    let mut errors = Vec::new();
    let mut parser = NQuadsParser::new().with_quoted_triples().parse();
    for chunk in chunks {
        parser.extend_from_slice(chunk);
        while let Some(result) = parser.read_next() {
            match result {
                Ok(quad) => quads.push(quad),
                Err(error) => errors.push(error),
            }
        }
    }
    parser.end();
    while let Some(result) = parser.read_next() {
        match result {
            Ok(quad) => quads.push(quad),
            Err(error) => errors.push(error),
        }
    }
    assert!(parser.is_end());
    (quads, errors)
}

fuzz_target!(|data: &[u8]| {
    // We parse with splitting
    let (quads, errors) = parse(data.split(|c| *c == 0xFF));
    // We parse without splitting
    let (quads_without_split, errors_without_split) = parse([data
        .iter()
        .copied()
        .filter(|c| *c != 0xFF)
        .collect::<Vec<_>>()
        .as_slice()]);
    assert_eq!(quads, quads_without_split);
    assert_eq!(errors.len(), errors_without_split.len());

    // We serialize
    let mut writer = NQuadsSerializer::new().serialize_to_write(Vec::new());
    for quad in &quads {
        writer.write_quad(quad).unwrap();
    }
    let new_serialization = writer.finish();

    // We parse the serialization
    let new_quads = NQuadsParser::new()
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
