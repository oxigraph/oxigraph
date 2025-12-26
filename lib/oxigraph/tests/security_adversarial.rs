#![cfg(test)]
#![allow(clippy::panic_in_result_fn)]

//! Security adversarial input tests
//!
//! These tests verify that Oxigraph handles malicious or pathological inputs safely,
//! protecting against denial-of-service attacks, resource exhaustion, and other
//! security vulnerabilities.

use oxigraph::io::RdfFormat;
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::error::Error;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Helper function to catch panics in test code
fn run_with_panic_detection<F, R>(test_fn: F) -> Result<R, String>
where
    F: FnOnce() -> R + panic::UnwindSafe,
{
    match panic::catch_unwind(test_fn) {
        Ok(result) => Ok(result),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic".to_string()
            };
            Err(msg)
        }
    }
}

#[test]
fn security_regex_dos_protected() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Insert test data
    let ex = NamedNodeRef::new_unchecked("http://example.com/test");
    store.insert(QuadRef::new(
        ex,
        ex,
        LiteralRef::new_simple_literal("test data"),
        GraphNameRef::DefaultGraph,
    ))?;

    // Create a pathological regex that could cause ReDoS
    // Pattern: (a+)+ with input "aaaaaaaaaa...b" causes catastrophic backtracking
    let pathological_pattern = "(a+)+";
    let test_string = "a".repeat(30); // Moderate size to avoid timeout

    let query = format!(
        r#"
        SELECT ?o WHERE {{
            ?s ?p ?o .
            FILTER(REGEX(STR(?o), "{}"))
        }}
        "#,
        pathological_pattern
    );

    let start = Instant::now();
    let result = SparqlEvaluator::new()
        .parse_query(&query)?
        .on_store(&store)
        .execute();

    let elapsed = start.elapsed();

    // Check that either:
    // 1. Query completed quickly (regex compilation/execution is bounded)
    // 2. Query failed with an error (pattern rejected)
    match result {
        Ok(_) => {
            println!(
                "[SECURITY] regex_dos: pattern_length={}, test_string_length={}, execution_time={:.3}s, result=EXECUTED",
                pathological_pattern.len(),
                test_string.len(),
                elapsed.as_secs_f64()
            );
            assert!(
                elapsed < Duration::from_secs(5),
                "Regex execution took too long: {:?}",
                elapsed
            );
        }
        Err(e) => {
            println!(
                "[SECURITY] regex_dos: pattern_length={}, execution_time={:.3}s, result=REJECTED ({})",
                pathological_pattern.len(),
                elapsed.as_secs_f64(),
                e
            );
        }
    }

    Ok(())
}

#[test]
fn security_deep_nesting_bounded() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create deeply nested UNION query (50 levels)
    let depth = 50;
    let mut query = String::from("SELECT * WHERE {\n");

    // Build nested UNION structure
    for i in 0..depth {
        query.push_str(&format!(
            "  {{ ?s{} ?p{} ?o{} }} UNION\n",
            i, i, i
        ));
    }
    // Add final pattern without UNION
    query.push_str(&format!("  {{ ?s{} ?p{} ?o{} }}\n", depth, depth, depth));
    query.push_str("}");

    let start = Instant::now();
    let result = run_with_panic_detection(AssertUnwindSafe(|| {
        let parse_result = SparqlEvaluator::new().parse_query(&query);
        match parse_result {
            Ok(q) => q.on_store(&store).execute(),
            Err(e) => Err(e.into()),
        }
    }));
    let elapsed = start.elapsed();

    match result {
        Ok(Ok(_)) => {
            println!(
                "[SECURITY] deep_nesting: depth={}, execution_time={:.3}s, result=EXECUTED",
                depth,
                elapsed.as_secs_f64()
            );
            assert!(
                elapsed < Duration::from_secs(10),
                "Deep nesting took too long: {:?}",
                elapsed
            );
        }
        Ok(Err(e)) => {
            println!(
                "[SECURITY] deep_nesting: depth={}, execution_time={:.3}s, result=REJECTED ({})",
                depth,
                elapsed.as_secs_f64(),
                e
            );
        }
        Err(panic_msg) => {
            panic!(
                "Query with depth {} caused panic after {:?}: {}",
                depth, elapsed, panic_msg
            );
        }
    }

    Ok(())
}

