spargeo implementation plan
===========================

This ledger is driven by an hourly scheduled task. Each run ticks off
exactly one unchecked item. Items marked `(hold)` are skipped until a
dependency decision is made. Do not reorder; process top to bottom.

Shared plumbing
---------------

- [ ] Extract `src/parse.rs` with public `parse_wkt_literal`, `parse_geo_json_literal`, `extract_argument`, plus a `result_to_wkt_literal(geom: geo::Geometry) -> oxrdf::Literal` helper that emits the CRS84 `wktLiteral`. Update `src/lib.rs` to import from this module.
- [ ] Add `src/units.rs` with `pub enum UnitKind { Length, Angle, Area }` and `pub fn units_to_factor(iri: &str, kind: UnitKind) -> Option<f64>` covering OGC uom IRIs: `http://www.opengis.net/def/uom/OGC/1.0/metre`, `kilometre`, `degree`, `radian`, and for area `http://www.opengis.net/def/uom/OGC/1.0/square_metre`, `square_kilometre`.

Phase 1 pure functions
----------------------

- [ ] Add `geof:distance` using `geo::HaversineDistance` for CRS84 input. Three args: two geometry literals plus a length units IRI.
- [ ] Add `geof:area` using `geo::GeodesicArea::geodesic_area_unsigned`. Two args: geometry plus area units IRI.
- [ ] Add `geof:length` using `geo::HaversineLength` (fallback to `geo::GeodesicLength` where relevant). Two args: line geometry plus length units IRI.
- [ ] Add `geof:envelope` using `geo::BoundingRect`. Returns bounding polygon as CRS84 wktLiteral.
- [ ] Add `geof:centroid` using `geo::Centroid`. Returns point wktLiteral.
- [ ] Add `geof:convexHull` using `geo::ConvexHull`. Returns polygon wktLiteral.
- [ ] Add `geof:getSRID`. Currently returns `<http://www.opengis.net/def/crs/OGC/1.3/CRS84>` for every input.
- [ ] Add `geof:isEmpty`. Returns xsd:boolean.
- [ ] Add `geof:isSimple`. Returns xsd:boolean. Use `geo::algorithm::is_convex::IsConvex` or `geo::relate` as needed.
- [ ] Add `geof:dimension`. Returns xsd:integer: 0 for Point/MultiPoint, 1 for LineString/MultiLineString, 2 for Polygon/MultiPolygon.
- [ ] Add `geof:coordinateDimension`. Returns 2 for all geometries (CRS84 is 2D for now).
- [ ] Add `geof:spatialDimension`. Same as `geof:dimension` for non-3D geometries.
- [ ] Add `geof:asText`. Returns xsd:string containing the WKT serialization of the geometry.
- [ ] Add `geof:asGeoJSON`. Returns xsd:string containing the GeoJSON serialization.

Phase 2 set operations (polygons only)
--------------------------------------

- [ ] Add `geof:intersection` using `geo::BooleanOps`.
- [ ] Add `geof:union` using `geo::BooleanOps`.
- [ ] Add `geof:difference` using `geo::BooleanOps`.
- [ ] Add `geof:symDifference` using `geo::BooleanOps`.

Phase 3 harder
--------------

- [ ] Add `geof:relate` taking (geom, geom, de9im_pattern: String). Extract DE-9IM matrix from `geo::Relate`, match against the pattern (9 positions, chars `T`, `F`, `*`, `0`, `1`, `2`).
- [ ] Add `geof:perimeter` using the same length primitive applied to the boundary of the polygon.
- [ ] (hold) `geof:buffer`. Pending dependency decision between `geo-buffer` crate (pure Rust, limited) and `geos-rs` (GEOS C library). Do not implement until user decides.

Phase 4 topology families (mechanical, from DE-9IM table)
---------------------------------------------------------

- [ ] Add `geof:ehEquals` from OGC 22-047r1 Egenhofer table.
- [ ] Add `geof:ehDisjoint`.
- [ ] Add `geof:ehMeet`.
- [ ] Add `geof:ehOverlap`.
- [ ] Add `geof:ehCovers`.
- [ ] Add `geof:ehCoveredBy`.
- [ ] Add `geof:ehInside`.
- [ ] Add `geof:ehContains`.
- [ ] Add `geof:rcc8eq`.
- [ ] Add `geof:rcc8dc`.
- [ ] Add `geof:rcc8ec`.
- [ ] Add `geof:rcc8po`.
- [ ] Add `geof:rcc8tppi`.
- [ ] Add `geof:rcc8tpp`.
- [ ] Add `geof:rcc8ntpp`.
- [ ] Add `geof:rcc8ntppi`.

Phase 5 additional serializations
---------------------------------

- [ ] (hold) gmlLiteral parsing. Needs GML crate choice.
- [ ] (hold) kmlLiteral parsing. Needs KML crate choice.
- [ ] (hold) `geof:asGML`.
- [ ] (hold) `geof:asKML`.
- [ ] (hold) `geof:asSVG`.

Bridge work (after Phase 1 completes)
-------------------------------------

- [ ] Add `src/vocab.rs` with GeoSPARQL IRI constants and a GeoSPARQL 1.1 ontology stub at `data/geosparql.ttl` declaring `geo:sfWithin` as transitive, `geo:sfContains` as inverseOf `geo:sfWithin`, `geo:sfEquals`/`geo:sfOverlaps`/`geo:sfTouches`/`geo:sfCrosses` as symmetric.
- [ ] Add `src/bridge.rs` with a `GeoBridge` struct exposing `materialize_relations(graph: &mut oxrdf::Graph)`. Walks `geo:hasGeometry` -> `geo:asWKT` chains, extracts geometries via the shared `parse.rs` helpers, runs pairwise `geo::Relate` between each pair of features in a caller-provided region set, and emits `geo:sfWithin` / `geo:sfContains` / `geo:sfTouches` triples into the graph.
- [ ] Gate the bridge behind a `bridge` feature in `Cargo.toml` so minimal builds stay pure functions only.
