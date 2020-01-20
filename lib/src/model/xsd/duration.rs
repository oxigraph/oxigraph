use super::parser::duration_lexical_rep;
use super::parser::parse_value;
use super::*;
use crate::model::xsd::decimal::DecimalOverflowError;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::i64;
use std::ops::Neg;
use std::str::FromStr;
use std::time::Duration as StdDuration;

/// [XML Schema `duration` datatype](https://www.w3.org/TR/xmlschema11-2/#duration) implementation.
///
/// It stores the duration using the two components model suggested by the specification:
/// - a number of months encoded using a `i64`
/// - a number of seconds encoded using a `Decimal`
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Default)]
pub struct Duration {
    months: i64,
    seconds: Decimal,
}

impl Duration {
    pub fn new(months: impl Into<i64>, seconds: impl Into<Decimal>) -> Self {
        Self {
            months: months.into(),
            seconds: seconds.into(),
        }
    }

    pub fn from_be_bytes(bytes: [u8; 24]) -> Self {
        let mut months = [0; 8];
        months.copy_from_slice(&bytes[0..8]);
        let mut seconds = [8; 16];
        seconds.copy_from_slice(&bytes[8..24]);
        Self {
            months: i64::from_be_bytes(months),
            seconds: Decimal::from_be_bytes(seconds),
        }
    }

    /// [fn:years-from-duration](https://www.w3.org/TR/xpath-functions/#func-years-from-duration)
    pub fn years(&self) -> i64 {
        self.months / 12
    }

    /// [fn:months-from-duration](https://www.w3.org/TR/xpath-functions/#func-months-from-duration)
    pub fn months(&self) -> i64 {
        self.months % 12
    }

    /// [fn:days-from-duration](https://www.w3.org/TR/xpath-functions/#func-days-from-duration)
    #[allow(clippy::cast_possible_truncation)]
    pub fn days(&self) -> i64 {
        (self.seconds.as_i128() / 86400) as i64
    }

    /// [fn:hours-from-duration](https://www.w3.org/TR/xpath-functions/#func-hours-from-duration)
    #[allow(clippy::cast_possible_truncation)]
    pub fn hours(&self) -> i64 {
        ((self.seconds.as_i128() % 86400) / 3600) as i64
    }

    /// [fn:minutes-from-duration](https://www.w3.org/TR/xpath-functions/#func-minutes-from-duration)
    #[allow(clippy::cast_possible_truncation)]
    pub fn minutes(&self) -> i64 {
        ((self.seconds.as_i128() % 3600) / 60) as i64
    }

    /// [fn:seconds-from-duration](https://www.w3.org/TR/xpath-functions/#func-seconds-from-duration)
    pub fn seconds(&self) -> Decimal {
        self.seconds.checked_rem(60).unwrap()
    }

    pub(super) const fn all_months(&self) -> i64 {
        self.months
    }

    pub(super) const fn all_seconds(&self) -> Decimal {
        self.seconds
    }

    pub fn to_be_bytes(&self) -> [u8; 24] {
        let mut bytes = [0; 24];
        bytes[0..8].copy_from_slice(&self.months.to_be_bytes());
        bytes[8..24].copy_from_slice(&self.seconds.to_be_bytes());
        bytes
    }

    /// [op:add-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDurations) and [op:add-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDurations)
    pub fn checked_add(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            months: self.months.checked_add(rhs.months)?,
            seconds: self.seconds.checked_add(rhs.seconds)?,
        })
    }

    /// [op:subtract-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDurations) and [op:subtract-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDurations)
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            months: self.months.checked_sub(rhs.months)?,
            seconds: self.seconds.checked_sub(rhs.seconds)?,
        })
    }
}

impl TryFrom<StdDuration> for Duration {
    type Error = DecimalOverflowError;

    fn try_from(value: StdDuration) -> Result<Self, DecimalOverflowError> {
        Ok(Self {
            months: 0,
            seconds: Decimal::new(
                i128::try_from(value.as_nanos()).map_err(|_| DecimalOverflowError)?,
                9,
            )?,
        })
    }
}

impl FromStr for Duration {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(duration_lexical_rep, input)
    }
}

