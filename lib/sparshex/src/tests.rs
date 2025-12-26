//! Unit tests for ShEx components.
//!
//! This module contains unit tests for parser, model, and validator components.

#![cfg(test)]

use crate::{
    error::{ShexError, ShexParseError, ShexValidationError},
    model::{
        NodeConstraint, ShapeExpression, ShapeLabel, ShapesSchema, TripleConstraint,
    },
    result::ValidationResult,
    validator::ShexValidator,
};
use oxrdf::{Graph, Literal, NamedNode, Term, Triple};
use oxrdfio::{RdfFormat, RdfParser};

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

/// Helper to create a simple NamedNode.
fn nn(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

/// Helper to create a Term from a NamedNode.
fn term(iri: &str) -> Term {
    Term::NamedNode(nn(iri))
}

// =============================================================================
// Parser Tests
// =============================================================================

#[test]
fn test_parse_empty_schema() {
    let shex = "";
    let result = parse_shex(shex);
    assert!(result.is_ok());
    let schema = result.unwrap();
    assert_eq!(schema.shapes().count(), 0);
}

#[test]
fn test_parse_simple_shape() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string
        }
    "#;
    let result = parse_shex(shex);
    assert!(result.is_ok(), "Failed to parse simple shape: {:?}", result);
    let schema = result.unwrap();
    assert_eq!(schema.shapes().count(), 1);
}

#[test]
fn test_parse_shape_with_cardinality() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:email xsd:string * ;
            ex:phone xsd:string +
        }
    "#;
    let result = parse_shex(shex);
    assert!(result.is_ok(), "Failed to parse shape with cardinality: {:?}", result);
}

#[test]
fn test_parse_shape_with_minmax() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string {1,1} ;
            ex:email xsd:string {0,3}
        }
    "#;
    let result = parse_shex(shex);
    assert!(result.is_ok(), "Failed to parse shape with min/max: {:?}", result);
}

#[test]
fn test_parse_invalid_syntax() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        ex:BadShape { { { }
    "#;
    let result = parse_shex(shex);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ShexError::Parse(_)));
}

#[test]
fn test_parse_missing_prefix() {
    let shex = r#"
        ex:PersonShape {
            ex:name xsd:string
        }
    "#;
    let result = parse_shex(shex);
    // Should fail due to missing PREFIX declarations
    assert!(result.is_err());
}

#[test]
fn test_parse_shape_or() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonOrOrgShape = ex:PersonShape OR ex:OrgShape

        ex:PersonShape {
            ex:name xsd:string
        }

        ex:OrgShape {
            ex:orgName xsd:string
        }
    "#;
    let result = parse_shex(shex);
    assert!(result.is_ok(), "Failed to parse shape OR: {:?}", result);
}

#[test]
fn test_parse_shape_and() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:EmployeeShape = ex:PersonShape AND ex:EmployeeInfoShape

        ex:PersonShape {
            ex:name xsd:string
        }

        ex:EmployeeInfoShape {
            ex:employeeId xsd:integer
        }
    "#;
    let result = parse_shex(shex);
    assert!(result.is_ok(), "Failed to parse shape AND: {:?}", result);
}

#[test]
fn test_parse_shape_not() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:NotPersonShape = NOT ex:PersonShape

        ex:PersonShape {
            ex:name xsd:string
        }
    "#;
    let result = parse_shex(shex);
    assert!(result.is_ok(), "Failed to parse shape NOT: {:?}", result);
}

#[test]
fn test_parse_node_constraint_string() {
    let shex = r#"
        PREFIX ex: <http://example.org/>

        ex:StringShape xsd:string
    "#;
    let result = parse_shex(shex);
    assert!(result.is_ok(), "Failed to parse string node constraint: {:?}", result);
}

#[test]
fn test_parse_node_constraint_iri() {
    let shex = r#"
        PREFIX ex: <http://example.org/>

        ex:IRIShape IRI
    "#;
    let result = parse_shex(shex);
    assert!(result.is_ok(), "Failed to parse IRI node constraint: {:?}", result);
}

// =============================================================================
// Model Tests
// =============================================================================

#[test]
fn test_shapes_schema_new() {
    let schema = ShapesSchema::new();
    assert_eq!(schema.shapes().count(), 0);
}

#[test]
fn test_shape_expression_construction() {
    // Test ShapeRef construction
    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let shape_ref = ShapeExpression::ShapeRef(ShapeRef::new(shape_id.clone()));
    assert!(matches!(shape_ref, ShapeExpression::ShapeRef(_)));
}

#[test]
fn test_triple_constraint_construction() {
    let predicate = nn("http://example.org/name");
    let constraint = TripleConstraint::new(predicate);
    assert!(constraint.predicate().as_str() == "http://example.org/name");
}

#[test]
fn test_node_constraint_datatype() {
    let datatype = nn("http://www.w3.org/2001/XMLSchema#string");
    let constraint = NodeConstraint::datatype(datatype);
    assert!(matches!(constraint, NodeConstraint::Datatype(_)));
}

