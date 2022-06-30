use crate::xsd::{Double, Float};
use std::error::Error;
use std::fmt;
use std::fmt::Write;
use std::ops::Neg;
use std::str::FromStr;

const DECIMAL_PART_DIGITS: usize = 18;
const DECIMAL_PART_POW: i128 = 1_000_000_000_000_000_000;
const DECIMAL_PART_POW_MINUS_ONE: i128 = 100_000_000_000_000_000;
const DECIMAL_PART_HALF_POW: i128 = 1_000_000_000;

/// [XML Schema `decimal` datatype](https://www.w3.org/TR/xmlschema11-2/#decimal) implementation.
///
/// It stores the decimal in a fix point encoding allowing nearly 18 digits before and 18 digits after ".".
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash, Default)]
pub struct Decimal {
    value: i128, // value * 10^18
}

impl Decimal {
    /// Constructs the decimal i / 10^n
    #[allow(clippy::cast_possible_truncation)]
    pub fn new(i: i128, n: u32) -> Result<Self, DecimalOverflowError> {
        if n > DECIMAL_PART_DIGITS as u32 {
            //TODO: check if end with zeros?
            return Err(DecimalOverflowError);
        }
        Ok(Self {
            value: i.checked_div(10_i128.pow(n)).ok_or(DecimalOverflowError)?,
        })
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 16]) -> Self {
        Self {
            value: i128::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 16] {
        self.value.to_be_bytes()
    }

    /// [op:numeric-add](https://www.w3.org/TR/xpath-functions/#func-numeric-add)
    #[inline]
    pub fn checked_add(&self, rhs: impl Into<Self>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_add(rhs.into().value)?,
        })
    }

    /// [op:numeric-subtract](https://www.w3.org/TR/xpath-functions/#func-numeric-subtract)
    #[inline]
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_sub(rhs.into().value)?,
        })
    }

    /// [op:numeric-multiply](https://www.w3.org/TR/xpath-functions/#func-numeric-multiply)
    #[inline]
    pub fn checked_mul(&self, rhs: impl Into<Self>) -> Option<Self> {
        //TODO: better algorithm to keep precision
        Some(Self {
            value: self
                .value
                .checked_div(DECIMAL_PART_HALF_POW)?
                .checked_mul(rhs.into().value.checked_div(DECIMAL_PART_HALF_POW)?)?,
        })
    }

    /// [op:numeric-divide](https://www.w3.org/TR/xpath-functions/#func-numeric-divide)
    #[inline]
    pub fn checked_div(&self, rhs: impl Into<Self>) -> Option<Self> {
        //TODO: better algorithm to keep precision
        Some(Self {
            value: self
                .value
                .checked_mul(DECIMAL_PART_HALF_POW)?
                .checked_div(rhs.into().value)?
                .checked_mul(DECIMAL_PART_HALF_POW)?,
        })
    }

    /// TODO: XSD? is well defined for not integer
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

    /// [fn:abs](https://www.w3.org/TR/xpath-functions/#func-abs)
    #[inline]
    pub const fn abs(&self) -> Self {
        Self {
            value: self.value.abs(),
        }
    }

    /// [fn:round](https://www.w3.org/TR/xpath-functions/#func-round)
    #[inline]
    pub fn round(&self) -> Self {
        let value = self.value / DECIMAL_PART_POW_MINUS_ONE;
        Self {
            value: if value >= 0 {
                (value / 10 + if value % 10 >= 5 { 1 } else { 0 }) * DECIMAL_PART_POW
            } else {
                (value / 10 + if -value % 10 > 5 { -1 } else { 0 }) * DECIMAL_PART_POW
            },
        }
    }

    /// [fn:ceiling](https://www.w3.org/TR/xpath-functions/#func-ceiling)
    #[inline]
    pub fn ceil(&self) -> Self {
        Self {
            value: if self.value >= 0 && self.value % DECIMAL_PART_POW != 0 {
                (self.value / DECIMAL_PART_POW + 1) * DECIMAL_PART_POW
            } else {
                (self.value / DECIMAL_PART_POW) * DECIMAL_PART_POW
            },
        }
    }

    /// [fn:floor](https://www.w3.org/TR/xpath-functions/#func-floor)
    #[inline]
    pub fn floor(&self) -> Self {
        Self {
            value: if self.value >= 0 || self.value % DECIMAL_PART_POW == 0 {
                (self.value / DECIMAL_PART_POW) * DECIMAL_PART_POW
            } else {
                (self.value / DECIMAL_PART_POW - 1) * DECIMAL_PART_POW
            },
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

    /// Creates a `Decimal` from a `Float` without taking care of precision
    #[inline]
    pub(crate) fn from_float(v: Float) -> Self {
        Self::from_double(v.into())
    }

    /// Creates a `Float` from a `Decimal` without taking care of precision
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_float(self) -> Float {
        (f64::from(self.to_double()) as f32).into()
    }

    /// Creates a `Decimal` from a `Double` without taking care of precision
    #[inline]
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub(crate) fn from_double(v: Double) -> Self {
        Self {
            value: (f64::from(v) * (DECIMAL_PART_POW as f64)) as i128,
        }
    }

    /// Creates a `Double` from a `Decimal` without taking care of precision
    #[inline]
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn to_double(self) -> Double {
        ((self.value as f64) / (DECIMAL_PART_POW as f64)).into()
    }

    /// Creates a `bool` from a `Decimal` according to xsd:boolean cast constraints
    #[inline]
    pub fn to_bool(self) -> bool {
        self.value != 0
    }

    #[inline]
    pub(super) const fn as_i128(&self) -> i128 {
        self.value / DECIMAL_PART_POW
    }

    #[cfg(test)]
    pub(super) const fn min_value() -> Self {
        Self { value: i128::MIN }
    }

    #[cfg(test)]
    pub(super) const fn max_value() -> Self {
        Self { value: i128::MAX }
    }

    #[cfg(test)]
    pub(super) const fn step() -> Self {
        Self { value: 1 }
    }
}

impl From<i8> for Decimal {
    #[inline]
    fn from(value: i8) -> Self {
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
    }
}

impl From<i16> for Decimal {
    #[inline]
    fn from(value: i16) -> Self {
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
    }
}

impl From<i32> for Decimal {
    #[inline]
    fn from(value: i32) -> Self {
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
    }
}

impl From<i64> for Decimal {
    #[inline]
    fn from(value: i64) -> Self {
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
    }
}

impl From<u8> for Decimal {
    #[inline]
    fn from(value: u8) -> Self {
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
    }
}

impl From<u16> for Decimal {
    #[inline]
    fn from(value: u16) -> Self {
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
    }
}

impl From<u32> for Decimal {
    #[inline]
    fn from(value: u32) -> Self {
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
    }
}

impl From<u64> for Decimal {
    #[inline]
    fn from(value: u64) -> Self {
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
    }
}

impl TryFrom<i128> for Decimal {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: i128) -> Result<Self, DecimalOverflowError> {
        Ok(Self {
            value: value
                .checked_mul(DECIMAL_PART_POW)
                .ok_or(DecimalOverflowError)?,
        })
    }
}

