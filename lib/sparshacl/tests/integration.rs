//! Integration tests for SHACL validation.

use oxrdf::{Graph, Literal, NamedNode, Triple};
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

#[test]
fn test_sequence_path() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ( ex:address ex:city ) ;
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
        ex:addr1 ex:city "New York" .
        ex:bob a ex:Person ;
            ex:address ex:addr2 .
        ex:addr2 ex:street "Main St" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    // bob's address has no city
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_sequence_path_multiple_hops() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ( ex:company ex:address ex:country ) ;
                sh:hasValue "USA"
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ;
            ex:company ex:corp1 .
        ex:corp1 ex:address ex:corpAddr .
        ex:corpAddr ex:country "USA" .
        ex:bob a ex:Person ;
            ex:company ex:corp2 .
        ex:corp2 ex:address ex:corpAddr2 .
        ex:corpAddr2 ex:country "Canada" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    // bob's company is in Canada, not USA
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_alternative_path() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path [ sh:alternativePath ( ex:email ex:phone ) ] ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ;
            ex:email "alice@example.com" .
        ex:bob a ex:Person ;
            ex:phone "555-1234" .
        ex:carol a ex:Person ;
            ex:name "Carol" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    // carol has neither email nor phone
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_alternative_path_multiple_values() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Product ;
            sh:property [
                sh:path [ sh:alternativePath ( ex:label ex:name ex:title ) ] ;
                sh:minLength 3
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:prod1 a ex:Product ;
            ex:label "Widget" .
        ex:prod2 a ex:Product ;
            ex:name "Gadget" .
        ex:prod3 a ex:Product ;
            ex:title "AB" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    // prod3's title is too short
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_zero_or_more_path() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path [ sh:zeroOrMorePath ex:parent ] ;
                sh:maxCount 5
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ;
            ex:parent ex:bob .
        ex:bob ex:parent ex:carol .
        ex:carol ex:parent ex:dave .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms());
    // alice has ancestors: bob, carol, dave (and alice itself via zero-or-more)
}

#[test]
fn test_one_or_more_path() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Employee ;
            sh:property [
                sh:path [ sh:oneOrMorePath ex:manager ] ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Employee ;
            ex:manager ex:bob .
        ex:bob ex:manager ex:ceo .
        ex:carol a ex:Employee .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    // carol has no manager (one-or-more requires at least one)
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_zero_or_one_path() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetNode ex:doc1 ;
            sh:property [
                sh:path [ sh:zeroOrOnePath ex:author ] ;
                sh:minCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:doc1 ex:title "Document" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    // Zero-or-one path includes the node itself (zero hops)
    // so minCount is satisfied even without ex:author
    assert!(report.conforms());
}

#[test]
fn test_combined_sequence_and_alternative_path() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ( ex:address [ sh:alternativePath ( ex:city ex:town ) ] ) ;
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
        ex:addr1 ex:city "New York" .
        ex:bob a ex:Person ;
            ex:address ex:addr2 .
        ex:addr2 ex:town "Springfield" .
        ex:carol a ex:Person ;
            ex:address ex:addr3 .
        ex:addr3 ex:street "Main St" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    // carol's address has neither city nor town
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

// =============================================================================
// Additional logical constraint tests
// =============================================================================

#[test]
fn test_and_constraint_conforming() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .

        ex:AdultShape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:and (
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:age ;
                        sh:minInclusive 18
                    ]
                ]
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:age ;
                        sh:datatype xsd:integer
                    ]
                ]
            ) .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ; ex:age "25"^^xsd:integer .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms());
    assert_eq!(report.violation_count(), 0);
}

#[test]
fn test_and_constraint_violation() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .

        ex:AdultShape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:and (
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:age ;
                        sh:minInclusive 18
                    ]
                ]
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:age ;
                        sh:datatype xsd:integer
                    ]
                ]
            ) .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ; ex:age "25"^^xsd:integer .
        ex:bob a ex:Person ; ex:age "16"^^xsd:integer .
        ex:carol a ex:Person ; ex:age "not a number" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 2);
}

#[test]
fn test_or_constraint_conforming() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:ContactShape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:or (
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:email ;
                        sh:minCount 1
                    ]
                ]
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:phone ;
                        sh:minCount 1
                    ]
                ]
            ) .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ; ex:email "alice@example.com" .
        ex:bob a ex:Person ; ex:phone "123-456-7890" .
        ex:carol a ex:Person ; ex:email "carol@example.com" ; ex:phone "098-765-4321" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms());
    assert_eq!(report.violation_count(), 0);
}

