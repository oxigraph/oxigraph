//! Micro-benchmarks for SPARQL property path evaluation.
//!
//! Compares the standard `QueryEvaluator` against the DataFusion `RecursiveQuery`
//! based evaluator on synthetic transitive graphs.
//!
//! Run with:
//! ```text
//! cargo bench -p oxigraph --features "rocksdb,datafusion" --bench property_paths
//! ```

#![cfg(feature = "datafusion")]
#![expect(clippy::panic)]

use codspeed_criterion_compat::{BenchmarkId, Criterion, criterion_group, criterion_main};
use oxigraph::io::RdfFormat;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use spargebra::SparqlParser;
use std::fmt::Write as _;

/// Generate an N-triples graph encoding two structures:
/// 1. A linear chain `:n_0 :next :n_1 :next ... :next :n_{n-1}` for transitive testing.
/// 2. A subclass hierarchy `:c_i rdfs:subClassOf :c_{i+1}` for `*` testing.
fn generate_graph(size: usize) -> Vec<u8> {
    let mut buf = String::with_capacity(size * 64);
    for i in 0..size.saturating_sub(1) {
        writeln!(
            buf,
            "<http://ex/n{i}> <http://ex/next> <http://ex/n{}> .",
            i + 1
        )
        .unwrap();
        writeln!(
            buf,
            "<http://ex/c{i}> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://ex/c{}> .",
            i + 1
        )
        .unwrap();
    }
    // Add a few cross links so `next/next` and alternatives have non trivial cardinality.
    for i in (0..size).step_by(7) {
        writeln!(
            buf,
            "<http://ex/n{i}> <http://ex/skip> <http://ex/n{}> .",
            (i + 3).min(size.saturating_sub(1))
        )
        .unwrap();
    }
    buf.into_bytes()
}

fn build_store(size: usize) -> Store {
    let store = Store::new().unwrap();
    let data = generate_graph(size);
    let mut loader = store.bulk_loader();
    loader
        .load_from_slice(RdfFormat::NTriples, data.as_slice())
        .unwrap();
    loader.commit().unwrap();
    store.optimize().unwrap();
    store
}

const QUERIES: &[(&str, &str)] = &[
    (
        "one_or_more_unbound",
        "PREFIX ex: <http://ex/>
         SELECT (COUNT(*) AS ?c) WHERE { ?s ex:next+ ?o }",
    ),
    (
        "zero_or_more_subclass_unbound",
        "PREFIX ex: <http://ex/>
         PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
         SELECT (COUNT(*) AS ?c) WHERE { ?s rdfs:subClassOf* ?o }",
    ),
    (
        "one_or_more_bound_subject",
        "PREFIX ex: <http://ex/>
         SELECT (COUNT(*) AS ?c) WHERE { ex:n0 ex:next+ ?o }",
    ),
    (
        "alternative_then_transitive",
        "PREFIX ex: <http://ex/>
         SELECT (COUNT(*) AS ?c) WHERE { ?s (ex:next|ex:skip)+ ?o }",
    ),
    (
        "sequence_with_transitive",
        "PREFIX ex: <http://ex/>
         SELECT (COUNT(*) AS ?c) WHERE { ?s ex:next/ex:next+ ?o }",
    ),
];

fn drain(results: QueryResults<'_>) {
    match results {
        QueryResults::Boolean(_) => (),
        QueryResults::Solutions(s) => {
            for r in s {
                r.unwrap();
            }
        }
        QueryResults::Graph(g) => {
            for r in g {
                r.unwrap();
            }
        }
    }
}

fn property_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("property paths");
    group.sample_size(10);

    for size in [200_usize, 1_000, 5_000] {
        let store = build_store(size);
        for (name, query) in QUERIES {
            let parsed_query = SparqlParser::new()
                .parse_query(query)
                .unwrap_or_else(|e| panic!("failed to parse {name}: {e}"));

            group.bench_with_input(
                BenchmarkId::new(format!("{name} standard"), size),
                &(),
                |b, ()| {
                    b.iter(|| {
                        let r = SparqlEvaluator::new()
                            .for_query(parsed_query.clone())
                            .on_store(&store)
                            .execute()
                            .unwrap();
                        drain(r);
                    })
                },
            );

            group.bench_with_input(
                BenchmarkId::new(format!("{name} datafusion"), size),
                &(),
                |b, ()| {
                    b.iter(|| {
                        match SparqlEvaluator::new()
                            .for_query(parsed_query.clone())
                            .datafusion(&store)
                        {
                            Ok(r) => drain(r),
                            Err(e) => {
                                let msg = e.to_string();
                                if !msg.contains("not implemented")
                                    && !msg.contains("not supported")
                                {
                                    panic!("datafusion failed on {name}: {e}");
                                }
                            }
                        }
                    })
                },
            );
        }
    }
}

criterion_group!(paths, property_paths);
criterion_main!(paths);
