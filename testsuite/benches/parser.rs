#![allow(clippy::print_stderr)]

use anyhow::Result;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxigraph_testsuite::files::read_file;
use oxigraph_testsuite::manifest::TestManifest;
use rio_api::parser::*;
use rio_turtle::*;
use std::io::Read;

fn test_data_from_testsuite(manifest_uri: String, include_tests_types: &[&str]) -> Result<Vec<u8>> {
    let manifest = TestManifest::new([manifest_uri]);
    let mut data = Vec::default();
    for test in manifest {
        let test = test?;
        if include_tests_types.contains(&test.kind.as_str()) {
            read_file(&test.action.unwrap())?.read_to_end(&mut data)?;
            data.push(b'\n');
        }
    }
    Ok(data)
}

fn ntriples_test_data() -> Result<Vec<u8>> {
    test_data_from_testsuite(
        "http://w3c.github.io/rdf-tests/ntriples/manifest.ttl".to_owned(),
        &["http://www.w3.org/ns/rdftest#TestNTriplesPositiveSyntax"],
    )
}

fn turtle_test_data() -> Result<Vec<u8>> {
    test_data_from_testsuite(
        "http://w3c.github.io/rdf-tests/turtle/manifest.ttl".to_owned(),
        &[
            "http://www.w3.org/ns/rdftest#TestTurtlePositiveSyntax",
            "http://www.w3.org/ns/rdftest#TestTurtleEval",
        ],
    )
}

fn parse_bench(
    c: &mut Criterion,
    parser_name: &str,
    data_name: &str,
    data: &[u8],
    bench: impl Fn(&[u8]),
) {
    let mut group = c.benchmark_group(parser_name);
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.bench_with_input(BenchmarkId::from_parameter(data_name), &data, |b, data| {
        b.iter(|| bench(data))
    });
    group.finish();
}

fn parse_oxttl_ntriples(c: &mut Criterion, name: &str, data: &[u8]) {
    parse_bench(c, "oxttl ntriples", name, data, |data| {
        let mut parser = oxttl::NTriplesParser::new().parse();
        parser.extend_from_slice(data);
        parser.end();
        while let Some(result) = parser.read_next() {
            result.unwrap();
        }
    });
}

fn parse_oxttl_turtle(c: &mut Criterion, name: &str, data: &[u8]) {
    parse_bench(c, "oxttl turtle", name, data, |data| {
        let mut parser = oxttl::TurtleParser::new().parse();
        parser.extend_from_slice(data);
        parser.end();
        while let Some(result) = parser.read_next() {
            result.unwrap();
        }
    });
}

fn parse_rio_ntriples(c: &mut Criterion, name: &str, data: &[u8]) {
    parse_bench(c, "rio ntriples", name, data, |data| {
        let mut count: u64 = 0;
        NTriplesParser::new(data)
            .parse_all::<TurtleError>(&mut |_| {
                count += 1;
                Ok(())
            })
            .unwrap();
    });
}

fn parse_rio_turtle(c: &mut Criterion, name: &str, data: &[u8]) {
    parse_bench(c, "rio turtle", name, data, |data| {
        let mut count: u64 = 0;
        TurtleParser::new(data, None)
            .parse_all::<TurtleError>(&mut |_| {
                count += 1;
                Ok(())
            })
            .unwrap();
    });
}

fn bench_parse_oxttl_ntriples_with_ntriples(c: &mut Criterion) {
    parse_oxttl_ntriples(
        c,
        "ntriples",
        &match ntriples_test_data() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        },
    )
}

fn bench_parse_oxttl_ntriples_with_turtle(c: &mut Criterion) {
    parse_oxttl_turtle(
        c,
        "ntriples",
        &match ntriples_test_data() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        },
    )
}

fn bench_parse_oxttl_turtle_with_turtle(c: &mut Criterion) {
    parse_oxttl_turtle(
        c,
        "turtle",
        &match turtle_test_data() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        },
    )
}

fn bench_parse_rio_ntriples_with_ntriples(c: &mut Criterion) {
    parse_rio_ntriples(
        c,
        "ntriples",
        &match ntriples_test_data() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        },
    )
}

fn bench_parse_rio_ntriples_with_turtle(c: &mut Criterion) {
    parse_rio_turtle(
        c,
        "ntriples",
        &match ntriples_test_data() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        },
    )
}

fn bench_parse_rio_turtle_with_turtle(c: &mut Criterion) {
    parse_rio_turtle(
        c,
        "turtle",
        &match turtle_test_data() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        },
    )
}

criterion_group!(
    w3c_testsuite,
    bench_parse_rio_ntriples_with_ntriples,
    bench_parse_rio_ntriples_with_turtle,
    bench_parse_rio_turtle_with_turtle,
    bench_parse_oxttl_ntriples_with_ntriples,
    bench_parse_oxttl_ntriples_with_turtle,
    bench_parse_oxttl_turtle_with_turtle
);

criterion_main!(w3c_testsuite);
