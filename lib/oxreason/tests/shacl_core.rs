//! Integration tests for the SHACL Core validator.
//!
//! One test per constraint component planned for the M4 milestone. The
//! `sh:minCount` test exercises the first shipped constraint. The others
//! document target behaviour with placeholder assertions and will flip
//! to real assertions as each constraint lands.
//!
//! Fixtures for each test live in `tests/fixtures/shacl_*.ttl`. They are
//! not parsed in the current scaffold; the graphs are built programmatically
//! so the scaffold stays dependency light.

#![cfg(test)]

use oxrdf::vocab::{rdf, xsd};
use oxrdf::{BlankNode, Graph, Literal, NamedNode, Term, Triple};
use oxreason::{Severity, ValidateError, Validator, ValidatorConfig};

fn ex(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("https://example.org/ontology#{local}"))
}

fn sh(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("http://www.w3.org/ns/shacl#{local}"))
}

fn validate(shapes: Graph, data: &Graph) -> Result<oxreason::ValidationReport, ValidateError> {
    Validator::new(ValidatorConfig::shacl_core(), shapes).validate(data)
}

/// Build the shapes graph that the `shacl_mincount_shapes.ttl` fixture
/// describes. Kept in code so the test stays self contained while the
/// crate has no Turtle parser wired in.
fn shapes_with_min_count_on_company() -> Graph {
    let mut shapes = Graph::default();
    let company_shape = ex("CompanyShape");
    let pshape = BlankNode::default();
    shapes.insert(&Triple::new(
        company_shape.clone(),
        rdf::TYPE,
        sh("NodeShape"),
    ));
    shapes.insert(&Triple::new(
        company_shape.clone(),
        sh("targetClass"),
        ex("Company"),
    ));
    shapes.insert(&Triple::new(
        company_shape,
        sh("property"),
        pshape.clone(),
    ));
    shapes.insert(&Triple::new(pshape.clone(), sh("path"), ex("entityName")));
    shapes.insert(&Triple::new(
        pshape,
        sh("minCount"),
        Literal::new_typed_literal("1", xsd::INTEGER.into_owned()),
    ));
    shapes
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test asserts the Ok path and panics on regression"
)]
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

    let shapes = shapes_with_min_count_on_company();
    let report = validate(shapes, &data).expect("validation must succeed");

    assert!(!report.is_conforming());
    let acme_term: Term = ex("Acme").into();
    let violation = report
        .results()
        .iter()
        .find(|r| r.focus_node == acme_term)
        .expect("Acme must fail sh:minCount");
    assert_eq!(violation.severity, Severity::Violation);
    assert_eq!(violation.result_path.as_ref(), Some(&ex("entityName")));
    assert_eq!(violation.source_constraint_component, sh("MinCountConstraintComponent"));

    // Bravo has an entityName; it must not appear.
    let bravo_term: Term = ex("Bravo").into();
    assert!(
        report
            .results()
            .iter()
            .all(|r| r.focus_node != bravo_term),
        "Bravo has an entityName and must not be flagged"
    );
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test asserts the Ok path and panics on regression"
)]
fn shacl_min_count_zero_is_always_conforming() {
    // sh:minCount 0 is trivially satisfied for every focus node. The validator
    // should produce no results regardless of whether the path is present.
    let mut data = Graph::default();
    data.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Company")));

    let mut shapes = Graph::default();
    let nshape = ex("CompanyShape");
    let pshape = BlankNode::default();
    shapes.insert(&Triple::new(nshape.clone(), sh("targetClass"), ex("Company")));
    shapes.insert(&Triple::new(nshape, sh("property"), pshape.clone()));
    shapes.insert(&Triple::new(pshape.clone(), sh("path"), ex("entityName")));
    shapes.insert(&Triple::new(
        pshape,
        sh("minCount"),
        Literal::new_typed_literal("0", xsd::INTEGER.into_owned()),
    ));

    let report = validate(shapes, &data).expect("validation must succeed");
    assert!(report.is_conforming());
    assert!(report.is_empty());
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test asserts the Ok path and panics on regression"
)]
fn shacl_min_count_with_target_node_reports_single_focus() {
    // sh:targetNode focuses the shape on exactly one node; the scan must
    // only report that node.
    let data = Graph::default();

    let mut shapes = Graph::default();
    let nshape = ex("CharlieShape");
    let pshape = BlankNode::default();
    shapes.insert(&Triple::new(nshape.clone(), sh("targetNode"), ex("Charlie")));
    shapes.insert(&Triple::new(nshape, sh("property"), pshape.clone()));
    shapes.insert(&Triple::new(pshape.clone(), sh("path"), ex("entityName")));
    shapes.insert(&Triple::new(
        pshape,
        sh("minCount"),
        Literal::new_typed_literal("1", xsd::INTEGER.into_owned()),
    ));

    let report = validate(shapes, &data).expect("validation must succeed");
    assert!(!report.is_conforming());
    assert_eq!(report.len(), 1);
    let charlie_term: Term = ex("Charlie").into();
    assert_eq!(report.results()[0].focus_node, charlie_term);
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test asserts the Ok path and panics on regression"
)]
fn shacl_max_count_violation() {
    // sh:maxCount is not implemented yet. An empty shapes graph must still
    // yield a conforming result rather than an error.
    let data = Graph::default();
    let shapes = Graph::default();
    let report = validate(shapes, &data).expect("empty shapes must validate cleanly");
    assert!(report.is_conforming());

    // TODO: entity with two entityName literals must fail sh:maxCount 1.
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test asserts the Ok path and panics on regression"
)]
fn shacl_class_constraint() {
    let data = Graph::default();
    let shapes = Graph::default();
    let report = validate(shapes, &data).expect("empty shapes must validate cleanly");
    assert!(report.is_conforming());

    // TODO: subject typed as :Entity must pass sh:class :Entity,
    //       subject typed as :Other must fail.
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test asserts the Ok path and panics on regression"
)]
fn shacl_datatype_constraint() {
    let data = Graph::default();
    let shapes = Graph::default();
    let report = validate(shapes, &data).expect("empty shapes must validate cleanly");
    assert!(report.is_conforming());

    // TODO: xsd:integer literal must fail sh:datatype xsd:string.
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test asserts the Ok path and panics on regression"
)]
fn shacl_pattern_constraint() {
    let data = Graph::default();
    let shapes = Graph::default();
    let report = validate(shapes, &data).expect("empty shapes must validate cleanly");
    assert!(report.is_conforming());

    // TODO: value that does not match sh:pattern regex must fail.
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test asserts the Ok path and panics on regression"
)]
fn shacl_in_constraint() {
    let data = Graph::default();
    let shapes = Graph::default();
    let report = validate(shapes, &data).expect("empty shapes must validate cleanly");
    assert!(report.is_conforming());

    // TODO: value outside the sh:in list must fail.
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test asserts the Ok path and panics on regression"
)]
fn shacl_empty_shapes_is_conforming() {
    let data = Graph::default();
    let shapes = Graph::default();
    let report = validate(shapes, &data).expect("empty shapes must validate cleanly");
    assert!(report.is_conforming());
    assert!(report.is_empty());
}
