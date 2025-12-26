//! Adversarial SPARQL Query Tests
//!
//! This test suite validates that unbounded SPARQL operations are properly limited
//! to prevent DoS attacks. Each test either:
//! 1. Proves the attack works (exposes vulnerability) - currently expected to fail/timeout
//! 2. Proves the mitigation works (attack blocked) - after implementing limits

use oxrdf::{Dataset, GraphName, Literal, NamedNode, Quad};
use spareval::{QueryEvaluationError, QueryEvaluator, QueryExecutionLimits, QueryResults};
use spargebra::SparqlParser;
use std::time::{Duration, Instant};

/// Helper to create a large dataset with N triples
fn create_large_dataset(num_triples: usize) -> Dataset {
    let mut dataset = Dataset::new();
    let pred = NamedNode::new("http://example.com/pred").unwrap();

    for i in 0..num_triples {
        let subj = NamedNode::new(format!("http://example.com/subj{}", i)).unwrap();
        let obj = Literal::from(i as i32);
        dataset.insert(&Quad::new(subj, pred.clone(), obj, GraphName::DefaultGraph));
    }
    dataset
}

/// Helper to create a deep chain graph for transitive closure testing
/// Creates chain: s0 -> s1 -> s2 -> ... -> sN
fn create_chain_dataset(depth: usize) -> Dataset {
    let mut dataset = Dataset::new();
    let pred = NamedNode::new("http://example.com/next").unwrap();

    for i in 0..depth {
        let subj = NamedNode::new(format!("http://example.com/s{}", i)).unwrap();
        let obj = NamedNode::new(format!("http://example.com/s{}", i + 1)).unwrap();
        dataset.insert(&Quad::new(subj, pred.clone(), obj, GraphName::DefaultGraph));
    }
    dataset
}

#[test]
fn test_unbounded_order_by_materializes_all_results() {
    // VULNERABILITY TEST: ORDER BY without LIMIT should either:
    // 1. Be rejected for large result sets
    // 2. Have a maximum materialization limit
    // 3. Complete in bounded time

    let dataset = create_large_dataset(10_000);
    let query = SparqlParser::new()
        .parse_query("SELECT ?s ?o WHERE { ?s ?p ?o } ORDER BY ?o")
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let start = Instant::now();

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            // Currently this materializes ALL results into a Vec
            // Count how many we get
            let mut count = 0;
            while let Some(result) = solutions.next() {
                result.expect("Query should not error");
                count += 1;

                // Safety: stop if it's taking too long
                if start.elapsed() > Duration::from_secs(5) {
                    panic!("ORDER BY materialization took >5s, unbounded!");
                }
            }

            // CURRENT STATE: This passes, showing ORDER BY materializes all 10k results
            // DESIRED STATE: Should be rejected or limited
            println!("ORDER BY materialized {} results", count);
            assert_eq!(count, 10_000, "Currently materializes all results");
        }
        Err(e) => {
            // Good! Query was rejected
            println!("ORDER BY was rejected: {}", e);
        }
        _ => panic!("Expected solutions"),
    }
}

#[test]
fn test_unbounded_group_by_high_cardinality() {
    // VULNERABILITY TEST: GROUP BY with high cardinality can create unlimited groups

    let dataset = create_large_dataset(5_000);

    // Each row has unique ?o value, creating 5000 groups
    let query = SparqlParser::new()
        .parse_query("SELECT ?o (COUNT(?s) AS ?count) WHERE { ?s ?p ?o } GROUP BY ?o")
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let start = Instant::now();

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut group_count = 0;
            while let Some(result) = solutions.next() {
                result.expect("Query should not error");
                group_count += 1;

                if start.elapsed() > Duration::from_secs(5) {
                    panic!("GROUP BY took >5s, unbounded!");
                }
            }

            // CURRENT STATE: Creates 5000 groups without limit
            // DESIRED STATE: Should be limited to reasonable cardinality (e.g., 1000)
            println!("GROUP BY created {} groups", group_count);
            assert_eq!(group_count, 5_000, "Currently creates all groups");
        }
        Err(e) => {
            println!("GROUP BY was rejected: {}", e);
        }
        _ => panic!("Expected solutions"),
    }
}

