//! In-memory S2 spatial index for GeoSPARQL features.
//!
//! Indexes `geo:hasGeometry -> geo:asWKT` feature geometries from an
//! `oxrdf::Graph` and answers Simple Features topology queries without
//! touching every stored geometry.
//!
//! The index covers each feature's bounding rectangle with an S2
//! [`CellUnion`](s2::cellunion::CellUnion) and keeps a sorted
//! `CellID -> FeatureId` map. A query geometry's cover is used to
//! gather candidate feature IDs through ancestor lookups and descendant
//! range scans; [`geo::Relate`] then runs on the (much smaller)
//! candidate set to produce the final answer.
//!
//! This module is gated behind the `spatial_index` cargo feature so
//! minimal builds stay free of the S2 dependency.

use spareval::geosparql::parse::extract_argument;
use crate::vocab;
use geo::{BoundingRect, Geometry, Relate};
use oxrdf::{Graph, NamedOrBlankNode, NamedOrBlankNodeRef, TermRef};
use s2::cellid::CellID;
use s2::latlng::LatLng;
use s2::rect::Rect as S2Rect;
use s2::region::RegionCoverer;
use std::collections::{BTreeMap, HashSet};

/// Default coverer cap on cell count per geometry.
const DEFAULT_MAX_CELLS: usize = 8;
/// Default deepest S2 level the coverer is allowed to use.
const DEFAULT_MAX_LEVEL: u8 = 30;

/// In-memory S2 spatial index for GeoSPARQL features.
///
/// Build one with [`SpatialIndex::from_graph`] or start empty with
/// [`SpatialIndex::new`] and add geometries via [`SpatialIndex::insert`].
/// Once populated, call [`SpatialIndex::query_within`] or
/// [`SpatialIndex::query_intersects`] to find features whose geometry
/// satisfies the corresponding Simple Features relation with the query
/// geometry.
///
/// Feature keys are the IRI strings of `geo:Feature` instances for named
/// nodes and `_:label` for blank nodes. They round trip through
/// [`SpatialIndex::query_within`] and related methods so callers can
/// look up the feature in the source graph without any extra bookkeeping.
pub struct SpatialIndex {
    features: Vec<(String, Geometry)>,
    cells: BTreeMap<CellID, Vec<u32>>,
    coverer: RegionCoverer,
}

impl Default for SpatialIndex {
    fn default() -> Self {
        Self {
            features: Vec::new(),
            cells: BTreeMap::new(),
            coverer: RegionCoverer {
                min_level: 0,
                max_level: DEFAULT_MAX_LEVEL,
                level_mod: 1,
                max_cells: DEFAULT_MAX_CELLS,
            },
        }
    }
}

impl SpatialIndex {
    /// Create an empty index with the default coverer settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Populate an index from every `geo:hasGeometry -> geo:asWKT` chain
    /// reachable in `graph`.
    ///
    /// Features without a usable WKT literal or whose geometry is empty
    /// are silently skipped, matching the behaviour of
    /// [`crate::bridge::GeoBridge::from_graph`].
    pub fn from_graph(graph: &Graph) -> Self {
        let mut this = Self::new();
        this.ingest(graph);
        this
    }

    /// Number of indexed features.
    pub fn len(&self) -> usize {
        self.features.len()
    }

    /// Whether the index holds no features.
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    /// Add a single feature to the index.
    ///
    /// Features with no bounding rectangle (empty geometries) are
    /// ignored to keep the internal invariants of the cell map simple.
    pub fn insert(&mut self, feature_key: String, geom: Geometry) {
        let cover = self.cover_geometry(&geom);
        if cover.is_empty() {
            return;
        }
        let feature_id: u32 = self
            .features
            .len()
            .try_into()
            .expect("spatial index does not support more than u32::MAX features");
        self.features.push((feature_key, geom));
        for cell in cover {
            self.cells.entry(cell).or_default().push(feature_id);
        }
    }

    /// Return the feature keys whose geometry is spatially within
    /// `query`.
    ///
    /// The order of the returned vector is stable: keys are sorted in
    /// byte order so that tests and downstream consumers see
    /// deterministic results regardless of insertion order.
    pub fn query_within(&self, query: &Geometry) -> Vec<String> {
        self.query(query, QueryMode::Within)
    }

    /// Return the feature keys whose geometry intersects `query`.
    pub fn query_intersects(&self, query: &Geometry) -> Vec<String> {
        self.query(query, QueryMode::Intersects)
    }

    fn ingest(&mut self, graph: &Graph) {
        let pairs: Vec<(String, NamedOrBlankNode)> = graph
            .triples_for_predicate(vocab::HAS_GEOMETRY)
            .filter_map(|triple| {
                let feature = feature_key_from_subject(triple.subject);
                let geom = match triple.object {
                    TermRef::NamedNode(n) => NamedOrBlankNode::NamedNode(n.into_owned()),
                    TermRef::BlankNode(b) => NamedOrBlankNode::BlankNode(b.into_owned()),
                    _ => return None,
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
            self.insert(feature, geom);
        }
    }

    fn cover_geometry(&self, geom: &Geometry) -> Vec<CellID> {
        let Some(bbox) = geom.bounding_rect() else {
            return Vec::new();
        };
        let min = bbox.min();
        let max = bbox.max();
        let s2_rect = S2Rect::from_point_pair(
            &LatLng::from_degrees(min.y, min.x),
            &LatLng::from_degrees(max.y, max.x),
        );
        let cu = self.coverer.covering(&s2_rect);
        cu.0
    }

    fn query(&self, query: &Geometry, mode: QueryMode) -> Vec<String> {
        let cover = self.cover_geometry(query);
        if cover.is_empty() {
            return Vec::new();
        }
        let mut candidates: HashSet<u32> = HashSet::new();
        for query_cell in &cover {
            // Ancestors: every strictly coarser cell that could contain
            // this query cell may be indexed as a feature cover member.
            let mut cursor = *query_cell;
            loop {
                if let Some(list) = self.cells.get(&cursor) {
                    candidates.extend(list.iter().copied());
                }
                if cursor.is_face() {
                    break;
                }
                cursor = cursor.immediate_parent();
            }
            // Descendants: every stored cell whose ID falls inside the
            // Hilbert range of this query cell is a strict descendant.
            let lo = query_cell.range_min();
            let hi = query_cell.range_max();
            for (_, list) in self.cells.range(lo..=hi) {
                candidates.extend(list.iter().copied());
            }
        }
        let mut matching: Vec<String> = Vec::new();
        for id in candidates {
            let (key, geom) = &self.features[id as usize];
            let matrix = geom.relate(query);
            let ok = match mode {
                QueryMode::Within => matrix.is_within(),
                QueryMode::Intersects => matrix.is_intersects(),
            };
            if ok {
                matching.push(key.clone());
            }
        }
        matching.sort();
        matching
    }
}

#[derive(Clone, Copy)]
enum QueryMode {
    Within,
    Intersects,
}

fn feature_key_from_subject(subject: NamedOrBlankNodeRef<'_>) -> String {
    match subject {
        NamedOrBlankNodeRef::NamedNode(n) => n.as_str().to_owned(),
        NamedOrBlankNodeRef::BlankNode(b) => format!("_:{}", b.as_str()),
    }
}
