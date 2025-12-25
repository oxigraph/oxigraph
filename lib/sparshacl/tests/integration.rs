//! Integration tests for SHACL validation.

use oxrdf::{Graph, NamedNode};
use oxrdfio::{RdfFormat, RdfParser};
use sparshacl::{ShaclValidator, Severity, ShapesGraph};

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

/// Helper to parse shapes from Turtle.
fn parse_shapes(turtle: &str) -> ShapesGraph {
    let graph = parse_turtle(turtle);
    ShapesGraph::from_graph(&graph).expect("Failed to parse shapes")
}

// =============================================================================
// Basic validation tests
// =============================================================================

#[test]
fn test_empty_shapes_graph() {
    let shapes = ShapesGraph::new();
    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ;
            ex:name "Alice" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms());
    assert_eq!(report.violation_count(), 0);
}

#[test]
fn test_target_class_conforming() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:PersonShape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ex:name ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ;
            ex:name "Alice" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms());
}

#[test]
fn test_target_class_violation() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:PersonShape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ex:name ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

// =============================================================================
// Target declaration tests
// =============================================================================

#[test]
fn test_target_node() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:AliceShape a sh:NodeShape ;
            sh:targetNode ex:alice ;
            sh:property [
                sh:path ex:age ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
        ex:bob ex:name "Bob" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    // Only ex:alice should be validated, not ex:bob
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_target_subjects_of() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:NamedEntityShape a sh:NodeShape ;
            sh:targetSubjectsOf ex:name ;
            sh:property [
                sh:path ex:id ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
        ex:bob ex:name "Bob" ; ex:id "123" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1); // Only ex:alice violates
}

#[test]
fn test_target_objects_of() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:AddressShape a sh:NodeShape ;
            sh:targetObjectsOf ex:address ;
            sh:property [
                sh:path ex:city ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:address ex:addr1 .
        ex:addr1 ex:street "Main St" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

// =============================================================================
// Cardinality constraint tests
// =============================================================================

#[test]
fn test_min_count_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:prop ;
                sh:minCount 2
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:prop "a" ; ex:prop "b" .
        ex:thing2 a ex:Thing ; ex:prop "c" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1); // thing2 violates
}

#[test]
fn test_max_count_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:prop ;
                sh:maxCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:prop "a" .
        ex:thing2 a ex:Thing ; ex:prop "b" ; ex:prop "c" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1); // thing2 violates
}

// =============================================================================
// Value type constraint tests
// =============================================================================

#[test]
fn test_datatype_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:count ;
                sh:datatype xsd:integer
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:count "5"^^xsd:integer .
        ex:thing2 a ex:Thing ; ex:count "not a number" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1); // thing2 violates
}

#[test]
fn test_class_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ex:friend ;
                sh:class ex:Person
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ; ex:friend ex:bob .
        ex:bob a ex:Person .
        ex:carol a ex:Person ; ex:friend ex:place1 .
        ex:place1 a ex:Place .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1); // carol's friend is not a Person
}

#[test]
fn test_node_kind_iri() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:ref ;
                sh:nodeKind sh:IRI
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:ref ex:other .
        ex:thing2 a ex:Thing ; ex:ref "not an IRI" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_node_kind_literal() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:label ;
                sh:nodeKind sh:Literal
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:label "A label" .
        ex:thing2 a ex:Thing ; ex:label ex:other .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

// =============================================================================
// String constraint tests
// =============================================================================

#[test]
fn test_min_length_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:name ;
                sh:minLength 3
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:name "Alice" .
        ex:thing2 a ex:Thing ; ex:name "AB" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_max_length_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:code ;
                sh:maxLength 5
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:code "ABC" .
        ex:thing2 a ex:Thing ; ex:code "TOOLONG" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_pattern_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:email ;
                sh:pattern "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:email "alice@example.com" .
        ex:thing2 a ex:Thing ; ex:email "not-an-email" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

// =============================================================================
// Value range constraint tests
// =============================================================================

#[test]
fn test_min_exclusive_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:age ;
                sh:minExclusive 0
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:age "25"^^xsd:integer .
        ex:thing2 a ex:Thing ; ex:age "0"^^xsd:integer .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_max_inclusive_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:score ;
                sh:maxInclusive 100
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:score "100"^^xsd:integer .
        ex:thing2 a ex:Thing ; ex:score "101"^^xsd:integer .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

