use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::num::ParseFloatError;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::str::FromStr;

/// [XML Schema `float` datatype](https://www.w3.org/TR/xmlschema11-2/#float) implementation.
///
/// The "==" implementation is identity, not equality
#[derive(Debug, Clone, Copy, Default)]
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

    /// Casts i64 into `Float`
    #[inline]
    #[allow(clippy::cast_precision_loss)]
    pub fn from_i64(value: i64) -> Self {
        Self {
            value: value as f32,
        }
    }

    /// Casts `Float` into i64 without taking care of loss
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_i64(self) -> i64 {
        self.value as i64
    }

    /// Creates a `bool` from a `Decimal` according to xsd:boolean cast constraints
    #[inline]
    pub fn to_bool(self) -> bool {
        self.value != 0. && !self.value.is_nan()
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

impl FromStr for Float {
    type Err = ParseFloatError;

    /// Parses decimals lexical mapping
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

impl PartialEq for Float {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.value.to_ne_bytes() == other.value.to_ne_bytes()
    }
}

impl Eq for Float {}

impl PartialOrd for Float {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl Hash for Float {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.value.to_ne_bytes())
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
        assert_eq!(Float::from(0_f32), Float::from(0_f32));
        assert_eq!(Float::from(f32::NAN), Float::from(f32::NAN));
        assert_ne!(Float::from(-0.), Float::from(0.));
    }

    #[test]
    fn from_str() -> Result<(), ParseFloatError> {
        assert_eq!(Float::from(f32::NAN), Float::from_str("NaN")?);
        assert_eq!(Float::from(f32::INFINITY), Float::from_str("INF")?);
        assert_eq!(Float::from(f32::INFINITY), Float::from_str("+INF")?);
        assert_eq!(Float::from(f32::NEG_INFINITY), Float::from_str("-INF")?);
        assert_eq!(Float::from(0.), Float::from_str("0.0E0")?);
        assert_eq!(Float::from(-0.), Float::from_str("-0.0E0")?);
        Ok(())
    }

    #[test]
    fn to_string() {
        assert_eq!("NaN", Float::from(f32::NAN).to_string());
        assert_eq!("INF", Float::from(f32::INFINITY).to_string());
        assert_eq!("-INF", Float::from(f32::NEG_INFINITY).to_string());
    }
}
