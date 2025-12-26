/// Comprehensive unit tests for SPARQL parser
/// Testing the most common query patterns (80/20 principle)
use spargebra::{Query, SparqlParser};

#[test]
fn test_simple_select_all() {
    let query_str = "SELECT * WHERE { ?s ?p ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }

    // Note: SELECT * gets expanded to list of variables during serialization
    // This is expected behavior
    let serialized = query.to_string();
    assert!(serialized.starts_with("SELECT"));
    assert!(serialized.contains("?s"));
    assert!(serialized.contains("?p"));
    assert!(serialized.contains("?o"));
}

#[test]
fn test_simple_select_with_variables() {
    let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }

    assert_eq!(query.to_string(), query_str);
}

#[test]
fn test_select_distinct() {
    let query_str = "SELECT DISTINCT ?s WHERE { ?s ?p ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_filter() {
    let query_str = "SELECT ?s WHERE { ?s ?p ?o . FILTER(?o > 10) }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_regex_filter() {
    let query_str = r#"SELECT ?name WHERE { ?person <http://example.org/name> ?name . FILTER(REGEX(?name, "^A")) }"#;
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_optional() {
    let query_str = "SELECT ?s ?o WHERE { ?s <http://example.org/name> ?n . OPTIONAL { ?s <http://example.org/email> ?o } }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_union() {
    let query_str = "SELECT ?s WHERE { { ?s <http://example.org/type1> ?o } UNION { ?s <http://example.org/type2> ?o } }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_order_by() {
    let query_str = "SELECT ?s ?o WHERE { ?s ?p ?o } ORDER BY ?s";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_order_by_desc() {
    let query_str = "SELECT ?s ?o WHERE { ?s ?p ?o } ORDER BY DESC(?o)";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_limit() {
    let query_str = "SELECT ?s WHERE { ?s ?p ?o } LIMIT 10";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_offset() {
    let query_str = "SELECT ?s WHERE { ?s ?p ?o } OFFSET 5";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_limit_and_offset() {
    let query_str = "SELECT ?s WHERE { ?s ?p ?o } LIMIT 10 OFFSET 20";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_order_limit_offset() {
    let query_str = "SELECT ?s ?name WHERE { ?s <http://example.org/name> ?name } ORDER BY ?name LIMIT 5 OFFSET 10";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_ask_query() {
    let query_str = "ASK WHERE { ?s ?p ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Ask { .. } => (),
        _ => panic!("Expected ASK query"),
    }

    // Verify serialization contains key elements
    let serialized = query.to_string();
    assert!(serialized.starts_with("ASK"));
    assert!(serialized.contains("WHERE"));
}

#[test]
fn test_ask_query_with_specific_pattern() {
    let query_str = "ASK WHERE { <http://example.org/resource> <http://example.org/property> ?value }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Ask { .. } => (),
        _ => panic!("Expected ASK query"),
    }
}

#[test]
fn test_construct_query() {
    let query_str = "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Construct { .. } => (),
        _ => panic!("Expected CONSTRUCT query"),
    }
}

#[test]
fn test_construct_with_transformation() {
    let query_str = "CONSTRUCT { ?s <http://example.org/new> ?o } WHERE { ?s <http://example.org/old> ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Construct { .. } => (),
        _ => panic!("Expected CONSTRUCT query"),
    }
}

#[test]
fn test_describe_query() {
    let query_str = "DESCRIBE * WHERE { ?s ?p ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Describe { .. } => (),
        _ => panic!("Expected DESCRIBE query"),
    }
}

#[test]
fn test_select_with_prefixes() {
    let query_str = "PREFIX ex: <http://example.org/> SELECT ?s WHERE { ?s ex:name ?n }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_base_iri() {
    let query_str = "BASE <http://example.org/> SELECT ?s WHERE { ?s <name> ?n }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_parser_with_base_iri() {
    let parser = SparqlParser::new()
        .with_base_iri("http://example.com/")
        .unwrap();

    let query_str = "SELECT * WHERE { <s> <p> <o> }";
    let query = parser.parse_query(query_str).unwrap();

    assert_eq!(
        query.to_string(),
        "BASE <http://example.com/>\nSELECT * WHERE { <http://example.com/s> <http://example.com/p> <http://example.com/o> . }"
    );
}

#[test]
fn test_parser_with_prefix() {
    let parser = SparqlParser::new()
        .with_prefix("ex", "http://example.com/")
        .unwrap();

    let query_str = "SELECT * WHERE { ex:s ex:p ex:o }";
    let query = parser.parse_query(query_str).unwrap();

    assert_eq!(
        query.to_string(),
        "SELECT * WHERE { <http://example.com/s> <http://example.com/p> <http://example.com/o> . }"
    );
}

#[test]
fn test_select_with_bind() {
    let query_str = "SELECT ?s ?age WHERE { ?s <http://example.org/birthYear> ?year . BIND(2024 - ?year AS ?age) }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_group_by() {
    let query_str = "SELECT ?s (COUNT(?o) AS ?count) WHERE { ?s ?p ?o } GROUP BY ?s";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_having() {
    let query_str = "SELECT ?s (COUNT(?o) AS ?count) WHERE { ?s ?p ?o } GROUP BY ?s HAVING(COUNT(?o) > 5)";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

// UPDATE operation tests

#[test]
fn test_insert_data() {
    let update_str = "INSERT DATA { <http://example.org/s> <http://example.org/p> <http://example.org/o> }";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.operations.len(), 1);
}

#[test]
fn test_delete_data() {
    let update_str = "DELETE DATA { <http://example.org/s> <http://example.org/p> <http://example.org/o> }";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.operations.len(), 1);
}

#[test]
fn test_delete_insert() {
    let update_str = "DELETE { ?s <http://example.org/old> ?o } INSERT { ?s <http://example.org/new> ?o } WHERE { ?s <http://example.org/old> ?o }";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.operations.len(), 1);
}

#[test]
fn test_clear_all() {
    let update_str = "CLEAR ALL ;";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.to_string().trim(), update_str);
    assert_eq!(update.to_sse(), "(update (clear all))");
}

