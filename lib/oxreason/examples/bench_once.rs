//! Bench harness entry point for the oxreason reasoner.
//!
//! Reads a Turtle file, loads it into an in-memory graph, runs
//! `Reasoner::expand`, and prints a one line JSON object with the
//! measured wall-clock duration and triple counts. The Python bench
//! script invokes this binary once per (size, repeat) cell and parses
//! the JSON line from stdout.
//!
//! Usage: `bench_once <path-to-turtle> [--profile owl2rl|rdfs]`
//!
//! The JSON shape is stable:
//!
//! ```json
//! {"parse_ms": 12.3, "reason_ms": 45.6, "triples_in": 1234, "triples_out": 3456, "rounds": 3, "firings": 12345}
//! ```

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "this is a bench harness binary; stdout carries the JSON result line and stderr carries usage and error messages"
)]

use oxrdf::Graph;
use oxreason::{Reasoner, ReasonerConfig, ReasoningProfile};
use oxttl::TurtleParser;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: bench_once <path-to-turtle> [--profile owl2rl|rdfs]");
        return ExitCode::from(2);
    }
    let path = &args[1];
    let profile = parse_profile(&args);

    match run(path, profile) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("bench_once failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_profile(args: &[String]) -> ReasoningProfile {
    let mut it = args.iter().skip(2);
    while let Some(flag) = it.next() {
        if flag == "--profile" {
            if let Some(value) = it.next() {
                return match value.as_str() {
                    "rdfs" => ReasoningProfile::Rdfs,
                    _ => ReasoningProfile::Owl2Rl,
                };
            }
        }
    }
    ReasoningProfile::Owl2Rl
}

fn run(path: &str, profile: ReasoningProfile) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(Path::new(path))?;
    let reader = BufReader::new(file);

    let parse_start = Instant::now();
    let mut graph = Graph::default();
    let parser = TurtleParser::new().for_reader(reader);
    for triple in parser {
        graph.insert(&triple?);
    }
    let parse_ms = ms(parse_start.elapsed());
    let triples_in = graph.len();

    let config = match profile {
        ReasoningProfile::Rdfs => ReasonerConfig::rdfs(),
        _ => ReasonerConfig::owl2_rl(),
    };
    let reasoner = Reasoner::new(config);

    let reason_start = Instant::now();
    let report = reasoner.expand(&mut graph)?;
    let reason_ms = ms(reason_start.elapsed());
    let triples_out = graph.len();

    // Stable one line JSON. No backslashes or quotes in the payload so
    // hand formatting is safe; pulling in serde for six fields is not
    // worth the build time.
    println!(
        "{{\"parse_ms\":{parse_ms:.3},\"reason_ms\":{reason_ms:.3},\"triples_in\":{triples_in},\"triples_out\":{triples_out},\"rounds\":{rounds},\"firings\":{firings}}}",
        rounds = report.rounds,
        firings = report.firings,
    );
    Ok(())
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
