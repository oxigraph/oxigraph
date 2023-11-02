#![no_main]

use libfuzzer_sys::fuzz_target;
use oxrdf::{Dataset, GraphName, Quad, Subject, Term, Triple};
use oxttl::{TriGParser, TriGSerializer};

fn parse<'a>(chunks: impl IntoIterator<Item = &'a [u8]>) -> (Vec<Quad>, Vec<String>) {
    let mut quads = Vec::new();
    let mut errors = Vec::new();
    let mut parser = TriGParser::new()
        .with_quoted_triples()
        .with_base_iri("http://example.com/")
        .unwrap()
        .parse();
    for chunk in chunks {
        parser.extend_from_slice(chunk);
        while let Some(result) = parser.read_next() {
            match result {
                Ok(quad) => quads.push(quad),
                Err(error) => errors.push(error.to_string()),
            }
        }
    }
    parser.end();
    while let Some(result) = parser.read_next() {
        match result {
            Ok(quad) => quads.push(quad),
            Err(error) => errors.push(error.to_string()),
        }
    }
    assert!(parser.is_end());
    (quads, errors)
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

fn count_quad_blank_nodes(quad: &Quad) -> usize {
    (match &quad.subject {
        Subject::BlankNode(_) => 1,
        Subject::Triple(t) => count_triple_blank_nodes(t),
        _ => 0,
    }) + (match &quad.object {
        Term::BlankNode(_) => 1,
        Term::Triple(t) => count_triple_blank_nodes(t),
        _ => 0,
    }) + usize::from(matches!(quad.graph_name, GraphName::BlankNode(_)))
}

fn serialize_quads(quads: &[Quad]) -> Vec<u8> {
    let mut writer = TriGSerializer::new().serialize_to_write(Vec::new());
    for quad in quads {
        writer.write_quad(quad).unwrap();
    }
    writer.finish().unwrap()
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
    let bnodes_count = quads.iter().map(count_quad_blank_nodes).sum::<usize>();
    if bnodes_count == 0 {
        assert_eq!(
            quads,
            quads_without_split,
            "With split:\n{}\nWithout split:\n{}",
            String::from_utf8_lossy(&serialize_quads(&quads)),
            String::from_utf8_lossy(&serialize_quads(&quads_without_split))
        );
    } else if bnodes_count <= 4 {
        let mut dataset_with_split = quads.iter().collect::<Dataset>();
        let mut dataset_without_split = quads_without_split.iter().collect::<Dataset>();
        dataset_with_split.canonicalize();
        dataset_without_split.canonicalize();
        assert_eq!(
            dataset_with_split,
            dataset_without_split,
            "With split:\n{}\nWithout split:\n{}",
            String::from_utf8_lossy(&serialize_quads(&quads)),
            String::from_utf8_lossy(&serialize_quads(&quads_without_split))
        );
    }
    assert_eq!(errors, errors_without_split);

    // We serialize
    let new_serialization = serialize_quads(&quads);

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