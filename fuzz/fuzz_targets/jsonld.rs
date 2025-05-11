#![no_main]

use libfuzzer_sys::fuzz_target;
use oxigraph_fuzz::count_quad_blank_nodes;
use oxjsonld::{JsonLdParser, JsonLdProfile, JsonLdSerializer};
use oxrdf::graph::CanonicalizationAlgorithm;
use oxrdf::Dataset;

fn parse(
    input: &[u8],
    lenient: bool,
    streaming: bool,
) -> (Dataset, Vec<String>, Vec<(String, String)>, Option<String>) {
    let mut quads = Dataset::new();
    let mut errors = Vec::new();
    let mut parser = JsonLdParser::new()
        .with_base_iri("http://example.com/")
        .unwrap();
    if lenient {
        parser = parser.lenient();
    }
    if streaming {
        parser = parser.with_profile(JsonLdProfile::Streaming);
    }
    let mut parser = parser.for_slice(input);
    for result in &mut parser {
        match result {
            Ok(quad) => {
                quads.insert(&quad);
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    (
        quads,
        errors,
        parser
            .prefixes()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        parser.base_iri().map(ToString::to_string),
    )
}

fn serialize_quads(
    quads: &Dataset,
    prefixes: Vec<(String, String)>,
    base_iri: Option<String>,
) -> Vec<u8> {
    let mut serializer = JsonLdSerializer::new();
    for (prefix_name, prefix_iri) in prefixes {
        serializer = serializer.with_prefix(prefix_name, prefix_iri).unwrap();
    }
    if let Some(base_iri) = base_iri {
        serializer = serializer.with_base_iri(base_iri).unwrap();
    }
    let mut serializer = serializer.for_writer(Vec::new());
    for quad in quads {
        serializer.serialize_quad(quad).unwrap();
    }
    serializer.finish().unwrap()
}

fuzz_target!(|data: &[u8]| {
    // We parse with splitting
    let (mut quads, errors, prefixes, base_iri) = parse(data, false, false);
    let (mut quads_streaming, errors_streaming, _, _) = parse(data, false, true);
    let (_, errors_lenient, _, _) = parse(data, true, false);
    if errors_streaming.is_empty() {
        assert!(errors.is_empty());
    }
    if errors.is_empty() {
        assert!(errors_lenient.is_empty());
    }

    if errors_streaming.is_empty() {
        let bnodes_count = quads
            .iter()
            .map(|q| count_quad_blank_nodes(q))
            .sum::<usize>();
        if bnodes_count <= 4 {
            quads.canonicalize(CanonicalizationAlgorithm::Unstable);
            quads_streaming.canonicalize(CanonicalizationAlgorithm::Unstable);
            assert_eq!(
                quads,
                quads_streaming,
                "Buffering:\n{}\nStreaming:\n{}",
                String::from_utf8_lossy(&serialize_quads(&quads, Vec::new(), None)),
                String::from_utf8_lossy(&serialize_quads(&quads_streaming, Vec::new(), None))
            );
        }
    }

    // We serialize
    let new_serialization = serialize_quads(&quads, prefixes, base_iri);

    // We parse the serialization
    let new_quads = JsonLdParser::new()
        .with_profile(JsonLdProfile::Streaming)
        .for_slice(&new_serialization)
        .collect::<Result<Dataset, _>>()
        .map_err(|e| {
            format!(
                "Error on {:?} from {quads:?} based on {:?}: {e}",
                String::from_utf8_lossy(&new_serialization),
                String::from_utf8_lossy(data)
            )
        })
        .unwrap();

    // We check the roundtrip has not changed anything
    assert_eq!(
        new_quads,
        quads,
        "{}\n{new_quads}\n{quads}",
        String::from_utf8_lossy(&new_serialization),
    );
});
