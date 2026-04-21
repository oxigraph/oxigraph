#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod parse;
mod units;

use crate::parse::extract_argument;
use crate::units::{extract_units_iri, units_to_factor, UnitKind};
use geo::{Distance, GeodesicArea, Geometry, Haversine, Length, Point, Relate};
use oxrdf::{Literal, NamedNodeRef, Term};

/// GeoSPARQL functions in name and implementation pairs
pub const GEOSPARQL_EXTENSION_FUNCTIONS: [(NamedNodeRef<'static>, fn(&[Term]) -> Option<Term>); 11] = [
    (geosparql_functions::AREA, geof_area),
    (geosparql_functions::DISTANCE, geof_distance),
    (geosparql_functions::LENGTH, geof_length),
    (geosparql_functions::SF_CONTAINS, geof_sf_contains),
    (geosparql_functions::SF_CROSSES, geof_sf_crosses),
    (geosparql_functions::SF_DISJOINT, geof_sf_disjoint),
    (geosparql_functions::SF_EQUALS, geof_sf_equals),
    (geosparql_functions::SF_INTERSECTS, geof_sf_intersects),
    (geosparql_functions::SF_OVERLAPS, geof_sf_overlaps),
    (geosparql_functions::SF_TOUCHES, geof_sf_touches),
    (geosparql_functions::SF_WITHIN, geof_sf_within),
];

/// `geof:area`. Computes the geodesic unsigned area of a geometry under the
/// CRS84 reference system and returns it as an `xsd:double` expressed in the
/// target units of measure.
///
/// Two arguments are expected: a geometry literal followed by an OGC units of
/// measure IRI for an area unit. Geometries with zero planar extent (points,
/// lines, multi points, multi line strings) return zero. Unknown units or
/// non-area unit IRIs return no binding.
fn geof_area(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let units_iri = extract_units_iri(&args[1])?;
    let factor = units_to_factor(units_iri, UnitKind::Area)?;
    let square_metres = geom.geodesic_area_unsigned();
    Some(Literal::from(square_metres / factor).into())
}

/// `geof:distance`. Computes the haversine distance between two point
/// geometries and returns it as an `xsd:double` expressed in the target
/// units of measure.
///
/// Three arguments are expected: two geometry literals followed by an OGC
/// units of measure IRI. Only `Point` geometries are supported, consistent
/// with the CRS84 assumption of this crate. Non point inputs or unknown
/// units return no binding.
fn geof_distance(args: &[Term]) -> Option<Term> {
    let args: &[Term; 3] = args.try_into().ok()?;
    let left = extract_argument(&args[0])?;
    let right = extract_argument(&args[1])?;
    let units_iri = extract_units_iri(&args[2])?;
    let factor = units_to_factor(units_iri, UnitKind::Length)?;
    let p1 = as_point(left)?;
    let p2 = as_point(right)?;
    let meters = Haversine.distance(p1, p2);
    Some(Literal::from(meters / factor).into())
}

#[inline]
fn as_point(geom: Geometry) -> Option<Point> {
    match geom {
        Geometry::Point(p) => Some(p),
        _ => None,
    }
}

/// `geof:length`. Computes the haversine length of a linear geometry under
/// the CRS84 reference system and returns it as an `xsd:double` expressed in
/// the target units of measure.
///
/// Two arguments are expected: a geometry literal followed by an OGC units
/// of measure IRI for a length unit. Line, LineString and MultiLineString
/// inputs produce their geodesic length. Geometries without linear extent
/// (points, polygons, collections of those) return zero, consistent with
/// the GeoSPARQL accessor semantics that reserve perimeter for polygons.
fn geof_length(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let units_iri = extract_units_iri(&args[1])?;
    let factor = units_to_factor(units_iri, UnitKind::Length)?;
    let metres = match geom {
        Geometry::Line(l) => Haversine.length(&l),
        Geometry::LineString(ls) => Haversine.length(&ls),
        Geometry::MultiLineString(mls) => Haversine.length(&mls),
        _ => 0.0,
    };
    Some(Literal::from(metres / factor).into())
}

fn geof_sf_equals(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_equal_topo())
}

fn geof_sf_disjoint(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_disjoint())
}

fn geof_sf_intersects(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_intersects())
}

fn geof_sf_touches(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_touches())
}

fn geof_sf_crosses(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_crosses())
}

fn geof_sf_within(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_within())
}

fn geof_sf_contains(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_contains())
}

fn geof_sf_overlaps(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_overlaps())
}

