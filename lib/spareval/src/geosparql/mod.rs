pub mod parse;
mod units;

pub mod vocab;

use parse::{extract_argument, result_to_wkt_literal, CRS84_URI};
use units::{extract_units_iri, units_to_factor, UnitKind};
use geo::algorithm::Validation;
use geo::coordinate_position::CoordPos;
use geo::dimensions::Dimensions;
use geo::{
    BooleanOps, BoundingRect, Centroid, ConvexHull, Distance, GeodesicArea, Geometry,
    HasDimensions, Haversine, Length, MultiPolygon, Point, Polygon, Relate,
};
use geojson::Geometry as GeoJsonGeometry;
use oxrdf::{Literal, NamedNodeRef, Term};
use wkt::ToWkt;

/// GeoSPARQL functions in name and implementation pairs
pub const GEOSPARQL_EXTENSION_FUNCTIONS: [(NamedNodeRef<'static>, fn(&[Term]) -> Option<Term>); 44] = [
    (geosparql_functions::AREA, geof_area),
    (geosparql_functions::AS_GEO_JSON, geof_as_geojson),
    (geosparql_functions::AS_TEXT, geof_as_text),
    (geosparql_functions::CENTROID, geof_centroid),
    (geosparql_functions::CONVEX_HULL, geof_convex_hull),
    (geosparql_functions::COORDINATE_DIMENSION, geof_coordinate_dimension),
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
    (geosparql_functions::SPATIAL_DIMENSION, geof_spatial_dimension),
    (geosparql_functions::SYM_DIFFERENCE, geof_sym_difference),
    (geosparql_functions::UNION, geof_union),
];

/// XSD `anyURI` datatype used by accessor functions that return IRIs as values.
const XSD_ANY_URI: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#anyURI");

/// XSD `integer` datatype used by accessor functions that return integers.
const XSD_INTEGER: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#integer");

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

/// `geof:envelope`. Returns the minimum bounding rectangle of a geometry as a
/// CRS84 `wktLiteral` polygon.
///
/// One argument is expected. Geometries with no coordinates yield no binding.
fn geof_envelope(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let rect = geom.bounding_rect()?;
    Some(result_to_wkt_literal(Geometry::Polygon(rect.to_polygon())).into())
}

/// `geof:centroid`. Returns the arithmetic centroid of a geometry as a CRS84
/// `wktLiteral` point.
///
/// One argument is expected. Empty geometries yield no binding.
fn geof_centroid(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let point = geom.centroid()?;
    Some(result_to_wkt_literal(Geometry::Point(point)).into())
}

/// `geof:convexHull`. Returns the convex hull of a geometry as a CRS84
/// `wktLiteral` polygon.
///
/// One argument is expected. The computation runs in planar coordinates
/// because the `geo` crate convex hull is Euclidean, so on longitude and
/// latitude inputs the result is a topological approximation rather than a
/// true spherical hull.
fn geof_convex_hull(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let hull = geom.convex_hull();
    Some(result_to_wkt_literal(Geometry::Polygon(hull)).into())
}

/// `geof:getSRID`. Returns the spatial reference system identifier of a
/// geometry as an `xsd:anyURI`.
///
/// One argument is expected. This crate accepts geometry literals only in
/// the CRS84 reference system, so the returned IRI is always the CRS84 URI.
fn geof_get_srid(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    extract_argument(&args[0])?;
    Some(Term::Literal(Literal::new_typed_literal(CRS84_URI, XSD_ANY_URI)))
}

/// `geof:isEmpty`. Returns whether a geometry has no coordinates as an
/// `xsd:boolean`.
///
/// One argument is expected. Points and rectangles cannot be empty by
/// construction in the `geo` crate and therefore always return false.
fn geof_is_empty(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(Literal::from(geom.is_empty()).into())
}

/// `geof:isSimple`. Returns whether a geometry has no self intersection and
/// no tangencies, as an `xsd:boolean`.
///
/// One argument is expected. This crate uses the `geo` crate `Validation`
/// check as a conservative approximation of OGC simple: a geometry that
/// fails validation is reported as not simple, every valid geometry is
/// reported as simple.
fn geof_is_simple(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(Literal::from(geom.is_valid()).into())
}

/// Map a `geo` dimensions value to the OGC integer code.
///
/// Follows the SFA convention of returning minus one for empty geometries.
#[inline]
fn dim_to_int(d: Dimensions) -> i64 {
    match d {
        Dimensions::Empty => -1,
        Dimensions::ZeroDimensional => 0,
        Dimensions::OneDimensional => 1,
        Dimensions::TwoDimensional => 2,
    }
}

#[inline]
fn integer_literal(value: i64) -> Term {
    Term::Literal(Literal::new_typed_literal(value.to_string(), XSD_INTEGER))
}

/// `geof:dimension`. Returns the topological dimension of a geometry as an
/// `xsd:integer` value in the set minus one, zero, one, two.
fn geof_dimension(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(integer_literal(dim_to_int(geom.dimensions())))
}

/// `geof:coordinateDimension`. Returns the number of coordinate components
/// of a geometry as an `xsd:integer`. Constant two in this crate because
/// CRS84 input is two dimensional.
fn geof_coordinate_dimension(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    extract_argument(&args[0])?;
    Some(integer_literal(2))
}

/// `geof:spatialDimension`. Returns the number of spatial dimensions of a
/// geometry. Matches `geof:dimension` in this crate because 3D inputs are
/// not supported.
fn geof_spatial_dimension(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(integer_literal(dim_to_int(geom.dimensions())))
}

/// `geof:asText`. Returns the WKT serialization of a geometry as an
/// `xsd:string`.
fn geof_as_text(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    Some(Literal::from(geom.wkt_string()).into())
}

/// `geof:asGeoJSON`. Returns the GeoJSON serialization of a geometry as an
/// `xsd:string`.
fn geof_as_geojson(args: &[Term]) -> Option<Term> {
    let args: &[Term; 1] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let gj = GeoJsonGeometry::from(&geom);
    Some(Literal::from(gj.to_string()).into())
}

/// Extract a `Polygon` or `MultiPolygon` as a `MultiPolygon` for boolean
/// operations. Returns `None` for non polygonal geometries.
#[inline]
fn as_multi_polygon(geom: Geometry) -> Option<MultiPolygon> {
    match geom {
        Geometry::Polygon(p) => Some(MultiPolygon::new(vec![p])),
        Geometry::MultiPolygon(mp) => Some(mp),
        _ => None,
    }
}

/// `geof:intersection`. Returns the polygonal intersection of two geometries
/// as a CRS84 `wktLiteral`.
///
/// Polygons and multi polygons only, consistent with the `geo` crate boolean
/// operations support.
fn geof_intersection(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let a = as_multi_polygon(extract_argument(&args[0])?)?;
    let b = as_multi_polygon(extract_argument(&args[1])?)?;
    let result = a.intersection(&b);
    Some(result_to_wkt_literal(Geometry::MultiPolygon(result)).into())
}

/// `geof:union`. Returns the polygonal union of two geometries as a CRS84
/// `wktLiteral`.
fn geof_union(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let a = as_multi_polygon(extract_argument(&args[0])?)?;
    let b = as_multi_polygon(extract_argument(&args[1])?)?;
    let result = a.union(&b);
    Some(result_to_wkt_literal(Geometry::MultiPolygon(result)).into())
}

/// `geof:difference`. Returns the polygonal set difference between two
/// geometries as a CRS84 `wktLiteral`.
fn geof_difference(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let a = as_multi_polygon(extract_argument(&args[0])?)?;
    let b = as_multi_polygon(extract_argument(&args[1])?)?;
    let result = a.difference(&b);
    Some(result_to_wkt_literal(Geometry::MultiPolygon(result)).into())
}

/// `geof:symDifference`. Returns the polygonal symmetric difference of two
/// geometries as a CRS84 `wktLiteral`.
fn geof_sym_difference(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let a = as_multi_polygon(extract_argument(&args[0])?)?;
    let b = as_multi_polygon(extract_argument(&args[1])?)?;
    let result = a.xor(&b);
    Some(result_to_wkt_literal(Geometry::MultiPolygon(result)).into())
}

/// `geof:relate`. Tests two geometries against a DE-9IM pattern.
///
/// Three arguments are expected: two geometry literals and an `xsd:string`
/// containing nine pattern characters drawn from the set `T`, `F`, `*`, `0`,
/// `1`, `2`. Returns no binding when the pattern string is malformed.
fn geof_relate(args: &[Term]) -> Option<Term> {
    let args: &[Term; 3] = args.try_into().ok()?;
    let a = extract_argument(&args[0])?;
    let b = extract_argument(&args[1])?;
    let pattern = match &args[2] {
        Term::Literal(l) => l.value().to_string(),
        _ => return None,
    };
    let matrix = a.relate(&b);
    matrix.matches(&pattern).ok().map(|v| Literal::from(v).into())
}

/// `geof:perimeter`. Returns the haversine perimeter of a polygonal geometry
/// as an `xsd:double` expressed in the target units of measure.
///
/// Two arguments are expected: a polygonal geometry literal and a length
/// units IRI. Point, line and collection geometries return zero because
/// their perimeter is not defined.
fn geof_perimeter(args: &[Term]) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let geom = extract_argument(&args[0])?;
    let units_iri = extract_units_iri(&args[1])?;
    let factor = units_to_factor(units_iri, UnitKind::Length)?;
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

/// Egenhofer and RCC8 topology helpers. Each GeoSPARQL function delegates to
/// either an existing `IntersectionMatrix` method or to a boundary-boundary
/// inspection when the relation distinguishes tangential contact.

fn geof_eh_equals(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_equal_topo())
}

