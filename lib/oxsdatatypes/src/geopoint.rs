use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;
use thiserror::Error;
use wkt::types::{Coord, Point};
use wkt::{Geometry, Wkt};

// use std::time::Geo as StdDuration;

/// [XML Schema `duration` datatype](https://www.w3.org/TR/xmlschema11-2/#duration)
///
/// It stores the duration using a pair of a [`YearMonthDuration`] and a [`DayTimeDuration`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GeoPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Error, Debug)]
pub enum GeoPointError {
    #[error("Unable to parse WKT: {0}")]
    WktParsingError(&'static str),
    #[error("WKT type {0} is not supported")]
    UnsupportedWktType(String),
}

impl GeoPoint {
    #[inline]
    pub fn new() -> Result<Self, GeoPointError> {
        Self::from_str("POINT(0 0)")
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self {
            x: f32::from_be_bytes(bytes[0..4].try_into().unwrap()),
            y: f32::from_be_bytes(bytes[4..8].try_into().unwrap()),
        }
    }
}

impl FromStr for GeoPoint {
    type Err = GeoPointError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let geo = Wkt::from_str(input).map_err(GeoPointError::WktParsingError)?;
        let Geometry::Point(Point(Some(Coord {
            x,
            y,
            z: None,
            m: None,
        }))) = geo.item
        else {
            return Err(GeoPointError::UnsupportedWktType(geo.item.to_string()));
        };
        Ok(Self { x, y })
    }
}

impl fmt::Display for GeoPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "POINT({} {})", self.x, self.y)
    }
}
#[cfg(test)]
mod tests {
    #![allow(clippy::panic_in_result_fn)]

    use super::*;

    #[test]
    fn from_str() {
        let pt = GeoPoint::from_str("POINT(10 -20)").unwrap().0 .0.unwrap();
        assert_eq!(pt.x, 10.0);
        assert_eq!(pt.y, -20.0);
    }
}