// =============================================================================
// Validator Tests
// =============================================================================

#[test]
fn test_validator_new() {
    let schema = ShapesSchema::new();
    let validator = ShexValidator::new(schema);
    assert!(validator.schema().shapes().count() == 0);
}

#[test]
fn test_validate_empty_schema_empty_data() {
    let schema = ShapesSchema::new();
    let validator = ShexValidator::new(schema);
    let data = Graph::new();

    let result = validator.validate(&data);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(report.conforms());
}

#[test]
fn test_validate_simple_conforming() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok(), "Validation failed: {:?}", result);
    let report = result.unwrap();
    assert!(report.conforms(), "Expected conformance");
}

#[test]
fn test_validate_missing_required_property() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Data without the required name property
    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:age 30 .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.conforms(), "Expected non-conformance due to missing property");
}

#[test]
fn test_validate_wrong_datatype() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:age xsd:integer
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Data with string instead of integer
    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:age "thirty" .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.conforms(), "Expected non-conformance due to wrong datatype");
}

#[test]
fn test_validate_cardinality_zero_or_more() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:email xsd:string *
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Test with zero emails
    let data1 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result1 = validator.validate_node(&data1, &term("http://example.org/alice"), &shape_id);
    assert!(result1.is_ok());
    assert!(result1.unwrap().conforms(), "Zero emails should conform");

    // Test with multiple emails
    let data2 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:email "alice@example.com" ;
                 ex:email "alice@work.com" .
    "#);

    let result2 = validator.validate_node(&data2, &term("http://example.org/alice"), &shape_id);
    assert!(result2.is_ok());
    assert!(result2.unwrap().conforms(), "Multiple emails should conform");
}

#[test]
fn test_validate_cardinality_one_or_more() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:email xsd:string +
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);
    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));

    // Test with zero emails - should fail
    let data1 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    let result1 = validator.validate_node(&data1, &term("http://example.org/alice"), &shape_id);
    assert!(result1.is_ok());
    assert!(!result1.unwrap().conforms(), "Zero emails should not conform");

    // Test with one email - should pass
    let data2 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:email "alice@example.com" .
    "#);

    let result2 = validator.validate_node(&data2, &term("http://example.org/alice"), &shape_id);
    assert!(result2.is_ok());
    assert!(result2.unwrap().conforms(), "One email should conform");
}

#[test]
fn test_validate_cardinality_exact() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string {1,1}
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);
    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));

    // Test with exactly one name - should pass
    let data1 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    let result1 = validator.validate_node(&data1, &term("http://example.org/alice"), &shape_id);
    assert!(result1.is_ok());
    assert!(result1.unwrap().conforms(), "Exactly one name should conform");

    // Test with two names - should fail
    let data2 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" ;
                 ex:name "Alicia" .
    "#);

    let result2 = validator.validate_node(&data2, &term("http://example.org/alice"), &shape_id);
    assert!(result2.is_ok());
    assert!(!result2.unwrap().conforms(), "Two names should not conform");
}

#[test]
fn test_validate_cardinality_range() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:email xsd:string {1,3}
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);
    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));

    // Test with zero emails - should fail
    let data0 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    let result0 = validator.validate_node(&data0, &term("http://example.org/alice"), &shape_id);
    assert!(!result0.unwrap().conforms(), "Zero emails should not conform");

    // Test with two emails - should pass
    let data2 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:email "alice@example.com" ;
                 ex:email "alice@work.com" .
    "#);

    let result2 = validator.validate_node(&data2, &term("http://example.org/alice"), &shape_id);
    assert!(result2.unwrap().conforms(), "Two emails should conform");

    // Test with four emails - should fail
    let data4 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:email "a@example.com" ;
                 ex:email "b@example.com" ;
                 ex:email "c@example.com" ;
                 ex:email "d@example.com" .
    "#);

    let result4 = validator.validate_node(&data4, &term("http://example.org/alice"), &shape_id);
    assert!(!result4.unwrap().conforms(), "Four emails should not conform");
}

// =============================================================================
// Edge Cases and Error Handling
// =============================================================================

#[test]
fn test_empty_shape() {
    let shex = r#"
        PREFIX ex: <http://example.org/>

        ex:EmptyShape { }
    "#;

    let result = parse_shex(shex);
    assert!(result.is_ok(), "Empty shape should parse successfully");

    let schema = result.unwrap();
    let validator = ShexValidator::new(schema);

    // Any node should conform to an empty shape
    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" ;
                 ex:age 30 .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/EmptyShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Empty shape should accept any node");
}

#[test]
fn test_nonexistent_node() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    // Try to validate a node that doesn't exist in the data
    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/bob"), &shape_id);

    // A nonexistent node should fail validation for a shape requiring properties
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.conforms(), "Nonexistent node should not conform to shape requiring properties");
}