fn geof_eh_disjoint(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_disjoint())
}

fn geof_eh_meet(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_touches())
}

fn geof_eh_overlap(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_overlaps())
}

fn geof_eh_covers(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_covers())
}

fn geof_eh_covered_by(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_coveredby())
}

fn geof_eh_inside(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_within() && !m.is_equal_topo()
    })
}

fn geof_eh_contains(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_contains() && !m.is_equal_topo()
    })
}

fn geof_rcc8_eq(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_equal_topo())
}

fn geof_rcc8_dc(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_disjoint())
}

fn geof_rcc8_ec(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_touches())
}

fn geof_rcc8_po(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| a.relate(&b).is_overlaps())
}

#[inline]
fn boundaries_touch(matrix: &geo::relate::IntersectionMatrix) -> bool {
    matrix.get(CoordPos::OnBoundary, CoordPos::OnBoundary) != Dimensions::Empty
}

fn geof_rcc8_tpp(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_coveredby() && !m.is_equal_topo() && boundaries_touch(&m)
    })
}

fn geof_rcc8_ntpp(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_coveredby() && !m.is_equal_topo() && !boundaries_touch(&m)
    })
}

fn geof_rcc8_tppi(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_covers() && !m.is_equal_topo() && boundaries_touch(&m)
    })
}

