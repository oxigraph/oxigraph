use std::str::FromStr;
use thiserror::Error;
use wkt::types::Point;
use wkt::{Geometry, Wkt};

// use std::time::Geo as StdDuration;

/// [XML Schema `duration` datatype](https://www.w3.org/TR/xmlschema11-2/#duration)
///
/// It stores the duration using a pair of a [`YearMonthDuration`] and a [`DayTimeDuration`].
#[derive(Debug, Clone)]
pub struct GeoPoint(Point<f64>);

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
}

impl FromStr for GeoPoint {
    type Err = GeoPointError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let geo = Wkt::from_str(input).map_err(GeoPointError::WktParsingError)?;
        let Geometry::Point(point) = geo.item else {
            return Err(GeoPointError::UnsupportedWktType(geo.item.to_string()));
        };
        Ok(Self(point))
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
