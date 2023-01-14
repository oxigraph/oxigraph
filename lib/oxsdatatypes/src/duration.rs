use super::decimal::DecimalOverflowError;
use super::parser::*;
use super::*;
use std::cmp::Ordering;
use std::fmt;
use std::ops::Neg;
use std::str::FromStr;
use std::time::Duration as StdDuration;

/// [XML Schema `duration` datatype](https://www.w3.org/TR/xmlschema11-2/#duration)
///
/// It stores the duration using a pair of a [`YearMonthDuration`] and a [`DayTimeDuration`].
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Default)]
pub struct Duration {
    year_month: YearMonthDuration,
    day_time: DayTimeDuration,
}

impl Duration {
    #[inline]
    pub fn new(months: impl Into<i64>, seconds: impl Into<Decimal>) -> Self {
        Self {
            year_month: YearMonthDuration::new(months),
            day_time: DayTimeDuration::new(seconds),
        }
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 24]) -> Self {
        Self {
            year_month: YearMonthDuration::from_be_bytes(bytes[0..8].try_into().unwrap()),
            day_time: DayTimeDuration::from_be_bytes(bytes[8..24].try_into().unwrap()),
        }
    }

    /// [fn:years-from-duration](https://www.w3.org/TR/xpath-functions/#func-years-from-duration)
    #[inline]
    pub fn years(&self) -> i64 {
        self.year_month.years()
    }

    /// [fn:months-from-duration](https://www.w3.org/TR/xpath-functions/#func-months-from-duration)
    #[inline]
    pub fn months(&self) -> i64 {
        self.year_month.months()
    }

    /// [fn:days-from-duration](https://www.w3.org/TR/xpath-functions/#func-days-from-duration)
    #[inline]
    pub fn days(&self) -> i64 {
        self.day_time.days()
    }

    /// [fn:hours-from-duration](https://www.w3.org/TR/xpath-functions/#func-hours-from-duration)
    #[inline]
    pub fn hours(&self) -> i64 {
        self.day_time.hours()
    }

    /// [fn:minutes-from-duration](https://www.w3.org/TR/xpath-functions/#func-minutes-from-duration)
    #[inline]
    pub fn minutes(&self) -> i64 {
        self.day_time.minutes()
    }

    /// [fn:seconds-from-duration](https://www.w3.org/TR/xpath-functions/#func-seconds-from-duration)
    #[inline]
    pub fn seconds(&self) -> Decimal {
        self.day_time.seconds()
    }

    #[inline]
    pub(super) const fn all_months(&self) -> i64 {
        self.year_month.all_months()
    }

    #[inline]
    pub(super) const fn all_seconds(&self) -> Decimal {
        self.day_time.all_seconds()
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 24] {
        let mut bytes = [0; 24];
        bytes[0..8].copy_from_slice(&self.year_month.to_be_bytes());
        bytes[8..24].copy_from_slice(&self.day_time.to_be_bytes());
        bytes
    }

    /// [op:add-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDurations) and [op:add-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDurations)
    #[inline]
    pub fn checked_add(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            year_month: self.year_month.checked_add(rhs.year_month)?,
            day_time: self.day_time.checked_add(rhs.day_time)?,
        })
    }

    /// [op:subtract-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDurations) and [op:subtract-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDurations)
    #[inline]
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            year_month: self.year_month.checked_sub(rhs.year_month)?,
            day_time: self.day_time.checked_sub(rhs.day_time)?,
        })
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self == other
    }
}

impl TryFrom<StdDuration> for Duration {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: StdDuration) -> Result<Self, DecimalOverflowError> {
        Ok(DayTimeDuration::try_from(value)?.into())
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
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ym = self.year_month.months;
        let mut ss = self.day_time.seconds;

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
                    write!(f, "{y}Y")?;
                } else {
                    write!(f, "{y}Y{m}M")?;
                }
            } else if m != 0 || ss == 0.into() {
                write!(f, "{m}M")?;
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
                write!(f, "{d}D")?;
            }

            if h != 0 || m != 0 || s != 0.into() {
                write!(f, "T")?;
                if h != 0 {
                    write!(f, "{h}H")?;
                }
                if m != 0 {
                    write!(f, "{m}M")?;
                }
                if s != 0.into() {
                    write!(f, "{s}S")?;
                }
            }
        }
        Ok(())
    }
}

