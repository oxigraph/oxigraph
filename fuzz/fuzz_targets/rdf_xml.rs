#![no_main]

use libfuzzer_sys::fuzz_target;
use oxrdf::graph::CanonicalizationAlgorithm;
use oxrdf::{Graph, Subject, Term, Triple};
use oxrdfxml::{RdfXmlParser, RdfXmlSerializer};

fn parse(data: &[u8], unchecked: bool) -> (Vec<Triple>, Vec<String>) {
    let mut triples = Vec::new();
    let mut errors = Vec::new();
    let mut parser = RdfXmlParser::new();
    if unchecked {
        parser = parser.unchecked();
    }
    for result in parser.parse_slice(data) {
        match result {
            Ok(triple) => triples.push(triple),
            Err(error) => errors.push(error.to_string()),
        }
    }
    (triples, errors)
}

fn count_triple_blank_nodes(triple: &Triple) -> usize {
    (match &triple.subject {
        Subject::BlankNode(_) => 1,
        Subject::Triple(t) => count_triple_blank_nodes(t),
        _ => 0,
    }) + (match &triple.object {
        Term::BlankNode(_) => 1,
        Term::Triple(t) => count_triple_blank_nodes(t),
        _ => 0,
    })
}

fuzz_target!(|data: &[u8]| {
    // We parse
    let (triples, errors) = parse(data, false);

    // We test also unchecked if valid
    if errors.is_empty() {
        let (triples_unchecked, errors_unchecked) = parse(data, true);
        assert!(errors_unchecked.is_empty());

        let bnodes_count = triples.iter().map(count_triple_blank_nodes).sum::<usize>();
        if bnodes_count == 0 {
            assert_eq!(triples, triples_unchecked);
        } else if bnodes_count <= 4 {
            let mut graph_with_split = triples.iter().collect::<Graph>();
            let mut graph_unchecked = triples_unchecked.iter().collect::<Graph>();
            graph_with_split.canonicalize(CanonicalizationAlgorithm::Unstable);
            graph_unchecked.canonicalize(CanonicalizationAlgorithm::Unstable);
            assert_eq!(graph_with_split, graph_unchecked);
        }
    }

    // We serialize
    let mut writer = RdfXmlSerializer::new().serialize_to_write(Vec::new());
    for triple in &triples {
        writer.write_triple(triple).unwrap();
    }
    let new_serialization = writer.finish().unwrap();

    // We parse the serialization
    let new_triples = RdfXmlParser::new()
        .parse_slice(&new_serialization)
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