impl fmt::Display for Duration {
    #[allow(clippy::many_single_char_names)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ym = self.months;
        let mut ss = self.seconds;

        if ym < 0 || ss < 0.into() {
            write!(f, "-")?;
            ym = -ym;
            ss = -ss;
        }
        write!(f, "P")?;

        if ym == 0 && ss == 0.into() {
            return write!(f, "T0S");
        }

        {
            let y = ym / 12;
            let m = ym % 12;

            if y != 0 {
                if m == 0 {
                    write!(f, "{}Y", y)?;
                } else {
                    write!(f, "{}Y{}M", y, m)?;
                }
            } else if m != 0 || ss == 0.into() {
                write!(f, "{}M", m)?;
            }
        }

        {
            let s_int = ss.as_i128();
            let d = s_int / 86400;
            let h = (s_int % 86400) / 3600;
            let m = (s_int % 3600) / 60;
            let s = ss
                .checked_sub(Decimal::try_from(d * 86400 + h * 3600 + m * 60).unwrap())
                .unwrap(); //could not fail

            if d != 0 {
                write!(f, "{}D", d)?;
            }

            if h != 0 || m != 0 || s != 0.into() {
                write!(f, "T")?;
                if h != 0 {
                    write!(f, "{}H", h)?;
                }
                if m != 0 {
                    write!(f, "{}M", m)?;
                }
                if s != 0.into() {
                    write!(f, "{}S", s)?;
                }
            }
        }
        Ok(())
    }
}

impl PartialOrd for Duration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let first = DateTime::new(1969, 9, 1, 0, 0, 0.into(), None).ok()?;
        let first_result = first
            .checked_add_duration(*self)?
            .partial_cmp(&first.checked_add_duration(*other)?);
        let second = DateTime::new(1697, 2, 1, 0, 0, 0.into(), None).ok()?;
        let second_result = second
            .checked_add_duration(*self)?
            .partial_cmp(&second.checked_add_duration(*other)?);
        let third = DateTime::new(1903, 3, 1, 0, 0, 0.into(), None).ok()?;
        let third_result = third
            .checked_add_duration(*self)?
            .partial_cmp(&third.checked_add_duration(*other)?);
        let fourth = DateTime::new(1903, 7, 1, 0, 0, 0.into(), None).ok()?;
        let fourth_result = fourth
            .checked_add_duration(*self)?
            .partial_cmp(&fourth.checked_add_duration(*other)?);
        if first_result == second_result
            && second_result == third_result
            && third_result == fourth_result
        {
            first_result
        } else {
            None
        }
    }
}