#[test]
fn test_or_constraint_violation() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:ContactShape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:or (
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:email ;
                        sh:minCount 1
                    ]
                ]
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:phone ;
                        sh:minCount 1
                    ]
                ]
            ) .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ; ex:email "alice@example.com" .
        ex:bob a ex:Person ; ex:name "Bob" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_xone_constraint_conforming() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:IdentifierShape a sh:NodeShape ;
            sh:targetClass ex:User ;
            sh:xone (
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:ssn ;
                        sh:minCount 1
                    ]
                ]
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:passport ;
                        sh:minCount 1
                    ]
                ]
            ) .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:User ; ex:ssn "123-45-6789" .
        ex:bob a ex:User ; ex:passport "P12345678" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms());
    assert_eq!(report.violation_count(), 0);
}

#[test]
fn test_xone_constraint_violation_none() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:IdentifierShape a sh:NodeShape ;
            sh:targetClass ex:User ;
            sh:xone (
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:ssn ;
                        sh:minCount 1
                    ]
                ]
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:passport ;
                        sh:minCount 1
                    ]
                ]
            ) .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:User ; ex:name "Alice" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_xone_constraint_violation_multiple() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:IdentifierShape a sh:NodeShape ;
            sh:targetClass ex:User ;
            sh:xone (
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:ssn ;
                        sh:minCount 1
                    ]
                ]
                [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:passport ;
                        sh:minCount 1
                    ]
                ]
            ) .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:User ; ex:ssn "123-45-6789" ; ex:passport "P12345678" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_qualified_value_shape_conforming() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:TeamShape a sh:NodeShape ;
            sh:targetClass ex:Team ;
            sh:property [
                sh:path ex:member ;
                sh:qualifiedValueShape [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:role ;
                        sh:hasValue "leader"
                    ]
                ] ;
                sh:qualifiedMinCount 1 ;
                sh:qualifiedMaxCount 2
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:team1 a ex:Team ;
            ex:member ex:alice ;
            ex:member ex:bob ;
            ex:member ex:carol .
        ex:alice ex:role "leader" .
        ex:bob ex:role "member" .
        ex:carol ex:role "member" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms());
    assert_eq!(report.violation_count(), 0);
}

#[test]
fn test_qualified_value_shape_violation_min() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:TeamShape a sh:NodeShape ;
            sh:targetClass ex:Team ;
            sh:property [
                sh:path ex:member ;
                sh:qualifiedValueShape [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:role ;
                        sh:hasValue "leader"
                    ]
                ] ;
                sh:qualifiedMinCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:team1 a ex:Team ;
            ex:member ex:alice ;
            ex:member ex:bob .
        ex:alice ex:role "member" .
        ex:bob ex:role "member" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_qualified_value_shape_violation_max() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:TeamShape a sh:NodeShape ;
            sh:targetClass ex:Team ;
            sh:property [
                sh:path ex:member ;
                sh:qualifiedValueShape [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:role ;
                        sh:hasValue "leader"
                    ]
                ] ;
                sh:qualifiedMaxCount 2
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:team1 a ex:Team ;
            ex:member ex:alice ;
            ex:member ex:bob ;
            ex:member ex:carol .
        ex:alice ex:role "leader" .
        ex:bob ex:role "leader" .
        ex:carol ex:role "leader" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(!report.conforms());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn test_qualified_value_shape_with_additional_constraint() {
    let shapes = parse_shapes(
        r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:PersonShape a sh:NodeShape ;
            sh:targetClass ex:Person ;
            sh:property [
                sh:path ex:contact ;
                sh:qualifiedValueShape [
                    a sh:NodeShape ;
                    sh:property [
                        sh:path ex:type ;
                        sh:hasValue "primary"
                    ]
                ] ;
                sh:qualifiedMinCount 1 ;
                sh:qualifiedMaxCount 1
            ] .
    "#,
    );

    let validator = ShaclValidator::new(shapes);

    let data = parse_turtle(
        r#"
        @prefix ex: <http://example.org/> .
        ex:alice a ex:Person ;
            ex:contact ex:contact1 ;
            ex:contact ex:contact2 .
        ex:contact1 ex:type "primary" ; ex:value "555-0001" .
        ex:contact2 ex:type "secondary" ; ex:value "555-0002" .
    "#,
    );

    let report = validator.validate(&data).expect("Validation failed");
    assert!(report.conforms());
    assert_eq!(report.violation_count(), 0);
}

// =============================================================================
// Security and validation error tests
// =============================================================================

#[test]
fn test_circular_rdf_list_in_sh_in() {
    // Create a shape with a circular RDF list in sh:in constraint
    let turtle = r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .
        @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:status ;
                sh:in ex:circularList
            ] .

        # Create a circular list: node1 -> node2 -> node1
        ex:circularList rdf:first "value1" ;
            rdf:rest ex:node2 .
        ex:node2 rdf:first "value2" ;
            rdf:rest ex:circularList .
    "#;

    let graph = parse_turtle(turtle);
    let result = ShapesGraph::from_graph(&graph);

    // Should fail with CircularList error
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        matches!(error, sparshacl::ShaclParseError::CircularList { .. }),
        "Expected CircularList error, got: {:?}",
        error
    );
}