impl PartialOrd for Duration {
    #[inline]
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

    #[inline]
    fn neg(self) -> Self {
        Self {
            year_month: self.year_month.neg(),
            day_time: self.day_time.neg(),
        }
    }
}

/// [XML Schema `yearMonthDuration` datatype](https://www.w3.org/TR/xmlschema11-2/#yearMonthDuration)
///
/// It stores the duration as a number of months encoded using a [`i64`].
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash, Default)]
pub struct YearMonthDuration {
    months: i64,
}

impl YearMonthDuration {
    #[inline]
    pub fn new(months: impl Into<i64>) -> Self {
        Self {
            months: months.into(),
        }
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self {
            months: i64::from_be_bytes(bytes),
        }
    }

    /// [fn:years-from-duration](https://www.w3.org/TR/xpath-functions/#func-years-from-duration)
    #[inline]
    pub fn years(self) -> i64 {
        self.months / 12
    }

    /// [fn:months-from-duration](https://www.w3.org/TR/xpath-functions/#func-months-from-duration)
    #[inline]
    pub fn months(self) -> i64 {
        self.months % 12
    }

    #[inline]
    pub(super) const fn all_months(self) -> i64 {
        self.months
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 8] {
        self.months.to_be_bytes()
    }

    /// [op:add-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDurations)
    #[inline]
    pub fn checked_add(self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            months: self.months.checked_add(rhs.months)?,
        })
    }

    /// [op:subtract-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDurations)
    #[inline]
    pub fn checked_sub(self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            months: self.months.checked_sub(rhs.months)?,
        })
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self == other
    }
}

impl From<YearMonthDuration> for Duration {
    #[inline]
    fn from(value: YearMonthDuration) -> Self {
        Self {
            year_month: value,
            day_time: DayTimeDuration::default(),
        }
    }
}

impl TryFrom<Duration> for YearMonthDuration {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: Duration) -> Result<Self, DecimalOverflowError> {
        if value.day_time == DayTimeDuration::default() {
            Ok(value.year_month)
        } else {
            Err(DecimalOverflowError {})
        }
    }
}

impl FromStr for YearMonthDuration {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(year_month_duration_lexical_rep, input)
    }
}

impl fmt::Display for YearMonthDuration {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.months == 0 {
            write!(f, "P0M")
        } else {
            Duration::from(*self).fmt(f)
        }
    }
}

impl PartialEq<Duration> for YearMonthDuration {
    #[inline]
    fn eq(&self, other: &Duration) -> bool {
        Duration::from(*self).eq(other)
    }
}

impl PartialEq<YearMonthDuration> for Duration {
    #[inline]
    fn eq(&self, other: &YearMonthDuration) -> bool {
        self.eq(&Self::from(*other))
    }
}

impl PartialOrd<Duration> for YearMonthDuration {
    #[inline]
    fn partial_cmp(&self, other: &Duration) -> Option<Ordering> {
        Duration::from(*self).partial_cmp(other)
    }
}

impl PartialOrd<YearMonthDuration> for Duration {
    #[inline]
    fn partial_cmp(&self, other: &YearMonthDuration) -> Option<Ordering> {
        self.partial_cmp(&Self::from(*other))
    }
}

impl Neg for YearMonthDuration {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self {
            months: self.months.neg(),
        }
    }
}

/// [XML Schema `dayTimeDuration` datatype](https://www.w3.org/TR/xmlschema11-2/#dayTimeDuration)
///
/// It stores the duration as a number of seconds encoded using a [`Decimal`].
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash, Default)]
pub struct DayTimeDuration {
    seconds: Decimal,
}

