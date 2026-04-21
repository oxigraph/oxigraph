//! Integration tests for the optional `spatial_index` module. Gated on
//! the `spatial_index` cargo feature. Each test builds a small graph
//! with `geo:hasGeometry` / `geo:asWKT` chains and asserts that the
//! index answers topology queries correctly without enumerating every
//! stored geometry.

#![cfg(feature = "spatial_index")]

use geo::{Geometry, Polygon};
use oxrdf::{Graph, Literal, NamedNode, NamedNodeRef, Term, Triple};
use spargeo::index::SpatialIndex;
use spargeo::vocab;
use wkt::TryFromWkt;

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

fn polygon(wkt: &str) -> Geometry {
    let p: Polygon<f64> = Polygon::try_from_wkt_str(wkt).expect("valid polygon wkt");
    Geometry::Polygon(p)
}

#[test]
fn query_within_returns_only_nested_feature() {
    let mut graph = Graph::new();
    add_feature(
        &mut graph,
        "http://example.org/inside",
        "http://example.org/inside/geom",
        "POLYGON((2 2, 3 2, 3 3, 2 3, 2 2))",
    );
    add_feature(
        &mut graph,
        "http://example.org/outside",
        "http://example.org/outside/geom",
        "POLYGON((50 50, 51 50, 51 51, 50 51, 50 50))",
    );

    let index = SpatialIndex::from_graph(&graph);
    assert_eq!(index.len(), 2);

    let query = polygon("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))");
    let hits = index.query_within(&query);
    assert_eq!(hits, vec!["http://example.org/inside".to_owned()]);
}

#[test]
fn query_intersects_returns_overlapping_features() {
    let mut graph = Graph::new();
    add_feature(
        &mut graph,
        "http://example.org/overlap",
        "http://example.org/overlap/geom",
        "POLYGON((8 8, 12 8, 12 12, 8 12, 8 8))",
    );
    add_feature(
        &mut graph,
        "http://example.org/far",
        "http://example.org/far/geom",
        "POLYGON((50 50, 51 50, 51 51, 50 51, 50 50))",
    );

    let index = SpatialIndex::from_graph(&graph);
    let query = polygon("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))");
    let hits = index.query_intersects(&query);
    assert_eq!(hits, vec!["http://example.org/overlap".to_owned()]);
}

#[test]
fn query_intersects_skips_disjoint_features() {
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

    let index = SpatialIndex::from_graph(&graph);
    let query = polygon("POLYGON((0 0, 2 0, 2 2, 0 2, 0 0))");
    let hits = index.query_intersects(&query);
    assert_eq!(hits, vec!["http://example.org/a".to_owned()]);
}

#[test]
fn index_skips_features_without_wkt() {
    let mut graph = Graph::new();
    let orphan = feature("http://example.org/orphan");
    let geom = feature("http://example.org/orphan/geom");
    graph.insert(&Triple::new(
        orphan,
        vocab::HAS_GEOMETRY.into_owned(),
        geom,
    ));

    let index = SpatialIndex::from_graph(&graph);
    assert!(index.is_empty());
    let query = polygon("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
    assert!(index.query_within(&query).is_empty());
    assert!(index.query_intersects(&query).is_empty());
}

#[test]
fn insert_accepts_point_and_answers_containment() {
    let mut index = SpatialIndex::new();
    let point = geo::Point::new(5.0_f64, 5.0_f64);
    index.insert(
        "http://example.org/p".to_owned(),
        Geometry::Point(point),
    );
    let query = polygon("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))");
    assert_eq!(
        index.query_within(&query),
        vec!["http://example.org/p".to_owned()]
    );
    let far = polygon("POLYGON((20 20, 30 20, 30 30, 20 30, 20 20))");
    assert!(index.query_within(&far).is_empty());
}