fn binary_geo_fn<R: Into<Literal>>(
    args: &[Term],
    operation: impl FnOnce(Geometry, Geometry) -> R,
) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let left = extract_argument(&args[0])?;
    let right = extract_argument(&args[1])?;
    Some(operation(left, right).into().into())
}

pub(crate) mod geosparql {
    //! [GeoSpatial](https://opengeospatial.github.io/ogc-geosparql/) vocabulary.
    use oxrdf::NamedNodeRef;

    pub const GEO_JSON_LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#geoJSONLiteral");
    pub const WKT_LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#wktLiteral");
}

mod geosparql_functions {
    //! [GeoSpatial](https://opengeospatial.github.io/ogc-geosparql/) functions vocabulary.
    use oxrdf::NamedNodeRef;

    pub const AREA: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/area");
    pub const DISTANCE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/distance");
    pub const LENGTH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/length");
    pub const SF_CONTAINS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfContains");
    pub const SF_CROSSES: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfCrosses");
    pub const SF_DISJOINT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfDisjoint");
    pub const SF_EQUALS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfEquals");
    pub const SF_INTERSECTS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfIntersects");
    pub const SF_OVERLAPS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfOverlaps");
    pub const SF_TOUCHES: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfTouches");
    pub const SF_WITHIN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfWithin");
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{Literal as OxLiteral, NamedNode};

    fn wkt_literal(value: &str) -> Term {
        Term::Literal(OxLiteral::new_typed_literal(value, geosparql::WKT_LITERAL))
    }

    fn units_named_node(iri: &str) -> Term {
        Term::NamedNode(NamedNode::new_unchecked(iri))
    }

