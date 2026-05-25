//! Rust harness for the reasonable side of the subclass closure benchmark.
//!
//! Usage: `fixpoint_bench_reasonable reasonable <path-to-turtle>`
//!
//! The single positional engine argument is accepted for symmetry with the
//! oxigraph-side binary, so the Python driver can subprocess either binary
//! with the same argv shape.
//!
//! Prints one JSON line to stdout:
//!
//! ```json
//! {"engine":"reasonable","load_ms":12.3,"compute_ms":45.6,"triples_in":1234,"answer_count":56789}
//! ```
//!
//! `answer_count` is the number of `(?i rdf:type ?c)` triples in the OWL 2
//! RL-closed graph. That matches what the SPARQL evaluators compute via
//! `?i a/rdfs:subClassOf* ?c`, because OWL 2 RL materialises every
//! transitive `rdfs:subClassOf` link as an additional `rdf:type` assertion.

use std::env;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: fixpoint_bench_reasonable reasonable <path-to-turtle>");
        return ExitCode::from(2);
    }
    let engine = args[1].as_str();
    let path = &args[2];
    if engine != "reasonable" {
        eprintln!("unknown engine '{engine}'; this binary supports only reasonable");
        return ExitCode::from(2);
    }

    match run(path) {
        Ok(r) => {
            println!(
                "{{\"engine\":\"reasonable\",\"load_ms\":{load:.3},\"compute_ms\":{compute:.3},\"triples_in\":{triples_in},\"answer_count\":{answer}}}",
                load = r.load_ms,
                compute = r.compute_ms,
                triples_in = r.triples_in,
                answer = r.answer_count,
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fixpoint_bench_reasonable failed: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Run {
    load_ms: f64,
    compute_ms: f64,
    triples_in: usize,
    answer_count: u64,
}

fn run(path: &str) -> Result<Run, Box<dyn std::error::Error>> {
    let mut reasoner = reasonable::reasoner::Reasoner::new();

    let load_start = Instant::now();
    reasoner
        .load_file(path)
        .map_err(|e| format!("reasonable load_file failed: {e:?}"))?;
    let load_ms = ms(load_start.elapsed());

    let triples_in = reasoner.get_input().len();

    let compute_start = Instant::now();
    reasoner.reason();
    let answer_count = reasoner
        .get_triples()
        .into_iter()
        .filter(|t| t.predicate.as_str() == RDF_TYPE)
        .count() as u64;
    let compute_ms = ms(compute_start.elapsed());

    Ok(Run {
        load_ms,
        compute_ms,
        triples_in,
        answer_count,
    })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000.0
}
