#![allow(clippy::print_stderr)]

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use oxigraph::io::{JsonLdProfile, JsonLdProfileSet, RdfFormat, RdfParser};
use oxigraph_testsuite::files::read_file;
use oxigraph_testsuite::manifest::TestManifest;
use oxrdf::Dataset;
use oxrdf::dataset::{CanonicalizationAlgorithm, CanonicalizationHashAlgorithm};
use std::io::Read;

fn test_data_from_testsuite(manifest_uri: String, include_tests_types: &[&str]) -> Vec<u8> {
    let manifest = TestManifest::new([manifest_uri]);
    let mut data = Vec::new();
    for test in manifest {
        let test = test.unwrap();
        if test
            .kinds
            .iter()
            .any(|kind| include_tests_types.contains(&kind.as_str()))
        {
            read_file(&test.action.unwrap())
                .unwrap()
                .read_to_end(&mut data)
                .unwrap();
            data.push(b'\n');
        }
    }
    data
}

fn ntriples_test_data() -> Vec<u8> {
    test_data_from_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-n-triples/manifest.ttl".to_owned(),
        &["http://www.w3.org/ns/rdftest#TestNTriplesPositiveSyntax"],
    )
}

fn turtle_test_data() -> Vec<u8> {
    test_data_from_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-turtle/manifest.ttl".to_owned(),
        &[
            "http://www.w3.org/ns/rdftest#TestTurtlePositiveSyntax",
            "http://www.w3.org/ns/rdftest#TestTurtleEval",
        ],
    )
}

fn json_test_data_from_testsuite(manifest_uri: String, include_tests_types: &[&str]) -> Vec<u8> {
    let manifest = TestManifest::new([manifest_uri]);
    let mut data = Vec::new();
    data.push(b'[');
    for test in manifest {
        let test = test.unwrap();
        if test
            .kinds
            .iter()
            .any(|kind| include_tests_types.contains(&kind.as_str()))
            && test.option.is_empty()
            && test.id.as_str()
                != "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tv006"
        // TODO: remove
        {
            read_file(&test.action.unwrap())
                .unwrap()
                .read_to_end(&mut data)
                .unwrap();
            data.push(b',');
        }
    }
    if data.len() > 1 {
        data.pop(); // Remove last ','
    }
    data.push(b']');
    data
}

fn jsonld_test_data() -> Vec<u8> {
    json_test_data_from_testsuite(
        "https://w3c.github.io/json-ld-api/tests/toRdf-manifest.jsonld".to_owned(),
        &["https://w3c.github.io/json-ld-api/tests/vocab#PositiveEvaluationTest"],
    )
}

fn streaming_jsonld_test_data() -> Vec<u8> {
    json_test_data_from_testsuite(
        "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld".to_owned(),
        &["https://w3c.github.io/json-ld-api/tests/vocab#PositiveEvaluationTest"],
    )
}

fn parse_bench(
    c: &mut Criterion,
    parser_name: &str,
    data_name: &str,
    format: RdfFormat,
    data: &[u8],
) {
    let mut group = c.benchmark_group(parser_name);
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.bench_with_input(data_name, &data, |b, data| {
        b.iter(|| {
            for result in RdfParser::from_format(format).for_slice(data) {
                result.unwrap();
            }
        })
    });
    group.finish();
}

fn bench_parse_ntriples_with_ntriples(c: &mut Criterion) {
    parse_bench(
        c,
        "oxttl ntriples",
        "ntriples",
        RdfFormat::NTriples,
        &ntriples_test_data(),
    )
}

fn bench_parse_ntriples_with_turtle(c: &mut Criterion) {
    parse_bench(
        c,
        "oxttl turtle",
        "ntriples",
        RdfFormat::Turtle,
        &ntriples_test_data(),
    )
}

fn bench_parse_turtle_with_turtle(c: &mut Criterion) {
    parse_bench(
        c,
        "oxttl turtle",
        "turtle",
        RdfFormat::Turtle,
        &turtle_test_data(),
    )
}

fn bench_parse_jsonld_with_jsonld(c: &mut Criterion) {
    parse_bench(
        c,
        "oxjsonld",
        "jsonld",
        RdfFormat::JsonLd {
            profile: JsonLdProfileSet::empty(),
        },
        &jsonld_test_data(),
    )
}

fn bench_parse_streaming_jsonld_with_jsonld(c: &mut Criterion) {
    parse_bench(
        c,
        "oxjsonld",
        "streaming jsonld",
        RdfFormat::JsonLd {
            profile: JsonLdProfileSet::empty(),
        },
        &streaming_jsonld_test_data(),
    )
}

fn bench_parse_streaming_jsonld_with_streaming_jsonld(c: &mut Criterion) {
    parse_bench(
        c,
        "oxjsonld streaming",
        "streaming jsonld",
        RdfFormat::JsonLd {
            profile: JsonLdProfile::Streaming.into(),
        },
        &streaming_jsonld_test_data(),
    )
}

fn canonicalization_test_data_from_testsuite() -> Dataset {
    let manifest = TestManifest::new(["https://w3c.github.io/rdf-canon/tests/manifest.ttl"]);
    let mut dataset = Dataset::new();
    for test in manifest {
        let test = test.unwrap();
        if test.kinds.iter().any(|kind| {
            kind.as_str() == "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10EvalTest"
        }) {
            for q in RdfParser::from_format(RdfFormat::NQuads)
                .rename_blank_nodes()
                .for_reader(read_file(&test.action.unwrap()).unwrap())
            {
                dataset.insert(&q.unwrap());
            }
        }
    }
    dataset
}

fn canonicalization_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("canonicalization");
    let dataset = canonicalization_test_data_from_testsuite();
    for (alg, alg_name) in [
        (CanonicalizationAlgorithm::Unstable, "unstable"),
        (
            CanonicalizationAlgorithm::Rdfc10 {
                hash_algorithm: CanonicalizationHashAlgorithm::Sha256,
            },
            "RDFC-1.0 (SHA256)",
        ),
    ] {
        group.bench_function(alg_name, |b| {
            b.iter(|| {
                dataset.clone().canonicalize(alg);
            });
        });
    }
    group.finish();
}

criterion_group!(
    w3c_testsuite,
    bench_parse_ntriples_with_ntriples,
    bench_parse_ntriples_with_turtle,
    bench_parse_turtle_with_turtle,
    bench_parse_jsonld_with_jsonld,
    bench_parse_streaming_jsonld_with_jsonld,
    bench_parse_streaming_jsonld_with_streaming_jsonld,
    canonicalization_bench
);

criterion_main!(w3c_testsuite);