    fn parse_double(term: &Term) -> f64 {
        match term {
            Term::Literal(l) => l.value().parse::<f64>().expect("double"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn distance_new_york_to_london_in_metres() {
        let nyc = wkt_literal("POINT(-74.006 40.7128)");
        let london = wkt_literal("POINT(-0.1278 51.5074)");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let result = geof_distance(&[nyc, london, metres]).expect("computes");
        let value = parse_double(&result);
        assert!(
            (value - 5_570_230.0).abs() < 50.0,
            "got {value}, expected near 5570230 metres"
        );
    }

    #[test]
    fn distance_is_unit_scaled() {
        let a = wkt_literal("POINT(0 0)");
        let b = wkt_literal("POINT(1 0)");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let kilometres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/kilometre");
        let in_metres =
            parse_double(&geof_distance(&[a.clone(), b.clone(), metres]).expect("metres"));
        let in_kilometres =
            parse_double(&geof_distance(&[a, b, kilometres]).expect("kilometres"));
        assert!((in_metres / 1000.0 - in_kilometres).abs() < 1e-6);
    }

    #[test]
    fn distance_accepts_units_as_literal() {
        let a = wkt_literal("POINT(0 0)");
        let b = wkt_literal("POINT(0 0)");
        let metres = Term::Literal(OxLiteral::new_simple_literal(
            "http://www.opengis.net/def/uom/OGC/1.0/metre",
        ));
        let result = geof_distance(&[a, b, metres]).expect("computes");
        assert!(parse_double(&result).abs() < 1e-9);
    }

    #[test]
    fn distance_rejects_unknown_units() {
        let a = wkt_literal("POINT(0 0)");
        let b = wkt_literal("POINT(1 0)");
        let bad = units_named_node("http://example.org/uom/parsec");
        assert!(geof_distance(&[a, b, bad]).is_none());
    }

    #[test]
    fn distance_rejects_non_point_geometry() {
        let line = wkt_literal("LINESTRING(0 0, 1 0)");
        let b = wkt_literal("POINT(0 0)");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        assert!(geof_distance(&[line, b, metres]).is_none());
    }

    #[test]
    fn distance_rejects_wrong_arity() {
        let a = wkt_literal("POINT(0 0)");
        let b = wkt_literal("POINT(1 0)");
        assert!(geof_distance(&[a, b]).is_none());
    }

    #[test]
    fn area_of_one_degree_square_near_equator() {
        let square = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let square_metres = units_named_node(
            "http://www.opengis.net/def/uom/OGC/1.0/square_metre",
        );
        let value = parse_double(
            &geof_area(&[square, square_metres]).expect("computes"),
        );
        // A one degree by one degree patch at the equator is about
        // 12309 square kilometres according to geodesic calculation.
        assert!(
            (value - 1.2309e10).abs() < 1.0e8,
            "got {value}, expected near 1.2309e10 square metres"
        );
    }

    #[test]
    fn area_is_unit_scaled() {
        let square = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let square_metres = units_named_node(
            "http://www.opengis.net/def/uom/OGC/1.0/square_metre",
        );
        let square_kilometres = units_named_node(
            "http://www.opengis.net/def/uom/OGC/1.0/square_kilometre",
        );
        let in_m2 = parse_double(
            &geof_area(&[square.clone(), square_metres]).expect("m2"),
        );
        let in_km2 = parse_double(
            &geof_area(&[square, square_kilometres]).expect("km2"),
        );
        assert!((in_m2 / 1_000_000.0 - in_km2).abs() < 1e-3);
    }

    #[test]
    fn area_of_point_is_zero() {
        let point = wkt_literal("POINT(10 20)");
        let square_metres = units_named_node(
            "http://www.opengis.net/def/uom/OGC/1.0/square_metre",
        );
        let value = parse_double(
            &geof_area(&[point, square_metres]).expect("computes"),
        );
        assert_eq!(value, 0.0);
    }

    #[test]
    fn area_of_line_is_zero() {
        let line = wkt_literal("LINESTRING(0 0, 1 1, 2 0)");
        let square_metres = units_named_node(
            "http://www.opengis.net/def/uom/OGC/1.0/square_metre",
        );
        let value = parse_double(
            &geof_area(&[line, square_metres]).expect("computes"),
        );
        assert_eq!(value, 0.0);
    }

    #[test]
    fn area_rejects_length_units() {
        let square = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        assert!(geof_area(&[square, metres]).is_none());
    }

    #[test]
    fn area_rejects_unknown_units() {
        let square = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let bad = units_named_node("http://example.org/uom/acre");
        assert!(geof_area(&[square, bad]).is_none());
    }

    #[test]
    fn area_rejects_wrong_arity() {
        let square = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        assert!(geof_area(&[square]).is_none());
    }

    #[test]
    fn length_of_london_to_paris_line() {
        // Single segment from central London to central Paris.
        let line = wkt_literal("LINESTRING(-0.1278 51.5074, 2.3522 48.8566)");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let value = parse_double(&geof_length(&[line, metres]).expect("computes"));
        // Haversine length is about 343.5 km. Allow 1 km tolerance.
        assert!(
            (value - 343_557.0).abs() < 1_000.0,
            "got {value}, expected near 343557 metres"
        );
    }

    #[test]
    fn length_is_unit_scaled() {
        let line = wkt_literal("LINESTRING(0 0, 1 0)");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let kilometres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/kilometre");
        let in_metres =
            parse_double(&geof_length(&[line.clone(), metres]).expect("metres"));
        let in_kilometres =
            parse_double(&geof_length(&[line, kilometres]).expect("kilometres"));
        assert!((in_metres / 1000.0 - in_kilometres).abs() < 1e-6);
    }

    #[test]
    fn length_of_multi_line_string_sums_parts() {
        let one = wkt_literal("LINESTRING(0 0, 1 0)");
        let many = wkt_literal("MULTILINESTRING((0 0, 1 0), (10 10, 10 11))");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let single =
            parse_double(&geof_length(&[one, metres.clone()]).expect("single"));
        let combined =
            parse_double(&geof_length(&[many, metres]).expect("combined"));
        // Combined length must exceed either single segment.
        assert!(combined > single);
        assert!(combined > 0.0);
    }

    #[test]
    fn length_of_point_is_zero() {
        let point = wkt_literal("POINT(10 20)");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let value = parse_double(&geof_length(&[point, metres]).expect("computes"));
        assert_eq!(value, 0.0);
    }

    #[test]
    fn length_of_polygon_is_zero() {
        let poly = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let value = parse_double(&geof_length(&[poly, metres]).expect("computes"));
        assert_eq!(value, 0.0);
    }

    #[test]
    fn length_rejects_area_units() {
        let line = wkt_literal("LINESTRING(0 0, 1 0)");
        let square_metres = units_named_node(
            "http://www.opengis.net/def/uom/OGC/1.0/square_metre",
        );
        assert!(geof_length(&[line, square_metres]).is_none());
    }

    #[test]
    fn length_rejects_unknown_units() {
        let line = wkt_literal("LINESTRING(0 0, 1 0)");
        let bad = units_named_node("http://example.org/uom/furlong");
        assert!(geof_length(&[line, bad]).is_none());
    }

    #[test]
    fn length_rejects_wrong_arity() {
        let line = wkt_literal("LINESTRING(0 0, 1 0)");
        assert!(geof_length(&[line]).is_none());
    }
}