impl Neg for Duration {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            months: self.months.neg(),
            seconds: self.seconds.neg(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str() {
        let min = Duration::new(
            i64::MIN + 1,
            decimal::MIN.checked_add(decimal::STEP).unwrap(),
        );
        let max = Duration::new(i64::MAX, decimal::MAX);

        assert_eq!(Duration::from_str("P1Y").unwrap().to_string(), "P1Y");
        assert_eq!(Duration::from_str("P1M").unwrap().to_string(), "P1M");
        assert_eq!(Duration::from_str("P1D").unwrap().to_string(), "P1D");
        assert_eq!(Duration::from_str("PT1H").unwrap().to_string(), "PT1H");
        assert_eq!(Duration::from_str("PT1M").unwrap().to_string(), "PT1M");
        assert_eq!(Duration::from_str("PT1.1S").unwrap().to_string(), "PT1.1S");
        assert_eq!(Duration::from_str("-P1Y").unwrap().to_string(), "-P1Y");
        assert_eq!(Duration::from_str("-P1M").unwrap().to_string(), "-P1M");
        assert_eq!(Duration::from_str("-P1D").unwrap().to_string(), "-P1D");
        assert_eq!(Duration::from_str("-PT1H").unwrap().to_string(), "-PT1H");
        assert_eq!(Duration::from_str("-PT1M").unwrap().to_string(), "-PT1M");
        assert_eq!(
            Duration::from_str("-PT1.1S").unwrap().to_string(),
            "-PT1.1S"
        );
        assert_eq!(Duration::from_str(&max.to_string()).unwrap(), max);
        assert_eq!(Duration::from_str(&min.to_string()).unwrap(), min);
    }

    #[test]
    fn equals() {
        assert_eq!(
            Duration::from_str("P1Y").unwrap(),
            Duration::from_str("P12M").unwrap()
        );
        assert_eq!(
            Duration::from_str("PT24H").unwrap(),
            Duration::from_str("P1D").unwrap()
        );
        assert_ne!(
            Duration::from_str("P1Y").unwrap(),
            Duration::from_str("P365D").unwrap()
        );
        assert_eq!(
            Duration::from_str("P0Y").unwrap(),
            Duration::from_str("P0D").unwrap()
        );
        assert_ne!(
            Duration::from_str("P1Y").unwrap(),
            Duration::from_str("P365D").unwrap()
        );
        assert_eq!(
            Duration::from_str("P2Y").unwrap(),
            Duration::from_str("P24M").unwrap()
        );
        assert_eq!(
            Duration::from_str("P10D").unwrap(),
            Duration::from_str("PT240H").unwrap()
        );
        assert_eq!(
            Duration::from_str("P2Y0M0DT0H0M0S").unwrap(),
            Duration::from_str("P24M").unwrap()
        );
        assert_eq!(
            Duration::from_str("P0Y0M10D").unwrap(),
            Duration::from_str("PT240H").unwrap()
        );
    }

    #[test]
    fn years() {
        assert_eq!(Duration::from_str("P20Y15M").unwrap().years(), 21);
        assert_eq!(Duration::from_str("-P15M").unwrap().years(), -1);
        assert_eq!(Duration::from_str("-P2DT15H").unwrap().years(), 0);
    }

    #[test]
    fn months() {
        assert_eq!(Duration::from_str("P20Y15M").unwrap().months(), 3);
        assert_eq!(Duration::from_str("-P20Y18M").unwrap().months(), -6);
        assert_eq!(Duration::from_str("-P2DT15H0M0S").unwrap().months(), 0);
    }

    #[test]
    fn days() {
        assert_eq!(Duration::from_str("P3DT10H").unwrap().days(), 3);
        assert_eq!(Duration::from_str("P3DT55H").unwrap().days(), 5);
        assert_eq!(Duration::from_str("P3Y5M").unwrap().days(), 0);
    }

    #[test]
    fn hours() {
        assert_eq!(Duration::from_str("P3DT10H").unwrap().hours(), 10);
        assert_eq!(Duration::from_str("P3DT12H32M12S").unwrap().hours(), 12);
        assert_eq!(Duration::from_str("PT123H").unwrap().hours(), 3);
        assert_eq!(Duration::from_str("-P3DT10H").unwrap().hours(), -10);
    }

    #[test]
    fn minutes() {
        assert_eq!(Duration::from_str("P3DT10H").unwrap().minutes(), 0);
        assert_eq!(Duration::from_str("-P5DT12H30M").unwrap().minutes(), -30);
    }

    #[test]
    fn seconds() {
        assert_eq!(
            Duration::from_str("P3DT10H12.5S").unwrap().seconds(),
            Decimal::from_str("12.5").unwrap()
        );
        assert_eq!(
            Duration::from_str("-PT256S").unwrap().seconds(),
            Decimal::from_str("-16.0").unwrap()
        );
    }

    #[test]
    fn add() {
        assert_eq!(
            Duration::from_str("P2Y11M")
                .unwrap()
                .checked_add(Duration::from_str("P3Y3M").unwrap()),
            Some(Duration::from_str("P6Y2M").unwrap())
        );
        assert_eq!(
            Duration::from_str("P2DT12H5M")
                .unwrap()
                .checked_add(Duration::from_str("P5DT12H").unwrap()),
            Some(Duration::from_str("P8DT5M").unwrap())
        );
    }

    #[test]
    fn sub() {
        assert_eq!(
            Duration::from_str("P2Y11M")
                .unwrap()
                .checked_sub(Duration::from_str("P3Y3M").unwrap()),
            Some(Duration::from_str("-P4M").unwrap())
        );
        assert_eq!(
            Duration::from_str("P2DT12H")
                .unwrap()
                .checked_sub(Duration::from_str("P1DT10H30M").unwrap()),
            Some(Duration::from_str("P1DT1H30M").unwrap())
        );
    }
}
