use crate::xsd::Float;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::num::ParseFloatError;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::str::FromStr;

/// [XML Schema `double` datatype](https://www.w3.org/TR/xmlschema11-2/#double) implementation.
///
/// The "==" implementation is identity, not equality
#[derive(Debug, Clone, Copy, Default)]
#[repr(transparent)]
pub struct Double {
    value: f64,
}

impl Double {
    #[inline]
    pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self {
            value: f64::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 8] {
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

    /// Casts i64 into `Double`
    #[inline]
    #[allow(clippy::cast_precision_loss)]
    pub fn from_i64(value: i64) -> Self {
        Self {
            value: value as f64,
        }
    }

    /// Casts `Double` into i64 without taking care of loss
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_i64(self) -> i64 {
        self.value as i64
    }

    /// Casts `Double` into f32 without taking care of loss
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_f32(self) -> f32 {
        self.value as f32
    }

    /// Creates a `bool` from a `Decimal` according to xsd:boolean cast constraints
    #[inline]
    pub fn to_bool(self) -> bool {
        self.value != 0. && !self.value.is_nan()
    }
}

impl From<Double> for f64 {
    #[inline]
    fn from(value: Double) -> Self {
        value.value
    }
}

impl From<f64> for Double {
    #[inline]
    fn from(value: f64) -> Self {
        Self { value }
    }
}

impl From<i8> for Double {
    #[inline]
    fn from(value: i8) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<i16> for Double {
    #[inline]
    fn from(value: i16) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<i32> for Double {
    #[inline]
    fn from(value: i32) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<u8> for Double {
    #[inline]
    fn from(value: u8) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<u16> for Double {
    #[inline]
    fn from(value: u16) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<u32> for Double {
    #[inline]
    fn from(value: u32) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<Float> for Double {
    #[inline]
    fn from(value: Float) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl FromStr for Double {
    type Err = ParseFloatError;

    /// Parses decimals lexical mapping
    #[inline]
    fn from_str(input: &str) -> Result<Self, ParseFloatError> {
        Ok(f64::from_str(input)?.into())
    }
}

impl fmt::Display for Double {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.value == f64::INFINITY {
            f.write_str("INF")
        } else if self.value == f64::NEG_INFINITY {
            f.write_str("-INF")
        } else {
            self.value.fmt(f)
        }
    }
}

impl PartialEq for Double {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.value.to_ne_bytes() == other.value.to_ne_bytes()
    }
}

impl Eq for Double {}

impl PartialOrd for Double {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl Hash for Double {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.value.to_ne_bytes())
    }
}

impl Neg for Double {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        (-self.value).into()
    }
}

impl Add for Double {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        (self.value + rhs.value).into()
    }
}

impl Sub for Double {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        (self.value - rhs.value).into()
    }
}

impl Mul for Double {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        (self.value * rhs.value).into()
    }
}

impl Div for Double {
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
        assert_eq!(Double::from(0_f64), Double::from(0_f64));
        assert_eq!(Double::from(f64::NAN), Double::from(f64::NAN));
        assert_ne!(Double::from(-0.), Double::from(0.));
    }

    #[test]
    fn from_str() -> Result<(), ParseFloatError> {
        assert_eq!(Double::from(f64::NAN), Double::from_str("NaN")?);
        assert_eq!(Double::from(f64::INFINITY), Double::from_str("INF")?);
        assert_eq!(Double::from(f64::INFINITY), Double::from_str("+INF")?);
        assert_eq!(Double::from(f64::NEG_INFINITY), Double::from_str("-INF")?);
        assert_eq!(Double::from(0.), Double::from_str("0.0E0")?);
        assert_eq!(Double::from(-0.), Double::from_str("-0.0E0")?);
        Ok(())
    }

    #[test]
    fn to_string() {
        assert_eq!("NaN", Double::from(f64::NAN).to_string());
        assert_eq!("INF", Double::from(f64::INFINITY).to_string());
        assert_eq!("-INF", Double::from(f64::NEG_INFINITY).to_string());
    }
}
