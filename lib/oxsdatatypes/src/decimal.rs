use crate::{Boolean, Double, Float, Integer};
use std::error::Error;
use std::fmt;
use std::fmt::Write;
use std::ops::Neg;
use std::str::FromStr;

const DECIMAL_PART_DIGITS: u32 = 18;
const DECIMAL_PART_POW: i128 = 1_000_000_000_000_000_000;
const DECIMAL_PART_POW_MINUS_ONE: i128 = 100_000_000_000_000_000;

/// [XML Schema `decimal` datatype](https://www.w3.org/TR/xmlschema11-2/#decimal)
///
/// It stores the decimal in a fix point encoding allowing nearly 18 digits before and 18 digits after ".".
///
/// It stores the value in a [`i128`] integer after multiplying it by 10¹⁸.
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash, Default)]
pub struct Decimal {
    value: i128, // value * 10^18
}

impl Decimal {
    /// Constructs the decimal i / 10^n
    #[inline]
    pub fn new(i: i128, n: u32) -> Result<Self, DecimalOverflowError> {
        let shift = DECIMAL_PART_DIGITS
            .checked_sub(n)
            .ok_or(DecimalOverflowError)?;
        Ok(Self {
            value: i
                .checked_mul(10_i128.pow(shift))
                .ok_or(DecimalOverflowError)?,
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
        // Idea: we shift right as much as possible to keep as much precision as possible
        // Do the multiplication and do the required left shift
        let mut left = self.value;
        let mut shift_left = 0_u32;
        if left != 0 {
            while left % 10 == 0 {
                left /= 10;
                shift_left += 1;
            }
        }

        let mut right = rhs.into().value;
        let mut shift_right = 0_u32;
        if right != 0 {
            while right % 10 == 0 {
                right /= 10;
                shift_right += 1;
            }
        }

        // We do multiplication + shift
        let shift = (shift_left + shift_right).checked_sub(DECIMAL_PART_DIGITS)?;
        Some(Self {
            value: left
                .checked_mul(right)?
                .checked_mul(10_i128.checked_pow(shift)?)?,
        })
    }

    /// [op:numeric-divide](https://www.w3.org/TR/xpath-functions-31/#func-numeric-divide)
    #[inline]
    pub fn checked_div(&self, rhs: impl Into<Self>) -> Option<Self> {
        // Idea: we shift the dividend left as much as possible to keep as much precision as possible
        // And we shift right the divisor as much as possible
        // Do the multiplication and do the required shift
        let mut left = self.value;
        let mut shift_left = 0_u32;
        if left != 0 {
            while let Some(r) = left.checked_mul(10) {
                assert_eq!(r / 10, left);
                left = r;
                shift_left += 1;
            }
        }
        let mut right = rhs.into().value;
        let mut shift_right = 0_u32;
        if right != 0 {
            while right % 10 == 0 {
                right /= 10;
                shift_right += 1;
            }
        }

        // We do division + shift
        let shift = (shift_left + shift_right).checked_sub(DECIMAL_PART_DIGITS)?;
        Some(Self {
            value: left
                .checked_div(right)?
                .checked_div(10_i128.checked_pow(shift)?)?,
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

    /// [fn:round](https://www.w3.org/TR/xpath-functions-31/#func-round)
    #[inline]
    pub fn round(&self) -> Self {
        let value = self.value / DECIMAL_PART_POW_MINUS_ONE;
        Self {
            value: if value >= 0 {
                (value / 10 + i128::from(value % 10 >= 5)) * DECIMAL_PART_POW
            } else {
                (value / 10 - i128::from(-value % 10 > 5)) * DECIMAL_PART_POW
            },
        }
    }

    /// [fn:ceiling](https://www.w3.org/TR/xpath-functions-31/#func-ceiling)
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

    /// [fn:floor](https://www.w3.org/TR/xpath-functions-31/#func-floor)
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

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self == other
    }

    #[inline]
    pub(super) const fn as_i128(&self) -> i128 {
        self.value / DECIMAL_PART_POW
    }

    pub const MIN: Self = Self { value: i128::MIN };

    pub const MAX: Self = Self { value: i128::MAX };

    #[cfg(test)]
    pub const STEP: Self = Self { value: 1 };
}

impl From<bool> for Decimal {
    #[inline]
    fn from(value: bool) -> Self {
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
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

impl From<Integer> for Decimal {
    #[inline]
    fn from(value: Integer) -> Self {
        i64::from(value).into()
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

impl From<Boolean> for Decimal {
    #[inline]
    fn from(value: Boolean) -> Self {
        bool::from(value).into()
    }
}

impl TryFrom<Float> for Decimal {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: Float) -> Result<Self, DecimalOverflowError> {
        Double::from(value).try_into()
    }
}

impl TryFrom<Double> for Decimal {
    type Error = DecimalOverflowError;

    #[inline]
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn try_from(value: Double) -> Result<Self, DecimalOverflowError> {
        let shifted = f64::from(value) * (DECIMAL_PART_POW as f64);
        if shifted.is_finite() && (i128::MIN as f64) <= shifted && shifted <= (i128::MAX as f64) {
            Ok(Self {
                value: shifted as i128,
            })
        } else {
            Err(DecimalOverflowError)
        }
    }
}

impl From<Decimal> for Float {
    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn from(value: Decimal) -> Self {
        Double::from(value).into()
    }
}

impl From<Decimal> for Double {
    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn from(value: Decimal) -> Self {
        let mut value = value.value;
        let mut shift = DECIMAL_PART_POW;

        // Hack to improve precision
        if value != 0 {
            while shift != 1 && value % 10 == 0 {
                value /= 10;
                shift /= 10;
            }
        }

        ((value as f64) / (shift as f64)).into()
    }
}

impl TryFrom<Decimal> for Integer {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: Decimal) -> Result<Self, DecimalOverflowError> {
        Ok(i64::try_from(
            value
                .value
                .checked_div(DECIMAL_PART_POW)
                .ok_or(DecimalOverflowError)?,
        )
        .map_err(|_| DecimalOverflowError)?
        .into())
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

        let (sign, mut input) = match input.first() {
            Some(b'+') => (1_i128, &input[1..]),
            Some(b'-') => (-1_i128, &input[1..]),
            _ => (1, input),
        };

        let mut value = 0_i128;
        let with_before_dot = input.first().map_or(false, u8::is_ascii_digit);
        while let Some(c) = input.first() {
            if c.is_ascii_digit() {
                value = value
                    .checked_mul(10)
                    .ok_or(PARSE_OVERFLOW)?
                    .checked_add(sign * i128::from(*c - b'0'))
                    .ok_or(PARSE_OVERFLOW)?;
                input = &input[1..];
            } else {
                break;
            }
        }

        let mut exp = DECIMAL_PART_POW;
        if let Some(c) = input.first() {
            if *c != b'.' {
                return Err(PARSE_UNEXPECTED_CHAR);
            }
            input = &input[1..];
            if input.is_empty() && !with_before_dot {
                //We only have a dot
                return Err(PARSE_UNEXPECTED_END);
            }
            while input.last() == Some(&b'0') {
                // Hack to avoid underflows
                input = &input[..input.len() - 1];
            }
            while let Some(c) = input.first() {
                if c.is_ascii_digit() {
                    exp /= 10;
                    value = value
                        .checked_mul(10)
                        .ok_or(PARSE_OVERFLOW)?
                        .checked_add(sign * i128::from(*c - b'0'))
                        .ok_or(PARSE_OVERFLOW)?;
                    input = &input[1..];
                } else {
                    return Err(PARSE_UNEXPECTED_CHAR);
                }
            }
            if exp == 0 {
                //Underflow
                return Err(PARSE_UNDERFLOW);
            }
        } else if !with_before_dot {
            //It's empty
            return Err(PARSE_UNEXPECTED_END);
        }

        Ok(Self {
            value: value.checked_mul(exp).ok_or(PARSE_OVERFLOW)?,
        })
    }
}

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

        let decimal_part_digits = usize::try_from(DECIMAL_PART_DIGITS).unwrap();
        if last_non_zero >= decimal_part_digits {
            let end = if let Some(mut width) = f.width() {
                if self.value.is_negative() {
                    width -= 1;
                }
                if last_non_zero - decimal_part_digits + 1 < width {
                    decimal_part_digits + width
                } else {
                    last_non_zero + 1
                }
            } else {
                last_non_zero + 1
            };
            for c in digits[decimal_part_digits..end].iter().rev() {
                f.write_char(char::from(*c))?;
            }
        } else {
            f.write_char('0')?
        }
        if decimal_part_digits > first_non_zero {
            f.write_char('.')?;
            let start = if let Some(precision) = f.precision() {
                if decimal_part_digits - first_non_zero > precision {
                    decimal_part_digits - precision
                } else {
                    first_non_zero
                }
            } else {
                first_non_zero
            };
            for c in digits[start..decimal_part_digits].iter().rev() {
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

/// An error when parsing a [`Decimal`].
#[derive(Debug, Clone)]
pub struct ParseDecimalError {
    kind: DecimalParseErrorKind,
}

#[derive(Debug, Clone)]
enum DecimalParseErrorKind {
    Overflow,
    Underflow,
    UnexpectedChar,
    UnexpectedEnd,
}

const PARSE_OVERFLOW: ParseDecimalError = ParseDecimalError {
    kind: DecimalParseErrorKind::Overflow,
};
const PARSE_UNDERFLOW: ParseDecimalError = ParseDecimalError {
    kind: DecimalParseErrorKind::Underflow,
};
const PARSE_UNEXPECTED_CHAR: ParseDecimalError = ParseDecimalError {
    kind: DecimalParseErrorKind::UnexpectedChar,
};
const PARSE_UNEXPECTED_END: ParseDecimalError = ParseDecimalError {
    kind: DecimalParseErrorKind::UnexpectedEnd,
};

impl fmt::Display for ParseDecimalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            DecimalParseErrorKind::Overflow => write!(f, "Value overflow"),
            DecimalParseErrorKind::Underflow => write!(f, "Value underflow"),
            DecimalParseErrorKind::UnexpectedChar => write!(f, "Unexpected character"),
            DecimalParseErrorKind::UnexpectedEnd => write!(f, "Unexpected end of string"),
        }
    }
}

impl Error for ParseDecimalError {}

impl From<DecimalOverflowError> for ParseDecimalError {
    fn from(_: DecimalOverflowError) -> Self {
        Self {
            kind: DecimalParseErrorKind::Overflow,
        }
    }
}

/// An overflow in [`Decimal`] computations.
#[derive(Debug, Clone, Copy)]
pub struct DecimalOverflowError;

impl fmt::Display for DecimalOverflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Value overflow")
    }
}

impl Error for DecimalOverflowError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() -> Result<(), ParseDecimalError> {
        assert_eq!(Decimal::new(1, 0)?.to_string(), "1");
        assert_eq!(Decimal::new(1, 1)?.to_string(), "0.1");
        assert_eq!(Decimal::new(10, 0)?.to_string(), "10");
        assert_eq!(Decimal::new(10, 1)?.to_string(), "1");
        assert_eq!(Decimal::new(10, 2)?.to_string(), "0.1");
        Ok(())
    }

    #[test]
    fn from_str() -> Result<(), ParseDecimalError> {
        assert!(Decimal::from_str("").is_err());
        assert!(Decimal::from_str("+").is_err());
        assert!(Decimal::from_str("-").is_err());
        assert!(Decimal::from_str(".").is_err());
        assert!(Decimal::from_str("+.").is_err());
        assert!(Decimal::from_str("-.").is_err());
        assert!(Decimal::from_str("a").is_err());
        assert!(Decimal::from_str(".a").is_err());
        assert_eq!(Decimal::from_str("210")?.to_string(), "210");
        assert_eq!(Decimal::from_str("1000")?.to_string(), "1000");
        assert_eq!(Decimal::from_str("-1.23")?.to_string(), "-1.23");
        assert_eq!(
            Decimal::from_str("12678967.543233")?.to_string(),
            "12678967.543233"
        );
        assert_eq!(Decimal::from_str("+100000.00")?.to_string(), "100000");
        assert_eq!(Decimal::from_str("0.1220")?.to_string(), "0.122");
        assert_eq!(Decimal::from_str(".12200")?.to_string(), "0.122");
        assert_eq!(Decimal::from_str("1.")?.to_string(), "1");
        assert_eq!(Decimal::from_str("1.0")?.to_string(), "1");
        assert_eq!(Decimal::from_str("01.0")?.to_string(), "1");
        assert_eq!(Decimal::from_str("0")?.to_string(), "0");
        assert_eq!(Decimal::from_str("-0")?.to_string(), "0");
        assert_eq!(Decimal::from_str(&Decimal::MAX.to_string())?, Decimal::MAX);
        assert_eq!(Decimal::from_str(&Decimal::MIN.to_string())?, Decimal::MIN);
        assert!(Decimal::from_str("0.0000000000000000001").is_err());
        assert!(Decimal::from_str("1000000000000000000000").is_err());
        assert_eq!(
            Decimal::from_str("0.100000000000000000000000000").unwrap(),
            Decimal::from_str("0.1").unwrap()
        );
        Ok(())
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
        assert!(Decimal::MIN.checked_add(Decimal::STEP).is_some());
        assert!(Decimal::MAX.checked_add(Decimal::STEP).is_none());
        assert_eq!(
            Decimal::MAX.checked_add(Decimal::MIN),
            Decimal::STEP.checked_neg()
        );
    }

    #[test]
    fn sub() {
        assert!(Decimal::MIN.checked_sub(Decimal::STEP).is_none());
        assert!(Decimal::MAX.checked_sub(Decimal::STEP).is_some());
    }

    #[test]
    fn mul() -> Result<(), ParseDecimalError> {
        assert_eq!(Decimal::from(1).checked_mul(-1), Some(Decimal::from(-1)));
        assert_eq!(
            Decimal::from(1000).checked_mul(1000),
            Some(Decimal::from(1_000_000))
        );
        assert_eq!(
            Decimal::from_str("0.1")?.checked_mul(Decimal::from_str("0.01")?),
            Some(Decimal::from_str("0.001")?)
        );
        assert_eq!(Decimal::from(0).checked_mul(1), Some(Decimal::from(0)));
        assert_eq!(Decimal::from(1).checked_mul(0), Some(Decimal::from(0)));
        assert_eq!(Decimal::MAX.checked_mul(1), Some(Decimal::MAX));
        assert_eq!(Decimal::MIN.checked_mul(1), Some(Decimal::MIN));
        assert_eq!(
            Decimal::from(1).checked_mul(Decimal::MAX),
            Some(Decimal::MAX)
        );
        assert_eq!(
            Decimal::from(1).checked_mul(Decimal::MIN),
            Some(Decimal::MIN)
        );
        assert_eq!(
            Decimal::MAX.checked_mul(-1),
            Some(Decimal::MIN.checked_add(Decimal::STEP).unwrap())
        );
        assert_eq!(Decimal::MIN.checked_mul(-1), None);
        assert_eq!(
            Decimal::MIN
                .checked_add(Decimal::STEP)
                .unwrap()
                .checked_mul(-1),
            Some(Decimal::MAX)
        );
        Ok(())
    }

    #[test]
    fn div() -> Result<(), ParseDecimalError> {
        assert_eq!(Decimal::from(1).checked_div(1), Some(Decimal::from(1)));
        assert_eq!(Decimal::from(100).checked_div(10), Some(Decimal::from(10)));
        assert_eq!(
            Decimal::from(10).checked_div(100),
            Some(Decimal::from_str("0.1")?)
        );
        assert_eq!(Decimal::from(1).checked_div(0), None);
        assert_eq!(Decimal::from(0).checked_div(1), Some(Decimal::from(0)));
        assert_eq!(Decimal::MAX.checked_div(1), Some(Decimal::MAX));
        assert_eq!(Decimal::MIN.checked_div(1), Some(Decimal::MIN));
        assert_eq!(
            Decimal::MAX.checked_div(-1),
            Some(Decimal::MIN.checked_add(Decimal::STEP).unwrap())
        );
        assert_eq!(Decimal::MIN.checked_div(-1), None);
        assert_eq!(
            Decimal::MIN
                .checked_add(Decimal::STEP)
                .unwrap()
                .checked_div(-1),
            Some(Decimal::MAX)
        );
        Ok(())
    }

    #[test]
    fn rem() -> Result<(), ParseDecimalError> {
        assert_eq!(Decimal::from(10).checked_rem(3), Some(Decimal::from(1)));
        assert_eq!(Decimal::from(6).checked_rem(-2), Some(Decimal::from(0)));
        assert_eq!(
            Decimal::from_str("4.5")?.checked_rem(Decimal::from_str("1.2")?),
            Some(Decimal::from_str("0.9")?)
        );
        assert_eq!(Decimal::from(1).checked_rem(0), None);
        Ok(())
    }

    #[test]
    fn round() -> Result<(), ParseDecimalError> {
        assert_eq!(Decimal::from(10).round(), Decimal::from(10));
        assert_eq!(Decimal::from(-10).round(), Decimal::from(-10));
        assert_eq!(Decimal::from_str("2.5")?.round(), Decimal::from(3));
        assert_eq!(Decimal::from_str("2.4999")?.round(), Decimal::from(2));
        assert_eq!(Decimal::from_str("-2.5")?.round(), Decimal::from(-2));
        assert_eq!(Decimal::from(i64::MIN).round(), Decimal::from(i64::MIN));
        assert_eq!(Decimal::from(i64::MAX).round(), Decimal::from(i64::MAX));
        Ok(())
    }

    #[test]
    fn ceil() -> Result<(), ParseDecimalError> {
        assert_eq!(Decimal::from(10).ceil(), Decimal::from(10));
        assert_eq!(Decimal::from(-10).ceil(), Decimal::from(-10));
        assert_eq!(Decimal::from_str("10.5")?.ceil(), Decimal::from(11));
        assert_eq!(Decimal::from_str("-10.5")?.ceil(), Decimal::from(-10));
        assert_eq!(Decimal::from(i64::MIN).ceil(), Decimal::from(i64::MIN));
        assert_eq!(Decimal::from(i64::MAX).ceil(), Decimal::from(i64::MAX));
        Ok(())
    }

    #[test]
    fn floor() -> Result<(), ParseDecimalError> {
        assert_eq!(Decimal::from(10).ceil(), Decimal::from(10));
        assert_eq!(Decimal::from(-10).ceil(), Decimal::from(-10));
        assert_eq!(Decimal::from_str("10.5")?.floor(), Decimal::from(10));
        assert_eq!(Decimal::from_str("-10.5")?.floor(), Decimal::from(-11));
        assert_eq!(Decimal::from(i64::MIN).floor(), Decimal::from(i64::MIN));
        assert_eq!(Decimal::from(i64::MAX).floor(), Decimal::from(i64::MAX));
        Ok(())
    }

    #[test]
    fn to_be_bytes() -> Result<(), ParseDecimalError> {
        assert_eq!(
            Decimal::from_be_bytes(Decimal::from(i64::MIN).to_be_bytes()),
            Decimal::from(i64::MIN)
        );
        assert_eq!(
            Decimal::from_be_bytes(Decimal::from(i64::MAX).to_be_bytes()),
            Decimal::from(i64::MAX)
        );
        assert_eq!(
            Decimal::from_be_bytes(Decimal::from(0).to_be_bytes()),
            Decimal::from(0)
        );
        assert_eq!(
            Decimal::from_be_bytes(Decimal::from(0).to_be_bytes()),
            Decimal::from(0)
        );
        assert_eq!(
            Decimal::from_be_bytes(Decimal::from_str("0.01")?.to_be_bytes()),
            Decimal::from_str("0.01")?
        );
        Ok(())
    }

    #[test]
    fn from_bool() {
        assert_eq!(Decimal::from(false), Decimal::from(0_u8));
        assert_eq!(Decimal::from(true), Decimal::from(1_u8));
    }

    #[test]
    fn from_float() -> Result<(), ParseDecimalError> {
        assert_eq!(
            Decimal::try_from(Float::from(0.)).ok(),
            Some(Decimal::from(0))
        );
        assert_eq!(
            Decimal::try_from(Float::from(-0.)).ok(),
            Some(Decimal::from(0))
        );
        assert_eq!(
            Decimal::try_from(Float::from(-123.5)).ok(),
            Some(Decimal::from_str("-123.5")?)
        );
        assert!(Decimal::try_from(Float::from(f32::NAN)).is_err());
        assert!(Decimal::try_from(Float::from(f32::INFINITY)).is_err());
        assert!(Decimal::try_from(Float::from(f32::NEG_INFINITY)).is_err());
        assert!(Decimal::try_from(Float::from(f32::MIN)).is_err());
        assert!(Decimal::try_from(Float::from(f32::MAX)).is_err());
        assert!(
            Decimal::try_from(Float::from(1_672_507_300_000.))
                .unwrap()
                .checked_sub(Decimal::from(1_672_507_293_696_i64))
                .unwrap()
                .abs()
                < Decimal::from(1)
        );
        Ok(())
    }

    #[test]
    fn from_double() -> Result<(), ParseDecimalError> {
        assert_eq!(
            Decimal::try_from(Double::from(0.)).ok(),
            Some(Decimal::from(0))
        );
        assert_eq!(
            Decimal::try_from(Double::from(-0.)).ok(),
            Some(Decimal::from(0))
        );
        assert_eq!(
            Decimal::try_from(Double::from(-123.1)).ok(),
            Some(Decimal::from_str("-123.1")?)
        );
        assert!(
            Decimal::try_from(Double::from(1_672_507_302_466.))
                .unwrap()
                .checked_sub(Decimal::from(1_672_507_302_466_i64))
                .unwrap()
                .abs()
                < Decimal::from(1)
        );
        assert!(Decimal::try_from(Double::from(f64::NAN)).is_err());
        assert!(Decimal::try_from(Double::from(f64::INFINITY)).is_err());
        assert!(Decimal::try_from(Double::from(f64::NEG_INFINITY)).is_err());
        assert!(Decimal::try_from(Double::from(f64::MIN)).is_err());
        assert!(Decimal::try_from(Double::from(f64::MAX)).is_err());
        Ok(())
    }

    #[test]
    fn to_float() -> Result<(), ParseDecimalError> {
        assert_eq!(Float::from(Decimal::from(0)), Float::from(0.));
        assert_eq!(Float::from(Decimal::from(1)), Float::from(1.));
        assert_eq!(Float::from(Decimal::from(10)), Float::from(10.));
        assert_eq!(Float::from(Decimal::from_str("0.1")?), Float::from(0.1));
        assert!((Float::from(Decimal::MAX) - Float::from(1.701_412e20)).abs() < Float::from(1.));
        assert!((Float::from(Decimal::MIN) - Float::from(-1.701_412e20)).abs() < Float::from(1.));
        Ok(())
    }

    #[test]
    fn to_double() -> Result<(), ParseDecimalError> {
        assert_eq!(Double::from(Decimal::from(0)), Double::from(0.));
        assert_eq!(Double::from(Decimal::from(1)), Double::from(1.));
        assert_eq!(Double::from(Decimal::from(10)), Double::from(10.));
        assert_eq!(Double::from(Decimal::from_str("0.1")?), Double::from(0.1));
        assert!(
            (Double::from(Decimal::MAX) - Double::from(1.701_411_834_604_692_4e20)).abs()
                < Double::from(1.)
        );
        assert!(
            (Double::from(Decimal::MIN) - Double::from(-1.701_411_834_604_692_4e20)).abs()
                < Double::from(1.)
        );
        Ok(())
    }

    #[test]
    fn minimally_conformant() -> Result<(), ParseDecimalError> {
        // All minimally conforming processors must support decimal values whose absolute value can be expressed as i / 10^k,
        // where i and k are nonnegative integers such that i < 10^16 and k ≤ 16 (i.e., those expressible with sixteen total digits).
        assert_eq!(
            Decimal::from_str("1234567890123456")?.to_string(),
            "1234567890123456"
        );
        assert_eq!(
            Decimal::from_str("-1234567890123456")?.to_string(),
            "-1234567890123456"
        );
        assert_eq!(
            Decimal::from_str("0.1234567890123456")?.to_string(),
            "0.1234567890123456"
        );
        assert_eq!(
            Decimal::from_str("-0.1234567890123456")?.to_string(),
            "-0.1234567890123456"
        );
        Ok(())
    }
}
