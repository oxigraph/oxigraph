#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod parse;
mod units;

pub mod vocab;

use crate::parse::{CRS84_URI, extract_argument, result_to_geojson_literal, result_to_wkt_literal};
use crate::units::{
    area_iri_to_square_metre_factor, extract_units_iri, length_iri_to_metre_factor,
};
use geo::algorithm::Validation;
use geo::coordinate_position::CoordPos;
use geo::dimensions::Dimensions;
use geo::{
    BooleanOps, BoundingRect, Centroid, ConvexHull, Distance, GeodesicArea, Geometry,
    HasDimensions, Haversine, Length, MultiPolygon, Point, Polygon, Relate,
};
use oxrdf::vocab::xsd;
use oxrdf::{Literal, NamedNodeRef, Term};

/// GeoSPARQL functions in name and implementation pairs
pub const GEOSPARQL_EXTENSION_FUNCTIONS: [(NamedNodeRef<'static>, fn(&[Term]) -> Option<Term>);
    43] = [
    (geosparql_functions::AREA, geof_area),
    (geosparql_functions::AS_GEO_JSON, geof_as_geojson),
    (geosparql_functions::CENTROID, geof_centroid),
    (geosparql_functions::CONVEX_HULL, geof_convex_hull),
    (
        geosparql_functions::COORDINATE_DIMENSION,
        geof_coordinate_dimension,
    ),
    (geosparql_functions::DIFFERENCE, geof_difference),
    (geosparql_functions::DIMENSION, geof_dimension),
    (geosparql_functions::DISTANCE, geof_distance),
    (geosparql_functions::EH_CONTAINS, geof_eh_contains),
    (geosparql_functions::EH_COVERED_BY, geof_eh_covered_by),
    (geosparql_functions::EH_COVERS, geof_eh_covers),
    (geosparql_functions::EH_DISJOINT, geof_eh_disjoint),
    (geosparql_functions::EH_EQUALS, geof_eh_equals),
    (geosparql_functions::EH_INSIDE, geof_eh_inside),
    (geosparql_functions::EH_MEET, geof_eh_meet),
    (geosparql_functions::EH_OVERLAP, geof_eh_overlap),
    (geosparql_functions::ENVELOPE, geof_envelope),
    (geosparql_functions::GET_SRID, geof_get_srid),
    (geosparql_functions::INTERSECTION, geof_intersection),
    (geosparql_functions::IS_EMPTY, geof_is_empty),
    (geosparql_functions::IS_SIMPLE, geof_is_simple),
    (geosparql_functions::LENGTH, geof_length),
    (geosparql_functions::PERIMETER, geof_perimeter),
    (geosparql_functions::RCC8_DC, geof_rcc8_dc),
    (geosparql_functions::RCC8_EC, geof_rcc8_ec),
    (geosparql_functions::RCC8_EQ, geof_rcc8_eq),
    (geosparql_functions::RCC8_NTPP, geof_rcc8_ntpp),
    (geosparql_functions::RCC8_NTPPI, geof_rcc8_ntppi),
    (geosparql_functions::RCC8_PO, geof_rcc8_po),
    (geosparql_functions::RCC8_TPP, geof_rcc8_tpp),
    (geosparql_functions::RCC8_TPPI, geof_rcc8_tppi),
    (geosparql_functions::RELATE, geof_relate),
    (geosparql_functions::SF_CONTAINS, geof_sf_contains),
    (geosparql_functions::SF_CROSSES, geof_sf_crosses),
    (geosparql_functions::SF_DISJOINT, geof_sf_disjoint),
    (geosparql_functions::SF_EQUALS, geof_sf_equals),
    (geosparql_functions::SF_INTERSECTS, geof_sf_intersects),
    (geosparql_functions::SF_OVERLAPS, geof_sf_overlaps),
    (geosparql_functions::SF_TOUCHES, geof_sf_touches),
    (geosparql_functions::SF_WITHIN, geof_sf_within),
    (
        geosparql_functions::SPATIAL_DIMENSION,
        geof_spatial_dimension,
    ),
    (geosparql_functions::SYM_DIFFERENCE, geof_sym_difference),
    (geosparql_functions::UNION, geof_union),
];

/// <http://www.opengis.net/def/function/geosparql/area>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:area>.
fn geof_area(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let units_iri = extract_units_iri(&args[1])?;
    let factor = area_iri_to_square_metre_factor(units_iri)?;
    let square_metres = geom.geodesic_area_unsigned();
    Some(Literal::from(square_metres / factor).into())
}

/// <http://www.opengis.net/def/function/geosparql/distance>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:distance>.
fn geof_distance(args: &[Term]) -> Option<Term> {
    let args: &[Term; 3] = args.try_into().ok()?;
    let left = extract_argument(&args[0])?;
    let right = extract_argument(&args[1])?;
    let units_iri = extract_units_iri(&args[2])?;
    let factor = length_iri_to_metre_factor(units_iri)?;
    let p1 = as_point(&left)?;
    let p2 = as_point(&right)?;
    let meters = Haversine.distance(p1, p2);
    Some(Literal::from(meters / factor).into())
}

fn as_point(geom: &Geometry) -> Option<Point> {
    match geom {
        Geometry::Point(p) => Some(*p),
        _ => None,
    }
}

/// Geometry literal datatype observed on an input argument. Used by geometry
/// returning functions to pick the matching output serialization so that WKT
/// inputs produce WKT outputs and GeoJSON inputs produce GeoJSON outputs.
#[derive(Copy, Clone)]
enum GeometryLiteralKind {
    Wkt,
    GeoJson,
}

fn detect_literal_kind(term: &Term) -> Option<GeometryLiteralKind> {
    let Term::Literal(literal) = term else {
        return None;
    };
    if literal.datatype() == geosparql::WKT_LITERAL {
        Some(GeometryLiteralKind::Wkt)
    } else if literal.datatype() == geosparql::GEO_JSON_LITERAL {
        Some(GeometryLiteralKind::GeoJson)
    } else {
        None
    }
}

/// Pick the output serialization format for a geometry returning function.
///
/// WKT inputs produce WKT output, GeoJSON inputs produce GeoJSON output, and
/// any mix (or any unrecognised datatype) falls back to WKT.
fn pick_output_kind(args: &[Term]) -> GeometryLiteralKind {
    let mut seen_geojson = false;
    for term in args {
        match detect_literal_kind(term) {
            Some(GeometryLiteralKind::Wkt) => return GeometryLiteralKind::Wkt,
            Some(GeometryLiteralKind::GeoJson) => seen_geojson = true,
            None => {}
        }
    }
    if seen_geojson {
        GeometryLiteralKind::GeoJson
    } else {
        GeometryLiteralKind::Wkt
    }
}

fn geometry_to_literal(geom: &Geometry, kind: GeometryLiteralKind) -> Literal {
    match kind {
        GeometryLiteralKind::Wkt => result_to_wkt_literal(geom),
        GeometryLiteralKind::GeoJson => result_to_geojson_literal(geom),
    }
}

/// <http://www.opengis.net/def/function/geosparql/length>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:length>.
fn geof_length(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let units_iri = extract_units_iri(&args[1])?;
    let factor = length_iri_to_metre_factor(units_iri)?;
    let metres = match geom {
        Geometry::Line(l) => Haversine.length(&l),
        Geometry::LineString(ls) => Haversine.length(&ls),
        Geometry::MultiLineString(mls) => Haversine.length(&mls),
        _ => 0.0,
    };
    Some(Literal::from(metres / factor).into())
}

/// <http://www.opengis.net/def/function/geosparql/envelope>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:envelope>.
fn geof_envelope(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let rect = geom.bounding_rect()?;
    Some(geometry_to_literal(&Geometry::Polygon(rect.to_polygon()), pick_output_kind(args)).into())
}

/// <http://www.opengis.net/def/function/geosparql/centroid>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:centroid>.
fn geof_centroid(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let point = geom.centroid()?;
    Some(geometry_to_literal(&Geometry::Point(point), pick_output_kind(args)).into())
}

/// <http://www.opengis.net/def/function/geosparql/convexHull>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:convexHull>.
fn geof_convex_hull(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let hull = geom.convex_hull();
    Some(geometry_to_literal(&Geometry::Polygon(hull), pick_output_kind(args)).into())
}

/// <http://www.opengis.net/def/function/geosparql/getSRID>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:getSRID>.
fn geof_get_srid(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    extract_argument(&args[0])?;
    Some(Literal::new_typed_literal(CRS84_URI, xsd::ANY_URI).into())
}

/// <http://www.opengis.net/def/function/geosparql/isEmpty>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:isEmpty>.
fn geof_is_empty(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(Literal::from(geom.is_empty()).into())
}

/// <http://www.opengis.net/def/function/geosparql/isSimple>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:isSimple>.
fn geof_is_simple(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(Literal::from(geom.is_valid()).into())
}

/// Map a `geo` dimensions value to the OGC integer code.
///
/// Follows the SFA convention of returning minus one for empty geometries.
fn dim_to_int(d: Dimensions) -> i64 {
    match d {
        Dimensions::Empty => -1,
        Dimensions::ZeroDimensional => 0,
        Dimensions::OneDimensional => 1,
        Dimensions::TwoDimensional => 2,
    }
}

/// <http://www.opengis.net/def/function/geosparql/dimension>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:dimension>.
fn geof_dimension(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(Literal::from(dim_to_int(geom.dimensions())).into())
}

/// <http://www.opengis.net/def/function/geosparql/coordinateDimension>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:coordinateDimension>.
fn geof_coordinate_dimension(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    extract_argument(&args[0])?;
    Some(Literal::from(2_i64).into())
}

/// <http://www.opengis.net/def/function/geosparql/spatialDimension>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:spatialDimension>.
fn geof_spatial_dimension(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(Literal::from(dim_to_int(geom.dimensions())).into())
}

/// <http://www.opengis.net/def/function/geosparql/asGeoJSON>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:asGeoJSON>.
fn geof_as_geojson(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(result_to_geojson_literal(&geom).into())
}

/// Extract a `Polygon` or `MultiPolygon` as a `MultiPolygon` for boolean
/// operations. Returns `None` for non polygonal geometries.
fn as_multi_polygon(geom: Geometry) -> Option<MultiPolygon> {
    match geom {
        Geometry::Polygon(p) => Some(MultiPolygon::new(vec![p])),
        Geometry::MultiPolygon(mp) => Some(mp),
        _ => None,
    }
}

/// <http://www.opengis.net/def/function/geosparql/intersection>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:intersection>.
fn geof_intersection(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let a = as_multi_polygon(extract_argument(&args[0])?)?;
    let b = as_multi_polygon(extract_argument(&args[1])?)?;
    let result = a.intersection(&b);
    Some(geometry_to_literal(&Geometry::MultiPolygon(result), pick_output_kind(args)).into())
}

/// <http://www.opengis.net/def/function/geosparql/union>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:union>.
fn geof_union(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let a = as_multi_polygon(extract_argument(&args[0])?)?;
    let b = as_multi_polygon(extract_argument(&args[1])?)?;
    let result = a.union(&b);
    Some(geometry_to_literal(&Geometry::MultiPolygon(result), pick_output_kind(args)).into())
}

/// <http://www.opengis.net/def/function/geosparql/difference>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:difference>.
fn geof_difference(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let a = as_multi_polygon(extract_argument(&args[0])?)?;
    let b = as_multi_polygon(extract_argument(&args[1])?)?;
    let result = a.difference(&b);
    Some(geometry_to_literal(&Geometry::MultiPolygon(result), pick_output_kind(args)).into())
}

/// <http://www.opengis.net/def/function/geosparql/symDifference>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:symDifference>.
fn geof_sym_difference(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let a = as_multi_polygon(extract_argument(&args[0])?)?;
    let b = as_multi_polygon(extract_argument(&args[1])?)?;
    let result = a.xor(&b);
    Some(geometry_to_literal(&Geometry::MultiPolygon(result), pick_output_kind(args)).into())
}

/// <http://www.opengis.net/def/function/geosparql/relate>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:relate>.
fn geof_relate(args: &[Term]) -> Option<Term> {
    let args: &[Term; 3] = args.try_into().ok()?;
    let a = extract_argument(&args[0])?;
    let b = extract_argument(&args[1])?;
    let Term::Literal(pattern) = &args[2] else {
        return None;
    };
    let matrix = a.relate(&b);
    matrix
        .matches(pattern.value())
        .ok()
        .map(|v| Literal::from(v).into())
}

/// <http://www.opengis.net/def/function/geosparql/perimeter>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:perimeter>.
fn geof_perimeter(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let units_iri = extract_units_iri(&args[1])?;
    let factor = length_iri_to_metre_factor(units_iri)?;
    let metres = polygonal_perimeter(&geom);
    Some(Literal::from(metres / factor).into())
}

fn polygonal_perimeter(geom: &Geometry) -> f64 {
    match geom {
        Geometry::Polygon(p) => polygon_perimeter(p),
        Geometry::MultiPolygon(mp) => mp.iter().map(polygon_perimeter).sum(),
        Geometry::Rect(r) => polygon_perimeter(&r.to_polygon()),
        Geometry::Triangle(t) => polygon_perimeter(&t.to_polygon()),
        _ => 0.0,
    }
}

fn polygon_perimeter(p: &Polygon) -> f64 {
    let mut total = Haversine.length(p.exterior());
    for interior in p.interiors() {
        total += Haversine.length(interior);
    }
    total
}

/// <http://www.opengis.net/def/function/geosparql/ehEquals>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:ehEquals>.
fn geof_eh_equals(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_equal_topo())
}