impl TryFrom<u128> for Decimal {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: u128) -> Result<Self, DecimalOverflowError> {
        Ok(Self {
            value: i128::try_from(value)
                .map_err(|_| DecimalOverflowError)?
                .checked_mul(DECIMAL_PART_POW)
                .ok_or(DecimalOverflowError)?,
        })
    }
}

impl FromStr for Decimal {
    type Err = ParseDecimalError;

    /// Parses decimals lexical mapping
    fn from_str(input: &str) -> Result<Self, ParseDecimalError> {
        // (\+|-)?([0-9]+(\.[0-9]*)?|\.[0-9]+)
        let input = input.as_bytes();
        if input.is_empty() {
            return Err(PARSE_UNEXPECTED_END);
        }

        let (sign, mut cursor) = match input.get(0) {
            Some(b'+') => (1, 1),
            Some(b'-') => (-1, 1),
            _ => (1, 0),
        };

        let mut value = 0_i128;
        let mut with_before_dot = false;
        while cursor < input.len() && b'0' <= input[cursor] && input[cursor] <= b'9' {
            value = value
                .checked_mul(10)
                .ok_or(PARSE_OVERFLOW)?
                .checked_add((input[cursor] - b'0').into())
                .ok_or(PARSE_OVERFLOW)?;
            cursor += 1;
            with_before_dot = true;
        }

        let mut exp = DECIMAL_PART_POW;
        if input.len() > cursor {
            if input[cursor] != b'.' {
                return Err(PARSE_UNEXPECTED_CHAR);
            }
            cursor += 1;

            let mut with_after_dot = false;
            while cursor < input.len() && b'0' <= input[cursor] && input[cursor] <= b'9' {
                exp = exp.checked_div(10).ok_or(PARSE_UNDERFLOW)?;
                value = value
                    .checked_mul(10)
                    .ok_or(PARSE_OVERFLOW)?
                    .checked_add((input[cursor] - b'0').into())
                    .ok_or(PARSE_OVERFLOW)?;
                cursor += 1;
                with_after_dot = true;
            }

            if !with_before_dot && !with_after_dot {
                //We only have a dot
                return Err(PARSE_UNEXPECTED_END);
            }
            if input.len() > cursor {
                return Err(PARSE_UNEXPECTED_CHAR);
            }
        } else if !with_before_dot {
            //It's empty
            return Err(PARSE_UNEXPECTED_END);
        }

        Ok(Self {
            value: value
                .checked_mul(sign)
                .ok_or(PARSE_OVERFLOW)?
                .checked_mul(exp)
                .ok_or(PARSE_OVERFLOW)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ParseDecimalError {
    kind: ParseDecimalErrorKind,
}

#[derive(Debug, Clone)]
enum ParseDecimalErrorKind {
    Overflow,
    Underflow,
    UnexpectedChar,
    UnexpectedEnd,
}

const PARSE_OVERFLOW: ParseDecimalError = ParseDecimalError {
    kind: ParseDecimalErrorKind::Overflow,
};
const PARSE_UNDERFLOW: ParseDecimalError = ParseDecimalError {
    kind: ParseDecimalErrorKind::Underflow,
};
const PARSE_UNEXPECTED_CHAR: ParseDecimalError = ParseDecimalError {
    kind: ParseDecimalErrorKind::UnexpectedChar,
};
const PARSE_UNEXPECTED_END: ParseDecimalError = ParseDecimalError {
    kind: ParseDecimalErrorKind::UnexpectedEnd,
};

impl fmt::Display for ParseDecimalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ParseDecimalErrorKind::Overflow => write!(f, "Value overflow"),
            ParseDecimalErrorKind::Underflow => write!(f, "Value underflow"),
            ParseDecimalErrorKind::UnexpectedChar => write!(f, "Unexpected character"),
            ParseDecimalErrorKind::UnexpectedEnd => write!(f, "Unexpected end of string"),
        }
    }
}

impl Error for ParseDecimalError {}

impl From<DecimalOverflowError> for ParseDecimalError {
    fn from(_: DecimalOverflowError) -> Self {
        Self {
            kind: ParseDecimalErrorKind::Overflow,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DecimalOverflowError;

impl fmt::Display for DecimalOverflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Value overflow")
    }
}

impl Error for DecimalOverflowError {}

impl fmt::Display for Decimal {
    /// Formats the decimal following its canonical representation.
    #[allow(clippy::cast_possible_truncation)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.value == 0 {
            return if let Some(width) = f.width() {
                for _ in 0..width {
                    f.write_char('0')?;
                }
                Ok(())
            } else {
                f.write_char('0')
            };
        }