// =============================================================================
// Value enumeration constraint tests
// =============================================================================

#[test]
fn test_in_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:status ;
                sh:in ("active" "inactive" "pending")
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing ; ex:status "active" .
        ex:thing2 a ex:Thing ; ex:status "invalid" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

// =============================================================================
// Logical constraint tests
// =============================================================================

#[test]
fn test_not_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:NotAdminShape a sh:NodeShape ;
            sh:targetClass ex:User ;
            sh:not [
                a sh:NodeShape ;
                sh:property [
                    sh:path ex:role ;
                    sh:hasValue "admin"
                ]
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:User ; ex:role "user" .
        ex:bob a ex:User ; ex:role "admin" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1); // bob violates
}

// =============================================================================
// Severity tests
// =============================================================================

#[test]
fn test_warning_severity() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:severity sh:Warning ;
            sh:property [
                sh:path ex:recommended ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms()); // Warnings don't affect conformance
    assert_eq!(report.warning_count(), 1);
    assert_eq!(report.violation_count(), 0);
}

#[test]
fn test_info_severity() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:severity sh:Info ;
            sh:property [
                sh:path ex:optional ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:thing1 a ex:Thing .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms());
    assert_eq!(report.info_count(), 1);
}

// =============================================================================
// Validation report tests
// =============================================================================

#[test]
fn test_validation_report_to_graph() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ex:name ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    let report_graph = report.to_graph();

    // Verify the report contains expected triples
    assert!(!report_graph.is_empty());

    // Check for ValidationReport type
    let shacl_validation_report =
        NamedNode::new("http://www.w3.org/ns/shacl#ValidationReport").unwrap();
    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").unwrap();
    let has_report = report_graph
        .iter()
        .any(|t| t.predicate == rdf_type.as_ref() && t.object == shacl_validation_report.as_ref().into());
    assert!(has_report);
}

#[test]
fn test_validation_result_properties() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ex:name ;
                sh:minCount 1 ;
                sh:message "A person must have a name"
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    let results = report.results();

    assert_eq!(results.len(), 1);
    let result = &results[0];

    // Check focus node
    assert_eq!(
        result.focus_node.to_string(),
        "<http://example.org/alice>"
    );

    // Check severity
    assert_eq!(result.result_severity, Severity::Violation);
}

// =============================================================================
// Property path tests
// =============================================================================

#[test]
fn test_inverse_path() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path [ sh:inversePath ex:parent ] ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person .
        ex:bob a ex:Person .
        ex:carol ex:parent ex:alice .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    // alice has a child (carol), but bob doesn't
    assert_eq!(report.violation_count(), 1);
}

// =============================================================================
// Edge cases and error handling
// =============================================================================

#[test]
fn test_no_matching_targets() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:NonExistent ;
            sh:property [
                sh:path ex:prop ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms()); // No violations because no targets matched
}

#[test]
fn test_multiple_shapes() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:PersonShape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ex:name ;
                sh:minCount 1
            ] .

        ex:EmployeeShape a sh:NodeShape ;
            sh:targetClass ex:Employee ;
            sh:property [
                sh:path ex:employeeId ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person .
        ex:bob a ex:Employee .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 2); // Both alice and bob violate
}

#[test]
fn test_deactivated_shape() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:deactivated true ;
            sh:property [
                sh:path ex:name ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms()); // Deactivated shape is not evaluated
}

// =============================================================================
// Complex scenario tests
// =============================================================================

#[test]
fn test_nested_property_shapes() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:PersonShape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ex:address ;
                sh:minCount 1 ;
                sh:node ex:AddressShape
            ] .

        ex:AddressShape a sh:NodeShape ;
            sh:property [
                sh:path ex:city ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ;
            ex:address ex:addr1 .
        ex:addr1 ex:street "Main St" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    // addr1 is missing city
    assert!(report.violation_count() >= 1);
}

#[test]
fn test_has_value_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Product ;
            sh:property [
                sh:path ex:status ;
                sh:hasValue "available"
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:prod1 a ex:Product ; ex:status "available" .
        ex:prod2 a ex:Product ; ex:status "sold" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1); // prod2 doesn't have "available" status
}
