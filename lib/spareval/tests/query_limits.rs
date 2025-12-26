use oxrdf::{Dataset, GraphName, NamedNode, Quad};
use spareval::{QueryEvaluator, QueryExecutionLimits, QueryResults};
use spargebra::SparqlParser;
use std::time::Duration;

fn create_test_dataset(size: usize) -> Dataset {
    let mut dataset = Dataset::new();
    let ex = NamedNode::new("http://example.com/").unwrap();

    for i in 0..size {
        let quad = Quad::new(
            NamedNode::new(format!("http://example.com/s{}", i)).unwrap(),
            ex.clone(),
            NamedNode::new(format!("http://example.com/o{}", i)).unwrap(),
            GraphName::DefaultGraph,
        );
        dataset.insert(&quad);
    }

    dataset
}

#[test]
fn test_default_limits() {
    let limits = QueryExecutionLimits::default();
    assert_eq!(limits.timeout, Some(Duration::from_secs(30)));
    assert_eq!(limits.max_result_rows, Some(10_000));
    assert_eq!(limits.max_groups, Some(1_000));
}

#[test]
fn test_strict_limits() {
    let limits = QueryExecutionLimits::strict();
    assert_eq!(limits.timeout, Some(Duration::from_secs(5)));
    assert_eq!(limits.max_result_rows, Some(1_000));
    assert_eq!(limits.max_groups, Some(100));
}

#[test]
fn test_permissive_limits() {
    let limits = QueryExecutionLimits::permissive();
    assert_eq!(limits.timeout, Some(Duration::from_secs(300)));
    assert_eq!(limits.max_result_rows, Some(100_000));
}

#[test]
fn test_unlimited() {
    let limits = QueryExecutionLimits::unlimited();
    assert!(limits.timeout.is_none());
    assert!(limits.max_result_rows.is_none());
    assert!(limits.max_groups.is_none());
}

#[test]
fn test_query_without_limits_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = create_test_dataset(100);
    let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;

    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 100);
    }

    Ok(())
}

#[test]
fn test_query_with_permissive_limits_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = create_test_dataset(100);
    let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::permissive());
    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 100);
    }

    Ok(())
}

#[test]
fn test_limits_are_cloneable() {
    let limits = QueryExecutionLimits::strict();
    let cloned = limits.clone();
    assert_eq!(limits, cloned);
}

#[test]
fn test_custom_limits() {
    let limits = QueryExecutionLimits {
        timeout: Some(Duration::from_secs(10)),
        max_result_rows: Some(5_000),
        max_groups: Some(500),
        max_property_path_depth: Some(500),
        max_memory_bytes: Some(512 * 1024 * 1024),
    };

    assert_eq!(limits.timeout, Some(Duration::from_secs(10)));
    assert_eq!(limits.max_result_rows, Some(5_000));
    assert_eq!(limits.max_groups, Some(500));
}

#[test]
fn test_evaluator_with_limits_builder() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = create_test_dataset(10);
    let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;

    // Test that the builder pattern works
    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 10);
    }

    Ok(())
}

#[test]
fn test_ask_query_with_limits() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = create_test_dataset(10);
    let query = SparqlParser::new().parse_query("ASK { ?s ?p ?o }")?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Boolean(result) = results {
        assert!(result);
    }

    Ok(())
}

#[test]
fn test_construct_query_with_limits() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = create_test_dataset(10);
    let query = SparqlParser::new().parse_query(
        "CONSTRUCT { ?s <http://example.com/new> ?o } WHERE { ?s ?p ?o }"
    )?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Graph(triples) = results {
        let count = triples.count();
        assert_eq!(count, 10);
    }

    Ok(())
}

#[test]
fn test_empty_dataset_with_limits() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = Dataset::new();
    let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 0);
    }

    Ok(())
}

#[test]
fn test_limits_struct_debug() {
    let limits = QueryExecutionLimits::strict();
    let debug_str = format!("{:?}", limits);
    assert!(debug_str.contains("QueryExecutionLimits"));
    assert!(debug_str.contains("timeout"));
}

#[test]
fn test_single_result_within_limit() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = create_test_dataset(1);
    let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits {
            max_result_rows: Some(10),
            ..QueryExecutionLimits::unlimited()
        });

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 1);
    }

    Ok(())
}

#[test]
fn test_order_by_with_limits() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = create_test_dataset(10);
    let query = SparqlParser::new().parse_query(
        "SELECT * WHERE { ?s ?p ?o } ORDER BY ?s"
    )?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 10);
    }

    Ok(())
}

#[test]
fn test_filter_with_limits() -> Result<(), Box<dyn std::error::Error>> {
    let mut dataset = Dataset::new();
    let ex = NamedNode::new("http://example.com/").unwrap();

    for i in 0..20 {
        let quad = Quad::new(
            ex.clone(),
            ex.clone(),
            oxrdf::Literal::from(i),
            GraphName::DefaultGraph,
        );
        dataset.insert(&quad);
    }

    let query = SparqlParser::new().parse_query(
        "SELECT * WHERE { ?s ?p ?o . FILTER(?o > 10) }"
    )?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 9); // 11, 12, 13, ..., 19
    }

    Ok(())
}

#[test]
fn test_distinct_with_limits() -> Result<(), Box<dyn std::error::Error>> {
    let mut dataset = Dataset::new();
    let ex = NamedNode::new("http://example.com/").unwrap();
    let val = NamedNode::new("http://example.com/value").unwrap();

    // Create duplicates
    for _ in 0..10 {
        let quad = Quad::new(
            ex.clone(),
            ex.clone(),
            val.clone(),
            GraphName::DefaultGraph,
        );
        dataset.insert(&quad);
    }

    let query = SparqlParser::new().parse_query(
        "SELECT DISTINCT * WHERE { ?s ?p ?o }"
    )?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 1); // Only one distinct result
    }

    Ok(())
}

#[test]
fn test_limit_clause_with_limits() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = create_test_dataset(100);
    let query = SparqlParser::new().parse_query(
        "SELECT * WHERE { ?s ?p ?o } LIMIT 5"
    )?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 5); // LIMIT clause takes precedence
    }

    Ok(())
}

#[test]
fn test_offset_with_limits() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = create_test_dataset(20);
    let query = SparqlParser::new().parse_query(
        "SELECT * WHERE { ?s ?p ?o } OFFSET 10"
    )?;

    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        let count = solutions.count();
        assert_eq!(count, 10); // 20 total - 10 offset = 10 results
    }

    Ok(())
}
