use crate::{Boolean, Double, Integer};
use std::cmp::Ordering;
use std::fmt;
use std::num::ParseFloatError;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::str::FromStr;

/// [XML Schema `float` datatype](https://www.w3.org/TR/xmlschema11-2/#float)
///
/// Uses internally a [`f32`].
///
/// Beware: serialization is currently buggy and do not follow the canonical mapping yet.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(transparent)]
pub struct Float {
    value: f32,
}

impl Float {
    #[inline]
    pub fn from_be_bytes(bytes: [u8; 4]) -> Self {
        Self {
            value: f32::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 4] {
        self.value.to_be_bytes()
    }

    /// [fn:abs](https://www.w3.org/TR/xpath-functions/#func-abs)
    #[inline]
    pub fn abs(self) -> Self {
        self.value.abs().into()
    }

    /// [fn:ceiling](https://www.w3.org/TR/xpath-functions/#func-ceiling)
    #[inline]
    pub fn ceil(self) -> Self {
        self.value.ceil().into()
    }

    /// [fn:floor](https://www.w3.org/TR/xpath-functions/#func-floor)
    #[inline]
    pub fn floor(self) -> Self {
        self.value.floor().into()
    }

    /// [fn:round](https://www.w3.org/TR/xpath-functions/#func-round)
    #[inline]
    pub fn round(self) -> Self {
        self.value.round().into()
    }

    #[inline]
    pub fn is_naan(self) -> bool {
        self.value.is_nan()
    }

    #[inline]
    pub fn is_finite(self) -> bool {
        self.value.is_finite()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.value.to_ne_bytes() == other.value.to_ne_bytes()
    }
}

impl From<Float> for f32 {
    #[inline]
    fn from(value: Float) -> Self {
        value.value
    }
}

impl From<Float> for f64 {
    #[inline]
    fn from(value: Float) -> Self {
        value.value.into()
    }
}

impl From<f32> for Float {
    #[inline]
    fn from(value: f32) -> Self {
        Self { value }
    }
}

impl From<i8> for Float {
    #[inline]
    fn from(value: i8) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<i16> for Float {
    #[inline]
    fn from(value: i16) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<u8> for Float {
    #[inline]
    fn from(value: u8) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<u16> for Float {
    #[inline]
    fn from(value: u16) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<Boolean> for Float {
    #[inline]
    fn from(value: Boolean) -> Self {
        if bool::from(value) { 1. } else { 0. }.into()
    }
}

impl From<Integer> for Float {
    #[inline]
    fn from(value: Integer) -> Self {
        (i64::from(value) as f32).into()
    }
}

impl From<Double> for Float {
    #[inline]
    fn from(value: Double) -> Self {
        Self {
            value: f64::from(value) as f32,
        }
    }
}

impl FromStr for Float {
    type Err = ParseFloatError;

    #[inline]
    fn from_str(input: &str) -> Result<Self, ParseFloatError> {
        Ok(f32::from_str(input)?.into())
    }
}

impl fmt::Display for Float {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.value == f32::INFINITY {
            f.write_str("INF")
        } else if self.value == f32::NEG_INFINITY {
            f.write_str("-INF")
        } else {
            self.value.fmt(f)
        }
    }
}

impl PartialOrd for Float {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl Neg for Float {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        (-self.value).into()
    }
}

impl Add for Float {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        (self.value + rhs.value).into()
    }
}

impl Sub for Float {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        (self.value - rhs.value).into()
    }
}

impl Mul for Float {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        (self.value * rhs.value).into()
    }
}

impl Div for Float {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self {
        (self.value / rhs.value).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq() {
        assert_eq!(Float::from(0.), Float::from(0.));
        assert_ne!(Float::from(f32::NAN), Float::from(f32::NAN));
        assert_eq!(Float::from(-0.), Float::from(0.));
    }

    #[test]
    fn cmp() {
        assert_eq!(
            Float::from(0.).partial_cmp(&Float::from(0.)),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Float::from(f32::INFINITY).partial_cmp(&Float::from(f32::MAX)),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Float::from(f32::NEG_INFINITY).partial_cmp(&Float::from(f32::MIN)),
            Some(Ordering::Less)
        );
        assert_eq!(Float::from(f32::NAN).partial_cmp(&Float::from(0.)), None);
        assert_eq!(
            Float::from(f32::NAN).partial_cmp(&Float::from(f32::NAN)),
            None
        );
        assert_eq!(
            Float::from(0.).partial_cmp(&Float::from(-0.)),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn is_identical_with() {
        assert!(Float::from(0.).is_identical_with(&Float::from(0.)));
        assert!(Float::from(f32::NAN).is_identical_with(&Float::from(f32::NAN)));
        assert!(!Float::from(-0.).is_identical_with(&Float::from(0.)));
    }

    #[test]
    fn from_str() -> Result<(), ParseFloatError> {
        assert_eq!(Float::from_str("NaN")?.to_string(), "NaN");
        assert_eq!(Float::from_str("INF")?.to_string(), "INF");
        assert_eq!(Float::from_str("+INF")?.to_string(), "INF");
        assert_eq!(Float::from_str("-INF")?.to_string(), "-INF");
        assert_eq!(Float::from_str("0.0E0")?.to_string(), "0");
        assert_eq!(Float::from_str("-0.0E0")?.to_string(), "-0");
        assert_eq!(Float::from_str("0.1e1")?.to_string(), "1");
        assert_eq!(Float::from_str("-0.1e1")?.to_string(), "-1");
        assert_eq!(Float::from_str("1.e1")?.to_string(), "10");
        assert_eq!(Float::from_str("-1.e1")?.to_string(), "-10");
        assert_eq!(Float::from_str("1")?.to_string(), "1");
        assert_eq!(Float::from_str("-1")?.to_string(), "-1");
        assert_eq!(Float::from_str("1.")?.to_string(), "1");
        assert_eq!(Float::from_str("-1.")?.to_string(), "-1");
        assert_eq!(
            Float::from_str(&f32::MIN.to_string())?,
            Float::from(f32::MIN)
        );
        assert_eq!(
            Float::from_str(&f32::MAX.to_string())?,
            Float::from(f32::MAX)
        );
        Ok(())
    }
}
