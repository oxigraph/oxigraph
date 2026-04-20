//! Rust-only bench harness for OWL 2 RL reasoners.
//!
//! Usage: `reasoner_bench <reasoner> <path-to-turtle>`
//!
//! `<reasoner>` is one of `oxreason`, `oxreason-eq` (equality rules on),
//! or `reasonable`.
//!
//! Prints a single line of JSON to stdout:
//!
//! ```json
//! {"reasoner":"oxreason","parse_ms":12.3,"reason_ms":45.6,"triples_in":1234,"triples_out":3456,"rounds":3,"firings":12345}
//! ```
//!
//! The point of this binary is to remove Python overhead from the bench
//! pipeline. The Python bench script subprocesses this once per
//! (reasoner, size, repeat) cell and parses the JSON line. Everything
//! measured here happens inside Rust: parsing, interning, reasoning, and
//! the final triple count.
//!
//! Note on fairness: parse_ms for `oxreason*` uses `oxttl::TurtleParser`
//! which loads into an `oxrdf::Graph`. parse_ms for `reasonable` uses
//! `reasonable::Reasoner::load_file` which loads into the reasonable
//! native index. These are different representations, but they are both
//! what the reasoner natively reasons over, so parse_ms is apples to
//! apples at the engine-consumer level.

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use oxrdf::Graph;
use oxreason::{Reasoner, ReasonerConfig};
use oxttl::TurtleParser;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: reasoner_bench <oxreason|oxreason-eq|reasonable> <path-to-turtle>");
        return ExitCode::from(2);
    }
    let reasoner = args[1].as_str();
    let path = &args[2];

    let result = match reasoner {
        "oxreason" => run_oxreason(path, false),
        "oxreason-eq" => run_oxreason(path, true),
        "reasonable" => run_reasonable(path),
        other => {
            eprintln!("unknown reasoner '{other}'; expected oxreason, oxreason-eq, or reasonable");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(r) => {
            println!(
                "{{\"reasoner\":\"{reasoner}\",\"parse_ms\":{parse:.3},\"reason_ms\":{reason:.3},\"triples_in\":{triples_in},\"triples_out\":{triples_out},\"rounds\":{rounds},\"firings\":{firings}}}",
                reasoner = r.reasoner,
                parse = r.parse_ms,
                reason = r.reason_ms,
                triples_in = r.triples_in,
                triples_out = r.triples_out,
                rounds = r.rounds,
                firings = r.firings,
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("reasoner_bench failed: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Run {
    reasoner: &'static str,
    parse_ms: f64,
    reason_ms: f64,
    triples_in: usize,
    triples_out: usize,
    rounds: u32,
    firings: u64,
}

fn run_oxreason(path: &str, equality_rules: bool) -> Result<Run, Box<dyn std::error::Error>> {
    let file = File::open(Path::new(path))?;
    let reader = BufReader::new(file);

    let parse_start = Instant::now();
    let mut graph = Graph::default();
    let mut parser = TurtleParser::new().for_reader(reader);
    while let Some(triple) = parser.next() {
        graph.insert(&triple?);
    }
    let parse_ms = ms(parse_start.elapsed());
    let triples_in = graph.len();

    let config = ReasonerConfig::owl2_rl().with_equality_rules(equality_rules);
    let r = Reasoner::new(config);

    let reason_start = Instant::now();
    let report = r.expand(&mut graph)?;
    let reason_ms = ms(reason_start.elapsed());

    Ok(Run {
        reasoner: if equality_rules { "oxreason-eq" } else { "oxreason" },
        parse_ms,
        reason_ms,
        triples_in,
        triples_out: graph.len(),
        rounds: report.rounds,
        firings: report.firings,
    })
}

fn run_reasonable(path: &str) -> Result<Run, Box<dyn std::error::Error>> {
    let mut r = reasonable::reasoner::Reasoner::new();

    let parse_start = Instant::now();
    r.load_file(path)
        .map_err(|e| format!("reasonable load_file failed: {e:?}"))?;
    let parse_ms = ms(parse_start.elapsed());
    let triples_in = r.get_input().len();

    let reason_start = Instant::now();
    r.reason();
    let reason_ms = ms(reason_start.elapsed());

    let triples_out = r.get_triples().len();

    Ok(Run {
        reasoner: "reasonable",
        parse_ms,
        reason_ms,
        triples_in,
        triples_out,
        // reasonable does not expose a round or firing counter; report 0
        // so the JSON schema stays stable across reasoners.
        rounds: 0,
        firings: 0,
    })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
