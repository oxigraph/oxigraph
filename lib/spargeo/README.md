spargeo
=======

[![Latest Version](https://img.shields.io/crates/v/spargeo.svg)](https://crates.io/crates/spargeo)
[![Released API docs](https://docs.rs/spargeo/badge.svg)](https://docs.rs/spargeo)
[![Crates.io downloads](https://img.shields.io/crates/d/spargeo)](https://crates.io/crates/spargeo)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

spargeo is a partial [GeoSPARQL 1.1](https://docs.ogc.org/is/22-047r1/22-047r1.html) implementation for Oxigraph.

Its entry point is the [`GEOSPARQL_EXTENSION_FUNCTIONS`] constant that lists GeoSPARQL extension functions ready to be registered in spargebra or oxigraph query evaluators.

Current scope covers the three OGC Simple Features, Egenhofer, and RCC8 topology families, the planar boolean set operations (intersection, union, difference, symmetric difference), the DE-9IM `relate` tester, the topological accessor functions (`dimension`, `coordinateDimension`, `spatialDimension`, `isEmpty`, `isSimple`), the `envelope`, `convexHull`, and `centroid` constructors, the `asGeoJSON` serialiser, and partial metric functions (`area`, `length`, `perimeter`, `distance`). WKT and GeoJSON inputs are honoured under the CRS84 reference system only. Geometry returning functions echo the input datatype so that WKT inputs yield `geo:wktLiteral` outputs and GeoJSON inputs yield `geo:geoJSONLiteral` outputs. No aggregate or transformation functions. No GML, KML, DGGS literals. No query rewrite extension.

Coverage vs OGC 22-047r1
------------------------

Function IRIs live under `http://www.opengis.net/def/function/geosparql/` and are abbreviated as `geof:` below. Conformance class names follow the spec.

### Topology Vocabulary Extension: Simple Features

| Function | Status |
|----------|--------|
| `geof:sfEquals` | implemented |
| `geof:sfDisjoint` | implemented |
| `geof:sfIntersects` | implemented |
| `geof:sfTouches` | implemented |
| `geof:sfCrosses` | implemented |
| `geof:sfWithin` | implemented |
| `geof:sfContains` | implemented |
| `geof:sfOverlaps` | implemented |

### Topology Vocabulary Extension: Egenhofer

| Function | Status |
|----------|--------|
| `geof:ehEquals` | implemented |
| `geof:ehDisjoint` | implemented |
| `geof:ehMeet` | implemented |
| `geof:ehOverlap` | implemented |
| `geof:ehCovers` | implemented |
| `geof:ehCoveredBy` | implemented |
| `geof:ehInside` | implemented |
| `geof:ehContains` | implemented |

### Topology Vocabulary Extension: RCC8

| Function | Status |
|----------|--------|
| `geof:rcc8eq` | implemented |
| `geof:rcc8dc` | implemented |
| `geof:rcc8ec` | implemented |
| `geof:rcc8po` | implemented |
| `geof:rcc8tppi` | implemented |
| `geof:rcc8tpp` | implemented |
| `geof:rcc8ntpp` | implemented |
| `geof:rcc8ntppi` | implemented |

### Non-topological query functions

| Function | Status | Notes |
|----------|--------|-------|
| `geof:distance` | partial | three arg form with units IRI. Haversine, CRS84, point to point only |
| `geof:buffer` | missing | three arg form with radius and units |
| `geof:convexHull` | partial | planar QuickHull over CRS84 input, not a true spherical hull |
| `geof:boundary` | missing | |
| `geof:envelope` | partial | axis aligned bounding rectangle in CRS84 coordinates |
| `geof:intersection` | partial | planar boolean intersection over CRS84 input |
| `geof:union` | partial | planar boolean union over CRS84 input |
| `geof:difference` | partial | planar boolean difference over CRS84 input |
| `geof:symDifference` | partial | planar symmetric difference over CRS84 input |
| `geof:getSRID` | partial | always returns the CRS84 URI because only CRS84 is parsed |
| `geof:relate` | implemented | three arg form with DE-9IM intersection matrix pattern |

### Accessor functions

| Function | Status | Notes |
|----------|--------|-------|
| `geof:dimension` | implemented | returns xsd:integer topological dimension |
| `geof:coordinateDimension` | implemented | always 2 because only CRS84 is parsed |
| `geof:spatialDimension` | implemented | matches `geof:dimension` because 3D inputs are not supported |
| `geof:isEmpty` | implemented | |
| `geof:isSimple` | partial | uses `geo::Validation` as a conservative approximation |
| `geof:hasSerialization` | missing | |
| `geof:asGML` | missing | |
| `geof:asGeoJSON` | implemented | returns the GeoJSON rendering as `geo:geoJSONLiteral` |
| `geof:asKML` | missing | |
| `geof:asSVG` | missing | |

### Metric functions

| Function | Status | Notes |
|----------|--------|-------|
| `geof:area` | partial | geodesic unsigned area, CRS84 only, square_metre and square_kilometre units |
| `geof:length` | partial | haversine length, CRS84, linear geometries only (Line, LineString, MultiLineString), returns zero for other types |
| `geof:perimeter` | partial | haversine length around polygon exteriors, CRS84 only, returns zero for non polygonal geometries |
| `geof:centroid` | partial | arithmetic centroid in CRS84 coordinates, not geodesic |

### Aggregate functions

| Function | Status |
|----------|--------|
| `geof:aggBoundingBox` | missing |
| `geof:aggBoundingCircle` | missing |
| `geof:aggCentroid` | missing |
| `geof:aggConcaveHull` | missing |
| `geof:aggConvexHull` | missing |

### Transformation

| Function | Status | Notes |
|----------|--------|-------|
| `geof:transform` | missing | reproject a geometry to a target CRS |

### Geometry literal datatypes

| Datatype | Status | Notes |
|----------|--------|-------|
| `geo:wktLiteral` | partial | parsed via the `wkt` crate. Only the CRS84 reference system is honoured; other `<uri>` prefixes are rejected |
| `geo:geoJSONLiteral` | partial | parsed via the `geojson` crate |
| `geo:gmlLiteral` | missing | |
| `geo:kmlLiteral` | missing | |
| `geo:dggsLiteral` | missing | |

### CRS support

| Capability | Status |
|------------|--------|
| CRS84 input | implemented |
| EPSG:4326 input (axis swap) | missing |
| Arbitrary CRS via `<uri>` prefix | missing |
| Reprojection via `geof:transform` | missing |

### Feature and geometry vocabulary

The Core conformance class defines classes such as `geo:Feature`, `geo:Geometry`, `geo:SpatialObject`, and properties such as `geo:hasGeometry`, `geo:hasDefaultGeometry`, `geo:defaultGeometry`, plus the serialization properties `geo:asWKT`, `geo:asGML`, `geo:asGeoJSON`, `geo:asKML`, `geo:hasSerialization`. spargeo ships these IRI constants in the `vocab` module.

### Query Rewrite Extension

| Capability | Status | Notes |
|------------|--------|-------|
| Function form (`FILTER(geof:sfWithin(?g1, ?g2))`) | implemented | via the Simple Features functions above |
| Property form (`?a geo:sfWithin ?b`) | missing | would require a rewrite pass in the query planner |

### Conformance class summary

| Conformance class | Status |
|-------------------|--------|
| Core | partial via oxigraph IRI handling plus vocab constants and ontology stub |
| Topology Vocabulary Extension (SF) | implemented |
| Topology Vocabulary Extension (Egenhofer) | implemented |
| Topology Vocabulary Extension (RCC8) | implemented |
| Geometry Extension | partial via accessor and boolean set functions |
| Geometry Topology Extension (SF) | implemented |
| Geometry Topology Extension (Egenhofer) | implemented |
| Geometry Topology Extension (RCC8) | implemented |
| RDFS Entailment Extension | missing (upstream reasoner scope) |
| Query Rewrite Extension | missing |

Relationship to upstream issue #1560
------------------------------------

[oxigraph#1560](https://github.com/oxigraph/oxigraph/issues/1560) proposes
a full GeoSPARQL 1.1 implementation baked into Oxigraph itself, behind a
`geosparql` feature flag, and explicitly contemplates replacing or
absorbing this crate. The gaps listed above split cleanly into two
buckets along the architectural line that #1560 draws:

**Pluggable gaps.** These can land inside spargeo as additional entries
in `GEOSPARQL_EXTENSION_FUNCTIONS` without touching the storage or
query evaluation layers. Each is a pure function over parsed geometries.

* Egenhofer and RCC8 topology families (16 functions). All three families
  reduce to the same DE-9IM matrix that the `geo` crate's `Relate`
  already produces; adding them is a matter of mapping the matrix bits
  to the named relation. Same pattern as the existing SF functions.
* Non-topological query functions: `distance`, `buffer`, `convexHull`,
  `boundary`, `envelope`, `intersection`, `union`, `difference`,
  `symDifference`, `relate`, `getSRID`. All available in the `geo`
  crate directly.
* Accessor functions: `dimension`, `coordinateDimension`,
  `spatialDimension`, `isEmpty`, `isSimple`, `hasSerialization`,
  `asGML`, `asKML`, `asSVG`. Serialization to text formats is a `geo`
  crate plus format crate job.
* Metric functions: `area`, `length`, `perimeter`, `centroid`. Area and
  length need a distance calculation strategy (planar vs spherical),
  parameterised by the units IRI.
* Additional literal datatypes: `gmlLiteral`, `kmlLiteral`. Would need
  `gml` and `kml` parser crates.

**Architectural gaps.** These are the ones #1560 is really after and
cannot be delivered by a functions-only plugin.

* Efficient binary geometry storage. #1560 proposes WKB with a new
  literal kind in `oxrdf`'s `numeric_encoder` and `binary_encoder` so
  the RocksDB backend holds pre-parsed geometries rather than reparsing
  WKT strings on every function call.
* Geometry index. #1560 proposes an S2 cell id column family in RocksDB,
  keyed `CellID -> LiteralID`, so spatial predicates turn into range
  scans rather than full graph enumerations. Current spargeo has no
  index awareness; every `geof:sfWithin` call reparses and compares
  every candidate geometry.
* Query Rewrite Extension. The GeoSPARQL 1.1 rewrite turns
  `?f geo:sfWithin <polygon>` into a function-form `FILTER` plus a
  `geo:hasGeometry` / `geo:asWKT` join. Lives in the query planner, not
  in an extension function list. Tpt's comment on #1560 points at an AST
  preprocessing step plus a new algebra operator that can convert the
  filter into a tuple generator.
* CRS reprojection. `geof:transform` itself is a pure function, but it
  requires a dependency on a projection library (likely `proj4rs` or
  equivalent) and a decision about whether reprojection happens at
  parse time, at index time, or at query time. Inside #1560's S2 index
  scheme, a common storage CRS is required, so CRS handling is entangled
  with indexing.
* GeoSPARQL 1.1 abstract test suite. #1560 notes this should be
  translated into the SPARQL test suite format so any implementation can
  run it.

Concretely, closing the pluggable gaps in this crate is useful short
term work that does not collide with #1560, and the resulting code can
be reused by the `geosparql` feature inside Oxigraph when #1560 lands.
Closing the architectural gaps belongs in that upstream effort rather
than here.

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