#[test]
fn test_transitive_closure_unbounded_depth() {
    // VULNERABILITY TEST: Property path with * operator has no depth limit

    let dataset = create_chain_dataset(1000); // Chain of depth 1000

    // Query: Find all nodes reachable from s0 via transitive closure
    let query = SparqlParser::new()
        .parse_query("SELECT ?end WHERE { <http://example.com/s0> <http://example.com/next>* ?end }")
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let start = Instant::now();

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut count = 0;
            while let Some(result) = solutions.next() {
                result.expect("Query should not error");
                count += 1;

                if start.elapsed() > Duration::from_secs(10) {
                    panic!("Transitive closure took >10s, unbounded!");
                }
            }

            // CURRENT STATE: Follows chain to arbitrary depth
            // DESIRED STATE: Should have max depth limit (e.g., 1000)
            println!("Transitive closure found {} nodes", count);
            assert!(count > 100, "Should traverse deep chain");
        }
        Err(e) => {
            println!("Transitive closure was rejected: {}", e);
        }
        _ => panic!("Expected solutions"),
    }
}

#[test]
fn test_cartesian_product_explosion() {
    // VULNERABILITY TEST: Cartesian product of large result sets

    let mut dataset = Dataset::new();
    let p1 = NamedNode::new("http://example.com/p1").unwrap();
    let p2 = NamedNode::new("http://example.com/p2").unwrap();

    // Create 100 triples with p1 and 100 with p2
    for i in 0..100 {
        let s = NamedNode::new(format!("http://example.com/s{}", i)).unwrap();
        let o = Literal::from(i as i32);
        dataset.insert(&Quad::new(s.clone(), p1.clone(), o.clone(), GraphName::DefaultGraph));
        dataset.insert(&Quad::new(s, p2.clone(), o, GraphName::DefaultGraph));
    }

    // Query creates 100 × 100 = 10,000 results (Cartesian product)
    let query = SparqlParser::new()
        .parse_query("SELECT ?s1 ?s2 WHERE { ?s1 <http://example.com/p1> ?o1 . ?s2 <http://example.com/p2> ?o2 }")
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let start = Instant::now();

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut count = 0;
            while let Some(result) = solutions.next() {
                result.expect("Query should not error");
                count += 1;

                if start.elapsed() > Duration::from_secs(5) {
                    panic!("Cartesian product took >5s!");
                }

                // Stop early if generating too many results
                if count > 20_000 {
                    panic!("Cartesian product generated >20k results!");
                }
            }

            // CURRENT STATE: Generates full Cartesian product
            // DESIRED STATE: Should be limited or warned
            println!("Cartesian product generated {} results", count);
            assert_eq!(count, 10_000, "Generates full product");
        }
        Err(e) => {
            println!("Cartesian product was rejected: {}", e);
        }
        _ => panic!("Expected solutions"),
    }
}

#[test]
fn test_order_by_with_limit_is_efficient() {
    // POSITIVE TEST: ORDER BY with LIMIT should NOT materialize all results
    // This should be efficient even with large datasets

    let dataset = create_large_dataset(10_000);
    let query = SparqlParser::new()
        .parse_query("SELECT ?s ?o WHERE { ?s ?p ?o } ORDER BY ?o LIMIT 10")
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let start = Instant::now();

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut count = 0;
            while let Some(result) = solutions.next() {
                result.expect("Query should not error");
                count += 1;
            }

            let elapsed = start.elapsed();

            // Should only return 10 results
            assert_eq!(count, 10);

            // Should be reasonably fast (though currently it still materializes all)
            // TODO: Optimize to use min-heap for ORDER BY + LIMIT
            println!("ORDER BY LIMIT 10 took {:?}", elapsed);
        }
        Err(e) => panic!("Query should succeed: {}", e),
        _ => panic!("Expected solutions"),
    }
}