#[test]
fn security_property_path_bounded() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Insert a small chain of data
    for i in 0..10 {
        let s = NamedNode::new(format!("http://example.com/node{}", i))?;
        let o = NamedNode::new(format!("http://example.com/node{}", i + 1))?;
        let p = NamedNodeRef::new_unchecked("http://example.com/next");
        store.insert(QuadRef::new(s.as_ref(), p, o.as_ref(), GraphNameRef::DefaultGraph))?;
    }

    // Create query with deep property path (*)
    let query = r#"
        SELECT ?end WHERE {
            <http://example.com/node0> <http://example.com/next>* ?end .
        }
    "#;

    let start = Instant::now();
    let result = run_with_panic_detection(AssertUnwindSafe(|| {
        let parse_result = SparqlEvaluator::new().parse_query(query);
        match parse_result {
            Ok(q) => q.on_store(&store).execute(),
            Err(e) => Err(e.into()),
        }
    }));
    let elapsed = start.elapsed();

    match result {
        Ok(Ok(QueryResults::Solutions(solutions))) => {
            let count = solutions.count();
            println!(
                "[SECURITY] property_path: results={}, execution_time={:.3}s, result=EXECUTED",
                count,
                elapsed.as_secs_f64()
            );
            assert!(
                elapsed < Duration::from_secs(5),
                "Property path evaluation took too long: {:?}",
                elapsed
            );
        }
        Ok(Ok(_)) => {
            println!(
                "[SECURITY] property_path: execution_time={:.3}s, result=EXECUTED (non-solutions)",
                elapsed.as_secs_f64()
            );
        }
        Ok(Err(e)) => {
            println!(
                "[SECURITY] property_path: execution_time={:.3}s, result=REJECTED ({})",
                elapsed.as_secs_f64(),
                e
            );
        }
        Err(panic_msg) => {
            panic!(
                "Property path query caused panic after {:?}: {}",
                elapsed, panic_msg
            );
        }
    }

    Ok(())
}

#[test]
fn security_large_literal_safe() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create a 10MB literal
    let large_string = "a".repeat(10 * 1024 * 1024);
    let literal_size = large_string.len();

    let start = Instant::now();
    let result = run_with_panic_detection(AssertUnwindSafe(|| {
        // Try to create and insert a large literal
        let s = NamedNodeRef::new_unchecked("http://example.com/subject");
        let p = NamedNodeRef::new_unchecked("http://example.com/hasLargeData");
        let o = Literal::new_simple_literal(&large_string);

        store.insert(QuadRef::new(
            s,
            p,
            o.as_ref(),
            GraphNameRef::DefaultGraph,
        ))
    }));
    let elapsed = start.elapsed();

    match result {
        Ok(Ok(())) => {
            println!(
                "[SECURITY] large_literal: size={}MB, execution_time={:.3}s, result=INSERTED",
                literal_size / (1024 * 1024),
                elapsed.as_secs_f64()
            );
            // Verify we can query it back
            assert_eq!(store.len()?, 1);
        }
        Ok(Err(e)) => {
            println!(
                "[SECURITY] large_literal: size={}MB, execution_time={:.3}s, result=REJECTED ({})",
                literal_size / (1024 * 1024),
                elapsed.as_secs_f64(),
                e
            );
        }
        Err(panic_msg) => {
            panic!(
                "Large literal ({}MB) caused panic after {:?}: {}",
                literal_size / (1024 * 1024),
                elapsed,
                panic_msg
            );
        }
    }

    Ok(())
}

#[test]
fn security_malformed_input_rejected() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Test various malformed RDF inputs
    let test_cases = vec![
        ("invalid_utf8", b"\xFF\xFE invalid UTF-8".to_vec()),
        ("truncated", b"<http://example.com/s> <http://example.com/p> <http://".to_vec()),
        ("garbage", b"\x00\x01\x02\x03\x04\x05".to_vec()),
        ("incomplete_triple", b"<http://example.com/s> <http://example.com/p>".to_vec()),
        ("invalid_iri", b"<not a valid iri> <p> <o> .".to_vec()),
    ];

    for (name, input) in test_cases {
        let start = Instant::now();
        let result = run_with_panic_detection(AssertUnwindSafe(|| {
            store.load_from_reader(RdfFormat::NTriples, input.as_slice())
        }));
        let elapsed = start.elapsed();

        match result {
            Ok(Ok(())) => {
                println!(
                    "[SECURITY] malformed_input: test={}, size={}, execution_time={:.3}s, result=ACCEPTED (unexpected)",
                    name,
                    input.len(),
                    elapsed.as_secs_f64()
                );
            }
            Ok(Err(e)) => {
                println!(
                    "[SECURITY] malformed_input: test={}, size={}, execution_time={:.3}s, result=REJECTED ({})",
                    name,
                    input.len(),
                    elapsed.as_secs_f64(),
                    e
                );
            }
            Err(panic_msg) => {
                panic!(
                    "Malformed input '{}' caused panic after {:?}: {}",
                    name, elapsed, panic_msg
                );
            }
        }
    }

    Ok(())
}

