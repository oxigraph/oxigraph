//! Determinism and reproducibility tests for SPARQL evaluation
//!
//! This test suite validates the audit claim that SPARQL SELECT queries
//! without ORDER BY may produce non-deterministic results due to FxHashMap usage.

use oxrdf::{BlankNode, Dataset, Literal, NamedNode, Quad, GraphName};
use spareval::{QueryEvaluator, QueryResults};
use spargebra::SparqlParser;

/// Helper to create a test dataset with multiple quads
fn create_test_dataset() -> Dataset {
    let mut dataset = Dataset::new();

    // Add multiple quads to test result ordering
    let foaf_knows = NamedNode::new_unchecked("http://xmlns.com/foaf/0.1/knows");
    let foaf_name = NamedNode::new_unchecked("http://xmlns.com/foaf/0.1/name");
    let ex_alice = NamedNode::new_unchecked("http://example.org/Alice");
    let ex_bob = NamedNode::new_unchecked("http://example.org/Bob");
    let ex_carol = NamedNode::new_unchecked("http://example.org/Carol");
    let ex_dave = NamedNode::new_unchecked("http://example.org/Dave");

    dataset.insert(&Quad::new(
        ex_alice.clone(),
        foaf_knows.clone(),
        ex_bob.clone(),
        GraphName::DefaultGraph,
    ));
    dataset.insert(&Quad::new(
        ex_alice.clone(),
        foaf_knows.clone(),
        ex_carol.clone(),
        GraphName::DefaultGraph,
    ));
    dataset.insert(&Quad::new(
        ex_bob.clone(),
        foaf_knows.clone(),
        ex_dave.clone(),
        GraphName::DefaultGraph,
    ));
    dataset.insert(&Quad::new(
        ex_alice.clone(),
        foaf_name.clone(),
        Literal::new_simple_literal("Alice"),
        GraphName::DefaultGraph,
    ));
    dataset.insert(&Quad::new(
        ex_bob.clone(),
        foaf_name.clone(),
        Literal::new_simple_literal("Bob"),
        GraphName::DefaultGraph,
    ));

    dataset
}

/// Execute a SPARQL query and return results as a string for comparison
fn execute_query_as_string(dataset: &Dataset, query_str: &str) -> String {
    let query = SparqlParser::new().parse_query(query_str).unwrap();
    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(dataset).unwrap();

    match results {
        QueryResults::Solutions(solutions) => {
            let mut result_strings = Vec::new();
            for solution in solutions {
                let solution = solution.unwrap();
                result_strings.push(format!("{:?}", solution));
            }
            result_strings.join("\n")
        }
        QueryResults::Boolean(b) => b.to_string(),
        QueryResults::Graph(_) => "GRAPH".to_string(),
    }
}

#[test]
fn test_select_with_order_by_is_deterministic() {
    // Query WITH ORDER BY should always produce the same results in the same order
    let query = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                 SELECT ?person ?friend
                 WHERE { ?person foaf:knows ?friend }
                 ORDER BY ?person ?friend";

    let dataset = create_test_dataset();

    // Run query 50 times
    let results: Vec<_> = (0..50)
        .map(|_| execute_query_as_string(&dataset, query))
        .collect();

    // All results should be identical
    let first_result = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_result, result,
            "Query with ORDER BY produced different results on run {}.\nExpected:\n{}\nGot:\n{}",
            i, first_result, result
        );
    }
}

#[test]
fn test_select_without_order_by_determinism() {
    // Query WITHOUT ORDER BY - testing if FxHashMap causes non-determinism
    let query = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                 SELECT ?person ?friend
                 WHERE { ?person foaf:knows ?friend }";

    let dataset = create_test_dataset();

    // Run query 100 times
    let results: Vec<_> = (0..100)
        .map(|_| execute_query_as_string(&dataset, query))
        .collect();

    // Check if all results are identical
    let first_result = &results[0];
    let mut different_results = Vec::new();

    for (i, result) in results.iter().enumerate().skip(1) {
        if first_result != result {
            different_results.push((i, result.clone()));
        }
    }

    if !different_results.is_empty() {
        println!("WARNING: Query without ORDER BY produced {} different result orderings out of 100 runs",
                 different_results.len() + 1);
        println!("First result:\n{}", first_result);
        for (i, result) in different_results.iter().take(3) {
            println!("Different result at run {}:\n{}", i, result);
        }

        // This is actually EXPECTED behavior due to FxHashMap
        // We document this rather than fail
        println!("NOTE: This is expected behavior - use ORDER BY for deterministic results");
    } else {
        println!("Query without ORDER BY produced consistent results (100/100 runs matched)");
    }

    // Don't fail the test - we're documenting behavior, not requiring determinism
    // Users should use ORDER BY for guaranteed ordering
}

