//! GeoSPARQL 1.1 vocabulary constants.
//!
//! Exposes the `geo:` namespace prefix plus the core class and property IRIs
//! used by the GeoSPARQL 1.1 ontology. Downstream consumers can reference
//! these constants instead of hard coding the IRI strings.

use oxrdf::NamedNodeRef;

/// `geo:` namespace prefix used throughout the GeoSPARQL 1.1 ontology.
pub const GEO_NS: &str = "http://www.opengis.net/ont/geosparql#";

/// Class `geo:Feature`.
pub const FEATURE: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#Feature");

/// Class `geo:Geometry`.
pub const GEOMETRY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#Geometry");

/// Class `geo:SpatialObject`.
pub const SPATIAL_OBJECT: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#SpatialObject");

/// Property `geo:hasGeometry`.
pub const HAS_GEOMETRY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#hasGeometry");

/// Property `geo:hasDefaultGeometry`.
pub const HAS_DEFAULT_GEOMETRY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#hasDefaultGeometry");

/// Property `geo:defaultGeometry`.
pub const DEFAULT_GEOMETRY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#defaultGeometry");

/// Property `geo:asWKT`.
pub const AS_WKT: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#asWKT");

/// Property `geo:asGeoJSON`.
pub const AS_GEO_JSON: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#asGeoJSON");

/// Property `geo:hasSerialization`.
pub const HAS_SERIALIZATION: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#hasSerialization");

/// Datatype `geo:wktLiteral`.
pub const WKT_LITERAL: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#wktLiteral");

/// Datatype `geo:geoJSONLiteral`.
pub const GEO_JSON_LITERAL: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#geoJSONLiteral");

/// Simple Features topology predicates used by the bridge when materialising
/// relations between feature geometries.
pub const SF_WITHIN: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#sfWithin");
pub const SF_CONTAINS: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#sfContains");
pub const SF_EQUALS: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#sfEquals");
pub const SF_OVERLAPS: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#sfOverlaps");
pub const SF_TOUCHES: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#sfTouches");
pub const SF_CROSSES: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#sfCrosses");
pub const SF_DISJOINT: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#sfDisjoint");
pub const SF_INTERSECTS: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#sfIntersects");
