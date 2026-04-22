//! Unit of measure helpers for GeoSPARQL extension functions.
//!
//! Several GeoSPARQL functions accept a units of measure IRI alongside their
//! geometry argument. This module centralises the OGC uom IRIs we support and
//! converts them into numeric factors so call sites can normalise their
//! results out of the natural SI base unit.
//!
//! The convention used here: [`units_to_factor`] returns the number of base
//! units contained in one target unit. For a measurement `value` already
//! expressed in the base unit, the value in the target unit is
//! `value / factor`.
//!
//! Base units used by this crate:
//!
//! * [`UnitKind::Length`] base is the metre.
//! * [`UnitKind::Angle`] base is the radian.
//! * [`UnitKind::Area`] base is the square metre.

use oxrdf::Term;

/// Category of a units of measure IRI.
///
/// Each GeoSPARQL extension function that takes a units argument is
/// interested in a specific kind of quantity. Passing the [`UnitKind`]
/// explicitly prevents accidentally mixing, for example, a length IRI with
/// an area measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitKind {
    /// Linear distance, base unit metre.
    Length,
    /// Angular measure, base unit radian.
    Angle,
    /// Planar area, base unit square metre.
    Area,
}

/// OGC uom IRI root shared by all supported units of measure.
const OGC_UOM_PREFIX: &str = "http://www.opengis.net/def/uom/OGC/1.0/";

/// Convert an OGC units of measure IRI into a conversion factor.
///
/// Returns the number of base units contained in one target unit, where the
/// base unit depends on [`UnitKind`]. Returns `None` when the IRI is not
/// recognised for the requested kind, which lets callers reject bad
/// arguments without panicking.
#[inline]
pub fn units_to_factor(iri: &str, kind: UnitKind) -> Option<f64> {
    let local = iri.strip_prefix(OGC_UOM_PREFIX)?;
    match kind {
        UnitKind::Length => match local {
            "metre" => Some(1.0),
            "kilometre" => Some(1000.0),
            _ => None,
        },
        UnitKind::Angle => match local {
            "radian" => Some(1.0),
            "degree" => Some(std::f64::consts::PI / 180.0),
            _ => None,
        },
        UnitKind::Area => match local {
            "square_metre" => Some(1.0),
            "square_kilometre" => Some(1_000_000.0),
            _ => None,
        },
    }
}

/// Extract an OGC units of measure IRI from a SPARQL argument term.
///
/// The GeoSPARQL specification defines units arguments as `xsd:anyURI`
/// but implementations in the wild also pass them as plain `NamedNode`
/// terms. This helper accepts either shape and returns the IRI string.
#[inline]
pub fn extract_units_iri(term: &Term) -> Option<&str> {
    match term {
        Term::NamedNode(node) => Some(node.as_str()),
        Term::Literal(literal) => Some(literal.value()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{Literal as OxLiteral, NamedNode};

    #[test]
    fn metre_is_base_length() {
        let factor = units_to_factor(
            "http://www.opengis.net/def/uom/OGC/1.0/metre",
            UnitKind::Length,
        )
        .expect("known length unit");
        assert_eq!(factor, 1.0);
    }

    #[test]
    fn kilometre_is_one_thousand_metres() {
        let factor = units_to_factor(
            "http://www.opengis.net/def/uom/OGC/1.0/kilometre",
            UnitKind::Length,
        )
        .expect("known length unit");
        assert_eq!(factor, 1000.0);
    }

    #[test]
    fn radian_is_base_angle() {
        let factor = units_to_factor(
            "http://www.opengis.net/def/uom/OGC/1.0/radian",
            UnitKind::Angle,
        )
        .expect("known angle unit");
        assert_eq!(factor, 1.0);
    }

    #[test]
    fn degree_is_pi_over_one_eighty() {
        let factor = units_to_factor(
            "http://www.opengis.net/def/uom/OGC/1.0/degree",
            UnitKind::Angle,
        )
        .expect("known angle unit");
        assert!((factor - std::f64::consts::PI / 180.0).abs() < 1e-15);
    }

    #[test]
    fn square_metre_is_base_area() {
        let factor = units_to_factor(
            "http://www.opengis.net/def/uom/OGC/1.0/square_metre",
            UnitKind::Area,
        )
        .expect("known area unit");
        assert_eq!(factor, 1.0);
    }

    #[test]
    fn square_kilometre_is_one_million_square_metres() {
        let factor = units_to_factor(
            "http://www.opengis.net/def/uom/OGC/1.0/square_kilometre",
            UnitKind::Area,
        )
        .expect("known area unit");
        assert_eq!(factor, 1_000_000.0);
    }

    #[test]
    fn length_iri_is_not_an_area_iri() {
        assert!(
            units_to_factor(
                "http://www.opengis.net/def/uom/OGC/1.0/metre",
                UnitKind::Area,
            )
            .is_none()
        );
    }

    #[test]
    fn area_iri_is_not_a_length_iri() {
        assert!(
            units_to_factor(
                "http://www.opengis.net/def/uom/OGC/1.0/square_metre",
                UnitKind::Length,
            )
            .is_none()
        );
    }

    #[test]
    fn angle_iri_is_not_a_length_iri() {
        assert!(
            units_to_factor(
                "http://www.opengis.net/def/uom/OGC/1.0/radian",
                UnitKind::Length,
            )
            .is_none()
        );
    }

    #[test]
    fn unknown_prefix_returns_none() {
        assert!(
            units_to_factor(
                "http://example.org/uom/metre",
                UnitKind::Length,
            )
            .is_none()
        );
    }

    #[test]
    fn unknown_local_name_returns_none() {
        assert!(
            units_to_factor(
                "http://www.opengis.net/def/uom/OGC/1.0/parsec",
                UnitKind::Length,
            )
            .is_none()
        );
    }

    #[test]
    fn extract_units_iri_from_named_node() {
        let node =
            NamedNode::new_unchecked("http://www.opengis.net/def/uom/OGC/1.0/metre");
        let term = Term::NamedNode(node);
        assert_eq!(
            extract_units_iri(&term),
            Some("http://www.opengis.net/def/uom/OGC/1.0/metre")
        );
    }

    #[test]
    fn extract_units_iri_from_literal() {
        let lit =
            OxLiteral::new_simple_literal("http://www.opengis.net/def/uom/OGC/1.0/kilometre");
        let term = Term::Literal(lit);
        assert_eq!(
            extract_units_iri(&term),
            Some("http://www.opengis.net/def/uom/OGC/1.0/kilometre")
        );
    }
}
