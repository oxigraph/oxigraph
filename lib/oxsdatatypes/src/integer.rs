use crate::{Boolean, Decimal, DecimalOverflowError, Double, Float};
use std::fmt;
use std::num::ParseIntError;
use std::ops::Neg;
use std::str::FromStr;

/// [XML Schema `integer` datatype](https://www.w3.org/TR/xmlschema11-2/#integer)
///
/// Uses internally a [`i64`].
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Integer {
    value: i64,
}

impl Integer {
    #[inline]
    pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self {
            value: i64::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 8] {
        self.value.to_be_bytes()
    }

    /// [op:numeric-add](https://www.w3.org/TR/xpath-functions-31/#func-numeric-add)
    #[inline]
    pub fn checked_add(&self, rhs: impl Into<Self>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_add(rhs.into().value)?,
        })
    }

    /// [op:numeric-subtract](https://www.w3.org/TR/xpath-functions-31/#func-numeric-subtract)
    #[inline]
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_sub(rhs.into().value)?,
        })
    }

    /// [op:numeric-multiply](https://www.w3.org/TR/xpath-functions-31/#func-numeric-multiply)
    #[inline]
    pub fn checked_mul(&self, rhs: impl Into<Self>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_mul(rhs.into().value)?,
        })
    }

    /// [op:numeric-divide](https://www.w3.org/TR/xpath-functions-31/#func-numeric-divide)
    #[inline]
    pub fn checked_div(&self, rhs: impl Into<Self>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_div(rhs.into().value)?,
        })
    }

    /// [op:numeric-mod](https://www.w3.org/TR/xpath-functions-31/#func-numeric-mod)
    #[inline]
    pub fn checked_rem(&self, rhs: impl Into<Self>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_rem(rhs.into().value)?,
        })
    }

    #[inline]
    pub fn checked_rem_euclid(&self, rhs: impl Into<Self>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_rem_euclid(rhs.into().value)?,
        })
    }

    /// [op:numeric-unary-minus](https://www.w3.org/TR/xpath-functions-31/#func-numeric-unary-minus)
    #[inline]
    pub fn checked_neg(&self) -> Option<Self> {
        Some(Self {
            value: self.value.checked_neg()?,
        })
    }

    /// [fn:abs](https://www.w3.org/TR/xpath-functions-31/#func-abs)
    #[inline]
    pub const fn abs(&self) -> Self {
        Self {
            value: self.value.abs(),
        }
    }

    #[inline]
    pub const fn is_negative(&self) -> bool {
        self.value < 0
    }

    #[inline]
    pub const fn is_positive(&self) -> bool {
        self.value > 0
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self == other
    }

    pub const MIN: Self = Self { value: i64::MIN };

    pub const MAX: Self = Self { value: i64::MAX };
}

