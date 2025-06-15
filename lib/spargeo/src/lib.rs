#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

use geo::{Geometry, Relate};
use geojson::GeoJson;
use oxigraph::model::{Literal, NamedNodeRef, Term};
use oxigraph::sparql::SparqlEvaluator;
use spareval::QueryEvaluator;
use std::str::FromStr;
use wkt::TryFromWkt;

/// Registers GeoSPARQL extension functions in the [`SparqlEvaluator`]
pub fn register_geosparql_functions(evaluator: SparqlEvaluator) -> SparqlEvaluator {
    evaluator
        .with_custom_function(geosparql_functions::SF_EQUALS.into(), geof_sf_equals)
        .with_custom_function(geosparql_functions::SF_DISJOINT.into(), geof_sf_disjoint)
        .with_custom_function(
            geosparql_functions::SF_INTERSECTS.into(),
            geof_sf_intersects,
        )
        .with_custom_function(geosparql_functions::SF_TOUCHES.into(), geof_sf_touches)
        .with_custom_function(geosparql_functions::SF_CROSSES.into(), geof_sf_crosses)
        .with_custom_function(geosparql_functions::SF_WITHIN.into(), geof_sf_within)
        .with_custom_function(geosparql_functions::SF_CONTAINS.into(), geof_sf_contains)
        .with_custom_function(geosparql_functions::SF_OVERLAPS.into(), geof_sf_overlaps)
}

/// Registers GeoSPARQL extension functions in the [`QueryEvaluator`]
pub fn add_geosparql_functions(evaluator: QueryEvaluator) -> QueryEvaluator {
    evaluator
        .with_custom_function(geosparql_functions::SF_EQUALS.into(), geof_sf_equals)
        .with_custom_function(geosparql_functions::SF_DISJOINT.into(), geof_sf_disjoint)
        .with_custom_function(
            geosparql_functions::SF_INTERSECTS.into(),
            geof_sf_intersects,
        )
        .with_custom_function(geosparql_functions::SF_TOUCHES.into(), geof_sf_touches)
        .with_custom_function(geosparql_functions::SF_CROSSES.into(), geof_sf_crosses)
        .with_custom_function(geosparql_functions::SF_WITHIN.into(), geof_sf_within)
        .with_custom_function(geosparql_functions::SF_CONTAINS.into(), geof_sf_contains)
        .with_custom_function(geosparql_functions::SF_OVERLAPS.into(), geof_sf_overlaps)
}

/// List of GeoSPARQL functions supported and registered by [`register_geosparql_functions`]
pub const GEOSPARQL_EXTENSION_FUNCTIONS: [NamedNodeRef<'static>; 8] = [
    geosparql_functions::SF_EQUALS,
    geosparql_functions::SF_DISJOINT,
    geosparql_functions::SF_INTERSECTS,
    geosparql_functions::SF_TOUCHES,
    geosparql_functions::SF_CROSSES,
    geosparql_functions::SF_WITHIN,
    geosparql_functions::SF_CONTAINS,
    geosparql_functions::SF_OVERLAPS,
];

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

// Parse
fn extract_argument(term: &Term) -> Option<Geometry> {
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

// Parse a WKT literal including reference system http://www.opengis.net/def/crs/OGC/1.3/CRS84
fn parse_wkt_literal(value: &str) -> Option<Geometry> {
    let mut value = value.trim_start();
    if let Some(val) = value.strip_prefix('<') {
        // We have a reference system
        let (system, val) = val.split_once('>').unwrap_or((val, ""));
        if system != "http://www.opengis.net/def/crs/OGC/1.3/CRS84" {
            // We only support CRS84
            return None;
        }
        value = val.trim_start();
    }
    Geometry::try_from_wkt_str(value).ok()
}

fn parse_geo_json_literal(value: &str) -> Option<Geometry> {
    GeoJson::from_str(value).ok()?.try_into().ok()
}

mod geosparql {
    //! [GeoSpatial](https://opengeospatial.github.io/ogc-geosparql/) vocabulary.
    use oxigraph::model::NamedNodeRef;

    pub const GEO_JSON_LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#geoJSONLiteral");
    pub const WKT_LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#wktLiteral");
}

mod geosparql_functions {
    //! [GeoSpatial](https://opengeospatial.github.io/ogc-geosparql/) functions vocabulary.
    use oxigraph::model::NamedNodeRef;

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
