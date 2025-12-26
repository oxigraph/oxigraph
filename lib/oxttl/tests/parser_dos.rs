//! Parser DoS Protection Tests
//!
//! These tests validate that the parser rejects maliciously crafted input
//! that could cause denial of service through excessive resource consumption.

use oxttl::TurtleParser;

/// Generate deeply nested RDF collections: ( ( ( ... ) ) )
fn generate_nested_collections(depth: usize) -> String {
    let mut turtle = String::from("@prefix : <http://example.org/> .\n:s :p ");

    // Opening parentheses
    for _ in 0..depth {
        turtle.push_str("( ");
    }

    // Add a value at the deepest level
    turtle.push_str(":value ");

    // Closing parentheses
    for _ in 0..depth {
        turtle.push_str(") ");
    }

    turtle.push_str(".");
    turtle
}

/// Generate deeply nested blank node property lists: [ :p [ :p [ :p ... ] ] ]
fn generate_nested_blank_nodes(depth: usize) -> String {
    let mut turtle = String::from("@prefix : <http://example.org/> .\n:s :p ");

    // Opening brackets with property
    for _ in 0..depth {
        turtle.push_str("[ :p ");
    }

    // Add a value at the deepest level
    turtle.push_str(":value ");

    // Closing brackets
    for _ in 0..depth {
        turtle.push_str("] ");
    }

    turtle.push_str(".");
    turtle
}

/// Generate a huge literal string
fn generate_huge_literal(size_bytes: usize) -> String {
    format!(
        "@prefix : <http://example.org/> .\n:s :p \"{}\" .",
        "a".repeat(size_bytes)
    )
}

#[test]
fn test_deeply_nested_collections_attack() {
    // Generate 10,000 levels of nested collections
    // This should be rejected by the nesting depth limit
    let nested_turtle = generate_nested_collections(10_000);

    let mut count = 0;
    let mut encountered_error = false;

    for result in TurtleParser::new().for_slice(&nested_turtle) {
        match result {
            Ok(_) => count += 1,
            Err(e) => {
                // Verify it's a nesting depth error
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("nesting") || error_msg.contains("depth"),
                    "Expected nesting depth error, got: {}",
                    error_msg
                );
                encountered_error = true;
                break;
            }
        }
    }

    assert!(
        encountered_error,
        "Parser should reject deeply nested input, but accepted {} triples",
        count
    );
}

#[test]
fn test_deeply_nested_blank_nodes_attack() {
    // Generate 10,000 levels of nested blank node property lists
    // This should be rejected by the nesting depth limit
    let nested_turtle = generate_nested_blank_nodes(10_000);

    let mut count = 0;
    let mut encountered_error = false;

    for result in TurtleParser::new().for_slice(&nested_turtle) {
        match result {
            Ok(_) => count += 1,
            Err(e) => {
                // Verify it's a nesting depth error
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("nesting") || error_msg.contains("depth"),
                    "Expected nesting depth error, got: {}",
                    error_msg
                );
                encountered_error = true;
                break;
            }
        }
    }

    assert!(
        encountered_error,
        "Parser should reject deeply nested blank nodes, but accepted {} triples",
        count
    );
}

#[test]
fn test_moderate_nesting_allowed() {
    // 50 levels should be allowed (under default limit of 100)
    let nested_turtle = generate_nested_collections(50);

    let mut count = 0;
    for result in TurtleParser::new().for_slice(&nested_turtle) {
        result.expect("Moderate nesting should be allowed");
        count += 1;
    }

    assert!(count > 0, "Should parse at least some triples");
}

#[test]
#[ignore] // Slow test - only run when verifying limits
fn test_huge_literal_attack() {
    // Try to parse a 100 MB literal
    let huge_literal = generate_huge_literal(100_000_000);

    for result in TurtleParser::new().for_slice(&huge_literal) {
        match result {
            Ok(_) => panic!("Parser should reject huge literals"),
            Err(_e) => {
                // TODO: Verify error is about size limit
                return;
            }
        }
    }
}

#[test]
fn test_normal_input_works() {
    // Verify normal input still works
    let normal_turtle = r#"
        @prefix : <http://example.org/> .
        :subject :predicate :object .
        :foo :bar ( :item1 :item2 :item3 ) .
        :baz :qux [ :nested :value ] .
    "#;

    let mut count = 0;
    for result in TurtleParser::new().for_slice(normal_turtle) {
        result.expect("Normal input should parse successfully");
        count += 1;
    }

    assert!(count >= 2, "Should parse multiple triples from normal input");
}

#[test]
fn test_collection_in_collection() {
    // Test nested collections at reasonable depth
    let turtle = r#"
        @prefix : <http://example.org/> .
        :s :p ( ( :a :b ) ( :c :d ) ) .
    "#;

    for result in TurtleParser::new().for_slice(turtle) {
        result.expect("Nested collections should work at reasonable depth");
    }
}

#[test]
fn test_blank_node_in_blank_node() {
    // Test nested blank nodes at reasonable depth
    let turtle = r#"
        @prefix : <http://example.org/> .
        :s :p [ :p1 :v1 ; :p2 [ :p3 :v3 ] ] .
    "#;

    for result in TurtleParser::new().for_slice(turtle) {
        result.expect("Nested blank nodes should work at reasonable depth");
    }
}
