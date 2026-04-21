spargeo implementation plan
===========================

This ledger is driven by an hourly scheduled task. Each run ticks off
exactly one unchecked item. Items marked `(hold)` are skipped until a
dependency decision is made. Do not reorder; process top to bottom.

Shared plumbing
---------------

- [x] Extract `src/parse.rs` with public `parse_wkt_literal`, `parse_geo_json_literal`, `extract_argument`, plus a `result_to_wkt_literal(geom: geo::Geometry) -> oxrdf::Literal` helper that emits the CRS84 `wktLiteral`. Update `src/lib.rs` to import from this module.
- [x] Add `src/units.rs` with `pub enum UnitKind { Length, Angle, Area }` and `pub fn units_to_factor(iri: &str, kind: UnitKind) -> Option<f64>` covering OGC uom IRIs: `http://www.opengis.net/def/uom/OGC/1.0/metre`, `kilometre`, `degree`, `radian`, and for area `http://www.opengis.net/def/uom/OGC/1.0/square_metre`, `square_kilometre`.

Phase 1 pure functions
----------------------

- [x] Add `geof:distance` using `geo::HaversineDistance` for CRS84 input. Three args: two geometry literals plus a length units IRI.
- [x] Add `geof:area` using `geo::GeodesicArea::geodesic_area_unsigned`. Two args: geometry plus area units IRI.
- [x] Add `geof:length` using `geo::HaversineLength` (fallback to `geo::GeodesicLength` where relevant). Two args: line geometry plus length units IRI.
- [x] Add `geof:envelope` using `geo::BoundingRect`. Returns bounding polygon as CRS84 wktLiteral.
- [x] Add `geof:centroid` using `geo::Centroid`. Returns point wktLiteral.
- [x] Add `geof:convexHull` using `geo::ConvexHull`. Returns polygon wktLiteral.
- [x] Add `geof:getSRID`. Currently returns `<http://www.opengis.net/def/crs/OGC/1.3/CRS84>` for every input.
- [x] Add `geof:isEmpty`. Returns xsd:boolean.
- [x] Add `geof:isSimple`. Returns xsd:boolean. Use `geo::algorithm::is_convex::IsConvex` or `geo::relate` as needed.
- [x] Add `geof:dimension`. Returns xsd:integer: 0 for Point/MultiPoint, 1 for LineString/MultiLineString, 2 for Polygon/MultiPolygon.
- [x] Add `geof:coordinateDimension`. Returns 2 for all geometries (CRS84 is 2D for now).
- [x] Add `geof:spatialDimension`. Same as `geof:dimension` for non-3D geometries.
- [x] Add `geof:asText`. Returns xsd:string containing the WKT serialization of the geometry.
- [x] Add `geof:asGeoJSON`. Returns xsd:string containing the GeoJSON serialization.

Phase 2 set operations (polygons only)
--------------------------------------

- [x] Add `geof:intersection` using `geo::BooleanOps`.
- [x] Add `geof:union` using `geo::BooleanOps`.
- [x] Add `geof:difference` using `geo::BooleanOps`.
- [x] Add `geof:symDifference` using `geo::BooleanOps`.

Phase 3 harder
--------------

- [x] Add `geof:relate` taking (geom, geom, de9im_pattern: String). Extract DE-9IM matrix from `geo::Relate`, match against the pattern (9 positions, chars `T`, `F`, `*`, `0`, `1`, `2`).
- [x] Add `geof:perimeter` using the same length primitive applied to the boundary of the polygon.
- [ ] (hold) `geof:buffer`. Pending dependency decision between `geo-buffer` crate (pure Rust, limited) and `geos-rs` (GEOS C library). Do not implement until user decides.

Phase 4 topology families (mechanical, from DE-9IM table)
---------------------------------------------------------

- [x] Add `geof:ehEquals` from OGC 22-047r1 Egenhofer table.
- [x] Add `geof:ehDisjoint`.
- [x] Add `geof:ehMeet`.
- [x] Add `geof:ehOverlap`.
- [x] Add `geof:ehCovers`.
- [x] Add `geof:ehCoveredBy`.
- [x] Add `geof:ehInside`.
- [x] Add `geof:ehContains`.
- [x] Add `geof:rcc8eq`.
- [x] Add `geof:rcc8dc`.
- [x] Add `geof:rcc8ec`.
- [x] Add `geof:rcc8po`.
- [x] Add `geof:rcc8tppi`.
- [x] Add `geof:rcc8tpp`.
- [x] Add `geof:rcc8ntpp`.
- [x] Add `geof:rcc8ntppi`.

Phase 5 additional serializations
---------------------------------

- [ ] (hold) gmlLiteral parsing. Needs GML crate choice.
- [ ] (hold) kmlLiteral parsing. Needs KML crate choice.
- [ ] (hold) `geof:asGML`.
- [ ] (hold) `geof:asKML`.
- [ ] (hold) `geof:asSVG`.

Bridge work (after Phase 1 completes)
-------------------------------------

- [x] Add `src/vocab.rs` with GeoSPARQL IRI constants and a GeoSPARQL 1.1 ontology stub at `data/geosparql.ttl` declaring `geo:sfWithin` as transitive, `geo:sfContains` as inverseOf `geo:sfWithin`, `geo:sfEquals`/`geo:sfOverlaps`/`geo:sfTouches`/`geo:sfCrosses` as symmetric.
- [x] Add `src/bridge.rs` with a `GeoBridge` struct exposing `materialize_relations(graph: &mut oxrdf::Graph)`. Walks `geo:hasGeometry` -> `geo:asWKT` chains, extracts geometries via the shared `parse.rs` helpers, runs pairwise `geo::Relate` between each pair of features, and emits Simple Features topology triples into the graph.
- [x] Gate the bridge behind a `bridge` feature in `Cargo.toml` so minimal builds stay pure functions only.

Spatial index (in-memory, validates cell handling and API shape)
----------------------------------------------------------------

- [x] Add `s2 = "0.0.13"` to workspace dependencies and an optional `spatial_index = ["dep:s2"]` feature to `lib/spargeo/Cargo.toml`.
- [x] Add `src/index.rs` with a `SpatialIndex` struct. `from_graph(&Graph)` walks `geo:hasGeometry` -> `geo:asWKT` chains, covers each feature's bounding rect with an S2 `CellUnion` via `RegionCoverer`, and stores a sorted `BTreeMap<CellID, Vec<u32>>`. `query_within(&Geometry)` and `query_intersects(&Geometry)` gather candidates through ancestor lookups plus `range_min..=range_max` descendant scans, then refine with `geo::Relate`.
- [x] Add `tests/index.rs` integration tests covering within, intersects, disjoint skip, empty graph, and point insert.
- [ ] (hold) RocksDB column family version. Belongs in oxigraph itself under the `geosparql` feature proposed in issue #1560, not in this crate.
- [ ] (hold) Query rewrite pass in `spareval` that turns `?f geo:sfWithin ?g` property form into a tuple generator over this index. Depends on the WKB literal storage decision in issue #1560.
