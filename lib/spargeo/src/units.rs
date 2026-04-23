//! Unit of measure helpers for GeoSPARQL extension functions.
//!
//! Several GeoSPARQL functions accept a units of measure IRI alongside their
//! geometry argument. This module centralises the OGC uom IRIs we support and
//! converts them into numeric factors so call sites can normalise their
//! results out of the natural SI base unit.
//!
//! The convention used here: every kind-specific conversion function returns
//! the number of base units contained in one target unit. For a measurement
//! `value` already expressed in the base unit, the value in the target unit
//! is `value / factor`.
//!
//! Base units used by this crate:
//!
//! * length base is the metre (see [`length_iri_to_metre_factor`]).
//! * angle base is the radian (see [`angle_iri_to_radian_factor`]).
//! * area base is the square metre (see [`area_iri_to_square_metre_factor`]).
//!
//! Each conversion function accepts both the canonical OGC British spellings
//! (`metre`, `kilometre`, `square_metre`, `square_kilometre`) and the US
//! spellings (`meter`, `kilometer`, `square_meter`, `square_kilometer`), so
//! we line up with Apache Jena's published constants.

use oxrdf::Term;
use oxrdf::vocab::xsd;

/// OGC uom IRI root shared by all supported units of measure.
const OGC_UOM_PREFIX: &str = "http://www.opengis.net/def/uom/OGC/1.0/";

/// Convert a length units of measure IRI into the number of metres it
/// represents.
///
/// Returns `None` for any IRI that is not a recognised OGC length unit.
pub fn length_iri_to_metre_factor(iri: &str) -> Option<f64> {
    let local = iri.strip_prefix(OGC_UOM_PREFIX)?;
    match local {
        "metre" | "meter" => Some(1.0),
        "kilometre" | "kilometer" => Some(1000.0),
        _ => None,
    }
}

/// Convert an angle units of measure IRI into the number of radians it
/// represents.
///
/// Returns `None` for any IRI that is not a recognised OGC angle unit.
#[expect(dead_code, reason = "Reserved for future angle measuring functions")]
pub fn angle_iri_to_radian_factor(iri: &str) -> Option<f64> {
    let local = iri.strip_prefix(OGC_UOM_PREFIX)?;
    match local {
        "radian" => Some(1.0),
        "degree" => Some(std::f64::consts::PI / 180.0),
        _ => None,
    }
}

/// Convert an area units of measure IRI into the number of square metres it
/// represents.
///
/// Returns `None` for any IRI that is not a recognised OGC area unit.
pub fn area_iri_to_square_metre_factor(iri: &str) -> Option<f64> {
    let local = iri.strip_prefix(OGC_UOM_PREFIX)?;
    match local {
        "square_metre" | "square_meter" => Some(1.0),
        "square_kilometre" | "square_kilometer" => Some(1_000_000.0),
        _ => None,
    }
}