#[test]
fn test_list_too_long_in_sh_in() {
    // Create a shape with an extremely long RDF list (> 10000 elements)
    // This would be impractical to create inline, so we'll construct it programmatically
    let mut graph = Graph::new();

    // Add the shape definition
    let ex_shape = NamedNode::new("http://example.org/Shape").unwrap();
    let ex_thing = NamedNode::new("http://example.org/Thing").unwrap();
    let ex_status = NamedNode::new("http://example.org/status").unwrap();
    let ex_list_head = NamedNode::new("http://example.org/listHead").unwrap();

    let sh_node_shape = NamedNode::new("http://www.w3.org/ns/shacl#NodeShape").unwrap();
    let sh_target_class = NamedNode::new("http://www.w3.org/ns/shacl#targetClass").unwrap();
    let sh_property = NamedNode::new("http://www.w3.org/ns/shacl#property").unwrap();
    let sh_path = NamedNode::new("http://www.w3.org/ns/shacl#path").unwrap();
    let sh_in = NamedNode::new("http://www.w3.org/ns/shacl#in").unwrap();
    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").unwrap();
    let rdf_first = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#first").unwrap();
    let rdf_rest = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest").unwrap();
    let rdf_nil = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil").unwrap();

    // Add shape triples
    graph.insert(&Triple::new(ex_shape.clone(), rdf_type.clone(), sh_node_shape.clone()));
    graph.insert(&Triple::new(ex_shape.clone(), sh_target_class.clone(), ex_thing.clone()));

    let prop_node = NamedNode::new("http://example.org/prop1").unwrap();
    graph.insert(&Triple::new(ex_shape.clone(), sh_property.clone(), prop_node.clone()));
    graph.insert(&Triple::new(prop_node.clone(), sh_path.clone(), ex_status.clone()));
    graph.insert(&Triple::new(prop_node.clone(), sh_in.clone(), ex_list_head.clone()));

    // Create a very long list (10001 elements, which exceeds MAX_LIST_LENGTH of 10000)
    let mut current = ex_list_head;
    for i in 0..10001 {
        let value = Literal::new_simple_literal(format!("value{}", i));
        graph.insert(&Triple::new(current.clone(), rdf_first.clone(), value.clone()));

        if i < 10000 {
            let next = NamedNode::new(format!("http://example.org/node{}", i + 1)).unwrap();
            graph.insert(&Triple::new(current.clone(), rdf_rest.clone(), next.clone()));
            current = next;
        } else {
            graph.insert(&Triple::new(current.clone(), rdf_rest.clone(), rdf_nil.clone()));
        }
    }

    let result = ShapesGraph::from_graph(&graph);

    // Should fail with ListTooLong error
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        matches!(error, sparshacl::ShaclParseError::ListTooLong { .. }),
        "Expected ListTooLong error, got: {:?}",
        error
    );
}

#[test]
fn test_negative_min_count_rejected() {
    let turtle = r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:prop ;
                sh:minCount -1
            ] .
    "#;

    let graph = parse_turtle(turtle);
    let result = ShapesGraph::from_graph(&graph);

    // Should fail with InvalidPropertyValue error
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        matches!(error, sparshacl::ShaclParseError::InvalidPropertyValue { .. }),
        "Expected InvalidPropertyValue error, got: {:?}",
        error
    );
}

#[test]
fn test_negative_max_count_rejected() {
    let turtle = r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:prop ;
                sh:maxCount -5
            ] .
    "#;

    let graph = parse_turtle(turtle);
    let result = ShapesGraph::from_graph(&graph);

    // Should fail with InvalidPropertyValue error
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        matches!(error, sparshacl::ShaclParseError::InvalidPropertyValue { .. }),
        "Expected InvalidPropertyValue error, got: {:?}",
        error
    );
}

#[test]
fn test_negative_min_length_rejected() {
    let turtle = r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:name ;
                sh:minLength -10
            ] .
    "#;

    let graph = parse_turtle(turtle);
    let result = ShapesGraph::from_graph(&graph);

    // Should fail with InvalidPropertyValue error
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        matches!(error, sparshacl::ShaclParseError::InvalidPropertyValue { .. }),
        "Expected InvalidPropertyValue error, got: {:?}",
        error
    );
}

#[test]
fn test_negative_max_length_rejected() {
    let turtle = r#"
        @prefix sh: <http://www.w3.org/ns/shacl#> .
        @prefix ex: <http://example.org/> .

        ex:Shape a sh:NodeShape ;
            sh:targetClass ex:Thing ;
            sh:property [
                sh:path ex:name ;
                sh:maxLength -100
            ] .
    "#;

    let graph = parse_turtle(turtle);
    let result = ShapesGraph::from_graph(&graph);

    // Should fail with InvalidPropertyValue error
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        matches!(error, sparshacl::ShaclParseError::InvalidPropertyValue { .. }),
        "Expected InvalidPropertyValue error, got: {:?}",
        error
    );
}
