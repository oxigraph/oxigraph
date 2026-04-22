//! Integration tests for the optional `bridge` module. Gated on the
//! `bridge` cargo feature. Each test builds a small graph with
//! `geo:hasGeometry` / `geo:asWKT` chains and asserts that the bridge
//! materialises the expected Simple Features topology predicates.

#![cfg(feature = "bridge")]

use oxrdf::{Graph, Literal, NamedNode, NamedNodeRef, Term, Triple};
use spargeo::bridge::GeoBridge;
use spargeo::vocab;

const WKT_LITERAL: NamedNodeRef<'_> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#wktLiteral");

fn feature(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

fn add_feature(graph: &mut Graph, feature_iri: &str, geometry_iri: &str, wkt: &str) {
    let feat = feature(feature_iri);
    let geom = feature(geometry_iri);
    graph.insert(&Triple::new(
        feat.clone(),
        vocab::HAS_GEOMETRY.into_owned(),
        geom.clone(),
    ));
    graph.insert(&Triple::new(
        geom,
        vocab::AS_WKT.into_owned(),
        Term::Literal(Literal::new_typed_literal(wkt, WKT_LITERAL)),
    ));
}

fn contains_triple(
    graph: &Graph,
    subject_iri: &str,
    predicate: NamedNodeRef<'_>,
    object_iri: &str,
) -> bool {
    let subject = feature(subject_iri);
    let object = feature(object_iri);
    graph.contains(&Triple::new(subject, predicate.into_owned(), object))
}

#[test]
fn bridge_emits_within_and_contains_for_nested_squares() {
    let mut graph = Graph::new();
    add_feature(
        &mut graph,
        "http://example.org/big",
        "http://example.org/big/geom",
        "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))",
    );
    add_feature(
        &mut graph,
        "http://example.org/small",
        "http://example.org/small/geom",
        "POLYGON((2 2, 3 2, 3 3, 2 3, 2 2))",
    );

    let bridge = GeoBridge::from_graph(&graph);
    let mut out = graph.clone();
    bridge.materialize_relations(&mut out);

    assert!(contains_triple(
        &out,
        "http://example.org/small",
        vocab::SF_WITHIN,
        "http://example.org/big",
    ));
    assert!(contains_triple(
        &out,
        "http://example.org/big",
        vocab::SF_CONTAINS,
        "http://example.org/small",
    ));
    // sfIntersects is symmetric; the bridge emits it once, left then right in
    // iteration order. Features are keyed alphabetically so "big" is the left.
    assert!(contains_triple(
        &out,
        "http://example.org/big",
        vocab::SF_INTERSECTS,
        "http://example.org/small",
    ));
}

#[test]
fn bridge_emits_disjoint_for_far_apart_features() {
    let mut graph = Graph::new();
    add_feature(
        &mut graph,
        "http://example.org/a",
        "http://example.org/a/geom",
        "POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))",
    );
    add_feature(
        &mut graph,
        "http://example.org/b",
        "http://example.org/b/geom",
        "POLYGON((50 50, 51 50, 51 51, 50 51, 50 50))",
    );

    let bridge = GeoBridge::from_graph(&graph);
    let mut out = graph.clone();
    bridge.materialize_relations(&mut out);

    assert!(contains_triple(
        &out,
        "http://example.org/a",
        vocab::SF_DISJOINT,
        "http://example.org/b",
    ));
    assert!(!contains_triple(
        &out,
        "http://example.org/a",
        vocab::SF_INTERSECTS,
        "http://example.org/b",
    ));
    assert!(!contains_triple(
        &out,
        "http://example.org/b",
        vocab::SF_INTERSECTS,
        "http://example.org/a",
    ));
}

#[test]
fn bridge_emits_equals_for_identical_polygons() {
    let mut graph = Graph::new();
    add_feature(
        &mut graph,
        "http://example.org/twin_a",
        "http://example.org/twin_a/geom",
        "POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))",
    );
    add_feature(
        &mut graph,
        "http://example.org/twin_b",
        "http://example.org/twin_b/geom",
        "POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))",
    );

    let bridge = GeoBridge::from_graph(&graph);
    let mut out = graph.clone();
    bridge.materialize_relations(&mut out);

    assert!(contains_triple(
        &out,
        "http://example.org/twin_a",
        vocab::SF_EQUALS,
        "http://example.org/twin_b",
    ));
}

#[test]
fn bridge_skips_features_without_wkt() {
    let mut graph = Graph::new();
    let orphan = feature("http://example.org/orphan");
    let geom = feature("http://example.org/orphan/geom");
    graph.insert(&Triple::new(
        orphan,
        vocab::HAS_GEOMETRY.into_owned(),
        geom,
    ));

    let bridge = GeoBridge::from_graph(&graph);
    let mut out = graph.clone();
    bridge.materialize_relations(&mut out);

    // No relations possible from a single feature, regardless.
    let predicates = [
        vocab::SF_WITHIN,
        vocab::SF_CONTAINS,
        vocab::SF_EQUALS,
        vocab::SF_INTERSECTS,
        vocab::SF_DISJOINT,
    ];
    for p in predicates {
        let count = out
            .iter()
            .filter(|t| t.predicate == p)
            .count();
        assert_eq!(count, 0, "expected no {} triples", p.as_str());
    }
}
