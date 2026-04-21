//! WKB serialization boundary for geometry literals.
//!
//! Wraps the [`wkb`] crate so the rest of the storage layer does not bind
//! directly to its API. Two directions are supported:
//!
//! * [`encode_wkt_value`] parses a WKT lexical value through `wkt::TryFromWkt`
//!   and emits the corresponding WKB bytes. Returns `None` for malformed
//!   WKT so the caller can fall back to the generic typed-literal path
//!   instead of silently dropping the input.
//! * [`decode_wkb_to_geometry`] and [`decode_wkb_to_wkt`] roundtrip stored
//!   bytes back to a `geo::Geometry` (fast path for spargeo extension
//!   functions) or to a canonical WKT string (for user-facing `Literal`
//!   reads).
//!
//! All encodings use little-endian byte order and 2D XY dimensions, which
//! matches the CRS84 assumption the `spargeo` crate already makes.

use geo::Geometry;
use geo_traits::to_geo::ToGeoGeometry;
use wkb::Endianness;
use wkb::reader::read_wkb;
use wkb::writer::write_geometry;
use wkt::{ToWkt, TryFromWkt};

/// Parse a WKT lexical value and return the equivalent WKB bytes.
///
/// Returns `None` when the lexical form does not parse as WKT, leaving
/// it to the caller to fall back to a generic encoding so malformed
/// input is not silently lost.
pub(crate) fn encode_wkt_value(value: &str) -> Option<Vec<u8>> {
    let geom: Geometry<f64> = Geometry::try_from_wkt_str(value).ok()?;
    let mut buf = Vec::with_capacity(32);
    write_geometry(&mut buf, &geom, Endianness::LittleEndian).ok()?;
    Some(buf)
}

/// Parse WKB bytes back into a `geo::Geometry<f64>`.
///
/// This is the fast accessor for callers that want the parsed geometry
/// directly without going through the WKT lexer. Returns `None` for
/// corrupt buffers. The `wkb` 0.8 reader hands back an opaque
/// `impl GeometryTrait<T = f64>`, so we materialise it into a concrete
/// `geo::Geometry<f64>` via the `geo_traits::to_geo` bridge.
pub(crate) fn decode_wkb_to_geometry(bytes: &[u8]) -> Option<Geometry<f64>> {
    let trait_obj = read_wkb(bytes).ok()?;
    Some(trait_obj.to_geometry())
}

/// Canonicalise WKB bytes back to a lexical WKT string.
///
/// Used by `EncodedTerm → Literal` so user-facing reads get the
/// geometry in the shape GeoSPARQL callers expect. Returns `None` if
/// the WKB is corrupt.
pub(crate) fn decode_wkb_to_wkt(bytes: &[u8]) -> Option<String> {
    Some(decode_wkb_to_geometry(bytes)?.wkt_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_point_fits_in_inline_capacity() {
        let bytes = encode_wkt_value("POINT(10 20)").expect("valid WKT");
        // Standard 2D point WKB: 1 byte order + 4 type + 8 X + 8 Y = 21.
        assert_eq!(bytes.len(), 21);
    }

    #[test]
    fn encode_then_decode_roundtrips_point() {
        let bytes = encode_wkt_value("POINT(1 2)").expect("encodes");
        let geom = decode_wkb_to_geometry(&bytes).expect("decodes");
        match geom {
            Geometry::Point(p) => {
                assert_eq!(p.x(), 1.0);
                assert_eq!(p.y(), 2.0);
            }
            other => panic!("unexpected geometry {other:?}"),
        }
    }

    #[test]
    fn encode_then_decode_to_wkt_produces_point() {
        let bytes = encode_wkt_value("POINT(3 4)").expect("encodes");
        let wkt = decode_wkb_to_wkt(&bytes).expect("decodes to wkt");
        assert!(wkt.to_uppercase().contains("POINT"));
        assert!(wkt.contains('3'));
        assert!(wkt.contains('4'));
    }

    #[test]
    fn encode_rejects_malformed_wkt() {
        assert!(encode_wkt_value("not a geometry").is_none());
    }

    #[test]
    fn decode_rejects_corrupt_bytes() {
        assert!(decode_wkb_to_geometry(&[0, 0, 0]).is_none());
    }
}
