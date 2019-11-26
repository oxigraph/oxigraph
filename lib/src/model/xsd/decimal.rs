use std::convert::{TryFrom, TryInto};
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
    pub fn from_le_bytes(bytes: [u8; 16]) -> Self {
        Self {
            value: i128::from_le_bytes(bytes),
        }
    }
}

impl<I: Into<i64>> From<I> for Decimal {
    fn from(value: I) -> Self {
        let value: i64 = value.into();
        Self {
            value: i128::from(value) * DECIMAL_PART_POW,
        }
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

        let mut value = 0i128;
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

impl fmt::Display for Decimal {
    /// Formats the decimal following its canonical representation
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = self.value;
        if value < 0 {
            f.write_char('-')?;
        }

        let mut digits = [b'0'; 40];
        let mut i = 0;
        while value != 0 {
            digits[i] = b'0' + ((value % 10).abs() as u8);
            value /= 10;
            i += 1;
        }

        if i == 0 {
            return f.write_char('0');
        }

        let last_non_zero = i - 1;
        let first_non_zero = digits
            .iter()
            .cloned()
            .enumerate()
            .find(|(_, v)| *v != b'0')
            .map(|(i, _)| i)
            .unwrap_or(40);

        if last_non_zero >= DECIMAL_PART_DIGITS {
            for c in digits[DECIMAL_PART_DIGITS..=last_non_zero].iter().rev() {
                f.write_char(char::from(*c))?;
            }
        } else {
            f.write_char('0')?
        }
        if DECIMAL_PART_DIGITS > first_non_zero {
            f.write_char('.')?;
            for c in digits[first_non_zero..DECIMAL_PART_DIGITS].iter().rev() {
                f.write_char(char::from(*c))?;
            }
        }

        Ok(())
    }
}

impl Neg for Decimal {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            value: self.value.neg(),
        }
    }
}

impl Decimal {
    /*pub fn trunc(self) -> i64 {
        (self.value / DECIMAL_PART_POW) as i64
    }*/

    pub fn to_le_bytes(&self) -> [u8; 16] {
        self.value.to_le_bytes()
    }

    /// [op:numeric-add](https://www.w3.org/TR/xpath-functions/#func-numeric-add)
    pub fn checked_add(&self, rhs: Self) -> Option<Self> {
        Some(Self {
            value: self.value.checked_add(rhs.value)?,
        })
    }

    /// [op:numeric-subtract](https://www.w3.org/TR/xpath-functions/#func-numeric-subtract)
    pub fn checked_sub(&self, rhs: Self) -> Option<Self> {
        Some(Self {
            value: self.value.checked_sub(rhs.value)?,
        })
    }

    /// [op:numeric-multiply](https://www.w3.org/TR/xpath-functions/#func-numeric-multiply)
    pub fn checked_mul(&self, rhs: Self) -> Option<Self> {
        //TODO: better algorithm to keep precision
        Some(Self {
            value: self
                .value
                .checked_div(DECIMAL_PART_HALF_POW)?
                .checked_mul(rhs.value.checked_div(DECIMAL_PART_HALF_POW)?)?,
        })
    }

    /// [op:numeric-divide](https://www.w3.org/TR/xpath-functions/#func-numeric-divide)
    pub fn checked_div(&self, rhs: Self) -> Option<Self> {
        //TODO: better algorithm to keep precision
        Some(Self {
            value: self
                .value
                .checked_mul(DECIMAL_PART_HALF_POW)?
                .checked_div(rhs.value)?
                .checked_mul(DECIMAL_PART_HALF_POW)?,
        })
    }

    /// [fn:abs](https://www.w3.org/TR/xpath-functions/#func-abs)
    pub fn abs(&self) -> Decimal {
        Self {
            value: self.value.abs(),
        }
    }

    /// [fn:round](https://www.w3.org/TR/xpath-functions/#func-round)
    pub fn round(&self) -> Decimal {
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
    pub fn ceil(&self) -> Decimal {
        Self {
            value: if self.value >= 0 && self.value % DECIMAL_PART_POW != 0 {
                (self.value / DECIMAL_PART_POW + 1) * DECIMAL_PART_POW
            } else {
                (self.value / DECIMAL_PART_POW) * DECIMAL_PART_POW
            },
        }
    }

    /// [fn:floor](https://www.w3.org/TR/xpath-functions/#func-floor)
    pub fn floor(&self) -> Decimal {
        Self {
            value: if self.value >= 0 || self.value % DECIMAL_PART_POW == 0 {
                (self.value / DECIMAL_PART_POW) * DECIMAL_PART_POW
            } else {
                (self.value / DECIMAL_PART_POW - 1) * DECIMAL_PART_POW
            },
        }
    }

    pub fn to_f32(&self) -> Option<f32> {
        //TODO: precision?
        Some((self.value as f32) / (DECIMAL_PART_POW as f32))
    }

    pub fn to_f64(&self) -> Option<f64> {
        //TODO: precision?
        Some((self.value as f64) / (DECIMAL_PART_POW as f64))
    }
}

impl TryFrom<Decimal> for i64 {
    type Error = ();

    fn try_from(value: Decimal) -> Result<i64, ()> {
        value
            .value
            .checked_div(DECIMAL_PART_POW)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::i128;
    use std::i64;

    const MIN: Decimal = Decimal { value: i128::MIN };
    const MAX: Decimal = Decimal { value: i128::MAX };
    const STEP: Decimal = Decimal { value: 1 };

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
        assert_eq!(Decimal::from_str(&MAX.to_string()).unwrap(), MAX);
        assert_eq!(
            Decimal::from_str(&MIN.checked_add(STEP).unwrap().to_string()).unwrap(),
            MIN.checked_add(STEP).unwrap()
        );
    }

    #[test]
    fn add() {
        assert!(MIN.checked_add(STEP).is_some());
        assert!(MAX.checked_add(STEP).is_none());
        assert_eq!(MAX.checked_add(MIN), Some(-STEP));
    }

    #[test]
    fn sub() {
        assert!(MIN.checked_sub(STEP).is_none());
        assert!(MAX.checked_sub(STEP).is_some());
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