impl From<bool> for Integer {
    #[inline]
    fn from(value: bool) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<i8> for Integer {
    #[inline]
    fn from(value: i8) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<i16> for Integer {
    #[inline]
    fn from(value: i16) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<i32> for Integer {
    #[inline]
    fn from(value: i32) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<i64> for Integer {
    #[inline]
    fn from(value: i64) -> Self {
        Self { value }
    }
}

impl From<u8> for Integer {
    #[inline]
    fn from(value: u8) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<u16> for Integer {
    #[inline]
    fn from(value: u16) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<u32> for Integer {
    #[inline]
    fn from(value: u32) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<Boolean> for Integer {
    #[inline]
    fn from(value: Boolean) -> Self {
        bool::from(value).into()
    }
}

impl From<Integer> for i64 {
    #[inline]
    fn from(value: Integer) -> Self {
        value.value
    }
}

impl FromStr for Integer {
    type Err = ParseIntError;

    #[inline]
    fn from_str(input: &str) -> Result<Self, ParseIntError> {
        Ok(i64::from_str(input)?.into())
    }
}

impl fmt::Display for Integer {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl Neg for Integer {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        (-self.value).into()
    }
}

impl TryFrom<Float> for Integer {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: Float) -> Result<Self, DecimalOverflowError> {
        Decimal::try_from(value)?.try_into()
    }
}

impl TryFrom<Double> for Integer {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: Double) -> Result<Self, DecimalOverflowError> {
        Decimal::try_from(value)?.try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str() -> Result<(), ParseIntError> {
        assert_eq!(Integer::from_str("0")?.to_string(), "0");
        assert_eq!(Integer::from_str("-0")?.to_string(), "0");
        assert_eq!(Integer::from_str("123")?.to_string(), "123");
        assert_eq!(Integer::from_str("-123")?.to_string(), "-123");
        assert!(Integer::from_str("123456789123456789123456789123456789123456789").is_err());
        Ok(())
    }

    #[test]
    fn from_float() -> Result<(), ParseIntError> {
        assert_eq!(
            Integer::try_from(Float::from(0.)).ok(),
            Some(Integer::from_str("0")?)
        );
        assert_eq!(
            Integer::try_from(Float::from(-0.)).ok(),
            Some(Integer::from_str("0")?)
        );
        assert_eq!(
            Integer::try_from(Float::from(-123.1)).ok(),
            Some(Integer::from_str("-123")?)
        );
        assert!(Integer::try_from(Float::from(f32::NAN)).is_err());
        assert!(Integer::try_from(Float::from(f32::INFINITY)).is_err());
        assert!(Integer::try_from(Float::from(f32::NEG_INFINITY)).is_err());
        assert!(Integer::try_from(Float::from(f32::MIN)).is_err());
        assert!(Integer::try_from(Float::from(f32::MAX)).is_err());
        assert!(
            Integer::try_from(Float::from(1_672_507_300_000.))
                .unwrap()
                .checked_sub(Integer::from_str("1672507300000")?)
                .unwrap()
                .abs()
                < Integer::from(1_000_000)
        );
        Ok(())
    }

    #[test]
    fn from_double() -> Result<(), ParseIntError> {
        assert_eq!(
            Integer::try_from(Double::from(0.0)).ok(),
            Some(Integer::from_str("0")?)
        );
        assert_eq!(
            Integer::try_from(Double::from(-0.0)).ok(),
            Some(Integer::from_str("0")?)
        );
        assert_eq!(
            Integer::try_from(Double::from(-123.1)).ok(),
            Some(Integer::from_str("-123")?)
        );
        assert!(
            Integer::try_from(Double::from(1_672_507_300_000.))
                .unwrap()
                .checked_sub(Integer::from_str("1672507300000").unwrap())
                .unwrap()
                .abs()
                < Integer::from(10)
        );
        assert!(Integer::try_from(Double::from(f64::NAN)).is_err());
        assert!(Integer::try_from(Double::from(f64::INFINITY)).is_err());
        assert!(Integer::try_from(Double::from(f64::NEG_INFINITY)).is_err());
        assert!(Integer::try_from(Double::from(f64::MIN)).is_err());
        assert!(Integer::try_from(Double::from(f64::MAX)).is_err());
        Ok(())
    }

    #[test]
    fn from_decimal() -> Result<(), ParseIntError> {
        assert_eq!(
            Integer::try_from(Decimal::from(0)).ok(),
            Some(Integer::from_str("0")?)
        );
        assert_eq!(
            Integer::try_from(Decimal::from_str("-123.1").unwrap()).ok(),
            Some(Integer::from_str("-123")?)
        );
        assert!(Integer::try_from(Decimal::MIN).is_err());
        assert!(Integer::try_from(Decimal::MAX).is_err());
        Ok(())
    }

    #[test]
    fn add() {
        assert_eq!(
            Integer::MIN.checked_add(1),
            Some(Integer::from(i64::MIN + 1))
        );
        assert_eq!(Integer::MAX.checked_add(1), None);
    }

    #[test]
    fn sub() {
        assert_eq!(Integer::MIN.checked_sub(1), None);
        assert_eq!(
            Integer::MAX.checked_sub(1),
            Some(Integer::from(i64::MAX - 1))
        );
    }

    #[test]
    fn mul() {
        assert_eq!(Integer::MIN.checked_mul(2), None);
        assert_eq!(Integer::MAX.checked_mul(2), None);
    }

    #[test]
    fn div() {
        assert_eq!(Integer::from(1).checked_div(0), None);
    }

    #[test]
    fn rem() {
        assert_eq!(Integer::from(10).checked_rem(3), Some(Integer::from(1)));
        assert_eq!(Integer::from(6).checked_rem(-2), Some(Integer::from(0)));
        assert_eq!(Integer::from(1).checked_rem(0), None);
    }
}