#[test]
fn test_insert_order_independence() {
    // Two graphs with same triples inserted in different orders
    // Query results should contain the same data (though order may vary)

    let foaf_knows = NamedNode::new_unchecked("http://xmlns.com/foaf/0.1/knows");
    let ex_alice = NamedNode::new_unchecked("http://example.org/Alice");
    let ex_bob = NamedNode::new_unchecked("http://example.org/Bob");
    let ex_carol = NamedNode::new_unchecked("http://example.org/Carol");

    // Dataset 1: Insert in order A, B, C
    let mut dataset1 = Dataset::new();
    dataset1.insert(&Quad::new(ex_alice.clone(), foaf_knows.clone(), ex_bob.clone(), GraphName::DefaultGraph));
    dataset1.insert(&Quad::new(ex_alice.clone(), foaf_knows.clone(), ex_carol.clone(), GraphName::DefaultGraph));
    dataset1.insert(&Quad::new(ex_bob.clone(), foaf_knows.clone(), ex_carol.clone(), GraphName::DefaultGraph));

    // Dataset 2: Insert in reverse order C, B, A
    let mut dataset2 = Dataset::new();
    dataset2.insert(&Quad::new(ex_bob.clone(), foaf_knows.clone(), ex_carol.clone(), GraphName::DefaultGraph));
    dataset2.insert(&Quad::new(ex_alice.clone(), foaf_knows.clone(), ex_carol.clone(), GraphName::DefaultGraph));
    dataset2.insert(&Quad::new(ex_alice.clone(), foaf_knows.clone(), ex_bob.clone(), GraphName::DefaultGraph));

    // Query with ORDER BY to ensure deterministic comparison
    let query = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                 SELECT ?person ?friend
                 WHERE { ?person foaf:knows ?friend }
                 ORDER BY ?person ?friend";

    let result1 = execute_query_as_string(&dataset1, query);
    let result2 = execute_query_as_string(&dataset2, query);

    assert_eq!(result1, result2,
               "Datasets with same quads in different insert orders should produce identical results when queried with ORDER BY");
}

#[test]
fn test_blank_node_query_determinism() {
    // Test if blank nodes cause non-deterministic query results
    let mut dataset = Dataset::new();

    let foaf_knows = NamedNode::new_unchecked("http://xmlns.com/foaf/0.1/knows");
    let ex_alice = NamedNode::new_unchecked("http://example.org/Alice");

    // Use the same blank node instances for consistency
    let blank1 = BlankNode::new_unchecked("b1");
    let blank2 = BlankNode::new_unchecked("b2");

    dataset.insert(&Quad::new(ex_alice.clone(), foaf_knows.clone(), blank1.clone(), GraphName::DefaultGraph));
    dataset.insert(&Quad::new(ex_alice.clone(), foaf_knows.clone(), blank2.clone(), GraphName::DefaultGraph));

    let query = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                 SELECT ?friend
                 WHERE { <http://example.org/Alice> foaf:knows ?friend }
                 ORDER BY ?friend";

    // Run query multiple times
    let results: Vec<_> = (0..50)
        .map(|_| execute_query_as_string(&dataset, query))
        .collect();

    let first_result = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_result, result,
            "Query with blank nodes produced different results on run {}", i
        );
    }
}

#[test]
fn test_concurrent_query_determinism() {
    // Test if concurrent queries produce consistent results
    // Note: This doesn't use actual threads, but validates that multiple
    // simultaneous query executions are independent

    let dataset = create_test_dataset();
    let query = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                 SELECT ?person
                 WHERE { ?person foaf:knows ?friend }
                 ORDER BY ?person";

    // Execute query multiple times "concurrently" (sequentially but simulating concurrent access)
    let results: Vec<_> = (0..20)
        .map(|_| execute_query_as_string(&dataset, query))
        .collect();

    let first_result = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_result, result,
            "Concurrent query execution produced different results on run {}", i
        );
    }
}

#[test]
fn test_distinct_determinism() {
    // DISTINCT relies on hash sets - test if it's deterministic
    let query = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                 SELECT DISTINCT ?person
                 WHERE { ?person foaf:knows ?friend }
                 ORDER BY ?person";

    let dataset = create_test_dataset();

    let results: Vec<_> = (0..50)
        .map(|_| execute_query_as_string(&dataset, query))
        .collect();

    let first_result = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_result, result,
            "DISTINCT query produced different results on run {}", i
        );
    }
}

#[test]
fn test_aggregation_determinism() {
    // GROUP BY with aggregates - test determinism
    let query = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                 SELECT ?person (COUNT(?friend) AS ?count)
                 WHERE { ?person foaf:knows ?friend }
                 GROUP BY ?person
                 ORDER BY ?person";

    let dataset = create_test_dataset();

    let results: Vec<_> = (0..50)
        .map(|_| execute_query_as_string(&dataset, query))
        .collect();

    let first_result = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_result, result,
            "Aggregation query produced different results on run {}", i
        );
    }
}

#[test]
fn test_optional_determinism() {
    // OPTIONAL patterns - test determinism
    let query = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                 SELECT ?person ?name ?friend
                 WHERE {
                   ?person foaf:knows ?friend .
                   OPTIONAL { ?person foaf:name ?name }
                 }
                 ORDER BY ?person ?friend";

    let dataset = create_test_dataset();

    let results: Vec<_> = (0..50)
        .map(|_| execute_query_as_string(&dataset, query))
        .collect();

    let first_result = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_result, result,
            "OPTIONAL query produced different results on run {}", i
        );
    }
}
