use crate::{Boolean, Float, Integer};
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Write as _;
use std::num::ParseFloatError;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::str::FromStr;

/// [XML Schema `double` datatype](https://www.w3.org/TR/xmlschema11-2/#double)
///
/// Uses internally a [`f64`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(transparent)]
pub struct Double {
    value: f64,
}

impl Double {
    pub const INFINITY: Self = Self {
        value: f64::INFINITY,
    };
    pub const MAX: Self = Self { value: f64::MAX };
    pub const MIN: Self = Self { value: f64::MIN };
    pub const NAN: Self = Self { value: f64::NAN };
    pub const NEG_INFINITY: Self = Self {
        value: f64::NEG_INFINITY,
    };

    #[inline]
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self {
            value: f64::from_be_bytes(bytes),
        }
    }

    #[inline]
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 8] {
        self.value.to_be_bytes()
    }

    /// [fn:abs](https://www.w3.org/TR/xpath-functions-31/#func-abs)
    #[inline]
    #[must_use]
    pub fn abs(self) -> Self {
        self.value.abs().into()
    }

    /// [fn:ceiling](https://www.w3.org/TR/xpath-functions-31/#func-ceiling)
    #[inline]
    #[must_use]
    pub fn ceil(self) -> Self {
        self.value.ceil().into()
    }

    /// [fn:floor](https://www.w3.org/TR/xpath-functions-31/#func-floor)
    #[inline]
    #[must_use]
    pub fn floor(self) -> Self {
        self.value.floor().into()
    }

    /// [fn:round](https://www.w3.org/TR/xpath-functions-31/#func-round)
    #[inline]
    #[must_use]
    pub fn round(self) -> Self {
        self.value.round().into()
    }

    #[inline]
    #[must_use]
    pub fn is_nan(self) -> bool {
        self.value.is_nan()
    }

    #[inline]
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.value.is_finite()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.value.to_bits() == other.value.to_bits()
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

impl From<Boolean> for Double {
    #[inline]
    fn from(value: Boolean) -> Self {
        f64::from(bool::from(value)).into()
    }
}

impl From<Integer> for Double {
    #[inline]
    #[expect(clippy::cast_precision_loss)]
    fn from(value: Integer) -> Self {
        (i64::from(value) as f64).into()
    }
}

impl FromStr for Double {
    type Err = ParseFloatError;

    #[inline]
    fn from_str(input: &str) -> Result<Self, Self::Err> {
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
        } else if self.value.is_nan() {
            f.write_str("NaN")
        } else {
            format_finite_float::<310>(self.value, f)
        }
    }
}

impl PartialOrd for Double {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
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

pub(super) fn format_finite_float<const DISPLAY_BUFFER_SIZE: usize>(
    value: impl fmt::Display,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    // We write the number to a buffer without exponential notation
    let mut writer = FixedWriter::<DISPLAY_BUFFER_SIZE>::new();
    write!(&mut writer, "{value}")?;
    let mut input = writer.as_ref();

    // We care about negative sign
    if let Some(stripped) = input.strip_prefix('-') {
        f.write_char('-')?;
        input = stripped;
    }

    let (mut before_dot, mut after_dot) = input.split_once('.').unwrap_or((input, ""));
    let mut exponent = 0;

    // We trim the trailing zeros
    while let Some(trimmed) = after_dot.strip_suffix('0') {
        after_dot = trimmed;
    }
    if after_dot.is_empty() {
        while let Some(trimmed) = before_dot.strip_suffix('0') {
            before_dot = trimmed;
            exponent += 1;
        }
    }

    // We trim the prefix zeros
    while let Some(trimmed) = before_dot.strip_prefix('0') {
        before_dot = trimmed;
    }
    if before_dot.is_empty() {
        while let Some(trimmed) = after_dot.strip_prefix('0') {
            after_dot = trimmed;
            exponent -= 1;
        }
    }

    // We write the mantissa with first digit then dot, then the other digits while updating the exponent
    if before_dot.is_empty() {
        if after_dot.is_empty() {
            f.write_str("0.0")?;
            exponent = 0;
        } else {
            f.write_str(&after_dot[..1])?;
            exponent -= 1;
            f.write_char('.')?;
            if after_dot.len() == 1 {
                f.write_char('0')?;
            } else {
                f.write_str(&after_dot[1..])?;
            }
        }
    } else {
        f.write_str(&before_dot[..1])?;
        exponent += i32::try_from(before_dot.len()).map_err(|_| fmt::Error)? - 1;
        f.write_char('.')?;
        if before_dot.len() == 1 && after_dot.is_empty() {
            f.write_char('0')?;
        } else {
            f.write_str(&before_dot[1..])?;
            f.write_str(after_dot)?;
        }
    }

    // We write the exponent
    f.write_char('E')?;
    fmt::Display::fmt(&exponent, f)
}

struct FixedWriter<const SIZE: usize> {
    output: [u8; SIZE],
    cursor: usize,
}

impl<const SIZE: usize> FixedWriter<SIZE> {
    #[inline]
    fn new() -> Self {
        Self {
            output: [0; SIZE],
            cursor: 0,
        }
    }
}

impl<const SIZE: usize> fmt::Write for FixedWriter<SIZE> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.output
            .get_mut(self.cursor..self.cursor + s.len())
            .ok_or(fmt::Error)?
            .copy_from_slice(s.as_bytes());
        self.cursor += s.len();
        Ok(())
    }
}