impl DayTimeDuration {
    #[inline]
    pub fn new(seconds: impl Into<Decimal>) -> Self {
        Self {
            seconds: seconds.into(),
        }
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 16]) -> Self {
        Self {
            seconds: Decimal::from_be_bytes(bytes),
        }
    }

    /// [fn:days-from-duration](https://www.w3.org/TR/xpath-functions/#func-days-from-duration)
    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    pub fn days(&self) -> i64 {
        (self.seconds.as_i128() / 86400) as i64
    }

    /// [fn:hours-from-duration](https://www.w3.org/TR/xpath-functions/#func-hours-from-duration)
    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    pub fn hours(&self) -> i64 {
        ((self.seconds.as_i128() % 86400) / 3600) as i64
    }

    /// [fn:minutes-from-duration](https://www.w3.org/TR/xpath-functions/#func-minutes-from-duration)
    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    pub fn minutes(&self) -> i64 {
        ((self.seconds.as_i128() % 3600) / 60) as i64
    }

    /// [fn:seconds-from-duration](https://www.w3.org/TR/xpath-functions/#func-seconds-from-duration)
    #[inline]
    pub fn seconds(&self) -> Decimal {
        self.seconds.checked_rem(60).unwrap()
    }

    #[inline]
    pub(super) const fn all_seconds(&self) -> Decimal {
        self.seconds
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 16] {
        self.seconds.to_be_bytes()
    }

    /// [op:add-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDurations)
    #[inline]
    pub fn checked_add(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            seconds: self.seconds.checked_add(rhs.seconds)?,
        })
    }

    /// [op:subtract-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDurations)
    #[inline]
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            seconds: self.seconds.checked_sub(rhs.seconds)?,
        })
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self == other
    }
}

impl From<DayTimeDuration> for Duration {
    #[inline]
    fn from(value: DayTimeDuration) -> Self {
        Self {
            year_month: YearMonthDuration::default(),
            day_time: value,
        }
    }
}

impl TryFrom<Duration> for DayTimeDuration {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: Duration) -> Result<Self, DecimalOverflowError> {
        if value.year_month == YearMonthDuration::default() {
            Ok(value.day_time)
        } else {
            Err(DecimalOverflowError {})
        }
    }
}

impl TryFrom<StdDuration> for DayTimeDuration {
    type Error = DecimalOverflowError;

    #[inline]
    fn try_from(value: StdDuration) -> Result<Self, DecimalOverflowError> {
        Ok(Self {
            seconds: Decimal::new(
                i128::try_from(value.as_nanos()).map_err(|_| DecimalOverflowError)?,
                9,
            )?,
        })
    }
}

impl FromStr for DayTimeDuration {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(day_time_duration_lexical_rep, input)
    }
}

impl fmt::Display for DayTimeDuration {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Duration::from(*self).fmt(f)
    }
}

impl PartialEq<Duration> for DayTimeDuration {
    #[inline]
    fn eq(&self, other: &Duration) -> bool {
        Duration::from(*self).eq(other)
    }
}

impl PartialEq<DayTimeDuration> for Duration {
    #[inline]
    fn eq(&self, other: &DayTimeDuration) -> bool {
        self.eq(&Self::from(*other))
    }
}

impl PartialEq<YearMonthDuration> for DayTimeDuration {
    #[inline]
    fn eq(&self, other: &YearMonthDuration) -> bool {
        Duration::from(*self).eq(&Duration::from(*other))
    }
}

impl PartialEq<DayTimeDuration> for YearMonthDuration {
    #[inline]
    fn eq(&self, other: &DayTimeDuration) -> bool {
        Duration::from(*self).eq(&Duration::from(*other))
    }
}

impl PartialOrd<Duration> for DayTimeDuration {
    #[inline]
    fn partial_cmp(&self, other: &Duration) -> Option<Ordering> {
        Duration::from(*self).partial_cmp(other)
    }
}

impl PartialOrd<DayTimeDuration> for Duration {
    #[inline]
    fn partial_cmp(&self, other: &DayTimeDuration) -> Option<Ordering> {
        self.partial_cmp(&Self::from(*other))
    }
}

impl PartialOrd<YearMonthDuration> for DayTimeDuration {
    #[inline]
    fn partial_cmp(&self, other: &YearMonthDuration) -> Option<Ordering> {
        Duration::from(*self).partial_cmp(&Duration::from(*other))
    }
}

impl PartialOrd<DayTimeDuration> for YearMonthDuration {
    #[inline]
    fn partial_cmp(&self, other: &DayTimeDuration) -> Option<Ordering> {
        Duration::from(*self).partial_cmp(&Duration::from(*other))
    }
}

