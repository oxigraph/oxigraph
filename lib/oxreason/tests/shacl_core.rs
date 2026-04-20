//! Integration tests for the SHACL Core validator.
//!
//! One test per constraint component in the M4 milestone. Today every test
//! asserts `ValidateError::NotImplemented`. Each test carries a TODO block
//! showing the expected validation report once the constraint lands.
//!
//! Fixtures for each test live in `tests/fixtures/shacl_*.ttl`. They are
//! not parsed in the current scaffold; the graphs are built programmatically
//! so the scaffold stays dependency light.

#![cfg(test)]

use oxrdf::vocab::rdf;
use oxrdf::{Graph, Literal, NamedNode, Triple};
use oxreason::{ValidateError, Validator, ValidatorConfig};

fn ex(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("https://example.org/ontology#{local}"))
}

fn validate(shapes: Graph, data: &Graph) -> Result<oxreason::ValidationReport, ValidateError> {
    Validator::new(ValidatorConfig::shacl_core(), shapes).validate(data)
}

#[test]
fn shacl_min_count_violation() {
    // Fixture: tests/fixtures/shacl_mincount.ttl plus shacl_mincount_shapes.ttl
    let mut data = Graph::default();
    data.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Company")));
    data.insert(&Triple::new(ex("Bravo"), rdf::TYPE, ex("Company")));
    data.insert(&Triple::new(
        ex("Bravo"),
        ex("entityName"),
        Literal::new_simple_literal("Bravo Corp"),
    ));

    // Shapes graph will encode sh:CompanyShape requiring sh:minCount 1 on
    // :entityName. Construction is omitted here because the validator does
    // not yet read it.
    let shapes = Graph::default();

    let err = validate(shapes, &data).unwrap_err();
    assert!(matches!(err, ValidateError::NotImplemented(_)));

    // TODO M4: flip to
    //   let report = validate(shapes, &data).unwrap();
    //   assert!(!report.is_conforming());
    //   let violation = report
    //       .results()
    //       .iter()
    //       .find(|r| r.focus_node == ex("Acme").into())
    //       .expect("Acme must fail sh:minCount");
    //   assert_eq!(violation.severity, Severity::Violation);
}

#[test]
fn shacl_max_count_violation() {
    let data = Graph::default();
    let shapes = Graph::default();

    let err = validate(shapes, &data).unwrap_err();
    assert!(matches!(err, ValidateError::NotImplemented(_)));

    // TODO M4: entity with two entityName literals must fail sh:maxCount 1.
}

#[test]
fn shacl_class_constraint() {
    let data = Graph::default();
    let shapes = Graph::default();

    let err = validate(shapes, &data).unwrap_err();
    assert!(matches!(err, ValidateError::NotImplemented(_)));

    // TODO M4: subject typed as :Entity must pass sh:class :Entity,
    //          subject typed as :Other must fail.
}

#[test]
fn shacl_datatype_constraint() {
    let data = Graph::default();
    let shapes = Graph::default();

    let err = validate(shapes, &data).unwrap_err();
    assert!(matches!(err, ValidateError::NotImplemented(_)));

    // TODO M4: xsd:integer literal must fail sh:datatype xsd:string.
}

#[test]
fn shacl_pattern_constraint() {
    let data = Graph::default();
    let shapes = Graph::default();

    let err = validate(shapes, &data).unwrap_err();
    assert!(matches!(err, ValidateError::NotImplemented(_)));

    // TODO M4: value that does not match sh:pattern regex must fail.
}

#[test]
fn shacl_in_constraint() {
    let data = Graph::default();
    let shapes = Graph::default();

    let err = validate(shapes, &data).unwrap_err();
    assert!(matches!(err, ValidateError::NotImplemented(_)));

    // TODO M4: value outside the sh:in list must fail.
}

#[test]
fn shacl_empty_shapes_is_conforming() {
    let data = Graph::default();
    let shapes = Graph::default();

    let err = validate(shapes, &data).unwrap_err();
    assert!(matches!(err, ValidateError::NotImplemented(_)));

    // TODO M4: empty shapes graph must produce a conforming report. At that
    // point change the assertion to validate(shapes, &data).unwrap().is_conforming().
}
