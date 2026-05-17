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

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