impl<const SIZE: usize> AsRef<str> for FixedWriter<SIZE> {
    #[inline]
    #[expect(unsafe_code)]
    fn as_ref(&self) -> &str {
        // SAFETY: we only write valid utf-8
        unsafe { str::from_utf8_unchecked(&self.output[..self.cursor]) }
    }
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn)]
mod tests {
    use super::*;

    #[test]
    fn eq() {
        assert_eq!(Double::from(0_f64), Double::from(0_f64));
        assert_ne!(Double::NAN, Double::NAN);
        assert_eq!(Double::from(-0.), Double::from(0.));
    }

    #[test]
    fn cmp() {
        assert_eq!(
            Double::from(0.).partial_cmp(&Double::from(0.)),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Double::INFINITY.partial_cmp(&Double::MAX),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Double::NEG_INFINITY.partial_cmp(&Double::MIN),
            Some(Ordering::Less)
        );
        assert_eq!(Double::NAN.partial_cmp(&Double::from(0.)), None);
        assert_eq!(Double::NAN.partial_cmp(&Double::NAN), None);
        assert_eq!(
            Double::from(0.).partial_cmp(&Double::from(-0.)),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn is_identical_with() {
        assert!(Double::from(0.).is_identical_with(Double::from(0.)));
        assert!(Double::NAN.is_identical_with(Double::NAN));
        assert!(!Double::from(-0.).is_identical_with(Double::from(0.)));
    }

    #[test]
    fn from_str() -> Result<(), ParseFloatError> {
        assert_eq!(Double::from_str("NaN")?.to_string(), "NaN");
        assert_eq!(Double::from_str("INF")?.to_string(), "INF");
        assert_eq!(Double::from_str("+INF")?.to_string(), "INF");
        assert_eq!(Double::from_str("-INF")?.to_string(), "-INF");
        assert_eq!(Double::from_str("0.0E0")?.to_string(), "0.0E0");
        assert_eq!(Double::from_str("-0.0E0")?.to_string(), "-0.0E0");
        assert_eq!(Double::from_str("0.1")?.to_string(), "1.0E-1");
        assert_eq!(Double::from_str("0.1e1")?.to_string(), "1.0E0");
        assert_eq!(Double::from_str("-0.1e1")?.to_string(), "-1.0E0");
        assert_eq!(Double::from_str("1.e1")?.to_string(), "1.0E1");
        assert_eq!(Double::from_str("-1.e1")?.to_string(), "-1.0E1");
        assert_eq!(Double::from_str("12")?.to_string(), "1.2E1");
        assert_eq!(Double::from_str("0.1")?.to_string(), "1.0E-1");
        assert_eq!(Double::from_str("0.01")?.to_string(), "1.0E-2");
        assert_eq!(Double::from_str("1")?.to_string(), "1.0E0");
        assert_eq!(Double::from_str("-1")?.to_string(), "-1.0E0");
        assert_eq!(Double::from_str("1.")?.to_string(), "1.0E0");
        assert_eq!(Double::from_str("-1.")?.to_string(), "-1.0E0");
        assert_eq!(Double::from_str("+1e20")?.to_string(), "1.0E20");
        assert_eq!(Double::from_str(&f64::MIN.to_string())?, Double::MIN);
        assert_eq!(Double::from_str(&f64::MAX.to_string())?, Double::MAX);
        Ok(())
    }

    #[test]
    fn format() {
        assert_eq!(Double::from(f64::INFINITY).to_string(), "INF");
        assert_eq!(Double::from(f64::NEG_INFINITY).to_string(), "-INF");
        assert_eq!(Double::from(f64::NAN).to_string(), "NaN");
        assert_eq!(
            Double::from(f64::MIN).to_string(),
            "-1.7976931348623157E308"
        );
        assert_eq!(Double::from(f64::MAX).to_string(), "1.7976931348623157E308");
        assert_eq!(
            Double::from(f64::EPSILON).to_string(),
            "2.220446049250313E-16"
        );
    }
}