/// <http://www.opengis.net/def/function/geosparql/ehDisjoint>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:ehDisjoint>.
fn geof_eh_disjoint(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_disjoint())
}

/// <http://www.opengis.net/def/function/geosparql/ehMeet>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:ehMeet>.
fn geof_eh_meet(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_touches())
}

/// <http://www.opengis.net/def/function/geosparql/ehOverlap>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:ehOverlap>.
fn geof_eh_overlap(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_overlaps())
}

/// <http://www.opengis.net/def/function/geosparql/ehCovers>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:ehCovers>.
fn geof_eh_covers(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_covers())
}

/// <http://www.opengis.net/def/function/geosparql/ehCoveredBy>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:ehCoveredBy>.
fn geof_eh_covered_by(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_coveredby())
}

/// <http://www.opengis.net/def/function/geosparql/ehInside>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:ehInside>.
fn geof_eh_inside(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_within() && !m.is_equal_topo()
    })
}

/// <http://www.opengis.net/def/function/geosparql/ehContains>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:ehContains>.
fn geof_eh_contains(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_contains() && !m.is_equal_topo()
    })
}

/// <http://www.opengis.net/def/function/geosparql/rcc8eq>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:rcc8eq>.
fn geof_rcc8_eq(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_equal_topo())
}

/// <http://www.opengis.net/def/function/geosparql/rcc8dc>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:rcc8dc>.
fn geof_rcc8_dc(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_disjoint())
}

