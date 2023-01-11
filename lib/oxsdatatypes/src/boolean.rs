use crate::{Decimal, Double, Float, Integer};
use std::fmt;
use std::str::{FromStr, ParseBoolError};

/// [XML Schema `boolean` datatype](https://www.w3.org/TR/xmlschema11-2/#boolean)
///
/// Uses internally a [`bool`].
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Boolean {
    value: bool,
}

impl Boolean {
    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self == other
    }
}

impl From<bool> for Boolean {
    #[inline]
    fn from(value: bool) -> Self {
        Self { value }
    }
}

impl From<Integer> for Boolean {
    #[inline]
    fn from(value: Integer) -> Self {
        (value != Integer::from(0)).into()
    }
}

impl From<Decimal> for Boolean {
    #[inline]
    fn from(value: Decimal) -> Self {
        (value != Decimal::from(0)).into()
    }
}

impl From<Float> for Boolean {
    #[inline]
    fn from(value: Float) -> Self {
        (value != Float::from(0.) && !value.is_naan()).into()
    }
}

impl From<Double> for Boolean {
    #[inline]
    fn from(value: Double) -> Self {
        (value != Double::from(0.) && !value.is_naan()).into()
    }
}

impl From<Boolean> for bool {
    #[inline]
    fn from(value: Boolean) -> Self {
        value.value
    }
}

impl FromStr for Boolean {
    type Err = ParseBoolError;

    #[inline]
    fn from_str(input: &str) -> Result<Self, ParseBoolError> {
        Ok(match input {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => bool::from_str(input)?,
        }
        .into())
    }
}

impl fmt::Display for Boolean {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str() -> Result<(), ParseBoolError> {
        assert_eq!(Boolean::from_str("true")?.to_string(), "true");
        assert_eq!(Boolean::from_str("1")?.to_string(), "true");
        assert_eq!(Boolean::from_str("false")?.to_string(), "false");
        assert_eq!(Boolean::from_str("0")?.to_string(), "false");
        Ok(())
    }

    #[test]
    fn from_integer() {
        assert_eq!(Boolean::from(false), Integer::from(0).into());
        assert_eq!(Boolean::from(true), Integer::from(1).into());
        assert_eq!(Boolean::from(true), Integer::from(2).into());
    }

    #[test]
    fn from_decimal() {
        assert_eq!(Boolean::from(false), Decimal::from(0).into());
        assert_eq!(Boolean::from(true), Decimal::from(1).into());
        assert_eq!(Boolean::from(true), Decimal::from(2).into());
    }

    #[test]
    fn from_float() {
        assert_eq!(Boolean::from(false), Float::from(0.).into());
        assert_eq!(Boolean::from(true), Float::from(1.).into());
        assert_eq!(Boolean::from(true), Float::from(2.).into());
        assert_eq!(Boolean::from(false), Float::from(f32::NAN).into());
        assert_eq!(Boolean::from(true), Float::from(f32::INFINITY).into());
    }

    #[test]
    fn from_double() {
        assert_eq!(Boolean::from(false), Double::from(0.).into());
        assert_eq!(Boolean::from(true), Double::from(1.).into());
        assert_eq!(Boolean::from(true), Double::from(2.).into());
        assert_eq!(Boolean::from(false), Double::from(f64::NAN).into());
        assert_eq!(Boolean::from(true), Double::from(f64::INFINITY).into());
    }
}
