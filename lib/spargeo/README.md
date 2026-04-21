spargeo
=======

[![Latest Version](https://img.shields.io/crates/v/spargeo.svg)](https://crates.io/crates/spargeo)
[![Released API docs](https://docs.rs/spargeo/badge.svg)](https://docs.rs/spargeo)
[![Crates.io downloads](https://img.shields.io/crates/d/spargeo)](https://crates.io/crates/spargeo)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

spargeo is a partial [GeoSPARQL 1.1](https://docs.ogc.org/is/22-047r1/22-047r1.html) implementation for Oxigraph.

Its entry point is the [`GEOSPARQL_EXTENSION_FUNCTIONS`] constant that lists GeoSPARQL extension functions ready to be registered in spargebra or oxigraph query evaluators.

Current scope is narrow: the 8 Simple Features topological relation functions, with WKT and GeoJSON inputs under the CRS84 reference system only. No metric, non-topological, accessor, serialization, aggregate, or transformation functions. No Egenhofer or RCC8 topology families. No query rewrite extension.

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
| `geof:ehEquals` | missing |
| `geof:ehDisjoint` | missing |
| `geof:ehMeet` | missing |
| `geof:ehOverlap` | missing |
| `geof:ehCovers` | missing |
| `geof:ehCoveredBy` | missing |
| `geof:ehInside` | missing |
| `geof:ehContains` | missing |

### Topology Vocabulary Extension: RCC8

| Function | Status |
|----------|--------|
| `geof:rcc8eq` | missing |
| `geof:rcc8dc` | missing |
| `geof:rcc8ec` | missing |
| `geof:rcc8po` | missing |
| `geof:rcc8tppi` | missing |
| `geof:rcc8tpp` | missing |
| `geof:rcc8ntpp` | missing |
| `geof:rcc8ntppi` | missing |

### Non-topological query functions

| Function | Status | Notes |
|----------|--------|-------|
| `geof:distance` | partial | three arg form with units IRI. Haversine, CRS84, point to point only |
| `geof:buffer` | missing | three arg form with radius and units |
| `geof:convexHull` | missing | |
| `geof:boundary` | missing | |
| `geof:envelope` | missing | minimum bounding rectangle |
| `geof:intersection` | missing | |
| `geof:union` | missing | |
| `geof:difference` | missing | |
| `geof:symDifference` | missing | |
| `geof:getSRID` | missing | |
| `geof:relate` | missing | DE-9IM intersection matrix pattern |

### Accessor functions

| Function | Status |
|----------|--------|
| `geof:dimension` | missing |
| `geof:coordinateDimension` | missing |
| `geof:spatialDimension` | missing |
| `geof:isEmpty` | missing |
| `geof:isSimple` | missing |
| `geof:hasSerialization` | missing |
| `geof:asText` | missing |
| `geof:asGML` | missing |
| `geof:asGeoJSON` | missing |
| `geof:asKML` | missing |
| `geof:asSVG` | missing |

### Metric functions

| Function | Status | Notes |
|----------|--------|-------|
| `geof:area` | partial | geodesic unsigned area, CRS84 only, square_metre and square_kilometre units |
| `geof:length` | partial | haversine length, CRS84, linear geometries only (Line, LineString, MultiLineString), returns zero for other types |
| `geof:perimeter` | missing | takes units argument |
| `geof:centroid` | missing | returns a point geometry |

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

The Core conformance class defines classes such as `geo:Feature`, `geo:Geometry`, `geo:SpatialObject`, and properties such as `geo:hasGeometry`, `geo:hasDefaultGeometry`, `geo:defaultGeometry`, plus the serialization properties `geo:asWKT`, `geo:asGML`, `geo:asGeoJSON`, `geo:asKML`, `geo:hasSerialization`. spargeo does not introduce or enforce these; it only consumes WKT and GeoJSON literals passed to the topology functions. Using them as RDF IRIs in a graph works today because oxigraph treats them as any other IRI.

### Query Rewrite Extension

| Capability | Status | Notes |
|------------|--------|-------|
| Function form (`FILTER(geof:sfWithin(?g1, ?g2))`) | implemented | via the Simple Features functions above |
| Property form (`?a geo:sfWithin ?b`) | missing | would require a rewrite pass in the query planner |

### Conformance class summary

| Conformance class | Status |
|-------------------|--------|
| Core | partial via oxigraph IRI handling only |
| Topology Vocabulary Extension (SF) | implemented |
| Topology Vocabulary Extension (Egenhofer) | missing |
| Topology Vocabulary Extension (RCC8) | missing |
| Geometry Extension | missing |
| Geometry Topology Extension (SF) | implemented |
| Geometry Topology Extension (Egenhofer) | missing |
| Geometry Topology Extension (RCC8) | missing |
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
  `asText`, `asGML`, `asGeoJSON`, `asKML`, `asSVG`. Serialization to
  text formats is a `geo` crate plus format crate job.
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