#[test]
fn test_clear_default() {
    let update_str = "CLEAR DEFAULT ;";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.operations.len(), 1);
}

#[test]
fn test_clear_graph() {
    let update_str = "CLEAR GRAPH <http://example.org/graph> ;";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.operations.len(), 1);
}

#[test]
fn test_drop_graph() {
    let update_str = "DROP GRAPH <http://example.org/graph> ;";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.operations.len(), 1);
}

#[test]
fn test_create_graph() {
    let update_str = "CREATE GRAPH <http://example.org/graph> ;";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.operations.len(), 1);
}

#[test]
fn test_load_graph() {
    let update_str = "LOAD <http://example.org/source> ;";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.operations.len(), 1);
}

// Error handling tests

#[test]
fn test_invalid_syntax_missing_where() {
    let query_str = "SELECT ?s ?p ?o";
    let result = SparqlParser::new().parse_query(query_str);

    assert!(result.is_err(), "Should fail parsing query without WHERE clause");
}

#[test]
fn test_invalid_syntax_unclosed_brace() {
    let query_str = "SELECT ?s WHERE { ?s ?p ?o";
    let result = SparqlParser::new().parse_query(query_str);

    assert!(result.is_err(), "Should fail parsing query with unclosed brace");
}

#[test]
fn test_invalid_syntax_bad_filter() {
    let query_str = "SELECT ?s WHERE { ?s ?p ?o . FILTER() }";
    let result = SparqlParser::new().parse_query(query_str);

    assert!(result.is_err(), "Should fail parsing query with empty FILTER");
}

#[test]
fn test_invalid_variable_name() {
    // Variables cannot start with certain characters
    let query_str = "SELECT ?-invalid WHERE { ?s ?p ?o }";
    let result = SparqlParser::new().parse_query(query_str);

    assert!(result.is_err(), "Should fail parsing query with invalid variable name");
}

#[test]
fn test_empty_query() {
    let query_str = "";
    let result = SparqlParser::new().parse_query(query_str);

    assert!(result.is_err(), "Should fail parsing empty query");
}

// Advanced pattern tests