        let mut value = self.value;
        if self.value.is_negative() {
            f.write_char('-')?;
        }

        let mut digits = [b'0'; 40];
        let mut i = 0;
        while value != 0 {
            digits[i] = b'0' + ((value % 10).unsigned_abs() as u8);
            value /= 10;
            i += 1;
        }

        let last_non_zero = i - 1;
        let first_non_zero = digits
            .iter()
            .copied()
            .enumerate()
            .find_map(|(i, v)| if v == b'0' { None } else { Some(i) })
            .unwrap_or(40);

        if last_non_zero >= DECIMAL_PART_DIGITS {
            let end = if let Some(mut width) = f.width() {
                if self.value.is_negative() {
                    width -= 1;
                }
                if last_non_zero - DECIMAL_PART_DIGITS + 1 < width {
                    DECIMAL_PART_DIGITS + width
                } else {
                    last_non_zero + 1
                }
            } else {
                last_non_zero + 1
            };
            for c in digits[DECIMAL_PART_DIGITS..end].iter().rev() {
                f.write_char(char::from(*c))?;
            }
        } else {
            f.write_char('0')?
        }
        if DECIMAL_PART_DIGITS > first_non_zero {
            f.write_char('.')?;
            let start = if let Some(precision) = f.precision() {
                if DECIMAL_PART_DIGITS - first_non_zero > precision {
                    DECIMAL_PART_DIGITS - precision
                } else {
                    first_non_zero
                }
            } else {
                first_non_zero
            };
            for c in digits[start..DECIMAL_PART_DIGITS].iter().rev() {
                f.write_char(char::from(*c))?;
            }
        }