/// Extract an OGC units of measure IRI from a SPARQL argument term.
///
/// The GeoSPARQL specification defines units arguments as `xsd:anyURI`
/// literals but implementations in the wild also pass them as plain
/// `NamedNode` terms. This helper accepts either shape. Literal terms must
/// carry the `xsd:anyURI` datatype; any other datatype is rejected.
pub fn extract_units_iri(term: &Term) -> Option<&str> {
    match term {
        Term::NamedNode(node) => Some(node.as_str()),
        Term::Literal(literal) if literal.datatype() == xsd::ANY_URI => Some(literal.value()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{Literal as OxLiteral, NamedNode};

    #[test]
    fn metre_is_base_length() {
        let factor = length_iri_to_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/metre")
            .expect("known length unit");
        assert_eq!(factor, 1.0);
    }

    #[test]
    fn meter_alias_matches_metre() {
        let metre =
            length_iri_to_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/metre").unwrap();
        let meter =
            length_iri_to_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/meter").unwrap();
        assert_eq!(metre, meter);
    }

    #[test]
    fn kilometre_is_one_thousand_metres() {
        let factor = length_iri_to_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/kilometre")
            .expect("known length unit");
        assert_eq!(factor, 1000.0);
    }

    #[test]
    fn kilometer_alias_matches_kilometre() {
        let kilometre =
            length_iri_to_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/kilometre").unwrap();
        let kilometer =
            length_iri_to_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/kilometer").unwrap();
        assert_eq!(kilometre, kilometer);
    }

    #[test]
    fn radian_is_base_angle() {
        let factor = angle_iri_to_radian_factor("http://www.opengis.net/def/uom/OGC/1.0/radian")
            .expect("known angle unit");
        assert_eq!(factor, 1.0);
    }

    #[test]
    fn degree_is_pi_over_one_eighty() {
        let factor = angle_iri_to_radian_factor("http://www.opengis.net/def/uom/OGC/1.0/degree")
            .expect("known angle unit");
        assert!((factor - std::f64::consts::PI / 180.0).abs() < 1e-15);
    }

    #[test]
    fn square_metre_is_base_area() {
        let factor =
            area_iri_to_square_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/square_metre")
                .expect("known area unit");
        assert_eq!(factor, 1.0);
    }

    #[test]
    fn square_meter_alias_matches_square_metre() {
        let metre =
            area_iri_to_square_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/square_metre")
                .unwrap();
        let meter =
            area_iri_to_square_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/square_meter")
                .unwrap();
        assert_eq!(metre, meter);
    }

    #[test]
    fn square_kilometre_is_one_million_square_metres() {
        let factor = area_iri_to_square_metre_factor(
            "http://www.opengis.net/def/uom/OGC/1.0/square_kilometre",
        )
        .expect("known area unit");
        assert_eq!(factor, 1_000_000.0);
    }

    #[test]
    fn length_iri_is_not_an_area_iri() {
        assert!(
            area_iri_to_square_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/metre",)
                .is_none()
        );
    }

    #[test]
    fn area_iri_is_not_a_length_iri() {
        assert!(
            length_iri_to_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/square_metre",)
                .is_none()
        );
    }

    #[test]
    fn angle_iri_is_not_a_length_iri() {
        assert!(
            length_iri_to_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/radian",).is_none()
        );
    }

    #[test]
    fn unknown_prefix_returns_none() {
        assert!(length_iri_to_metre_factor("http://example.org/uom/metre",).is_none());
    }

    #[test]
    fn unknown_local_name_returns_none() {
        assert!(
            length_iri_to_metre_factor("http://www.opengis.net/def/uom/OGC/1.0/parsec",).is_none()
        );
    }

    #[test]
    fn extract_units_iri_from_named_node() {
        let node = NamedNode::new_unchecked("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let term = Term::NamedNode(node);
        assert_eq!(
            extract_units_iri(&term),
            Some("http://www.opengis.net/def/uom/OGC/1.0/metre")
        );
    }

    #[test]
    fn extract_units_iri_from_any_uri_literal() {
        let lit = OxLiteral::new_typed_literal(
            "http://www.opengis.net/def/uom/OGC/1.0/kilometre",
            xsd::ANY_URI,
        );
        let term = Term::Literal(lit);
        assert_eq!(
            extract_units_iri(&term),
            Some("http://www.opengis.net/def/uom/OGC/1.0/kilometre")
        );
    }

    #[test]
    fn extract_units_iri_rejects_simple_literal() {
        let lit = OxLiteral::new_simple_literal("http://www.opengis.net/def/uom/OGC/1.0/metre");
        assert!(extract_units_iri(&Term::Literal(lit)).is_none());
    }

    #[test]
    fn extract_units_iri_rejects_string_typed_literal() {
        let lit = OxLiteral::new_typed_literal(
            "http://www.opengis.net/def/uom/OGC/1.0/metre",
            xsd::STRING,
        );
        assert!(extract_units_iri(&Term::Literal(lit)).is_none());
    }
}