/// <http://www.opengis.net/def/function/geosparql/rcc8ec>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:rcc8ec>.
fn geof_rcc8_ec(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_touches())
}

/// <http://www.opengis.net/def/function/geosparql/rcc8po>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:rcc8po>.
fn geof_rcc8_po(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_overlaps())
}

fn boundaries_touch(matrix: &geo::relate::IntersectionMatrix) -> bool {
    matrix.get(CoordPos::OnBoundary, CoordPos::OnBoundary) != Dimensions::Empty
}

/// <http://www.opengis.net/def/function/geosparql/rcc8tpp>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:rcc8tpp>.
fn geof_rcc8_tpp(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_coveredby() && !m.is_equal_topo() && boundaries_touch(&m)
    })
}

/// <http://www.opengis.net/def/function/geosparql/rcc8ntpp>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:rcc8ntpp>.
fn geof_rcc8_ntpp(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_coveredby() && !m.is_equal_topo() && !boundaries_touch(&m)
    })
}

/// <http://www.opengis.net/def/function/geosparql/rcc8tppi>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:rcc8tppi>.
fn geof_rcc8_tppi(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_covers() && !m.is_equal_topo() && boundaries_touch(&m)
    })
}

/// <http://www.opengis.net/def/function/geosparql/rcc8ntppi>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:rcc8ntppi>.
fn geof_rcc8_ntppi(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_covers() && !m.is_equal_topo() && !boundaries_touch(&m)
    })
}