        Ok(())
    }
}

impl Neg for Decimal {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self {
            value: self.value.neg(),
        }
    }
}

impl TryFrom<Decimal> for i64 {
    type Error = DecimalOverflowError;

    fn try_from(value: Decimal) -> Result<Self, DecimalOverflowError> {
        value
            .value
            .checked_div(DECIMAL_PART_POW)
            .ok_or(DecimalOverflowError)?
            .try_into()
            .map_err(|_| DecimalOverflowError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str() {
        assert_eq!(Decimal::from_str("210").unwrap().to_string(), "210");
        assert_eq!(Decimal::from_str("1000").unwrap().to_string(), "1000");
        assert_eq!(Decimal::from_str("-1.23").unwrap().to_string(), "-1.23");
        assert_eq!(
            Decimal::from_str("12678967.543233").unwrap().to_string(),
            "12678967.543233"
        );
        assert_eq!(
            Decimal::from_str("+100000.00").unwrap().to_string(),
            "100000"
        );
        assert_eq!(Decimal::from_str("0.1220").unwrap().to_string(), "0.122");
        assert_eq!(Decimal::from_str(".12200").unwrap().to_string(), "0.122");
        assert_eq!(
            Decimal::from_str(&Decimal::max_value().to_string()).unwrap(),
            Decimal::max_value()
        );
        assert_eq!(
            Decimal::from_str(
                &Decimal::min_value()
                    .checked_add(Decimal::step())
                    .unwrap()
                    .to_string()
            )
            .unwrap(),
            Decimal::min_value().checked_add(Decimal::step()).unwrap()
        );
    }

    #[test]
    fn format() {
        assert_eq!(format!("{}", Decimal::from(0)), "0");
        assert_eq!(format!("{}", Decimal::from(1)), "1");
        assert_eq!(format!("{}", Decimal::from(10)), "10");
        assert_eq!(format!("{}", Decimal::from(100)), "100");
        assert_eq!(format!("{}", Decimal::from(-1)), "-1");
        assert_eq!(format!("{}", Decimal::from(-10)), "-10");

        assert_eq!(format!("{:02}", Decimal::from(0)), "00");
        assert_eq!(format!("{:02}", Decimal::from(1)), "01");
        assert_eq!(format!("{:02}", Decimal::from(10)), "10");
        assert_eq!(format!("{:02}", Decimal::from(100)), "100");
        assert_eq!(format!("{:02}", Decimal::from(-1)), "-1");
        assert_eq!(format!("{:02}", Decimal::from(-10)), "-10");
    }

    #[test]
    fn add() {
        assert!(Decimal::min_value().checked_add(Decimal::step()).is_some());
        assert!(Decimal::max_value().checked_add(Decimal::step()).is_none());
        assert_eq!(
            Decimal::max_value().checked_add(Decimal::min_value()),
            Some(-Decimal::step())
        );
    }

    #[test]
    fn sub() {
        assert!(Decimal::min_value().checked_sub(Decimal::step()).is_none());
        assert!(Decimal::max_value().checked_sub(Decimal::step()).is_some());
    }

    #[test]
    fn mul() {
        assert_eq!(
            Decimal::from_str("1")
                .unwrap()
                .checked_mul(Decimal::from_str("-1").unwrap()),
            Some(Decimal::from_str("-1").unwrap())
        );
        assert_eq!(
            Decimal::from_str("1000")
                .unwrap()
                .checked_mul(Decimal::from_str("1000").unwrap()),
            Some(Decimal::from_str("1000000").unwrap())
        );
        assert_eq!(
            Decimal::from_str("0.1")
                .unwrap()
                .checked_mul(Decimal::from_str("0.01").unwrap()),
            Some(Decimal::from_str("0.001").unwrap())
        );
    }

    #[test]
    fn div() {
        assert_eq!(
            Decimal::from_str("1")
                .unwrap()
                .checked_div(Decimal::from_str("1").unwrap()),
            Some(Decimal::from_str("1").unwrap())
        );
        assert_eq!(
            Decimal::from_str("100")
                .unwrap()
                .checked_div(Decimal::from_str("10").unwrap()),
            Some(Decimal::from_str("10").unwrap())
        );
        assert_eq!(
            Decimal::from_str("10")
                .unwrap()
                .checked_div(Decimal::from_str("100").unwrap()),
            Some(Decimal::from_str("0.1").unwrap())
        );
    }

    #[test]
    fn round() {
        assert_eq!(Decimal::from_str("10").unwrap().round(), Decimal::from(10));
        assert_eq!(
            Decimal::from_str("-10").unwrap().round(),
            Decimal::from(-10)
        );
        assert_eq!(Decimal::from_str("2.5").unwrap().round(), Decimal::from(3));
        assert_eq!(
            Decimal::from_str("2.4999").unwrap().round(),
            Decimal::from(2)
        );
        assert_eq!(
            Decimal::from_str("-2.5").unwrap().round(),
            Decimal::from(-2)
        );
        assert_eq!(Decimal::from(i64::MIN).round(), Decimal::from(i64::MIN));
        assert_eq!(Decimal::from(i64::MAX).round(), Decimal::from(i64::MAX));
    }

    #[test]
    fn ceil() {
        assert_eq!(Decimal::from_str("10").unwrap().ceil(), Decimal::from(10));
        assert_eq!(Decimal::from_str("-10").unwrap().ceil(), Decimal::from(-10));
        assert_eq!(Decimal::from_str("10.5").unwrap().ceil(), Decimal::from(11));
        assert_eq!(
            Decimal::from_str("-10.5").unwrap().ceil(),
            Decimal::from(-10)
        );
        assert_eq!(Decimal::from(i64::MIN).ceil(), Decimal::from(i64::MIN));
        assert_eq!(Decimal::from(i64::MAX).ceil(), Decimal::from(i64::MAX));
    }

    #[test]
    fn floor() {
        assert_eq!(Decimal::from_str("10").unwrap().ceil(), Decimal::from(10));
        assert_eq!(Decimal::from_str("-10").unwrap().ceil(), Decimal::from(-10));
        assert_eq!(
            Decimal::from_str("10.5").unwrap().floor(),
            Decimal::from(10)
        );
        assert_eq!(
            Decimal::from_str("-10.5").unwrap().floor(),
            Decimal::from(-11)
        );
        assert_eq!(Decimal::from(i64::MIN).floor(), Decimal::from(i64::MIN));
        assert_eq!(Decimal::from(i64::MAX).floor(), Decimal::from(i64::MAX));
    }
}
