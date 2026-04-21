//! Diff oxreason's closure against reasonable's closure on a Turtle fixture.
//!
//! Usage: `reasoner_diff <path-to-turtle>`
//!
//! Runs both reasoners on the same input, computes the symmetric
//! difference of their closures, and prints a categorised report.
//!
//! Output sections:
//!
//! 1. Totals (sizes of each closure and of each one-sided diff).
//! 2. Top predicates in the oxreason-only diff (rule hypotheses).
//! 3. Top predicates in the reasonable-only diff.
//! 4. A handful of sample triples from each bucket for eyeballing.
//!
//! Canonical triple form: the N-Triples `Display` impl shipped by every
//! oxrdf version in the dependency graph. Produces `<s> <p> <o>` for
//! IRI-only triples, `_:b` for blanks, and `"lex"^^<dt>` / `"lex"@lang`
//! for literals. reasonable 0.2 pulls in oxrdf 0.1.7 transitively, and
//! the workspace oxreason uses the workspace oxrdf; the two `Triple`
//! types are therefore distinct to the compiler but agree byte-for-byte
//! on their `Display` output, so both closures land in the same
//! `HashSet<String>` via `Triple::to_string()`.
//!
//! Known caveats:
//!
//! * Blank node labels are not canonicalised across reasoners. LUBM
//!   inputs have no blanks, and OWL 2 RL rules do not introduce new
//!   blanks, so this is not expected to cause spurious diffs on this
//!   workload. If other inputs hit blanks, the report will split their
//!   buckets accordingly and a manual sanity check can fold them.
//! * Literals are compared by whatever oxrdf and reasonable each emit.
//!   Slight lexical-vs-canonical differences on typed literals could in
//!   principle register as diffs. LUBM has string literals only, so
//!   again not expected to bite on this workload.

use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::Infallible;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::ExitCode;

use oxrdf::Triple;
use oxreason::{ReasonStreamError, Reasoner, ReasonerConfig};
use oxttl::TurtleParser;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: reasoner_diff <path-to-turtle>");
        return ExitCode::from(2);
    }
    let path = &args[1];

    match run(path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("reasoner_diff failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("loading {path}");
    let file = File::open(Path::new(path))?;
    let reader = BufReader::new(file);
    let mut seeds: Vec<Triple> = Vec::new();
    let mut parser = TurtleParser::new().for_reader(reader);
    while let Some(triple) = parser.next() {
        seeds.push(triple?);
    }
    eprintln!("parsed {} seed triples", seeds.len());

    // oxreason closure: seeds + everything the sink receives.
    eprintln!("running oxreason ...");
    let mut ox: HashSet<String> = HashSet::with_capacity(seeds.len() * 3);
    for t in &seeds {
        ox.insert(t.to_string());
    }
    let config = ReasonerConfig::owl2_rl();
    let r = Reasoner::new(config);
    let seeds_for_ox = seeds.clone();
    r.expand_streaming_from(seeds_for_ox, |t: &Triple| -> Result<(), Infallible> {
        ox.insert(t.to_string());
        Ok(())
    })
    .map_err(|e| -> Box<dyn std::error::Error> {
        match e {
            ReasonStreamError::Reason(r) => Box::new(r),
            ReasonStreamError::Sink(never) => match never {},
        }
    })?;
    eprintln!("oxreason closure: {} triples", ox.len());

    // reasonable closure: its internal triple index after reason().
    // reasonable 0.2 returns `Vec<oxrdf::Triple>` from oxrdf 0.1.7, not
    // the workspace oxrdf oxreason uses. The type is left inferred, and
    // its `Display` impl produces the same N-Triples line form as the
    // workspace oxrdf, so both closures key into the same `HashSet`.
    eprintln!("running reasonable ...");
    let mut re_reasoner = reasonable::reasoner::Reasoner::new();
    re_reasoner
        .load_file(path)
        .map_err(|e| format!("reasonable load_file failed: {e:?}"))?;
    re_reasoner.reason();
    let re_triples = re_reasoner.get_triples();
    let mut re: HashSet<String> = HashSet::with_capacity(re_triples.len());
    for t in &re_triples {
        re.insert(t.to_string());
    }
    eprintln!("reasonable closure: {} triples", re.len());

    // Diff.
    let only_ox: Vec<&String> = ox.difference(&re).collect();
    let only_re: Vec<&String> = re.difference(&ox).collect();

    println!("== totals ==");
    println!("ox_total        {:>10}", ox.len());
    println!("re_total        {:>10}", re.len());
    println!("ox \\ re         {:>10}", only_ox.len());
    println!("re \\ ox         {:>10}", only_re.len());
    println!("intersection    {:>10}", ox.intersection(&re).count());

    println!();
    println!("== predicates in ox \\ re (top 30) ==");
    print_by_predicate(&only_ox, 30);

    println!();
    println!("== predicates in re \\ ox (top 30) ==");
    print_by_predicate(&only_re, 30);

    println!();
    println!("== sample ox \\ re (first 20) ==");
    print_samples(&only_ox, 20);

    println!();
    println!("== sample re \\ ox (first 20) ==");
    print_samples(&only_re, 20);

    Ok(())
}

fn split_predicate(line: &str) -> Option<&str> {
    let mut parts = line.splitn(3, ' ');
    let _s = parts.next()?;
    let p = parts.next()?;
    Some(p)
}

fn print_by_predicate(lines: &[&String], top_n: usize) {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for line in lines {
        if let Some(p) = split_predicate(line) {
            *counts.entry(p).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<(&&str, &usize)> = counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (p, n) in sorted.into_iter().take(top_n) {
        println!("{:>8}  {}", n, p);
    }
}

fn print_samples(lines: &[&String], n: usize) {
    for line in lines.iter().take(n) {
        println!("{line}");
    }
}
