#![no_main]

use libfuzzer_sys::fuzz_target;
use oxrdf::Quad;
use oxttl::{NQuadsParser, NQuadsSerializer};
use std::str;
use std::str::FromStr;

fn parse<'a>(
    chunks: impl IntoIterator<Item = &'a [u8]>,
    unchecked: bool,
) -> (Vec<Quad>, Vec<String>) {
    let mut quads = Vec::new();
    let mut errors = Vec::new();
    let mut parser = NQuadsParser::new().with_quoted_triples();
    if unchecked {
        parser = parser.unchecked();
    }
    let mut reader = parser.parse();
    for chunk in chunks {
        reader.extend_from_slice(chunk);
        while let Some(result) = reader.read_next() {
            match result {
                Ok(quad) => quads.push(quad),
                Err(error) => errors.push(error.to_string()),
            }
        }
    }
    reader.end();
    while let Some(result) = reader.read_next() {
        match result {
            Ok(quad) => quads.push(quad),
            Err(error) => errors.push(error.to_string()),
        }
    }
    assert!(reader.is_end());
    (quads, errors)
}

fuzz_target!(|data: &[u8]| {
    // We parse with splitting
    let (quads, errors) = parse(data.split(|c| *c == 0xFF), false);
    // We parse without splitting
    let data_without_breaks = data
        .iter()
        .copied()
        .filter(|c| *c != 0xFF)
        .collect::<Vec<_>>();
    let (quads_without_split, errors_without_split) =
        parse([data_without_breaks.as_slice()], false);
    assert_eq!(quads, quads_without_split);
    assert_eq!(errors, errors_without_split);

    // We test also unchecked if valid
    if errors.is_empty() {
        let (quads_unchecked, errors_unchecked) = parse(data.split(|c| *c == 0xFF), true);
        assert!(errors_unchecked.is_empty());
        assert_eq!(quads, quads_unchecked);
    }

    // We serialize
    let mut writer = NQuadsSerializer::new().serialize_to_write(Vec::new());
    for quad in &quads {
        writer.write_quad(quad).unwrap();
    }
    let new_serialization = writer.finish();

    // We parse the serialization
    let new_quads = NQuadsParser::new()
        .with_quoted_triples()
        .parse_slice(&new_serialization)
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

    // We parse with Quad::from_str if there is no comments
    if !data_without_breaks.contains(&b'#') {
        match str::from_utf8(&data_without_breaks)
            .map_err(|e| e.to_string())
            .and_then(|d| {
                d.split('\n')
                    .filter(|l| !l.trim().is_empty())
                    .map(Quad::from_str)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())
            }) {
            Ok(term_quads) => {
                for quad in quads {
                    assert!(
                        term_quads.contains(&quad),
                        "Quad::from_str has not managed to parse {quad}"
                    )
                }
            }
            Err(e) => {
                if errors.is_empty() {
                    println!("{}", String::from_utf8_lossy(&data_without_breaks));
                }
                assert!(
                    !errors.is_empty(),
                    "Unexpected error from Quad::from_str: {e}"
                )
            }
        }
    }
});