#[test]
fn test_distinct_with_large_result_set() {
    // VULNERABILITY TEST: DISTINCT materializes all results into HashSet

    let dataset = create_large_dataset(5_000);
    let query = SparqlParser::new()
        .parse_query("SELECT DISTINCT ?o WHERE { ?s ?p ?o }")
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let start = Instant::now();

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut count = 0;
            while let Some(result) = solutions.next() {
                result.expect("Query should not error");
                count += 1;

                if start.elapsed() > Duration::from_secs(5) {
                    panic!("DISTINCT took >5s!");
                }
            }

            // DISTINCT materializes all unique values
            println!("DISTINCT found {} unique values", count);
            assert_eq!(count, 5_000);
        }
        Err(e) => panic!("Query should succeed: {}", e),
        _ => panic!("Expected solutions"),
    }
}

#[test]
#[ignore] // Ignore by default - this tests cancellation, not limits
fn test_query_cancellation_works() {
    // This tests that CancellationToken works, not limits
    use spareval::CancellationToken;
    use std::sync::Arc;
    use std::thread;

    let dataset = Arc::new(create_large_dataset(100_000));
    let cancellation_token = CancellationToken::new();
    let cancel_handle = cancellation_token.clone();

    // Spawn thread that cancels after 100ms
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        cancel_handle.cancel();
    });

    let query = SparqlParser::new()
        .parse_query("SELECT ?s ?o WHERE { ?s ?p ?o } ORDER BY ?o")
        .unwrap();

    let evaluator = QueryEvaluator::new()
        .with_cancellation_token(cancellation_token);

    match evaluator.prepare(&query).execute(&*dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut count = 0;
            while let Some(result) = solutions.next() {
                if result.is_err() {
                    println!("Query was cancelled after {} results", count);
                    return; // Success - cancellation worked
                }
                count += 1;
            }
            panic!("Query should have been cancelled");
        }
        Err(e) => {
            println!("Query error: {}", e);
        }
        _ => panic!("Expected solutions"),
    }
}

// ============================================================================
// LIMIT ENFORCEMENT TESTS
// These tests verify that QueryExecutionLimits are actually enforced
// ============================================================================

#[test]
#[should_panic(expected = "exceeded")] // Will fail until limits are enforced
fn test_max_result_rows_limit_enforced() {
    // MITIGATION TEST: When max_result_rows is set, query should stop or error

    let dataset = create_large_dataset(10_000);
    let query = SparqlParser::new()
        .parse_query("SELECT ?s ?o WHERE { ?s ?p ?o } ORDER BY ?o")
        .unwrap();

    let limits = QueryExecutionLimits {
        max_result_rows: Some(100), // Limit to 100 rows
        ..QueryExecutionLimits::default()
    };

    let evaluator = QueryEvaluator::new().with_limits(limits);

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut count = 0;
            while let Some(result) = solutions.next() {
                match result {
                    Ok(_) => count += 1,
                    Err(QueryEvaluationError::ResultLimitExceeded(_)) => {
                        // Good! Limit was enforced
                        println!("Query stopped after {} rows (limit: 100)", count);
                        assert!(count <= 100, "Should not exceed limit");
                        panic!("exceeded"); // Expected panic message
                    }
                    Err(e) => panic!("Unexpected error: {}", e),
                }
            }
            panic!("Should have hit max_result_rows limit but got {} rows", count);
        }
        Err(QueryEvaluationError::ResultLimitExceeded(_)) => {
            // Also acceptable - rejected before iteration
            panic!("exceeded");
        }
        Err(e) => panic!("Unexpected error: {}", e),
        _ => panic!("Expected solutions"),
    }
}

