use super::decimal::DecimalOverflowError;
use super::parser::*;
use super::*;
use std::cmp::Ordering;
use std::fmt;
use std::ops::Neg;
use std::str::FromStr;
use std::time::Duration as StdDuration;

/// [XML Schema `duration` datatype](https://www.w3.org/TR/xmlschema11-2/#duration) implementation.
///
/// It stores the duration using a pair of a `YearMonthDuration` and a `DayTimeDuration`.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Default)]
pub struct Duration {
    year_month: YearMonthDuration,
    day_time: DayTimeDuration,
}

impl Duration {
    pub fn new(months: impl Into<i64>, seconds: impl Into<Decimal>) -> Self {
        Self {
            year_month: YearMonthDuration::new(months),
            day_time: DayTimeDuration::new(seconds),
        }
    }

    pub fn from_be_bytes(bytes: [u8; 24]) -> Self {
        let mut months = [0; 8];
        months.copy_from_slice(&bytes[0..8]);
        let mut seconds = [8; 16];
        seconds.copy_from_slice(&bytes[8..24]);
        Self {
            year_month: YearMonthDuration::from_be_bytes(months),
            day_time: DayTimeDuration::from_be_bytes(seconds),
        }
    }

    /// [fn:years-from-duration](https://www.w3.org/TR/xpath-functions/#func-years-from-duration)
    pub fn years(&self) -> i64 {
        self.year_month.years()
    }

    /// [fn:months-from-duration](https://www.w3.org/TR/xpath-functions/#func-months-from-duration)
    pub fn months(&self) -> i64 {
        self.year_month.months()
    }

    /// [fn:days-from-duration](https://www.w3.org/TR/xpath-functions/#func-days-from-duration)
    pub fn days(&self) -> i64 {
        self.day_time.days()
    }

    /// [fn:hours-from-duration](https://www.w3.org/TR/xpath-functions/#func-hours-from-duration)
    pub fn hours(&self) -> i64 {
        self.day_time.hours()
    }

    /// [fn:minutes-from-duration](https://www.w3.org/TR/xpath-functions/#func-minutes-from-duration)
    pub fn minutes(&self) -> i64 {
        self.day_time.minutes()
    }

    /// [fn:seconds-from-duration](https://www.w3.org/TR/xpath-functions/#func-seconds-from-duration)
    pub fn seconds(&self) -> Decimal {
        self.day_time.seconds()
    }

    pub(super) const fn all_months(&self) -> i64 {
        self.year_month.all_months()
    }

    pub(super) const fn all_seconds(&self) -> Decimal {
        self.day_time.all_seconds()
    }

    pub fn to_be_bytes(self) -> [u8; 24] {
        let mut bytes = [0; 24];
        bytes[0..8].copy_from_slice(&self.year_month.to_be_bytes());
        bytes[8..24].copy_from_slice(&self.day_time.to_be_bytes());
        bytes
    }

    /// [op:add-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDurations) and [op:add-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDurations)
    pub fn checked_add(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            year_month: self.year_month.checked_add(rhs.year_month)?,
            day_time: self.day_time.checked_add(rhs.day_time)?,
        })
    }

    /// [op:subtract-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDurations) and [op:subtract-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDurations)
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            year_month: self.year_month.checked_sub(rhs.year_month)?,
            day_time: self.day_time.checked_sub(rhs.day_time)?,
        })
    }
}

impl TryFrom<StdDuration> for Duration {
    type Error = DecimalOverflowError;

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
            year_month: self.year_month.neg(),
            day_time: self.day_time.neg(),
        }
    }
}

/// [XML Schema `yearMonthDuration` datatype](https://www.w3.org/TR/xmlschema11-2/#yearMonthDuration) implementation.
///
/// It stores the duration as a number of months encoded using a `i64`
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash, Default)]
pub struct YearMonthDuration {
    months: i64,
}

impl YearMonthDuration {
    pub fn new(months: impl Into<i64>) -> Self {
        Self {
            months: months.into(),
        }
    }

    pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self {
            months: i64::from_be_bytes(bytes),
        }
    }

    /// [fn:years-from-duration](https://www.w3.org/TR/xpath-functions/#func-years-from-duration)
    pub fn years(self) -> i64 {
        self.months / 12
    }

    /// [fn:months-from-duration](https://www.w3.org/TR/xpath-functions/#func-months-from-duration)
    pub fn months(self) -> i64 {
        self.months % 12
    }

    /// [fn:days-from-duration](https://www.w3.org/TR/xpath-functions/#func-days-from-duration)
    #[allow(clippy::unused_self)]
    pub fn days(self) -> i64 {
        0
    }

    /// [fn:hours-from-duration](https://www.w3.org/TR/xpath-functions/#func-hours-from-duration)
    #[allow(clippy::unused_self)]
    pub fn hours(self) -> i64 {
        0
    }

    /// [fn:minutes-from-duration](https://www.w3.org/TR/xpath-functions/#func-minutes-from-duration)
    #[allow(clippy::unused_self)]
    pub fn minutes(self) -> i64 {
        0
    }

    /// [fn:seconds-from-duration](https://www.w3.org/TR/xpath-functions/#func-seconds-from-duration)
    #[allow(clippy::unused_self)]
    pub fn seconds(self) -> Decimal {
        Decimal::default()
    }

    pub(super) const fn all_months(self) -> i64 {
        self.months
    }

    pub fn to_be_bytes(self) -> [u8; 8] {
        self.months.to_be_bytes()
    }

    /// [op:add-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDurations)
    pub fn checked_add(self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            months: self.months.checked_add(rhs.months)?,
        })
    }

    /// [op:subtract-yearMonthDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDurations)
    pub fn checked_sub(self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            months: self.months.checked_sub(rhs.months)?,
        })
    }
}

impl From<YearMonthDuration> for Duration {
    fn from(value: YearMonthDuration) -> Self {
        Self {
            year_month: value,
            day_time: DayTimeDuration::default(),
        }
    }
}

impl TryFrom<Duration> for YearMonthDuration {
    type Error = DecimalOverflowError;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.months == 0 {
            write!(f, "P0M")
        } else {
            Duration::from(*self).fmt(f)
        }
    }
}

impl PartialEq<Duration> for YearMonthDuration {
    fn eq(&self, other: &Duration) -> bool {
        Duration::from(*self).eq(other)
    }
}

impl PartialEq<YearMonthDuration> for Duration {
    fn eq(&self, other: &YearMonthDuration) -> bool {
        self.eq(&Self::from(*other))
    }
}

impl PartialOrd<Duration> for YearMonthDuration {
    fn partial_cmp(&self, other: &Duration) -> Option<Ordering> {
        Duration::from(*self).partial_cmp(other)
    }
}

impl PartialOrd<YearMonthDuration> for Duration {
    fn partial_cmp(&self, other: &YearMonthDuration) -> Option<Ordering> {
        self.partial_cmp(&Self::from(*other))
    }
}

impl Neg for YearMonthDuration {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            months: self.months.neg(),
        }
    }
}

/// [XML Schema `dayTimeDuration` datatype](https://www.w3.org/TR/xmlschema11-2/#dayTimeDuration) implementation.
///
/// It stores the duration as a number of seconds encoded using a `Decimal`
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash, Default)]
pub struct DayTimeDuration {
    seconds: Decimal,
}

impl DayTimeDuration {
    pub fn new(seconds: impl Into<Decimal>) -> Self {
        Self {
            seconds: seconds.into(),
        }
    }

    pub fn from_be_bytes(bytes: [u8; 16]) -> Self {
        Self {
            seconds: Decimal::from_be_bytes(bytes),
        }
    }

    /// [fn:years-from-duration](https://www.w3.org/TR/xpath-functions/#func-years-from-duration)
    #[allow(clippy::unused_self)]
    pub fn years(&self) -> i64 {
        0
    }

    /// [fn:months-from-duration](https://www.w3.org/TR/xpath-functions/#func-months-from-duration)
    #[allow(clippy::unused_self)]
    pub fn months(&self) -> i64 {
        0
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

    pub(super) const fn all_seconds(&self) -> Decimal {
        self.seconds
    }

    pub fn to_be_bytes(self) -> [u8; 16] {
        self.seconds.to_be_bytes()
    }

    /// [op:add-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDurations)
    pub fn checked_add(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            seconds: self.seconds.checked_add(rhs.seconds)?,
        })
    }

    /// [op:subtract-dayTimeDurations](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDurations)
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            seconds: self.seconds.checked_sub(rhs.seconds)?,
        })
    }
}

impl From<DayTimeDuration> for Duration {
    fn from(value: DayTimeDuration) -> Self {
        Self {
            year_month: YearMonthDuration::default(),
            day_time: value,
        }
    }
}

impl TryFrom<Duration> for DayTimeDuration {
    type Error = DecimalOverflowError;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Duration::from(*self).fmt(f)
    }
}

impl PartialEq<Duration> for DayTimeDuration {
    fn eq(&self, other: &Duration) -> bool {
        Duration::from(*self).eq(other)
    }
}

impl PartialEq<DayTimeDuration> for Duration {
    fn eq(&self, other: &DayTimeDuration) -> bool {
        self.eq(&Self::from(*other))
    }
}

impl PartialEq<YearMonthDuration> for DayTimeDuration {
    fn eq(&self, other: &YearMonthDuration) -> bool {
        Duration::from(*self).eq(&Duration::from(*other))
    }
}

impl PartialEq<DayTimeDuration> for YearMonthDuration {
    fn eq(&self, other: &DayTimeDuration) -> bool {
        Duration::from(*self).eq(&Duration::from(*other))
    }
}

