//! Parsing and serialization helpers for GeoSPARQL literals.
//!
//! This module contains the shared plumbing used by every GeoSPARQL
//! extension function in this crate. Parsing accepts `wktLiteral` and
//! `geoJSONLiteral` inputs restricted to the CRS84 reference system.
//! Serialization always produces CRS84 `wktLiteral` output.

use crate::geosparql;
use geo::Geometry;
use geojson::{GeoJson, Geometry as GeoJsonGeometry};
use oxrdf::{Literal, Term};
use std::str::FromStr;
use wkt::{ToWkt, TryFromWkt};

/// CRS84 reference system URI used as the implicit coordinate reference
/// system for GeoSPARQL literals in this crate.
pub const CRS84_URI: &str = "http://www.opengis.net/def/crs/OGC/1.3/CRS84";

/// Parse a GeoSPARQL literal argument term into a `geo::Geometry`.
///
/// Supports `geosparql:wktLiteral` and `geosparql:geoJSONLiteral`. Returns
/// `None` for any other term shape or datatype, or when parsing fails.
pub fn extract_argument(term: &Term) -> Option<Geometry> {
    let Term::Literal(literal) = term else {
        return None;
    };
    if literal.datatype() == geosparql::WKT_LITERAL {
        parse_wkt_literal(literal.value().trim())
    } else if literal.datatype() == geosparql::GEO_JSON_LITERAL {
        parse_geo_json_literal(literal.value().trim())
    } else {
        None
    }
}

/// Parse a WKT literal value, optionally prefixed with a reference system URI.
///
/// Only the CRS84 reference system is accepted. Literals with any other
/// reference system URI return `None`.
pub fn parse_wkt_literal(value: &str) -> Option<Geometry> {
    let mut value = value.trim_start();
    if let Some(val) = value.strip_prefix('<') {
        let (system, val) = val.split_once('>').unwrap_or((val, ""));
        if system != CRS84_URI {
            return None;
        }
        value = val.trim_start();
    }
    Geometry::try_from_wkt_str(value).ok()
}

/// Parse a GeoJSON literal value into a geometry.
pub fn parse_geo_json_literal(value: &str) -> Option<Geometry> {
    GeoJson::from_str(value).ok()?.try_into().ok()
}

/// Serialize a geometry as a CRS84 `wktLiteral`.
///
/// The produced value is prefixed with the CRS84 reference system URI so
/// that downstream consumers can round trip the literal through
/// [`parse_wkt_literal`] without loss of the coordinate reference system.
pub fn result_to_wkt_literal(geom: &Geometry) -> Literal {
    let wkt_body = geom.wkt_string();
    let value = format!("<{CRS84_URI}> {wkt_body}");
    Literal::new_typed_literal(value, geosparql::WKT_LITERAL)
}

/// Serialize a geometry as a `geoJSONLiteral`.
pub fn result_to_geojson_literal(geom: &Geometry) -> Literal {
    let gj = GeoJsonGeometry::from(geom);
    Literal::new_typed_literal(gj.to_string(), geosparql::GEO_JSON_LITERAL)
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, clippy::panic)]
    use super::*;
    use oxrdf::Literal as OxLiteral;

    #[test]
    fn parse_wkt_without_crs() {
        let geom = parse_wkt_literal("POINT(1 2)").expect("parses");
        match geom {
            Geometry::Point(p) => {
                assert_eq!(p.x(), 1.0);
                assert_eq!(p.y(), 2.0);
            }
            _ => panic!("expected point"),
        }
    }

    #[test]
    fn parse_wkt_with_crs84() {
        let geom = parse_wkt_literal("<http://www.opengis.net/def/crs/OGC/1.3/CRS84> POINT(10 20)")
            .expect("parses");
        match geom {
            Geometry::Point(p) => {
                assert_eq!(p.x(), 10.0);
                assert_eq!(p.y(), 20.0);
            }
            _ => panic!("expected point"),
        }
    }

    #[test]
    fn parse_wkt_rejects_non_crs84() {
        assert!(
            parse_wkt_literal("<http://www.opengis.net/def/crs/EPSG/0/4326> POINT(1 2)").is_none()
        );
    }

    #[test]
    fn parse_geojson_point() {
        let geom =
            parse_geo_json_literal(r#"{"type":"Point","coordinates":[1.5,2.5]}"#).expect("parses");
        match geom {
            Geometry::Point(p) => {
                assert_eq!(p.x(), 1.5);
                assert_eq!(p.y(), 2.5);
            }
            _ => panic!("expected point"),
        }
    }

    #[test]
    fn extract_argument_wkt_literal() {
        let lit = OxLiteral::new_typed_literal("POINT(1 2)", geosparql::WKT_LITERAL);
        let geom = extract_argument(&Term::Literal(lit)).expect("parses");
        assert!(matches!(geom, Geometry::Point(_)));
    }

    #[test]
    fn extract_argument_rejects_plain_string() {
        let lit = OxLiteral::new_simple_literal("POINT(1 2)");
        assert!(extract_argument(&Term::Literal(lit)).is_none());
    }

    #[test]
    fn result_literal_roundtrips_through_parse() {
        let original = parse_wkt_literal("POINT(3 4)").expect("parses");
        let literal = result_to_wkt_literal(&original);
        assert_eq!(literal.datatype(), geosparql::WKT_LITERAL);
        assert!(literal.value().starts_with('<'));
        let parsed = parse_wkt_literal(literal.value()).expect("roundtrip parses");
        match parsed {
            Geometry::Point(p) => {
                assert_eq!(p.x(), 3.0);
                assert_eq!(p.y(), 4.0);
            }
            _ => panic!("expected point"),
        }
    }
}