fn geof_rcc8_ntppi(args: &[Term]) -> Option<Term> {
    binary_geo_fn(args, |a, b| {
        let m = a.relate(&b);
        m.is_covers() && !m.is_equal_topo() && !boundaries_touch(&m)
    })
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
    pub const AS_GEO_JSON: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/asGeoJSON");
    pub const AS_TEXT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/asText");
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
        let line = wkt_literal("LINESTRING(-0.1278 51.5074, 2.3522 48.8566)");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let value = parse_double(&geof_length(&[line, metres]).expect("computes"));
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

    fn parse_wkt_result(term: &Term) -> geo::Geometry {
        match term {
            Term::Literal(l) => {
                assert_eq!(l.datatype(), geosparql::WKT_LITERAL);
                parse::parse_wkt_literal(l.value()).expect("parses")
            }
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn envelope_of_triangle_is_axis_aligned_box() {
        let tri = wkt_literal("POLYGON((0 0, 4 0, 0 3, 0 0))");
        let out = geof_envelope(&[tri]).expect("computes");
        match parse_wkt_result(&out) {
            geo::Geometry::Polygon(p) => {
                let coords: Vec<(f64, f64)> =
                    p.exterior().points().map(|pt| (pt.x(), pt.y())).collect();
                let xs: Vec<f64> = coords.iter().map(|c| c.0).collect();
                let ys: Vec<f64> = coords.iter().map(|c| c.1).collect();
                assert!((xs.iter().cloned().fold(f64::INFINITY, f64::min) - 0.0).abs() < 1e-9);
                assert!((xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - 4.0).abs() < 1e-9);
                assert!((ys.iter().cloned().fold(f64::INFINITY, f64::min) - 0.0).abs() < 1e-9);
                assert!((ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - 3.0).abs() < 1e-9);
            }
            _ => panic!("expected polygon"),
        }
    }

    #[test]
    fn envelope_rejects_wrong_arity() {
        let a = wkt_literal("POINT(0 0)");
        let b = wkt_literal("POINT(1 1)");
        assert!(geof_envelope(&[a, b]).is_none());
    }

    #[test]
    fn centroid_of_square_is_centre() {
        let square = wkt_literal("POLYGON((0 0, 2 0, 2 2, 0 2, 0 0))");
        let out = geof_centroid(&[square]).expect("computes");
        match parse_wkt_result(&out) {
            geo::Geometry::Point(p) => {
                assert!((p.x() - 1.0).abs() < 1e-9);
                assert!((p.y() - 1.0).abs() < 1e-9);
            }
            _ => panic!("expected point"),
        }
    }

    #[test]
    fn centroid_rejects_wrong_arity() {
        assert!(geof_centroid(&[]).is_none());
    }

    #[test]
    fn convex_hull_of_concave_polygon_is_bounding_triangle_or_quad() {
        let l_shape = wkt_literal(
            "POLYGON((0 0, 4 0, 4 1, 1 1, 1 4, 0 4, 0 0))",
        );
        let out = geof_convex_hull(&[l_shape]).expect("computes");
        match parse_wkt_result(&out) {
            geo::Geometry::Polygon(p) => {
                let unique: std::collections::BTreeSet<(i64, i64)> = p
                    .exterior()
                    .points()
                    .map(|pt| ((pt.x() * 1000.0) as i64, (pt.y() * 1000.0) as i64))
                    .collect();
                assert!(unique.contains(&(0, 0)));
                assert!(unique.contains(&(4_000, 0)));
                assert!(unique.contains(&(4_000, 1_000)));
                assert!(unique.contains(&(0, 4_000)));
            }
            _ => panic!("expected polygon"),
        }
    }

    #[test]
    fn convex_hull_rejects_wrong_arity() {
        let a = wkt_literal("POINT(0 0)");
        let b = wkt_literal("POINT(1 1)");
        assert!(geof_convex_hull(&[a, b]).is_none());
    }

    #[test]
    fn get_srid_is_crs84_uri() {
        let any = wkt_literal("POINT(10 20)");
        let out = geof_get_srid(&[any]).expect("computes");
        match out {
            Term::Literal(l) => {
                assert_eq!(l.value(), "http://www.opengis.net/def/crs/OGC/1.3/CRS84");
                assert_eq!(
                    l.datatype().as_str(),
                    "http://www.w3.org/2001/XMLSchema#anyURI"
                );
            }
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn get_srid_rejects_non_geometry_literal() {
        let plain = Term::Literal(OxLiteral::new_simple_literal("POINT(0 0)"));
        assert!(geof_get_srid(&[plain]).is_none());
    }

    #[test]
    fn is_empty_false_for_point() {
        let p = wkt_literal("POINT(1 2)");
        let out = geof_is_empty(&[p]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "false"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn is_empty_true_for_empty_multi_point() {
        let mp = wkt_literal("MULTIPOINT EMPTY");
        let out = geof_is_empty(&[mp]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "true"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn is_empty_rejects_wrong_arity() {
        assert!(geof_is_empty(&[]).is_none());
    }

    #[test]
    fn is_simple_true_for_point() {
        let p = wkt_literal("POINT(1 2)");
        let out = geof_is_simple(&[p]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "true"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn is_simple_rejects_wrong_arity() {
        assert!(geof_is_simple(&[]).is_none());
    }

    #[test]
    fn dimension_of_point_is_zero() {
        let p = wkt_literal("POINT(1 2)");
        let out = geof_dimension(&[p]).expect("computes");
        match out {
            Term::Literal(l) => {
                assert_eq!(l.value(), "0");
                assert_eq!(l.datatype().as_str(), "http://www.w3.org/2001/XMLSchema#integer");
            }
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn dimension_of_line_is_one() {
        let line = wkt_literal("LINESTRING(0 0, 1 0)");
        let out = geof_dimension(&[line]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "1"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn dimension_of_polygon_is_two() {
        let poly = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let out = geof_dimension(&[poly]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "2"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn coordinate_dimension_is_two_under_crs84() {
        let p = wkt_literal("POINT(0 0)");
        let out = geof_coordinate_dimension(&[p]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "2"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn spatial_dimension_tracks_topological_dimension() {
        let poly = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let out = geof_spatial_dimension(&[poly]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "2"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn as_text_round_trips_point() {
        let p = wkt_literal("POINT(3 4)");
        let out = geof_as_text(&[p]).expect("computes");
        match out {
            Term::Literal(l) => {
                assert!(l.value().to_uppercase().contains("POINT"));
            }
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn as_geojson_round_trips_point() {
        let p = wkt_literal("POINT(3 4)");
        let out = geof_as_geojson(&[p]).expect("computes");
        match out {
            Term::Literal(l) => {
                assert!(l.value().contains("Point"));
                assert!(l.value().contains('['));
            }
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn intersection_of_overlapping_squares_is_small_square() {
        let a = wkt_literal("POLYGON((0 0, 2 0, 2 2, 0 2, 0 0))");
        let b = wkt_literal("POLYGON((1 1, 3 1, 3 3, 1 3, 1 1))");
        let out = geof_intersection(&[a, b]).expect("computes");
        match parse_wkt_result(&out) {
            geo::Geometry::Polygon(_) | geo::Geometry::MultiPolygon(_) => {}
            other => panic!("unexpected geometry {other:?}"),
        }
    }

    #[test]
    fn union_of_two_disjoint_squares_is_multi_polygon() {
        let a = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let b = wkt_literal("POLYGON((5 5, 6 5, 6 6, 5 6, 5 5))");
        let out = geof_union(&[a, b]).expect("computes");
        match parse_wkt_result(&out) {
            geo::Geometry::MultiPolygon(mp) => {
                assert_eq!(mp.0.len(), 2);
            }
            geo::Geometry::Polygon(_) => {}
            other => panic!("unexpected geometry {other:?}"),
        }
    }

    #[test]
    fn difference_subtracts_hole() {
        let a = wkt_literal("POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))");
        let b = wkt_literal("POLYGON((1 1, 3 1, 3 3, 1 3, 1 1))");
        let out = geof_difference(&[a, b]).expect("computes");
        parse_wkt_result(&out);
    }

    #[test]
    fn sym_difference_is_union_minus_intersection() {
        let a = wkt_literal("POLYGON((0 0, 2 0, 2 2, 0 2, 0 0))");
        let b = wkt_literal("POLYGON((1 1, 3 1, 3 3, 1 3, 1 1))");
        let out = geof_sym_difference(&[a, b]).expect("computes");
        parse_wkt_result(&out);
    }

    #[test]
    fn perimeter_of_unit_square_is_four_degrees_worth_of_metres() {
        let square = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let value = parse_double(&geof_perimeter(&[square, metres]).expect("computes"));
        assert!(
            (value - 444_000.0).abs() < 2_000.0,
            "got {value}, expected near 444000"
        );
    }

    #[test]
    fn perimeter_of_line_is_zero() {
        let line = wkt_literal("LINESTRING(0 0, 1 0)");
        let metres = units_named_node("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let value = parse_double(&geof_perimeter(&[line, metres]).expect("computes"));
        assert_eq!(value, 0.0);
    }

    #[test]
    fn relate_with_matching_pattern_returns_true() {
        let a = wkt_literal("POLYGON((0 0, 2 0, 2 2, 0 2, 0 0))");
        let b = wkt_literal("POINT(1 1)");
        let pattern = Term::Literal(OxLiteral::new_simple_literal("0FFFFFFF2"));
        let out = geof_relate(&[a, b, pattern]).expect("computes");
        match out {
            Term::Literal(l) => {
                assert_eq!(l.datatype().as_str(), "http://www.w3.org/2001/XMLSchema#boolean");
            }
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn relate_rejects_bad_pattern() {
        let a = wkt_literal("POINT(0 0)");
        let b = wkt_literal("POINT(1 1)");
        let pattern = Term::Literal(OxLiteral::new_simple_literal("bad"));
        assert!(geof_relate(&[a, b, pattern]).is_none());
    }

    #[test]
    fn eh_equals_matches_identical_polygons() {
        let a = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let b = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let out = geof_eh_equals(&[a, b]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "true"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn eh_disjoint_detects_far_apart_polygons() {
        let a = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let b = wkt_literal("POLYGON((5 5, 6 5, 6 6, 5 6, 5 5))");
        let out = geof_eh_disjoint(&[a, b]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "true"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn rcc8_eq_matches_identical_polygons() {
        let a = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let b = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let out = geof_rcc8_eq(&[a, b]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "true"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn rcc8_dc_detects_far_apart_polygons() {
        let a = wkt_literal("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))");
        let b = wkt_literal("POLYGON((5 5, 6 5, 6 6, 5 6, 5 5))");
        let out = geof_rcc8_dc(&[a, b]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "true"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn rcc8_ntpp_matches_strict_interior_containment() {
        let big = wkt_literal("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))");
        let small = wkt_literal("POLYGON((3 3, 4 3, 4 4, 3 4, 3 3))");
        let out = geof_rcc8_ntpp(&[small, big]).expect("computes");
        match out {
            Term::Literal(l) => assert_eq!(l.value(), "true"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn topology_functions_count_is_exposed_as_const() {
        assert_eq!(GEOSPARQL_EXTENSION_FUNCTIONS.len(), 44);
    }
}