#[test]
fn test_nonexistent_shape() {
    let schema = ShapesSchema::new();
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    // Try to validate against a shape that doesn't exist
    let shape_id = ShapeId::new(nn("http://example.org/NonexistentShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    // Should return an error
    assert!(result.is_err(), "Validating against nonexistent shape should fail");
}

#[test]
fn test_max_recursion_depth() {
    // Create a schema with circular shape references
    let shex = r#"
        PREFIX ex: <http://example.org/>

        ex:PersonShape {
            ex:knows @ex:PersonShape *
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Create deeply nested data
    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:knows ex:bob .
        ex:bob ex:knows ex:charlie .
        ex:charlie ex:knows ex:alice .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    // Should handle cycles gracefully
    assert!(result.is_ok(), "Circular references should be handled");
}

#[test]
fn test_nested_shape_validation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:address @ex:AddressShape
        }

        ex:AddressShape {
            ex:street xsd:string ;
            ex:city xsd:string
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Valid nested data
    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" ;
                 ex:address ex:addr1 .
        ex:addr1 ex:street "123 Main St" ;
                 ex:city "Springfield" .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Valid nested data should conform");
}

#[test]
fn test_nested_shape_validation_failure() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:address @ex:AddressShape
        }

        ex:AddressShape {
            ex:street xsd:string ;
            ex:city xsd:string
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Invalid nested data (address missing city)
    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" ;
                 ex:address ex:addr1 .
        ex:addr1 ex:street "123 Main St" .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok());
    assert!(!result.unwrap().conforms(), "Invalid nested data should not conform");
}

// =============================================================================
// Validation Report Tests
// =============================================================================

#[test]
fn test_validation_report_conforms() {
    let report = ValidationReport::new_conforming();
    assert!(report.conforms());
    assert_eq!(report.results().count(), 0);
}

#[test]
fn test_validation_report_violation() {
    let mut report = ValidationReport::new();
    let result = ValidationResult::new_violation(
        term("http://example.org/alice"),
        ShapeId::new(nn("http://example.org/PersonShape")),
        "Missing required property".to_string(),
    );
    report.add_result(result);

    assert!(!report.conforms());
    assert_eq!(report.results().count(), 1);
}

// =============================================================================
// Boolean Operators Tests
// =============================================================================

#[test]
fn test_shape_or_validation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonOrOrgShape = ex:PersonShape OR ex:OrgShape

        ex:PersonShape {
            ex:name xsd:string
        }

        ex:OrgShape {
            ex:orgName xsd:string
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);
    let shape_id = ShapeId::new(nn("http://example.org/PersonOrOrgShape"));

    // Data matching PersonShape
    let data1 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    let result1 = validator.validate_node(&data1, &term("http://example.org/alice"), &shape_id);
    assert!(result1.is_ok());
    assert!(result1.unwrap().conforms(), "Should conform to PersonShape in OR");

    // Data matching OrgShape
    let data2 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:acme ex:orgName "ACME Corp" .
    "#);

    let result2 = validator.validate_node(&data2, &term("http://example.org/acme"), &shape_id);
    assert!(result2.is_ok());
    assert!(result2.unwrap().conforms(), "Should conform to OrgShape in OR");
}

#[test]
fn test_shape_and_validation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:EmployeeShape = ex:PersonShape AND ex:EmployeeInfoShape

        ex:PersonShape {
            ex:name xsd:string
        }

        ex:EmployeeInfoShape {
            ex:employeeId xsd:integer
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);
    let shape_id = ShapeId::new(nn("http://example.org/EmployeeShape"));

    // Data matching both shapes
    let data1 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" ;
                 ex:employeeId 12345 .
    "#);

    let result1 = validator.validate_node(&data1, &term("http://example.org/alice"), &shape_id);
    assert!(result1.is_ok());
    assert!(result1.unwrap().conforms(), "Should conform to both shapes in AND");

    // Data matching only PersonShape
    let data2 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:bob ex:name "Bob" .
    "#);

    let result2 = validator.validate_node(&data2, &term("http://example.org/bob"), &shape_id);
    assert!(result2.is_ok());
    assert!(!result2.unwrap().conforms(), "Should not conform if missing EmployeeInfoShape");
}

#[test]
fn test_shape_not_validation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:NotPersonShape = NOT ex:PersonShape

        ex:PersonShape {
            ex:name xsd:string
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);
    let shape_id = ShapeId::new(nn("http://example.org/NotPersonShape"));

    // Data NOT matching PersonShape - should conform to NOT
    let data1 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:org ex:orgName "ACME" .
    "#);

    let result1 = validator.validate_node(&data1, &term("http://example.org/org"), &shape_id);
    assert!(result1.is_ok());
    assert!(result1.unwrap().conforms(), "Should conform to NOT PersonShape");

    // Data matching PersonShape - should NOT conform to NOT
    let data2 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    let result2 = validator.validate_node(&data2, &term("http://example.org/alice"), &shape_id);
    assert!(result2.is_ok());
    assert!(!result2.unwrap().conforms(), "Should not conform to NOT PersonShape when matching PersonShape");
}
