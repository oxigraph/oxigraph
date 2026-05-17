//! Rust harness for the oxigraph side of the subclass closure benchmark.
//!
//! Usage: `fixpoint_bench_oxigraph <engine> <path-to-turtle>`
//!
//! `<engine>` is one of `standard` or `datafusion`. The `reasonable`
//! engine lives in the sibling binary `fixpoint_bench_reasonable` because
//! reasonable 0.2 transitively depends on an old oxigraph that conflicts
//! with the workspace's local oxrocksdb-sys on the `links = "rocksdb"`
//! cargo invariant.
//!
//! Workload semantics: count the number of `(instance, class)` pairs in the
//! transitive closure of `rdf:type/rdfs:subClassOf*`. All three engines
//! produce the same answer set; the difference is how they get there.
//!
//! * `standard` and `datafusion` run the SPARQL query
//!   `SELECT (COUNT(*) AS ?c) WHERE { ?i a/rdfs:subClassOf* ?c }` directly
//!   over the oxigraph store.
//! * `reasonable` loads the data into its native index, materialises every
//!   OWL 2 RL inference, then counts `(?i a ?c)` triples in the closed
//!   graph.
//!
//! The harness prints one JSON line to stdout:
//!
//! ```json
//! {"engine":"datafusion","load_ms":12.3,"compute_ms":45.6,"triples_in":1234,"answer_count":56789}
//! ```
//!
//! Where:
//! * `load_ms` is the time to ingest the turtle file into the engine's
//!   native representation (oxigraph store for `standard` / `datafusion`,
//!   reasonable's index for `reasonable`).
//! * `compute_ms` is the time to compute the answer (run the SPARQL query
//!   for the SPARQL engines, materialise then count for `reasonable`).

use std::env;
use std::fs;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use oxrdf::Term;

const QUERY: &str = "SELECT (COUNT(*) AS ?c) WHERE {
    ?i <http://www.w3.org/1999/02/22-rdf-syntax-ns#type>/<http://www.w3.org/2000/01/rdf-schema#subClassOf>* ?c
}";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: fixpoint_bench <standard|datafusion|reasonable> <path-to-turtle>");
        return ExitCode::from(2);
    }
    let engine = args[1].as_str();
    let path = &args[2];

    let result = match engine {
        "standard" => run_oxigraph(path, false),
        "datafusion" => run_oxigraph(path, true),
        other => {
            eprintln!("unknown engine '{other}'; this binary supports standard or datafusion");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(r) => {
            println!(
                "{{\"engine\":\"{engine}\",\"load_ms\":{load:.3},\"compute_ms\":{compute:.3},\"triples_in\":{triples_in},\"answer_count\":{answer}}}",
                engine = r.engine,
                load = r.load_ms,
                compute = r.compute_ms,
                triples_in = r.triples_in,
                answer = r.answer_count,
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fixpoint_bench failed: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Run {
    engine: &'static str,
    load_ms: f64,
    compute_ms: f64,
    triples_in: usize,
    answer_count: u64,
}

fn run_oxigraph(path: &str, use_datafusion: bool) -> Result<Run, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let store = Store::new()?;

    let load_start = Instant::now();
    let mut loader = store.bulk_loader();
    loader.load_from_slice(
        RdfParser::from_format(RdfFormat::Turtle).lenient(),
        bytes.as_slice(),
    )?;
    loader.commit()?;
    store.optimize()?;
    let load_ms = ms(load_start.elapsed());

    let triples_in = store.len()?;

    let evaluator = SparqlEvaluator::new();
    let prepared = evaluator.parse_query(QUERY)?;

    let compute_start = Instant::now();
    let results = if use_datafusion {
        prepared.datafusion(&store)?
    } else {
        prepared.on_store(&store).execute()?
    };
    let answer_count = count_answer(results)?;
    let compute_ms = ms(compute_start.elapsed());

    Ok(Run {
        engine: if use_datafusion {
            "datafusion"
        } else {
            "standard"
        },
        load_ms,
        compute_ms,
        triples_in,
        answer_count,
    })
}

fn count_answer(results: QueryResults<'_>) -> Result<u64, Box<dyn std::error::Error>> {
    match results {
        QueryResults::Solutions(mut s) => {
            let row = s.next().ok_or("empty SELECT result")??;
            let term = row.get("c").ok_or("no ?c binding")?;
            match term {
                Term::Literal(lit) => Ok(lit.value().parse::<u64>()?),
                other => Err(format!("expected literal for ?c, got {other:?}").into()),
            }
        }
        QueryResults::Boolean(_) => Err("expected SELECT, got ASK".into()),
        QueryResults::Graph(_) => Err("expected SELECT, got CONSTRUCT".into()),
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000.0
}