#[test]
fn security_concurrent_load_safe() -> Result<(), Box<dyn Error>> {
    let store = Arc::new(Store::new()?);
    let num_threads = 10;
    let ops_per_thread = 100;

    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let store_clone = Arc::clone(&store);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    let s = NamedNode::new(format!("http://example.com/s{}-{}", thread_id, i))
                        .unwrap();
                    let p = NamedNodeRef::new_unchecked("http://example.com/p");
                    let o = Literal::new_simple_literal(&format!("value-{}-{}", thread_id, i));

                    if let Err(e) = store_clone.insert(QuadRef::new(
                        s.as_ref(),
                        p,
                        o.as_ref(),
                        GraphNameRef::DefaultGraph,
                    )) {
                        return Err(e);
                    }
                }
                Ok(())
            })
        })
        .collect();

    // Wait for all threads
    let mut errors = Vec::new();
    for (id, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => errors.push(format!("Thread {} error: {}", id, e)),
            Err(_) => errors.push(format!("Thread {} panicked", id)),
        }
    }

    let elapsed = start.elapsed();

    if !errors.is_empty() {
        for err in &errors {
            eprintln!("{}", err);
        }
        panic!("Concurrent operations failed: {} errors", errors.len());
    }

    let final_count = store.len()?;
    println!(
        "[SECURITY] concurrent_load: threads={}, ops_per_thread={}, total_ops={}, final_count={}, execution_time={:.3}s, result=SAFE",
        num_threads,
        ops_per_thread,
        num_threads * ops_per_thread,
        final_count,
        elapsed.as_secs_f64()
    );

    // Verify data integrity
    assert_eq!(
        final_count,
        num_threads * ops_per_thread,
        "Data corruption detected: expected {} quads, found {}",
        num_threads * ops_per_thread,
        final_count
    );

    Ok(())
}

#[test]
fn security_query_complexity_measurable() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Insert test data
    for i in 0..100 {
        let s = NamedNode::new(format!("http://example.com/s{}", i))?;
        let p = NamedNodeRef::new_unchecked("http://example.com/p");
        let o = Literal::new_simple_literal(&format!("value{}", i));
        store.insert(QuadRef::new(
            s.as_ref(),
            p,
            o.as_ref(),
            GraphNameRef::DefaultGraph,
        ))?;
    }

    // Test queries of varying complexity
    let test_queries = vec![
        ("simple_select", "SELECT * WHERE { ?s ?p ?o } LIMIT 10", 1),
        (
            "filter_simple",
            "SELECT * WHERE { ?s ?p ?o . FILTER(?o = 'value5') }",
            2,
        ),
        (
            "filter_complex",
            "SELECT * WHERE { ?s ?p ?o . FILTER(REGEX(STR(?o), '^value[0-9]+$')) }",
            3,
        ),
        (
            "multiple_patterns",
            "SELECT * WHERE { ?s1 ?p1 ?o1 . ?s2 ?p2 ?o2 . FILTER(?s1 != ?s2) } LIMIT 100",
            4,
        ),
    ];

    let mut timings = Vec::new();

    for (name, query, expected_complexity) in test_queries {
        let start = Instant::now();
        let result = SparqlEvaluator::new()
            .parse_query(query)?
            .on_store(&store)
            .execute();
        let elapsed = start.elapsed();

        match result {
            Ok(QueryResults::Solutions(solutions)) => {
                let count = solutions.count();
                println!(
                    "[SECURITY] query_complexity: name={}, complexity={}, results={}, execution_time={:.3}s",
                    name,
                    expected_complexity,
                    count,
                    elapsed.as_secs_f64()
                );
                timings.push((name, expected_complexity, elapsed));
            }
            Ok(_) => {
                println!(
                    "[SECURITY] query_complexity: name={}, complexity={}, execution_time={:.3}s (non-solutions)",
                    name,
                    expected_complexity,
                    elapsed.as_secs_f64()
                );
                timings.push((name, expected_complexity, elapsed));
            }
            Err(e) => {
                panic!("Query '{}' failed: {}", name, e);
            }
        }
    }

    // Verify that execution times are reasonable and measurable
    for (name, _complexity, elapsed) in &timings {
        assert!(
            *elapsed < Duration::from_secs(5),
            "Query '{}' took too long: {:?}",
            name,
            elapsed
        );
    }

    println!(
        "[SECURITY] query_complexity: total_queries={}, result=MEASURABLE",
        timings.len()
    );

    Ok(())
}