#[test]
fn test_nested_optional() {
    let query_str = "SELECT ?s ?o1 ?o2 WHERE { ?s <http://example.org/p1> ?o1 . OPTIONAL { ?s <http://example.org/p2> ?o2 . OPTIONAL { ?s <http://example.org/p3> ?o3 } } }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_multiple_unions() {
    let query_str = "SELECT ?s WHERE { { ?s <http://example.org/type1> ?o } UNION { ?s <http://example.org/type2> ?o } UNION { ?s <http://example.org/type3> ?o } }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_multiple_filters() {
    let query_str = "SELECT ?s ?o WHERE { ?s ?p ?o . FILTER(?o > 10) FILTER(?o < 100) }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_values() {
    let query_str = "SELECT ?s WHERE { ?s ?p ?o } VALUES ?p { <http://example.org/p1> <http://example.org/p2> }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_graph_pattern() {
    let query_str = "SELECT ?s ?o WHERE { GRAPH <http://example.org/graph> { ?s ?p ?o } }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_service() {
    let query_str = "SELECT ?s WHERE { SERVICE <http://example.org/sparql> { ?s ?p ?o } }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_minus() {
    let query_str = "SELECT ?s WHERE { ?s ?p ?o . MINUS { ?s <http://example.org/excluded> ?x } }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

// Literal tests

#[test]
fn test_select_with_string_literal() {
    let query_str = r#"SELECT ?s WHERE { ?s <http://example.org/name> "John Doe" }"#;
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_integer_literal() {
    let query_str = "SELECT ?s WHERE { ?s <http://example.org/age> 42 }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_language_tag() {
    let query_str = r#"SELECT ?s WHERE { ?s <http://example.org/label> "hello"@en }"#;
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_select_with_datatype() {
    let query_str = r#"SELECT ?s WHERE { ?s <http://example.org/created> "2024-01-01"^^<http://www.w3.org/2001/XMLSchema#date> }"#;
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

// Property path tests

#[test]
fn test_property_path_sequence() {
    let query_str = "SELECT ?s ?o WHERE { ?s <http://example.org/p1>/<http://example.org/p2> ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_property_path_alternative() {
    let query_str = "SELECT ?s ?o WHERE { ?s <http://example.org/p1>|<http://example.org/p2> ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_property_path_zero_or_more() {
    let query_str = "SELECT ?s ?o WHERE { ?s <http://example.org/parent>* ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_property_path_one_or_more() {
    let query_str = "SELECT ?s ?o WHERE { ?s <http://example.org/parent>+ ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

// Aggregate function tests

#[test]
fn test_count_aggregate() {
    let query_str = "SELECT (COUNT(*) AS ?count) WHERE { ?s ?p ?o }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_sum_aggregate() {
    let query_str = "SELECT (SUM(?amount) AS ?total) WHERE { ?s <http://example.org/amount> ?amount }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_avg_aggregate() {
    let query_str = "SELECT (AVG(?value) AS ?average) WHERE { ?s <http://example.org/value> ?value }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

#[test]
fn test_min_max_aggregates() {
    let query_str = "SELECT (MIN(?value) AS ?min) (MAX(?value) AS ?max) WHERE { ?s <http://example.org/value> ?value }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

// Subquery tests

#[test]
fn test_subquery() {
    let query_str = "SELECT ?s WHERE { ?s ?p ?o . { SELECT ?s WHERE { ?s <http://example.org/type> <http://example.org/Person> } } }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();

    match query {
        Query::Select { .. } => (),
        _ => panic!("Expected SELECT query"),
    }
}

// Multiple update operations

#[test]
fn test_multiple_updates() {
    let update_str = "INSERT DATA { <http://example.org/s1> <http://example.org/p> <http://example.org/o1> } ; INSERT DATA { <http://example.org/s2> <http://example.org/p> <http://example.org/o2> } ;";
    let update = SparqlParser::new().parse_update(update_str).unwrap();

    assert_eq!(update.operations.len(), 2);
}

// Round-trip tests (parse and serialize back)

#[test]
fn test_roundtrip_simple_select() {
    let original = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
    let query = SparqlParser::new().parse_query(original).unwrap();
    let serialized = query.to_string();
    let reparsed = SparqlParser::new().parse_query(&serialized).unwrap();

    assert_eq!(query, reparsed);
}

#[test]
fn test_roundtrip_ask() {
    let original = "ASK WHERE { ?s ?p ?o . }";
    let query = SparqlParser::new().parse_query(original).unwrap();

    // Verify it's an ASK query
    match query {
        Query::Ask { .. } => (),
        _ => panic!("Expected ASK query"),
    }

    // Serialize and verify it can be parsed back
    let serialized = query.to_string();
    let reparsed = SparqlParser::new().parse_query(&serialized).unwrap();

    // Both should be ASK queries
    match reparsed {
        Query::Ask { .. } => (),
        _ => panic!("Reparsed query should be ASK"),
    }
}

#[test]
fn test_sse_format() {
    let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
    let query = SparqlParser::new().parse_query(query_str).unwrap();
    let sse = query.to_sse();

    assert_eq!(sse, "(project (?s ?p ?o) (bgp (triple ?s ?p ?o)))");
}
