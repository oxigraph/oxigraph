//! Adversarial attack tests for ShEx validation security.
//!
//! This test suite validates the security claims in SECURITY.md by attempting
//! to exploit known attack vectors. Each test attempts an attack and verifies
//! that the appropriate limit is enforced.
//!
//! **EXPECTED STATUS**: Most of these tests will FAIL because the limits
//! infrastructure exists but is not connected to the actual validator.

use oxrdf::{Graph, Literal, NamedNode, Term, Triple};
use oxrdfio::{RdfFormat, RdfParser};
use sparshex::{ShapeExpression, ShapeId, ShapesSchema, ShexValidator, ValidationReport};
use std::time::{Duration, Instant};

// =============================================================================
// Helper Functions
// =============================================================================

/// Helper to parse a Turtle string into a Graph.
fn parse_turtle(turtle: &str) -> Graph {
    let mut graph = Graph::new();
    let parser = RdfParser::from_format(RdfFormat::Turtle);
    for quad_result in parser.for_reader(turtle.as_bytes()) {
        let quad = quad_result.expect("Failed to parse turtle");
        graph.insert(quad.as_ref());
    }
    graph
}

/// Helper to create a NamedNode.
fn nn(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

/// Helper to create a Term from IRI.
fn term(iri: &str) -> Term {
    Term::NamedNode(nn(iri))
}

// =============================================================================
// Attack Vector 1: Deep Recursion
// =============================================================================

#[test]
#[should_panic(expected = "recursion")]
fn test_deep_recursion_rejected() {
    // Create schema with 200 levels of nesting (exceeds default limit of 100)
    let mut shex = String::from(
        r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
        "#,
    );

    // Generate deep chain: Shape1 -> Shape2 -> ... -> Shape200
    for i in 1..=200 {
        if i < 200 {
            shex.push_str(&format!(
                "ex:Shape{} {{ ex:next @ex:Shape{} }}\n",
                i,
                i + 1
            ));
        } else {
            shex.push_str(&format!("ex:Shape{} {{ ex:value xsd:string }}\n", i));
        }
    }

    // Parse schema (this might fail first)
    let schema = match sparshex::parse_shex(&shex) {
        Ok(s) => s,
        Err(e) => panic!("Failed to parse deep schema: {}", e),
    };

    let validator = ShexValidator::new(schema);

    // Create data that exercises the full depth
    let mut turtle = String::from(
        r#"
        @prefix ex: <http://example.org/> .
        "#,
    );

    for i in 1..200 {
        turtle.push_str(&format!("ex:node{} ex:next ex:node{} .\n", i, i + 1));
    }
    turtle.push_str("ex:node200 ex:value \"deep\" .\n");

    let data = parse_turtle(&turtle);

    // This should be rejected at depth limit (100)
    let shape_id = ShapeId::new(nn("http://example.org/Shape1"));
    let result = validator.validate_node(&data, &term("http://example.org/node1"), &shape_id);

    // Should fail with recursion depth error
    match result {
        Err(e) => {
            let error_msg = e.to_string().to_lowercase();
            assert!(
                error_msg.contains("recursion"),
                "Expected recursion error, got: {}",
                e
            );
        }
        Ok(report) => {
            panic!(
                "SECURITY FAILURE: Deep recursion not blocked! Report: {:?}",
                report
            );
        }
    }
}

#[test]
fn test_cyclic_schema_terminates() {
    // Schema: PersonShape -> PersonShape (self-reference via foaf:knows)
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:friend @ex:PersonShape *
        }
    "#;

    let schema = sparshex::parse_shex(shex).expect("Failed to parse cyclic schema");
    let validator = ShexValidator::new(schema);

    // Data with cycle: alice -> bob -> charlie -> alice
    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .

        ex:alice ex:name "Alice" ;
                 ex:friend ex:bob .

        ex:bob ex:name "Bob" ;
               ex:friend ex:charlie .

        ex:charlie ex:name "Charlie" ;
                  ex:friend ex:alice .
    "#,
    );

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));

    // Validation should terminate (not hang)
    let start = Instant::now();
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);
    let elapsed = start.elapsed();

    // Should complete within 2 seconds (not hang infinitely)
    assert!(
        elapsed < Duration::from_secs(2),
        "Validation took too long: {:?} - possible infinite loop",
        elapsed
    );

    // Should succeed (cycles are allowed via visited set)
    assert!(result.is_ok(), "Cyclic validation failed: {:?}", result);
}

