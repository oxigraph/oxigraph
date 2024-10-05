#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

use geo::{Contains, Geometry, Within};
use geojson::GeoJson;
use oxigraph::model::{Literal, NamedNodeRef, Term};
use oxigraph::sparql::QueryOptions;
use std::str::FromStr;
use wkt::TryFromWkt;

/// Registers GeoSPARQL extension functions in the [`QueryOptions`]
pub fn register_geosparql_functions(options: QueryOptions) -> QueryOptions {
    options
        .with_custom_function(geosparql_functions::SF_EQUALS.into(), geof_sf_equals)
        .with_custom_function(geosparql_functions::SF_CONTAINS.into(), geof_sf_contains)
        .with_custom_function(geosparql_functions::SF_WITHIN.into(), geof_sf_within)
}

/// List of GeoSPARQL functions supported and registered by [`register_geosparql_functions`]
pub const GEOSPARQL_EXTENSION_FUNCTIONS: [NamedNodeRef<'static>; 3] = [
    geosparql_functions::SF_EQUALS,
    geosparql_functions::SF_CONTAINS,
    geosparql_functions::SF_WITHIN,
];

fn geof_sf_equals(args: &[Term]) -> Option<Term> {
    binary_boolean_geo_fn(args, |a, b| a == b)
}

fn geof_sf_contains(args: &[Term]) -> Option<Term> {
    binary_boolean_geo_fn(args, |a, b| a.contains(&b))
}

fn geof_sf_within(args: &[Term]) -> Option<Term> {
    binary_boolean_geo_fn(args, |a, b| a.is_within(&b))
}

fn binary_boolean_geo_fn(
    args: &[Term],
    operation: impl FnOnce(Geometry, Geometry) -> bool,
) -> Option<Term> {
    let args: &[Term; 2] = args.try_into().ok()?;
    let left = extract_argument(&args[0])?;
    let right = extract_argument(&args[1])?;
    Some(Literal::from(operation(left, right)).into())
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

    pub const SF_EQUALS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfEquals");
    pub const SF_CONTAINS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfContains");
    pub const SF_WITHIN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/def/function/geosparql/sfWithin");
}