impl Neg for DayTimeDuration {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self {
            seconds: self.seconds.neg(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str() -> Result<(), XsdParseError> {
        let min = Duration::new(
            i64::MIN + 1,
            Decimal::MIN.checked_add(Decimal::step()).unwrap(),
        );
        let max = Duration::new(i64::MAX, Decimal::MAX);

        assert_eq!(YearMonthDuration::from_str("P1Y")?.to_string(), "P1Y");
        assert_eq!(Duration::from_str("P1Y")?.to_string(), "P1Y");
        assert_eq!(YearMonthDuration::from_str("P1M")?.to_string(), "P1M");
        assert_eq!(Duration::from_str("P1M")?.to_string(), "P1M");
        assert_eq!(DayTimeDuration::from_str("P1D")?.to_string(), "P1D");
        assert_eq!(Duration::from_str("P1D")?.to_string(), "P1D");
        assert_eq!(DayTimeDuration::from_str("PT1H")?.to_string(), "PT1H");
        assert_eq!(Duration::from_str("PT1H")?.to_string(), "PT1H");
        assert_eq!(DayTimeDuration::from_str("PT1M")?.to_string(), "PT1M");
        assert_eq!(Duration::from_str("PT1M")?.to_string(), "PT1M");
        assert_eq!(DayTimeDuration::from_str("PT1.1S")?.to_string(), "PT1.1S");
        assert_eq!(Duration::from_str("PT1.1S")?.to_string(), "PT1.1S");
        assert_eq!(YearMonthDuration::from_str("-P1Y")?.to_string(), "-P1Y");
        assert_eq!(Duration::from_str("-P1Y")?.to_string(), "-P1Y");
        assert_eq!(YearMonthDuration::from_str("-P1M")?.to_string(), "-P1M");
        assert_eq!(Duration::from_str("-P1M")?.to_string(), "-P1M");
        assert_eq!(DayTimeDuration::from_str("-P1D")?.to_string(), "-P1D");
        assert_eq!(Duration::from_str("-P1D")?.to_string(), "-P1D");
        assert_eq!(DayTimeDuration::from_str("-PT1H")?.to_string(), "-PT1H");
        assert_eq!(Duration::from_str("-PT1H")?.to_string(), "-PT1H");
        assert_eq!(DayTimeDuration::from_str("-PT1M")?.to_string(), "-PT1M");
        assert_eq!(Duration::from_str("-PT1M")?.to_string(), "-PT1M");
        assert_eq!(DayTimeDuration::from_str("-PT1S")?.to_string(), "-PT1S");
        assert_eq!(Duration::from_str("-PT1S")?.to_string(), "-PT1S");
        assert_eq!(DayTimeDuration::from_str("-PT1.1S")?.to_string(), "-PT1.1S");
        assert_eq!(Duration::from_str("-PT1.1S")?.to_string(), "-PT1.1S");
        assert_eq!(Duration::from_str(&max.to_string())?, max);
        assert_eq!(Duration::from_str(&min.to_string())?, min);
        assert_eq!(Duration::from_str("PT0H")?.to_string(), "PT0S");
        assert_eq!(Duration::from_str("-PT0H")?.to_string(), "PT0S");
        assert_eq!(YearMonthDuration::from_str("P0Y")?.to_string(), "P0M");
        assert_eq!(DayTimeDuration::from_str("PT0H")?.to_string(), "PT0S");
        Ok(())
    }

    #[test]
    fn from_std() {
        assert_eq!(
            Duration::try_from(StdDuration::new(10, 10))
                .unwrap()
                .to_string(),
            "PT10.00000001S"
        );
    }

    #[test]
    fn equals() -> Result<(), XsdParseError> {
        assert_eq!(
            YearMonthDuration::from_str("P1Y")?,
            YearMonthDuration::from_str("P12M")?
        );
        assert_eq!(
            YearMonthDuration::from_str("P1Y")?,
            Duration::from_str("P12M")?
        );
        assert_eq!(
            Duration::from_str("P1Y")?,
            YearMonthDuration::from_str("P12M")?
        );
        assert_eq!(Duration::from_str("P1Y")?, Duration::from_str("P12M")?);
        assert_eq!(
            DayTimeDuration::from_str("PT24H")?,
            DayTimeDuration::from_str("P1D")?
        );
        assert_eq!(
            DayTimeDuration::from_str("PT24H")?,
            Duration::from_str("P1D")?
        );
        assert_eq!(
            Duration::from_str("PT24H")?,
            DayTimeDuration::from_str("P1D")?
        );
        assert_eq!(Duration::from_str("PT24H")?, Duration::from_str("P1D")?);
        assert_ne!(Duration::from_str("P1Y")?, Duration::from_str("P365D")?);
        assert_eq!(Duration::from_str("P0Y")?, Duration::from_str("P0D")?);
        assert_ne!(Duration::from_str("P1Y")?, Duration::from_str("P365D")?);
        assert_eq!(Duration::from_str("P2Y")?, Duration::from_str("P24M")?);
        assert_eq!(Duration::from_str("P10D")?, Duration::from_str("PT240H")?);
        assert_eq!(
            Duration::from_str("P2Y0M0DT0H0M0S")?,
            Duration::from_str("P24M")?
        );
        assert_eq!(
            Duration::from_str("P0Y0M10D")?,
            Duration::from_str("PT240H")?
        );
        assert_ne!(Duration::from_str("P1M")?, Duration::from_str("P30D")?);
        Ok(())
    }

    #[test]
    fn years() -> Result<(), XsdParseError> {
        assert_eq!(Duration::from_str("P20Y15M")?.years(), 21);
        assert_eq!(Duration::from_str("-P15M")?.years(), -1);
        assert_eq!(Duration::from_str("-P2DT15H")?.years(), 0);
        Ok(())
    }

    #[test]
    fn months() -> Result<(), XsdParseError> {
        assert_eq!(Duration::from_str("P20Y15M")?.months(), 3);
        assert_eq!(Duration::from_str("-P20Y18M")?.months(), -6);
        assert_eq!(Duration::from_str("-P2DT15H0M0S")?.months(), 0);
        Ok(())
    }

    #[test]
    fn days() -> Result<(), XsdParseError> {
        assert_eq!(Duration::from_str("P3DT10H")?.days(), 3);
        assert_eq!(Duration::from_str("P3DT55H")?.days(), 5);
        assert_eq!(Duration::from_str("P3Y5M")?.days(), 0);
        Ok(())
    }

    #[test]
    fn hours() -> Result<(), XsdParseError> {
        assert_eq!(Duration::from_str("P3DT10H")?.hours(), 10);
        assert_eq!(Duration::from_str("P3DT12H32M12S")?.hours(), 12);
        assert_eq!(Duration::from_str("PT123H")?.hours(), 3);
        assert_eq!(Duration::from_str("-P3DT10H")?.hours(), -10);
        Ok(())
    }

    #[test]
    fn minutes() -> Result<(), XsdParseError> {
        assert_eq!(Duration::from_str("P3DT10H")?.minutes(), 0);
        assert_eq!(Duration::from_str("-P5DT12H30M")?.minutes(), -30);
        Ok(())
    }

    #[test]
    fn seconds() -> Result<(), XsdParseError> {
        assert_eq!(
            Duration::from_str("P3DT10H12.5S")?.seconds(),
            Decimal::from_str("12.5")?
        );
        assert_eq!(
            Duration::from_str("-PT256S")?.seconds(),
            Decimal::from_str("-16.0")?
        );
        Ok(())
    }

    #[test]
    fn add() -> Result<(), XsdParseError> {
        assert_eq!(
            Duration::from_str("P2Y11M")?.checked_add(Duration::from_str("P3Y3M")?),
            Some(Duration::from_str("P6Y2M")?)
        );
        assert_eq!(
            Duration::from_str("P2DT12H5M")?.checked_add(Duration::from_str("P5DT12H")?),
            Some(Duration::from_str("P8DT5M")?)
        );
        Ok(())
    }

    #[test]
    fn sub() -> Result<(), XsdParseError> {
        assert_eq!(
            Duration::from_str("P2Y11M")?.checked_sub(Duration::from_str("P3Y3M")?),
            Some(Duration::from_str("-P4M")?)
        );
        assert_eq!(
            Duration::from_str("P2DT12H")?.checked_sub(Duration::from_str("P1DT10H30M")?),
            Some(Duration::from_str("P1DT1H30M")?)
        );
        Ok(())
    }
}