// =============================================================================
// Attack Vector 2: High Cardinality
// =============================================================================

#[test]
#[ignore] // This test exposes lack of cardinality limit
fn test_high_cardinality_bounded() {
    // Shape with very high cardinality {0,100000}
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:Shape {
            ex:value xsd:string {0,100000}
        }
    "#;

    // This should either:
    // 1. Be rejected during parsing (cardinality too high)
    // 2. Be bounded by max_triples_examined during validation

    let schema = match sparshex::parse_shex(shex) {
        Ok(s) => s,
        Err(_) => {
            // Good - rejected at parse time
            return;
        }
    };

    // If parsing succeeded, create data with many values
    let validator = ShexValidator::new(schema);
    let mut turtle = String::from(
        r#"
        @prefix ex: <http://example.org/> .
        ex:node "#,
    );

    // Add 1000 values
    for i in 0..1000 {
        if i > 0 {
            turtle.push_str("; ");
        }
        turtle.push_str(&format!("ex:value \"val{}\"", i));
    }
    turtle.push_str(" .");

    let data = parse_turtle(&turtle);
    let shape_id = ShapeId::new(nn("http://example.org/Shape"));

    // Should be bounded by max_triples_examined
    let result = validator.validate_node(&data, &term("http://example.org/node"), &shape_id);

    // If no limit, this will process all 1000 triples
    println!("SECURITY WARNING: No cardinality limit enforced. Result: {:?}", result);
}

// =============================================================================
// Attack Vector 3: ReDoS (Regular Expression Denial of Service)
// =============================================================================

#[test]
#[ignore] // This test exposes lack of regex protection
fn test_redos_regex_blocked() {
    // Classic ReDoS pattern: (a+)+
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:Shape {
            ex:value xsd:string /^(a+)+$/
        }
    "#;

    let schema = match sparshex::parse_shex(shex) {
        Ok(s) => s,
        Err(e) => {
            // Good - dangerous regex rejected
            assert!(
                e.to_string().contains("regex") || e.to_string().contains("pattern"),
                "Should reject dangerous regex pattern"
            );
            return;
        }
    };

    // If parsing succeeded, test with pathological input
    let validator = ShexValidator::new(schema);

    // This input causes catastrophic backtracking in the (a+)+ pattern
    let pathological_input = "a".repeat(30) + "b"; // 30 a's followed by b

    let data = parse_turtle(&format!(
        r#"
        @prefix ex: <http://example.org/> .
        ex:node ex:value "{}" .
    "#,
        pathological_input
    ));

    let shape_id = ShapeId::new(nn("http://example.org/Shape"));
    let start = Instant::now();
    let result = validator.validate_node(&data, &term("http://example.org/node"), &shape_id);
    let elapsed = start.elapsed();

    // Should timeout or complete quickly
    assert!(
        elapsed < Duration::from_secs(2),
        "SECURITY FAILURE: ReDoS pattern caused slow validation: {:?}",
        elapsed
    );

    println!("ReDoS test completed in {:?}: {:?}", elapsed, result);
}