#[test]
#[should_panic(expected = "exceeded")] // Will fail until limits are enforced
fn test_max_groups_limit_enforced() {
    // MITIGATION TEST: When max_groups is set, GROUP BY should stop or error

    let dataset = create_large_dataset(5_000);
    let query = SparqlParser::new()
        .parse_query("SELECT ?o (COUNT(?s) AS ?count) WHERE { ?s ?p ?o } GROUP BY ?o")
        .unwrap();

    let limits = QueryExecutionLimits {
        max_groups: Some(100), // Limit to 100 groups
        ..QueryExecutionLimits::default()
    };

    let evaluator = QueryEvaluator::new().with_limits(limits);

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut count = 0;
            while let Some(result) = solutions.next() {
                match result {
                    Ok(_) => count += 1,
                    Err(QueryEvaluationError::GroupLimitExceeded(_)) => {
                        println!("Query stopped after {} groups (limit: 100)", count);
                        assert!(count <= 100, "Should not exceed limit");
                        panic!("exceeded");
                    }
                    Err(e) => panic!("Unexpected error: {}", e),
                }
            }
            panic!("Should have hit max_groups limit but got {} groups", count);
        }
        Err(QueryEvaluationError::GroupLimitExceeded(_)) => {
            panic!("exceeded");
        }
        Err(e) => panic!("Unexpected error: {}", e),
        _ => panic!("Expected solutions"),
    }
}

#[test]
#[should_panic(expected = "exceeded")] // Will fail until limits are enforced
fn test_max_property_path_depth_enforced() {
    // MITIGATION TEST: When max_property_path_depth is set, transitive closure should stop

    let dataset = create_chain_dataset(1000);
    let query = SparqlParser::new()
        .parse_query("SELECT ?end WHERE { <http://example.com/s0> <http://example.com/next>* ?end }")
        .unwrap();

    let limits = QueryExecutionLimits {
        max_property_path_depth: Some(50), // Limit depth to 50
        ..QueryExecutionLimits::default()
    };

    let evaluator = QueryEvaluator::new().with_limits(limits);

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut count = 0;
            while let Some(result) = solutions.next() {
                match result {
                    Ok(_) => count += 1,
                    Err(QueryEvaluationError::PropertyPathDepthExceeded(_)) => {
                        println!("Query stopped at depth ~{} (limit: 50)", count);
                        assert!(count <= 51, "Should not exceed depth limit significantly");
                        panic!("exceeded");
                    }
                    Err(e) => panic!("Unexpected error: {}", e),
                }
            }
            panic!("Should have hit depth limit but got {} nodes", count);
        }
        Err(QueryEvaluationError::PropertyPathDepthExceeded(_)) => {
            panic!("exceeded");
        }
        Err(e) => panic!("Unexpected error: {}", e),
        _ => panic!("Expected solutions"),
    }
}

#[test]
fn test_unlimited_mode_allows_all_operations() {
    // Verify that QueryExecutionLimits::unlimited() truly disables all limits

    let dataset = create_large_dataset(1_000);
    let query = SparqlParser::new()
        .parse_query("SELECT ?s ?o WHERE { ?s ?p ?o } ORDER BY ?o")
        .unwrap();

    let limits = QueryExecutionLimits::unlimited();
    let evaluator = QueryEvaluator::new().with_limits(limits);

    match evaluator.prepare(&query).execute(&dataset) {
        Ok(QueryResults::Solutions(mut solutions)) => {
            let mut count = 0;
            while let Some(result) = solutions.next() {
                result.expect("Should not error in unlimited mode");
                count += 1;
            }
            println!("Unlimited mode processed {} results", count);
            assert_eq!(count, 1_000);
        }
        Err(e) => panic!("Unlimited mode should not error: {}", e),
        _ => panic!("Expected solutions"),
    }
}
