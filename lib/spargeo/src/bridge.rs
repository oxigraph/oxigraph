//! Bridge between a mutable `oxrdf::Graph` and the spargeo geometry
//! primitives.
//!
//! The bridge is intentionally small. It walks
//! `feature --geo:hasGeometry--> geometry --geo:asWKT--> literal` chains,
//! extracts the geometry via the shared parse helpers, runs pairwise
//! `geo::Relate` between each pair of features, and emits `geo:sfWithin`,
//! `geo:sfContains`, `geo:sfEquals`, `geo:sfTouches`, `geo:sfOverlaps`,
//! `geo:sfCrosses`, `geo:sfIntersects`, and `geo:sfDisjoint` triples for the
//! pairs where the relation holds.
//!
//! This module is gated behind the `bridge` cargo feature so that minimal
//! builds which only want the extension function list stay free of graph
//! manipulation machinery.

use spareval::geosparql::parse::extract_argument;
use crate::vocab;
use geo::{Geometry, Relate};
use oxrdf::{
    BlankNode, Graph, NamedNode, NamedNodeRef, NamedOrBlankNode, NamedOrBlankNodeRef, Term,
    TermRef, Triple,
};
use std::collections::BTreeMap;

/// Bridge between an `oxrdf::Graph` and the spargeo topology primitives.
///
/// Build one with [`GeoBridge::from_graph`] and call
/// [`GeoBridge::materialize_relations`] to emit every Simple Features
/// topology triple that holds between the extracted features.
#[derive(Default)]
pub struct GeoBridge {
    features: BTreeMap<String, Geometry>,
}

impl GeoBridge {
    /// Create an empty bridge.
    pub fn new() -> Self {
        Self::default()
    }

    /// Extract every feature geometry from a graph.
    ///
    /// Follows `?feature geo:hasGeometry ?geom . ?geom geo:asWKT ?wkt` and
    /// keeps one geometry per feature. Features without a usable WKT
    /// serialisation are silently skipped.
    pub fn from_graph(graph: &Graph) -> Self {
        let mut this = Self::new();
        this.ingest(graph);
        this
    }

    fn ingest(&mut self, graph: &Graph) {
        let pairs: Vec<(String, NamedOrBlankNode)> = graph
            .triples_for_predicate(vocab::HAS_GEOMETRY)
            .filter_map(|triple| {
                let feature = feature_key_from_subject(triple.subject);
                let geom = match triple.object {
                    TermRef::NamedNode(n) => NamedOrBlankNode::NamedNode(n.into_owned()),
                    TermRef::BlankNode(b) => NamedOrBlankNode::BlankNode(b.into_owned()),
                    TermRef::Literal(_) => return None,
                };
                Some((feature, geom))
            })
            .collect();
        for (feature, geom_subject) in pairs {
            let wkt_object = graph
                .object_for_subject_predicate(geom_subject.as_ref(), vocab::AS_WKT)
                .map(TermRef::into_owned);
            let Some(wkt_object) = wkt_object else {
                continue;
            };
            let Some(geom) = extract_argument(&wkt_object) else {
                continue;
            };
            self.features.insert(feature, geom);
        }
    }

    /// Walk every pair of features and emit matching topology triples into
    /// `graph`.
    ///
    /// The bridge emits the Simple Features predicate set only. Predicates
    /// are oriented where the SF relation is asymmetric (within, contains)
    /// and duplicated in both directions for inverse pairs so that consumers
    /// that do not run a reasoner still see both orientations. Symmetric
    /// predicates are emitted in one direction only.
    pub fn materialize_relations(&self, graph: &mut Graph) {
        let feature_iris: Vec<&str> = self.features.keys().map(String::as_str).collect();
        for (i, left_key) in feature_iris.iter().enumerate() {
            let Some(left_geom) = self.features.get(*left_key) else {
                continue;
            };
            for right_key in &feature_iris[i + 1..] {
                let Some(right_geom) = self.features.get(*right_key) else {
                    continue;
                };
                let matrix = left_geom.relate(right_geom);
                let left = feature_subject(left_key);
                let right = feature_subject(right_key);
                if matrix.is_equal_topo() {
                    push(graph, &left, vocab::SF_EQUALS, &right);
                }
                if matrix.is_within() {
                    push(graph, &left, vocab::SF_WITHIN, &right);
                    push(graph, &right, vocab::SF_CONTAINS, &left);
                }
                if matrix.is_contains() {
                    push(graph, &left, vocab::SF_CONTAINS, &right);
                    push(graph, &right, vocab::SF_WITHIN, &left);
                }
                if matrix.is_touches() {
                    push(graph, &left, vocab::SF_TOUCHES, &right);
                }
                if matrix.is_overlaps() {
                    push(graph, &left, vocab::SF_OVERLAPS, &right);
                }
                if matrix.is_crosses() {
                    push(graph, &left, vocab::SF_CROSSES, &right);
                }
                if matrix.is_intersects() {
                    push(graph, &left, vocab::SF_INTERSECTS, &right);
                } else {
                    push(graph, &left, vocab::SF_DISJOINT, &right);
                }
            }
        }
    }
}

fn feature_key_from_subject(subject: NamedOrBlankNodeRef<'_>) -> String {
    match subject {
        NamedOrBlankNodeRef::NamedNode(n) => n.as_str().to_owned(),
        NamedOrBlankNodeRef::BlankNode(b) => format!("_:{}", b.as_str()),
    }
}

fn feature_subject(key: &str) -> NamedOrBlankNode {
    if let Some(inner) = key.strip_prefix("_:") {
        NamedOrBlankNode::BlankNode(BlankNode::new_unchecked(inner))
    } else {
        NamedOrBlankNode::NamedNode(NamedNode::new_unchecked(key))
    }
}

fn push(graph: &mut Graph, s: &NamedOrBlankNode, p: NamedNodeRef<'_>, o: &NamedOrBlankNode) {
    let object_term: Term = match o {
        NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
        NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
    };
    graph.insert(&Triple::new(s.clone(), p.into_owned(), object_term));
}