#[test]
#[ignore] // This test exposes lack of regex length limit
fn test_very_long_regex_rejected() {
    // Regex pattern with 2000 characters (exceeds default limit of 1000)
    let long_pattern = "a".repeat(2000);

    let shex = format!(
        r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:Shape {{
            ex:value xsd:string /^{}$/
        }}
    "#,
        long_pattern
    );

    // Should be rejected during parsing
    let result = sparshex::parse_shex(&shex);

    match result {
        Err(e) => {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("regex") || error_msg.contains("long") || error_msg.contains("length"),
                "Expected regex length error, got: {}",
                error_msg
            );
        }
        Ok(_) => {
            panic!("SECURITY FAILURE: Very long regex was not rejected!");
        }
    }
}

// =============================================================================
// Attack Vector 4: Combinatorial Explosion
// =============================================================================

#[test]
#[ignore] // This test exposes lack of shape reference limit
fn test_combinatorial_explosion_prevented() {
    // Multiple nested shapes with high fan-out
    let mut shex = String::from(
        r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:Root {
    "#,
    );

    // Create 20 properties, each with OR of 10 shapes
    for i in 1..=20 {
        shex.push_str(&format!("ex:prop{} (", i));
        for j in 1..=10 {
            if j > 1 {
                shex.push_str(" OR ");
            }
            shex.push_str(&format!("@ex:Sub{}_{}", i, j));
        }
        shex.push_str(") ;");
    }

    shex.push_str("}\n");

    // Define all sub-shapes
    for i in 1..=20 {
        for j in 1..=10 {
            shex.push_str(&format!(
                "ex:Sub{}_{} {{ ex:value xsd:string }}\n",
                i, j
            ));
        }
    }

    let schema = match sparshex::parse_shex(&shex) {
        Ok(s) => s,
        Err(e) => {
            println!("Schema rejected (good): {}", e);
            return;
        }
    };

    let validator = ShexValidator::new(schema);

    // Create data that matches
    let mut turtle = String::from(
        r#"
        @prefix ex: <http://example.org/> .
        ex:root "#,
    );

    for i in 1..=20 {
        if i > 1 {
            turtle.push_str("; ");
        }
        turtle.push_str(&format!("ex:prop{} ex:sub{}_{}", i, i, 1));
    }
    turtle.push_str(" .\n");

    for i in 1..=20 {
        turtle.push_str(&format!("ex:sub{}_{} ex:value \"test\" .\n", i, 1));
    }

    let data = parse_turtle(&turtle);
    let shape_id = ShapeId::new(nn("http://example.org/Root"));

    // Should be limited by max_shape_references
    let start = Instant::now();
    let result = validator.validate_node(&data, &term("http://example.org/root"), &shape_id);
    let elapsed = start.elapsed();

    // Should either fail with limit error or complete quickly
    if elapsed > Duration::from_secs(5) {
        panic!(
            "SECURITY FAILURE: Combinatorial explosion not prevented! Took: {:?}",
            elapsed
        );
    }

    println!("Combinatorial test result: {:?} in {:?}", result, elapsed);
}

// =============================================================================
// Attack Vector 5: Large Graph Validation
// =============================================================================

#[test]
#[ignore] // This test exposes lack of triple examination limit
fn test_large_graph_validation_bounded() {
    // Simple shape but large graph
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:NodeShape {
            ex:value xsd:string *
        }
    "#;

    let schema = sparshex::parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Create graph with 10,000 triples on one node
    let mut turtle = String::from(
        r#"
        @prefix ex: <http://example.org/> .
        ex:node "#,
    );

    for i in 0..10000 {
        if i > 0 {
            turtle.push_str("; ");
        }
        turtle.push_str(&format!("ex:value \"value{}\"", i));
    }
    turtle.push_str(" .");

    let data = parse_turtle(&turtle);
    let shape_id = ShapeId::new(nn("http://example.org/NodeShape"));

    // Should be bounded by max_triples_examined
    let start = Instant::now();
    let result = validator.validate_node(&data, &term("http://example.org/node"), &shape_id);
    let elapsed = start.elapsed();

    match result {
        Err(e) => {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("triples") || error_msg.contains("limit"),
                "Expected triples limit error, got: {}",
                error_msg
            );
        }
        Ok(_) => {
            println!(
                "SECURITY WARNING: No triple examination limit enforced. Processed 10K triples in {:?}",
                elapsed
            );
        }
    }
}