impl PartialOrd<Duration> for DayTimeDuration {
    fn partial_cmp(&self, other: &Duration) -> Option<Ordering> {
        Duration::from(*self).partial_cmp(other)
    }
}

impl PartialOrd<DayTimeDuration> for Duration {
    fn partial_cmp(&self, other: &DayTimeDuration) -> Option<Ordering> {
        self.partial_cmp(&Self::from(*other))
    }
}

impl PartialOrd<YearMonthDuration> for DayTimeDuration {
    fn partial_cmp(&self, other: &YearMonthDuration) -> Option<Ordering> {
        Duration::from(*self).partial_cmp(&Duration::from(*other))
    }
}

impl PartialOrd<DayTimeDuration> for YearMonthDuration {
    fn partial_cmp(&self, other: &DayTimeDuration) -> Option<Ordering> {
        Duration::from(*self).partial_cmp(&Duration::from(*other))
    }
}

impl Neg for DayTimeDuration {
    type Output = Self;

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
    fn from_str() {
        let min = Duration::new(
            i64::MIN + 1,
            Decimal::min_value().checked_add(Decimal::step()).unwrap(),
        );
        let max = Duration::new(i64::MAX, Decimal::max_value());

        assert_eq!(
            YearMonthDuration::from_str("P1Y").unwrap().to_string(),
            "P1Y"
        );
        assert_eq!(Duration::from_str("P1Y").unwrap().to_string(), "P1Y");
        assert_eq!(
            YearMonthDuration::from_str("P1M").unwrap().to_string(),
            "P1M"
        );
        assert_eq!(Duration::from_str("P1M").unwrap().to_string(), "P1M");
        assert_eq!(DayTimeDuration::from_str("P1D").unwrap().to_string(), "P1D");
        assert_eq!(Duration::from_str("P1D").unwrap().to_string(), "P1D");
        assert_eq!(
            DayTimeDuration::from_str("PT1H").unwrap().to_string(),
            "PT1H"
        );
        assert_eq!(Duration::from_str("PT1H").unwrap().to_string(), "PT1H");
        assert_eq!(
            DayTimeDuration::from_str("PT1M").unwrap().to_string(),
            "PT1M"
        );
        assert_eq!(Duration::from_str("PT1M").unwrap().to_string(), "PT1M");
        assert_eq!(
            DayTimeDuration::from_str("PT1.1S").unwrap().to_string(),
            "PT1.1S"
        );
        assert_eq!(Duration::from_str("PT1.1S").unwrap().to_string(), "PT1.1S");
        assert_eq!(
            YearMonthDuration::from_str("-P1Y").unwrap().to_string(),
            "-P1Y"
        );
        assert_eq!(Duration::from_str("-P1Y").unwrap().to_string(), "-P1Y");
        assert_eq!(
            YearMonthDuration::from_str("-P1M").unwrap().to_string(),
            "-P1M"
        );
        assert_eq!(Duration::from_str("-P1M").unwrap().to_string(), "-P1M");
        assert_eq!(
            DayTimeDuration::from_str("-P1D").unwrap().to_string(),
            "-P1D"
        );
        assert_eq!(Duration::from_str("-P1D").unwrap().to_string(), "-P1D");
        assert_eq!(
            DayTimeDuration::from_str("-PT1H").unwrap().to_string(),
            "-PT1H"
        );
        assert_eq!(Duration::from_str("-PT1H").unwrap().to_string(), "-PT1H");
        assert_eq!(
            DayTimeDuration::from_str("-PT1M").unwrap().to_string(),
            "-PT1M"
        );
        assert_eq!(Duration::from_str("-PT1M").unwrap().to_string(), "-PT1M");
        assert_eq!(
            DayTimeDuration::from_str("-PT1.1S").unwrap().to_string(),
            "-PT1.1S"
        );
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
            YearMonthDuration::from_str("P1Y").unwrap(),
            YearMonthDuration::from_str("P12M").unwrap()
        );
        assert_eq!(
            YearMonthDuration::from_str("P1Y").unwrap(),
            Duration::from_str("P12M").unwrap()
        );
        assert_eq!(
            Duration::from_str("P1Y").unwrap(),
            YearMonthDuration::from_str("P12M").unwrap()
        );
        assert_eq!(
            Duration::from_str("P1Y").unwrap(),
            Duration::from_str("P12M").unwrap()
        );
        assert_eq!(
            DayTimeDuration::from_str("PT24H").unwrap(),
            DayTimeDuration::from_str("P1D").unwrap()
        );
        assert_eq!(
            DayTimeDuration::from_str("PT24H").unwrap(),
            Duration::from_str("P1D").unwrap()
        );
        assert_eq!(
            Duration::from_str("PT24H").unwrap(),
            DayTimeDuration::from_str("P1D").unwrap()
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