/// <http://www.opengis.net/def/function/geosparql/sfEquals>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:sfEquals>.
fn geof_sf_equals(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_equal_topo())
}

/// <http://www.opengis.net/def/function/geosparql/sfDisjoint>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:sfDisjoint>.
fn geof_sf_disjoint(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_disjoint())
}

/// <http://www.opengis.net/def/function/geosparql/sfIntersects>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:sfIntersects>.
fn geof_sf_intersects(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_intersects())
}

/// <http://www.opengis.net/def/function/geosparql/sfTouches>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:sfTouches>.
fn geof_sf_touches(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_touches())
}

/// <http://www.opengis.net/def/function/geosparql/sfCrosses>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:sfCrosses>.
fn geof_sf_crosses(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_crosses())
}

/// <http://www.opengis.net/def/function/geosparql/sfWithin>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:sfWithin>.
fn geof_sf_within(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_within())
}

/// <http://www.opengis.net/def/function/geosparql/sfContains>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:sfContains>.
fn geof_sf_contains(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_contains())
}

/// <http://www.opengis.net/def/function/geosparql/sfOverlaps>.
///
/// See <https://defs.opengis.net/prez/catalogs/ogc-cat:datamodels/col/catalog:geosparql/it1/function:geosparql/it2/ogcf:sfOverlaps>.
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
    pub const AS_GEO_JSON: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/asGeoJSON");
    pub const CENTROID: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/centroid");
    pub const CONVEX_HULL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/convexHull");
    pub const COORDINATE_DIMENSION: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.opengis.net/def/function/geosparql/coordinateDimension",
    );
    pub const DIFFERENCE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/difference");
    pub const DIMENSION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/dimension");
    pub const DISTANCE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/distance");
    pub const EH_CONTAINS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/ehContains");
    pub const EH_COVERED_BY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/ehCoveredBy");
    pub const EH_COVERS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/ehCovers");
    pub const EH_DISJOINT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/ehDisjoint");
    pub const EH_EQUALS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/ehEquals");
    pub const EH_INSIDE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/ehInside");
    pub const EH_MEET: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/ehMeet");
    pub const EH_OVERLAP: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/ehOverlap");
    pub const ENVELOPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/envelope");
    pub const GET_SRID: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/getSRID");
    pub const INTERSECTION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/intersection");
    pub const IS_EMPTY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/isEmpty");
    pub const IS_SIMPLE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/isSimple");
    pub const LENGTH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/length");
    pub const PERIMETER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/perimeter");
    pub const RCC8_DC: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/rcc8dc");
    pub const RCC8_EC: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/rcc8ec");
    pub const RCC8_EQ: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/rcc8eq");
    pub const RCC8_NTPP: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/rcc8ntpp");
    pub const RCC8_NTPPI: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/rcc8ntppi");
    pub const RCC8_PO: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/rcc8po");
    pub const RCC8_TPP: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/rcc8tpp");
    pub const RCC8_TPPI: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/rcc8tppi");
    pub const RELATE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/relate");
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
    pub const SPATIAL_DIMENSION: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.opengis.net/def/function/geosparql/spatialDimension",
    );
    pub const SYM_DIFFERENCE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/symDifference");
    pub const UNION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/union");
}