// =============================================================================
// Attack Vector 6: Timeout Enforcement
// =============================================================================

#[test]
#[ignore] // This test exposes lack of timeout
fn test_validation_timeout_enforced() {
    // Create a schema that's slow to validate
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:SlowShape {
            ex:p1 @ex:SubShape * ;
            ex:p2 @ex:SubShape * ;
            ex:p3 @ex:SubShape * ;
            ex:p4 @ex:SubShape * ;
            ex:p5 @ex:SubShape *
        }

        ex:SubShape {
            ex:value xsd:string
        }
    "#;

    let schema = sparshex::parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Create data with many nodes
    let mut turtle = String::from(
        r#"
        @prefix ex: <http://example.org/> .
        ex:root ex:p1 ex:n1 ; ex:p2 ex:n2 ; ex:p3 ex:n3 ; ex:p4 ex:n4 ; ex:p5 ex:n5 .
    "#,
    );

    for i in 1..=5 {
        turtle.push_str(&format!("ex:n{} ex:value \"test\" .\n", i));
    }

    let data = parse_turtle(&turtle);
    let shape_id = ShapeId::new(nn("http://example.org/SlowShape"));

    // Should timeout (default 30s, strict 5s)
    let start = Instant::now();
    let result = validator.validate_node(&data, &term("http://example.org/root"), &shape_id);
    let elapsed = start.elapsed();

    // Check if timeout was enforced
    match result {
        Err(e) => {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("timeout") || error_msg.contains("time"),
                "Expected timeout error, got: {}",
                error_msg
            );
        }
        Ok(_) => {
            println!(
                "SECURITY WARNING: No timeout enforced. Validation completed in {:?}",
                elapsed
            );
        }
    }
}

// =============================================================================
// Limit Verification Tests
// =============================================================================

#[test]
#[ignore] // ValidationLimits not in public API
fn test_validation_limits_struct_exists() {
    // This test verifies that ValidationLimits is exported
    // EXPECTED TO FAIL: ValidationLimits is not in public API

    // Uncommenting this would cause compilation error:
    // let limits = sparshex::ValidationLimits::default();

    panic!("CRITICAL: ValidationLimits is not exported in public API! See lib.rs");
}

// =============================================================================
// Summary Test
// =============================================================================

#[test]
fn test_security_audit_summary() {
    println!("\n=== ShEx Security Audit Summary ===\n");
    println!("Attack Vectors Tested:");
    println!("1. Deep recursion: PARTIAL (hardcoded to 100, not configurable)");
    println!("2. Cyclic schemas: WORKING (visited set prevents infinite loops)");
    println!("3. High cardinality: NOT ENFORCED (no limit)");
    println!("4. ReDoS regex: NOT ENFORCED (no pattern validation)");
    println!("5. Regex length: NOT ENFORCED (no length check)");
    println!("6. Combinatorial explosion: NOT ENFORCED (no shape ref counting)");
    println!("7. Large graphs: NOT ENFORCED (no triple counting)");
    println!("8. Timeout: NOT ENFORCED (no timeout check)");
    println!("\nLimits Infrastructure Status:");
    println!("- ValidationLimits struct: EXISTS in limits.rs");
    println!("- ValidationContext: EXISTS in limits.rs");
    println!("- Public API export: NOT EXPORTED");
    println!("- Validator integration: NOT INTEGRATED");
    println!("\n=== VERDICT: SECURITY CLAIMS NOT VALIDATED ===");
    println!("The limits infrastructure exists but is disconnected from the validator.");
    println!("Most security claims in SECURITY.md are aspirational, not implemented.");
}
