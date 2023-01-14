use super::parser::{date_lexical_rep, date_time_lexical_rep, parse_value, time_lexical_rep};
use super::{DayTimeDuration, Decimal, Duration, XsdParseError, YearMonthDuration};
use crate::parser::{
    g_day_lexical_rep, g_month_day_lexical_rep, g_month_lexical_rep, g_year_lexical_rep,
    g_year_month_lexical_rep,
};
use std::cmp::{min, Ordering};
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::time::SystemTimeError;

/// [XML Schema `dateTime` datatype](https://www.w3.org/TR/xmlschema11-2/#dateTime)
///
/// It encodes the value using a number of seconds from the Gregorian calendar era using a [`Decimal`]
/// and an optional timezone offset in minutes.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct DateTime {
    timestamp: Timestamp,
}

impl DateTime {
    #[inline]
    pub(super) fn new(
        year: i64,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: Decimal,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeError> {
        Ok(Self {
            timestamp: Timestamp::new(&DateTimeSevenPropertyModel {
                year: Some(year),
                month: Some(month),
                day: Some(day),
                hour: Some(hour),
                minute: Some(minute),
                second: Some(second),
                timezone_offset,
            })?,
        })
    }

    #[inline]
    pub fn now() -> Result<Self, DateTimeError> {
        Ok(Self {
            timestamp: Timestamp::now()?,
        })
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    /// [fn:year-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-year-from-dateTime)
    #[inline]
    pub fn year(&self) -> i64 {
        self.timestamp.year()
    }

    /// [fn:month-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-month-from-dateTime)
    #[inline]
    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    /// [fn:day-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-day-from-dateTime)
    #[inline]
    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    /// [fn:hour-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-hour-from-dateTime)
    #[inline]
    pub fn hour(&self) -> u8 {
        self.timestamp.hour()
    }

    /// [fn:minute-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-minute-from-dateTime)
    #[inline]
    pub fn minute(&self) -> u8 {
        self.timestamp.minute()
    }

    /// [fn:second-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-second-from-dateTime)
    #[inline]
    pub fn second(&self) -> Decimal {
        self.timestamp.second()
    }

    /// [fn:timezone-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-timezone-from-dateTime)
    #[inline]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    fn properties(&self) -> DateTimeSevenPropertyModel {
        DateTimeSevenPropertyModel {
            year: Some(self.year()),
            month: Some(self.month()),
            day: Some(self.day()),
            hour: Some(self.hour()),
            minute: Some(self.minute()),
            second: Some(self.second()),
            timezone_offset: self.timezone_offset(),
        }
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// [op:subtract-dateTimes](https://www.w3.org/TR/xpath-functions/#func-subtract-dateTimes)
    #[inline]
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<DayTimeDuration> {
        self.timestamp.checked_sub(rhs.into().timestamp)
    }

    /// [op:add-yearMonthDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDuration-to-dateTime)
    #[inline]
    pub fn checked_add_year_month_duration(
        &self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-dateTime)
    #[inline]
    pub fn checked_add_day_time_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            timestamp: self.timestamp.checked_add_seconds(rhs.all_seconds())?,
        })
    }

    /// [op:add-yearMonthDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDuration-to-dateTime) and [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-dateTime)
    #[inline]
    pub fn checked_add_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        let rhs = rhs.into();
        if let Ok(rhs) = DayTimeDuration::try_from(rhs) {
            self.checked_add_day_time_duration(rhs)
        } else {
            Some(Self {
                timestamp: Timestamp::new(&date_time_plus_duration(rhs, &self.properties())?)
                    .ok()?,
            })
        }
    }

    /// [op:subtract-yearMonthDuration-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDuration-from-dateTime)
    #[inline]
    pub fn checked_sub_year_month_duration(
        &self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-dayTimeDuration-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-dateTime)
    #[inline]
    pub fn checked_sub_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            timestamp: self.timestamp.checked_sub_seconds(rhs.all_seconds())?,
        })
    }

    /// [op:subtract-yearMonthDuration-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDuration-from-dateTime) and [op:subtract-dayTimeDuration-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-dateTime)
    #[inline]
    pub fn checked_sub_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        let rhs = rhs.into();
        if let Ok(rhs) = DayTimeDuration::try_from(rhs) {
            self.checked_sub_day_time_duration(rhs)
        } else {
            Some(Self {
                timestamp: Timestamp::new(&date_time_plus_duration(-rhs, &self.properties())?)
                    .ok()?,
            })
        }
    }

    /// [fn:adjust-dateTime-to-timezone](https://www.w3.org/TR/xpath-functions/#func-adjust-dateTime-to-timezone)
    #[inline]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for DateTime {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date: Date) -> Result<Self, DateTimeError> {
        Self::new(
            date.year(),
            date.month(),
            date.day(),
            0,
            0,
            Decimal::default(),
            date.timezone_offset(),
        )
    }
}

impl FromStr for DateTime {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(date_time_lexical_rep, input)
    }
}

impl fmt::Display for DateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            write!(f, "-")?;
        }
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            year.abs(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        )?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{timezone_offset}")?;
        }
        Ok(())
    }
}

/// [XML Schema `time` datatype](https://www.w3.org/TR/xmlschema11-2/#time)
///
/// It encodes the value using a number of seconds from the Gregorian calendar era using a [`Decimal`],
/// when combined with the date 1972-12-31, and an optional timezone offset in minutes.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct Time {
    timestamp: Timestamp,
}

impl Time {
    #[inline]
    pub(super) fn new(
        mut hour: u8,
        minute: u8,
        second: Decimal,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeError> {
        if hour == 24 && minute == 0 && second == Decimal::default() {
            hour = 0;
        }
        Ok(Self {
            timestamp: Timestamp::new(&DateTimeSevenPropertyModel {
                year: None,
                month: None,
                day: None,
                hour: Some(hour),
                minute: Some(minute),
                second: Some(second),
                timezone_offset,
            })?,
        })
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    /// [fn:hour-from-time](https://www.w3.org/TR/xpath-functions/#func-hour-from-time)
    #[inline]
    pub fn hour(&self) -> u8 {
        self.timestamp.hour()
    }

    /// [fn:minute-from-time](https://www.w3.org/TR/xpath-functions/#func-minute-from-time)
    #[inline]
    pub fn minute(&self) -> u8 {
        self.timestamp.minute()
    }

    /// [fn:second-from-time](https://www.w3.org/TR/xpath-functions/#func-second-from-time)
    #[inline]
    pub fn second(&self) -> Decimal {
        self.timestamp.second()
    }

    /// [fn:timezone-from-time](https://www.w3.org/TR/xpath-functions/#func-timezone-from-time)
    #[inline]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// [op:subtract-times](https://www.w3.org/TR/xpath-functions/#func-subtract-times)
    #[inline]
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<DayTimeDuration> {
        self.timestamp.checked_sub(rhs.into().timestamp)
    }

    /// [op:add-dayTimeDuration-to-time](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-time)
    #[inline]
    pub fn checked_add_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-dayTimeDuration-to-time](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-time)
    #[inline]
    pub fn checked_add_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        DateTime::new(
            1972,
            12,
            31,
            self.hour(),
            self.minute(),
            self.second(),
            self.timezone_offset(),
        )
        .ok()?
        .checked_add_duration(rhs)?
        .try_into()
        .ok()
    }

    /// [op:subtract-dayTimeDuration-from-time](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-time)
    #[inline]
    pub fn checked_sub_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-dayTimeDuration-from-time](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-time)
    #[inline]
    pub fn checked_sub_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        DateTime::new(
            1972,
            12,
            31,
            self.hour(),
            self.minute(),
            self.second(),
            self.timezone_offset(),
        )
        .ok()?
        .checked_sub_duration(rhs)?
        .try_into()
        .ok()
    }

    // [fn:adjust-time-to-timezone](https://www.w3.org/TR/xpath-functions/#func-adjust-time-to-timezone)
    #[inline]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        DateTime::new(
            1972,
            12,
            31,
            self.hour(),
            self.minute(),
            self.second(),
            self.timezone_offset(),
        )
        .ok()?
        .adjust(timezone_offset)?
        .try_into()
        .ok()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for Time {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(
            date_time.hour(),
            date_time.minute(),
            date_time.second(),
            date_time.timezone_offset(),
        )
    }
}

impl FromStr for Time {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(time_lexical_rep, input)
    }
}

impl fmt::Display for Time {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02}:{:02}:{:02}",
            self.hour(),
            self.minute(),
            self.second()
        )?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{timezone_offset}")?;
        }
        Ok(())
    }
}

/// [XML Schema `date` datatype](https://www.w3.org/TR/xmlschema11-2/#date)
///
/// It encodes the value using a number of seconds from the Gregorian calendar era using a [`Decimal`],
/// when combined with the time 00:00:00, and an optional timezone offset in minutes.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct Date {
    timestamp: Timestamp,
}

impl Date {
    #[inline]
    pub(super) fn new(
        year: i64,
        month: u8,
        day: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeError> {
        Ok(Self {
            timestamp: Timestamp::new(&DateTimeSevenPropertyModel {
                year: Some(year),
                month: Some(month),
                day: Some(day),
                hour: None,
                minute: None,
                second: None,
                timezone_offset,
            })?,
        })
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    /// [fn:year-from-date](https://www.w3.org/TR/xpath-functions/#func-year-from-date)
    #[inline]
    pub fn year(&self) -> i64 {
        self.timestamp.year()
    }

    /// [fn:month-from-date](https://www.w3.org/TR/xpath-functions/#func-month-from-date)
    #[inline]
    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    /// [fn:day-from-date](https://www.w3.org/TR/xpath-functions/#func-day-from-date)
    #[inline]
    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    /// [fn:timezone-from-date](https://www.w3.org/TR/xpath-functions/#func-timezone-from-date)
    #[inline]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// [op:subtract-dates](https://www.w3.org/TR/xpath-functions/#func-subtract-dates)
    #[inline]
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<DayTimeDuration> {
        self.timestamp.checked_sub(rhs.into().timestamp)
    }

    /// [op:add-yearMonthDuration-to-date](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDuration-to-date)
    #[inline]
    pub fn checked_add_year_month_duration(
        &self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-date)
    #[inline]
    pub fn checked_add_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-yearMonthDuration-to-date](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDuration-to-date) and [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-date)
    #[inline]
    pub fn checked_add_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        DateTime::try_from(*self)
            .ok()?
            .checked_add_duration(rhs)?
            .try_into()
            .ok()
    }

    /// [op:subtract-yearMonthDuration-from-date](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDuration-from-date)
    #[inline]
    pub fn checked_sub_year_month_duration(
        &self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-dayTimeDuration-from-date](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-date)
    #[inline]
    pub fn checked_sub_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-yearMonthDuration-from-date](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDuration-from-date) and [op:subtract-dayTimeDuration-from-date](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-date)
    #[inline]
    pub fn checked_sub_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        DateTime::try_from(*self)
            .ok()?
            .checked_sub_duration(rhs)?
            .try_into()
            .ok()
    }

    // [fn:adjust-date-to-timezone](https://www.w3.org/TR/xpath-functions/#func-adjust-date-to-timezone)
    #[inline]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        DateTime::new(
            self.year(),
            self.month(),
            self.day(),
            0,
            0,
            Decimal::default(),
            self.timezone_offset(),
        )
        .ok()?
        .adjust(timezone_offset)?
        .try_into()
        .ok()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for Date {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(
            date_time.year(),
            date_time.month(),
            date_time.day(),
            date_time.timezone_offset(),
        )
    }
}

impl FromStr for Date {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(date_lexical_rep, input)
    }
}

impl fmt::Display for Date {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            write!(f, "-")?;
        }
        write!(f, "{:04}-{:02}-{:02}", year.abs(), self.month(), self.day())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{timezone_offset}")?;
        }
        Ok(())
    }
}

/// [XML Schema `gYearMonth` datatype](https://www.w3.org/TR/xmlschema11-2/#gYearMonth)
///
/// It encodes the value using a number of seconds from the Gregorian calendar era using a [`Decimal`],
/// when combined with the day-time 31T00:00:00, and an optional timezone offset in minutes.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GYearMonth {
    timestamp: Timestamp,
}

impl GYearMonth {
    #[inline]
    pub(super) fn new(
        year: i64,
        month: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeError> {
        Ok(Self {
            timestamp: Timestamp::new(&DateTimeSevenPropertyModel {
                year: Some(year),
                month: Some(month),
                day: None,
                hour: None,
                minute: None,
                second: None,
                timezone_offset,
            })?,
        })
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn year(&self) -> i64 {
        self.timestamp.year()
    }

    #[inline]
    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    #[inline]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GYearMonth {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(
            date_time.year(),
            date_time.month(),
            date_time.timezone_offset(),
        )
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for GYearMonth {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date: Date) -> Result<Self, DateTimeError> {
        Self::new(date.year(), date.month(), date.timezone_offset())
    }
}

impl FromStr for GYearMonth {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(g_year_month_lexical_rep, input)
    }
}

impl fmt::Display for GYearMonth {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            write!(f, "-")?;
        }
        write!(f, "{:04}-{:02}", year.abs(), self.month())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{timezone_offset}")?;
        }
        Ok(())
    }
}

/// [XML Schema `gYear` datatype](https://www.w3.org/TR/xmlschema11-2/#gYear)
///
/// It encodes the value using a number of seconds from the Gregorian calendar era using a [`Decimal`],
/// when combined with the month-day-time 12-31T00:00:00, and an optional timezone offset in minutes.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GYear {
    timestamp: Timestamp,
}

impl GYear {
    #[inline]
    pub(super) fn new(
        year: i64,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeError> {
        Ok(Self {
            timestamp: Timestamp::new(&DateTimeSevenPropertyModel {
                year: Some(year),
                month: None,
                day: None,
                hour: None,
                minute: None,
                second: None,
                timezone_offset,
            })?,
        })
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn year(&self) -> i64 {
        self.timestamp.year()
    }

    #[inline]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GYear {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(date_time.year(), date_time.timezone_offset())
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for GYear {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date: Date) -> Result<Self, DateTimeError> {
        Self::new(date.year(), date.timezone_offset())
    }
}

impl TryFrom<GYearMonth> for GYear {
    type Error = DateTimeError;

    #[inline]
    fn try_from(year_month: GYearMonth) -> Result<Self, DateTimeError> {
        Self::new(year_month.year(), year_month.timezone_offset())
    }
}

impl FromStr for GYear {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(g_year_lexical_rep, input)
    }
}

impl fmt::Display for GYear {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            write!(f, "-")?;
        }
        write!(f, "{:04}", year.abs())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{timezone_offset}")?;
        }
        Ok(())
    }
}

/// [XML Schema `gMonthDay` datatype](https://www.w3.org/TR/xmlschema11-2/#gMonthDay)
///
/// It encodes the value using a number of seconds from the Gregorian calendar era using a [`Decimal`],
/// when combined with the year 1972 and the time 31T00:00:00, and an optional timezone offset in minutes.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GMonthDay {
    timestamp: Timestamp,
}

impl GMonthDay {
    #[inline]
    pub(super) fn new(
        month: u8,
        day: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeError> {
        Ok(Self {
            timestamp: Timestamp::new(&DateTimeSevenPropertyModel {
                year: None,
                month: Some(month),
                day: Some(day),
                hour: None,
                minute: None,
                second: None,
                timezone_offset,
            })?,
        })
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    #[inline]
    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    #[inline]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GMonthDay {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(
            date_time.month(),
            date_time.day(),
            date_time.timezone_offset(),
        )
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for GMonthDay {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date: Date) -> Result<Self, DateTimeError> {
        Self::new(date.month(), date.day(), date.timezone_offset())
    }
}

impl FromStr for GMonthDay {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(g_month_day_lexical_rep, input)
    }
}

impl fmt::Display for GMonthDay {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "--{:02}-{:02}", self.month(), self.day())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{timezone_offset}")?;
        }
        Ok(())
    }
}

/// [XML Schema `gMonth` datatype](https://www.w3.org/TR/xmlschema11-2/#gMonth)
///
/// It encodes the value using a number of seconds from the Gregorian calendar era using a [`Decimal`],
/// when combined with the year 1972 and the day-time 31T00:00:00, and an optional timezone offset in minutes.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GMonth {
    timestamp: Timestamp,
}

impl GMonth {
    #[inline]
    pub(super) fn new(
        month: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeError> {
        Ok(Self {
            timestamp: Timestamp::new(&DateTimeSevenPropertyModel {
                year: None,
                month: Some(month),
                day: None,
                hour: None,
                minute: None,
                second: None,
                timezone_offset,
            })?,
        })
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    #[inline]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GMonth {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(date_time.month(), date_time.timezone_offset())
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for GMonth {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date: Date) -> Result<Self, DateTimeError> {
        Self::new(date.month(), date.timezone_offset())
    }
}

impl TryFrom<GYearMonth> for GMonth {
    type Error = DateTimeError;

    #[inline]
    fn try_from(year_month: GYearMonth) -> Result<Self, DateTimeError> {
        Self::new(year_month.month(), year_month.timezone_offset())
    }
}

impl TryFrom<GMonthDay> for GMonth {
    type Error = DateTimeError;

    #[inline]
    fn try_from(month_day: GMonthDay) -> Result<Self, DateTimeError> {
        Self::new(month_day.month(), month_day.timezone_offset())
    }
}

impl FromStr for GMonth {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(g_month_lexical_rep, input)
    }
}

impl fmt::Display for GMonth {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "--{:02}", self.month())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{timezone_offset}")?;
        }
        Ok(())
    }
}

/// [XML Schema `date` datatype](https://www.w3.org/TR/xmlschema11-2/#date)
///
/// It encodes the value using a number of seconds from the Gregorian calendar era using a [`Decimal`],
/// when combined with the year-month 1972-12 and the 00:00:00, and an optional timezone offset in minutes.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GDay {
    timestamp: Timestamp,
}

impl GDay {
    #[inline]
    pub(super) fn new(
        day: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeError> {
        Ok(Self {
            timestamp: Timestamp::new(&DateTimeSevenPropertyModel {
                year: None,
                month: None,
                day: Some(day),
                hour: None,
                minute: None,
                second: None,
                timezone_offset,
            })?,
        })
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    #[inline]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GDay {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(date_time.day(), date_time.timezone_offset())
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for GDay {
    type Error = DateTimeError;

    #[inline]
    fn try_from(date: Date) -> Result<Self, DateTimeError> {
        Self::new(date.day(), date.timezone_offset())
    }
}

impl TryFrom<GMonthDay> for GDay {
    type Error = DateTimeError;

    #[inline]
    fn try_from(month_day: GMonthDay) -> Result<Self, DateTimeError> {
        Self::new(month_day.day(), month_day.timezone_offset())
    }
}

impl FromStr for GDay {
    type Err = XsdParseError;

    fn from_str(input: &str) -> Result<Self, XsdParseError> {
        parse_value(g_day_lexical_rep, input)
    }
}

impl fmt::Display for GDay {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "---{:02}", self.day())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{timezone_offset}")?;
        }
        Ok(())
    }
}

/// A timezone offset with respect to UTC.
///
/// It is encoded as a number of minutes between -PT14H and PT14H.
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct TimezoneOffset {
    offset: i16, // in minute with respect to UTC
}

impl TimezoneOffset {
    /// From offset in minute with respect to UTC
    #[inline]
    pub fn new(offset_in_minutes: i16) -> Result<Self, DateTimeError> {
        let value = Self {
            offset: offset_in_minutes,
        };
        if Self::MIN <= value && value <= Self::MAX {
            Ok(value)
        } else {
            Err(DATE_TIME_OVERFLOW)
        }
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 2]) -> Self {
        Self {
            offset: i16::from_be_bytes(bytes),
        }
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 2] {
        self.offset.to_be_bytes()
    }

    pub const MIN: Self = Self { offset: -14 * 60 };
    pub const UTC: Self = Self { offset: 0 };
    pub const MAX: Self = Self { offset: 14 * 60 };
}

impl TryFrom<DayTimeDuration> for TimezoneOffset {
    type Error = DateTimeError;

    #[inline]
    fn try_from(value: DayTimeDuration) -> Result<Self, DateTimeError> {
        let result = Self::new((value.minutes() + value.hours() * 60) as i16)?;
        if DayTimeDuration::from(result) == value {
            Ok(result)
        } else {
            // The value is not an integral number of minutes or overflow problems
            Err(DATE_TIME_OVERFLOW)
        }
    }
}

impl TryFrom<Duration> for TimezoneOffset {
    type Error = DateTimeError;

    #[inline]
    fn try_from(value: Duration) -> Result<Self, DateTimeError> {
        DayTimeDuration::try_from(value)
            .map_err(|_| DATE_TIME_OVERFLOW)?
            .try_into()
    }
}

impl From<TimezoneOffset> for DayTimeDuration {
    #[inline]
    fn from(value: TimezoneOffset) -> Self {
        Self::new(i64::from(value.offset) * 60)
    }
}

impl From<TimezoneOffset> for Duration {
    #[inline]
    fn from(value: TimezoneOffset) -> Self {
        DayTimeDuration::from(value).into()
    }
}

impl fmt::Display for TimezoneOffset {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.offset {
            0 => write!(f, "Z"),
            offset if offset < 0 => write!(f, "-{:02}:{:02}", -offset / 60, -offset % 60),
            offset => write!(f, "+{:02}:{:02}", offset / 60, offset % 60),
        }
    }
}

/// [The Date/time Seven-property model](https://www.w3.org/TR/xmlschema11-2/#dt-dt-7PropMod)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
struct DateTimeSevenPropertyModel {
    year: Option<i64>,
    month: Option<u8>,
    day: Option<u8>,
    hour: Option<u8>,
    minute: Option<u8>,
    second: Option<Decimal>,
    timezone_offset: Option<TimezoneOffset>,
}

#[derive(Debug, Clone, Copy)]
struct Timestamp {
    value: Decimal,
    timezone_offset: Option<TimezoneOffset>,
}

impl PartialEq for Timestamp {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self.timezone_offset, other.timezone_offset) {
            (Some(_), Some(_)) | (None, None) => self.value.eq(&other.value),
            _ => false, //TODO: implicit timezone
        }
    }
}

impl Eq for Timestamp {}

impl PartialOrd for Timestamp {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.timezone_offset, other.timezone_offset) {
            (Some(_), Some(_)) | (None, None) => self.value.partial_cmp(&other.value),
            (Some(_), None) => {
                let plus_result = self
                    .value
                    .partial_cmp(&(other.value.checked_add(14 * 3600)?));
                let minus_result = self
                    .value
                    .partial_cmp(&(other.value.checked_sub(14 * 3600)?));
                if plus_result == minus_result {
                    plus_result
                } else {
                    None
                }
            }
            (None, Some(_)) => {
                let plus_result = self.value.checked_add(14 * 3600)?.partial_cmp(&other.value);
                let minus_result = self.value.checked_sub(14 * 3600)?.partial_cmp(&other.value);
                if plus_result == minus_result {
                    plus_result
                } else {
                    None
                }
            }
        }
    }
}

impl Hash for Timestamp {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state)
    }
}

impl Timestamp {
    #[inline]
    fn new(props: &DateTimeSevenPropertyModel) -> Result<Self, DateTimeError> {
        // Validation
        if let (Some(day), Some(month)) = (props.day, props.month) {
            // Constraint: Day-of-month Values
            if day > days_in_month(props.year, month) {
                return Err(DateTimeError {
                    kind: DateTimeErrorKind::InvalidDayOfMonth { day, month },
                });
            }
        }

        Ok(Self {
            timezone_offset: props.timezone_offset,
            value: time_on_timeline(props).ok_or(DATE_TIME_OVERFLOW)?,
        })
    }

    #[inline]
    fn now() -> Result<Self, DateTimeError> {
        Self::new(
            &date_time_plus_duration(
                since_unix_epoch()?,
                &DateTimeSevenPropertyModel {
                    year: Some(1970),
                    month: Some(1),
                    day: Some(1),
                    hour: Some(0),
                    minute: Some(0),
                    second: Some(Decimal::default()),
                    timezone_offset: Some(TimezoneOffset::UTC),
                },
            )
            .ok_or(DATE_TIME_OVERFLOW)?,
        )
    }

    #[inline]
    fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            value: Decimal::from_be_bytes(bytes[0..16].try_into().unwrap()),
            timezone_offset: if bytes[16..18] == [u8::MAX; 2] {
                None
            } else {
                Some(TimezoneOffset::from_be_bytes(
                    bytes[16..18].try_into().unwrap(),
                ))
            },
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[inline]
    fn year_month_day(&self) -> (i64, u8, u8) {
        let mut days = (self.value.as_i128()
            + i128::from(self.timezone_offset.unwrap_or(TimezoneOffset::UTC).offset) * 60)
            .div_euclid(86400)
            + 366;

        // Make days positive
        let shift = if days < 0 {
            let shift = days / 146_097 - 1;
            days -= shift * 146_097;
            shift * 400
        } else {
            0
        };

        let year_mul_400 = days / 146_097;
        days -= year_mul_400 * 146_097;

        days -= 1;
        let year_mul_100 = days / 36524;
        days -= year_mul_100 * 36524;

        days += 1;
        let year_mul_4 = days / 1461;
        days -= year_mul_4 * 1461;

        days -= 1;
        let year_mod_4 = days / 365;
        days -= year_mod_4 * 365;

        let year =
            (400 * year_mul_400 + 100 * year_mul_100 + 4 * year_mul_4 + year_mod_4 + shift) as i64;

        let is_leap_year = (year_mul_100 == 0 || year_mul_4 != 0) && year_mod_4 == 0;
        days += i128::from(is_leap_year);

        let mut month = 0;
        for month_i in 1..=12 {
            let days_in_month = i128::from(days_in_month(Some(year), month_i));
            if days_in_month > days {
                month = month_i;
                break;
            }
            days -= days_in_month
        }
        let day = days as u8 + 1;

        (year, month, day)
    }

    #[inline]
    fn year(&self) -> i64 {
        let (year, _, _) = self.year_month_day();
        year
    }

    #[inline]
    fn month(&self) -> u8 {
        let (_, month, _) = self.year_month_day();
        month
    }

    #[inline]
    fn day(&self) -> u8 {
        let (_, _, day) = self.year_month_day();
        day
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[inline]
    fn hour(&self) -> u8 {
        (((self.value.as_i128()
            + i128::from(self.timezone_offset.unwrap_or(TimezoneOffset::UTC).offset) * 60)
            .rem_euclid(86400))
            / 3600) as u8
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[inline]
    fn minute(&self) -> u8 {
        (((self.value.as_i128()
            + i128::from(self.timezone_offset.unwrap_or(TimezoneOffset::UTC).offset) * 60)
            .rem_euclid(3600))
            / 60) as u8
    }

    #[inline]
    fn second(&self) -> Decimal {
        self.value.checked_rem_euclid(60).unwrap().abs()
    }

    #[inline]
    const fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timezone_offset
    }

    #[inline]
    fn checked_add_seconds(&self, seconds: impl Into<Decimal>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_add(seconds.into())?,
            timezone_offset: self.timezone_offset,
        })
    }

    #[inline]
    fn checked_sub(&self, rhs: Self) -> Option<DayTimeDuration> {
        match (self.timezone_offset, rhs.timezone_offset) {
            (Some(_), Some(_)) | (None, None) => {
                Some(DayTimeDuration::new(self.value.checked_sub(rhs.value)?))
            }
            _ => None, //TODO: implicit timezone
        }
    }

    #[inline]
    fn checked_sub_seconds(&self, seconds: Decimal) -> Option<Self> {
        Some(Self {
            value: self.value.checked_sub(seconds)?,
            timezone_offset: self.timezone_offset,
        })
    }

    #[inline]
    fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(if let Some(from_timezone) = self.timezone_offset {
            if let Some(to_timezone) = timezone_offset {
                Self {
                    value: self.value, // We keep the timestamp
                    timezone_offset: Some(to_timezone),
                }
            } else {
                Self {
                    value: self
                        .value
                        .checked_add(i64::from(from_timezone.offset) * 60)?, // We keep the literal value
                    timezone_offset: None,
                }
            }
        } else if let Some(to_timezone) = timezone_offset {
            Self {
                value: self.value.checked_sub(i64::from(to_timezone.offset) * 60)?, // We keep the literal value
                timezone_offset: Some(to_timezone),
            }
        } else {
            Self {
                value: self.value,
                timezone_offset: None,
            }
        })
    }

    #[inline]
    fn to_be_bytes(self) -> [u8; 18] {
        let mut bytes = [0; 18];
        bytes[0..16].copy_from_slice(&self.value.to_be_bytes());
        bytes[16..18].copy_from_slice(&match &self.timezone_offset {
            Some(timezone_offset) => timezone_offset.to_be_bytes(),
            None => [u8::MAX; 2],
        });
        bytes
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.value == other.value && self.timezone_offset == other.timezone_offset
    }
}

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
fn since_unix_epoch() -> Result<Duration, DateTimeError> {
    Ok(Duration::new(
        0,
        Decimal::try_from(crate::Double::from(js_sys::Date::now() / 1000.))
            .map_err(|_| DATE_TIME_OVERFLOW)?,
    ))
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
fn since_unix_epoch() -> Result<Duration, DateTimeError> {
    use std::time::SystemTime;

    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .try_into()
        .map_err(|_| DATE_TIME_OVERFLOW)
}

/// The [normalizeMonth](https://www.w3.org/TR/xmlschema11-2/#f-dt-normMo) function
fn normalize_month(yr: i64, mo: i64) -> Option<(i64, u8)> {
    if mo >= 0 {
        let yr = yr.checked_add(mo.checked_sub(1)?.checked_div(12)?)?;
        let mo = u8::try_from(mo.checked_sub(1)?.checked_rem(12)?.abs().checked_add(1)?).ok()?;
        Some((yr, mo))
    } else {
        // Needed to make it work with negative durations
        let yr = yr.checked_add(mo.checked_sub(1)?.checked_div(12)?.checked_sub(1)?)?;
        let mo = u8::try_from(
            12_i64
                .checked_add(mo.checked_sub(1)?.checked_rem(12)?)?
                .checked_add(1)?,
        )
        .ok()?;
        Some((yr, mo))
    }
}

/// The [normalizeDa](https://www.w3.org/TR/xmlschema11-2/#f-dt-normDa) function
fn normalize_day(yr: i64, mo: i64, mut da: i64) -> Option<(i64, u8, u8)> {
    let (mut yr, mut mo) = normalize_month(yr, mo)?;
    loop {
        if da <= 0 {
            let (yr2, mo2) = normalize_month(yr, i64::from(mo).checked_sub(1)?)?;
            yr = yr2;
            mo = mo2;
            da = da.checked_add(days_in_month(Some(yr), mo).into())?;
        } else if da > days_in_month(Some(yr), mo).into() {
            da = da.checked_sub(days_in_month(Some(yr), mo).into())?;
            let (yr2, mo2) = normalize_month(yr, i64::from(mo).checked_add(1)?)?;
            yr = yr2;
            mo = mo2;
        } else {
            return Some((yr, mo, u8::try_from(da).ok()?));
        };
    }
}

/// The [normalizeMinute](https://www.w3.org/TR/xmlschema11-2/#f-dt-normMi) function
fn normalize_minute(yr: i64, mo: i64, da: i64, hr: i64, mi: i64) -> Option<(i64, u8, u8, u8, u8)> {
    let hr = hr.checked_add(mi.checked_div(60)?)?;
    let mi = mi.checked_rem(60)?;
    let da = da.checked_add(hr.checked_div(24)?)?;
    let hr = hr.checked_rem(24)?;
    let (yr, mo, da) = normalize_day(yr, mo, da)?;
    Some((yr, mo, da, u8::try_from(hr).ok()?, u8::try_from(mi).ok()?))
}

/// The [normalizeSecond](https://www.w3.org/TR/xmlschema11-2/#f-dt-normSe) function
fn normalize_second(
    yr: i64,
    mo: i64,
    da: i64,
    hr: i64,
    mi: i64,
    se: Decimal,
) -> Option<(i64, u8, u8, u8, u8, Decimal)> {
    let mi = mi.checked_add(i64::try_from(se.as_i128().checked_div(60)?).ok()?)?; //TODO: good idea?
    let se = se.checked_rem(60)?;
    let (yr, mo, da, hr, mi) = normalize_minute(yr, mo, da, hr, mi)?;
    Some((yr, mo, da, hr, mi, se))
}

/// The [daysInMonth](https://www.w3.org/TR/xmlschema11-2/#f-daysInMonth) function
fn days_in_month(y: Option<i64>, m: u8) -> u8 {
    match m {
        2 => {
            if let Some(y) = y {
                if y % 4 != 0 || (y % 100 == 0 && y % 400 != 0) {
                    28
                } else {
                    29
                }
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The [dateTimePlusDuration](https://www.w3.org/TR/xmlschema11-2/#vp-dt-dateTimePlusDuration) function
fn date_time_plus_duration(
    du: Duration,
    dt: &DateTimeSevenPropertyModel,
) -> Option<DateTimeSevenPropertyModel> {
    let yr = dt.year.unwrap_or(1);
    let mo = dt.month.unwrap_or(1);
    let da = dt.day.unwrap_or(1);
    let hr = dt.hour.unwrap_or(0);
    let mi = dt.minute.unwrap_or(0);
    let se = dt.second.unwrap_or_default();
    let mo = i64::from(mo).checked_add(du.all_months())?;
    let (yr, mo) = normalize_month(yr, mo)?;
    let da = min(da, days_in_month(Some(yr), mo));
    let se = se.checked_add(du.all_seconds())?;
    let (yr, mo, da, hr, mi, se) =
        normalize_second(yr, mo.into(), da.into(), hr.into(), mi.into(), se)?;

    Some(DateTimeSevenPropertyModel {
        year: dt.year.map(|_| yr),
        month: dt.month.map(|_| mo),
        day: dt.day.map(|_| da),
        hour: dt.hour.map(|_| hr),
        minute: dt.minute.map(|_| mi),
        second: dt.second.map(|_| se),
        timezone_offset: dt.timezone_offset,
    })
}

/// The [timeOnTimeline](https://www.w3.org/TR/xmlschema11-2/#vp-dt-timeOnTimeline) function
fn time_on_timeline(props: &DateTimeSevenPropertyModel) -> Option<Decimal> {
    let yr = props.year.map_or(1971, |y| y - 1);
    let mo = props.month.unwrap_or(12);
    let da = props
        .day
        .map_or_else(|| days_in_month(Some(yr + 1), mo) - 1, |d| d - 1);
    let hr = props.hour.unwrap_or(0);
    let mi = i128::from(props.minute.unwrap_or(0))
        - i128::from(props.timezone_offset.unwrap_or(TimezoneOffset::UTC).offset);
    let se = props.second.unwrap_or_default();

    Decimal::try_from(
        31_536_000 * i128::from(yr)
            + 86400 * i128::from(yr.div_euclid(400) - yr.div_euclid(100) + yr.div_euclid(4))
            + 86400
                * (1..mo)
                    .map(|m| i128::from(days_in_month(Some(yr + 1), m)))
                    .sum::<i128>()
            + 86400 * i128::from(da)
            + 3600 * i128::from(hr)
            + 60 * mi,
    )
    .ok()?
    .checked_add(se)
}

/// An error when doing [`DateTime`] operations.
#[derive(Debug, Clone)]
pub struct DateTimeError {
    kind: DateTimeErrorKind,
}

#[derive(Debug, Clone)]
enum DateTimeErrorKind {
    InvalidDayOfMonth { day: u8, month: u8 },
    Overflow,
    SystemTime(SystemTimeError),
}

impl fmt::Display for DateTimeError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DateTimeErrorKind::InvalidDayOfMonth { day, month } => {
                write!(f, "{day} is not a valid day of {month}")
            }
            DateTimeErrorKind::Overflow => write!(f, "Overflow during date time normalization"),
            DateTimeErrorKind::SystemTime(error) => error.fmt(f),
        }
    }
}

impl Error for DateTimeError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            DateTimeErrorKind::SystemTime(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SystemTimeError> for DateTimeError {
    #[inline]
    fn from(error: SystemTimeError) -> Self {
        Self {
            kind: DateTimeErrorKind::SystemTime(error),
        }
    }
}

const DATE_TIME_OVERFLOW: DateTimeError = DateTimeError {
    kind: DateTimeErrorKind::Overflow,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str() -> Result<(), XsdParseError> {
        assert_eq!(Time::from_str("00:00:00Z")?.to_string(), "00:00:00Z");
        assert_eq!(Time::from_str("00:00:00+00:00")?.to_string(), "00:00:00Z");
        assert_eq!(Time::from_str("00:00:00-00:00")?.to_string(), "00:00:00Z");
        assert_eq!(Time::from_str("00:00:00")?.to_string(), "00:00:00");
        assert_eq!(
            Time::from_str("00:00:00+02:00")?.to_string(),
            "00:00:00+02:00"
        );
        assert_eq!(
            Time::from_str("00:00:00+14:00")?.to_string(),
            "00:00:00+14:00"
        );
        assert_eq!(Time::from_str("24:00:00")?.to_string(), "00:00:00");
        assert_eq!(Time::from_str("24:00:00.00")?.to_string(), "00:00:00");
        assert_eq!(
            Time::from_str("23:59:59.9999999999")?.to_string(),
            "23:59:59.9999999999"
        );

        assert_eq!(Date::from_str("0001-01-01Z")?.to_string(), "0001-01-01Z");
        assert_eq!(Date::from_str("0001-01-01")?.to_string(), "0001-01-01");
        assert_eq!(
            DateTime::from_str("0001-01-01T00:00:00Z")?.to_string(),
            "0001-01-01T00:00:00Z"
        );
        assert_eq!(
            DateTime::from_str("0001-01-01T00:00:00")?.to_string(),
            "0001-01-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("1000000000-01-01T00:00:00")?.to_string(),
            "1000000000-01-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("2001-12-31T23:59:59")?.to_string(),
            "2001-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2004-12-31T23:59:59")?.to_string(),
            "2004-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("1900-12-31T23:59:59")?.to_string(),
            "1900-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2000-12-31T23:59:59")?.to_string(),
            "2000-12-31T23:59:59",
        );
        assert_eq!(
            DateTime::from_str("1899-12-31T23:59:59")?.to_string(),
            "1899-12-31T23:59:59"
        );

        assert_eq!(
            DateTime::from_str("2001-02-28T23:59:59")?.to_string(),
            "2001-02-28T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2004-02-29T23:59:59")?.to_string(),
            "2004-02-29T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("1900-02-28T23:59:59")?.to_string(),
            "1900-02-28T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2000-02-29T23:59:59")?.to_string(),
            "2000-02-29T23:59:59",
        );
        assert_eq!(
            DateTime::from_str("1899-02-28T23:59:59")?.to_string(),
            "1899-02-28T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2001-03-01T00:00:00")?.to_string(),
            "2001-03-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("2004-03-01T00:00:00")?.to_string(),
            "2004-03-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("1900-03-01T00:00:00")?.to_string(),
            "1900-03-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("2000-03-01T00:00:00")?.to_string(),
            "2000-03-01T00:00:00",
        );
        assert_eq!(
            DateTime::from_str("1899-03-01T00:00:00")?.to_string(),
            "1899-03-01T00:00:00"
        );

        assert_eq!(
            DateTime::from_str("-1000000000-01-01T00:00:00")?.to_string(),
            "-1000000000-01-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("-2001-12-31T23:59:59")?.to_string(),
            "-2001-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("-2004-12-31T23:59:59")?.to_string(),
            "-2004-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("-1900-12-31T23:59:59")?.to_string(),
            "-1900-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("-2000-12-31T23:59:59")?.to_string(),
            "-2000-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("-1899-12-31T23:59:59")?.to_string(),
            "-1899-12-31T23:59:59"
        );

        assert_eq!(
            GYearMonth::from_str("-1899-12+01:00")?.to_string(),
            "-1899-12+01:00"
        );
        assert_eq!(GYearMonth::from_str("-1899-12")?.to_string(), "-1899-12");
        assert_eq!(GYear::from_str("-1899+01:00")?.to_string(), "-1899+01:00");
        assert_eq!(GYear::from_str("-1899")?.to_string(), "-1899");
        assert_eq!(
            GMonthDay::from_str("--01-01+01:00")?.to_string(),
            "--01-01+01:00"
        );
        assert_eq!(GMonthDay::from_str("--01-01")?.to_string(), "--01-01");
        assert_eq!(GDay::from_str("---01+01:00")?.to_string(), "---01+01:00");
        assert_eq!(GDay::from_str("---01")?.to_string(), "---01");
        assert_eq!(GMonth::from_str("--01+01:00")?.to_string(), "--01+01:00");
        assert_eq!(GMonth::from_str("--01")?.to_string(), "--01");
        Ok(())
    }

    #[test]
    fn equals() -> Result<(), XsdParseError> {
        assert_eq!(
            DateTime::from_str("2002-04-02T12:00:00-01:00")?,
            DateTime::from_str("2002-04-02T17:00:00+04:00")?
        );
        assert_eq!(
            DateTime::from_str("2002-04-02T12:00:00-05:00")?,
            DateTime::from_str("2002-04-02T23:00:00+06:00")?
        );
        assert_ne!(
            DateTime::from_str("2002-04-02T12:00:00-05:00")?,
            DateTime::from_str("2002-04-02T17:00:00-05:00")?
        );
        assert_eq!(
            DateTime::from_str("2002-04-02T12:00:00-05:00")?,
            DateTime::from_str("2002-04-02T12:00:00-05:00")?
        );
        assert_eq!(
            DateTime::from_str("2002-04-02T23:00:00-04:00")?,
            DateTime::from_str("2002-04-03T02:00:00-01:00")?
        );
        assert_eq!(
            DateTime::from_str("1999-12-31T24:00:00-05:00")?,
            DateTime::from_str("2000-01-01T00:00:00-05:00")?
        );
        assert_ne!(
            DateTime::from_str("2005-04-04T24:00:00-05:00")?,
            DateTime::from_str("2005-04-04T00:00:00-05:00")?
        );

        assert_ne!(
            Date::from_str("2004-12-25Z")?,
            Date::from_str("2004-12-25+07:00")?
        );
        assert_eq!(
            Date::from_str("2004-12-25-12:00")?,
            Date::from_str("2004-12-26+12:00")?
        );

        assert_ne!(
            Time::from_str("08:00:00+09:00")?,
            Time::from_str("17:00:00-06:00")?
        );
        assert_eq!(
            Time::from_str("21:30:00+10:30")?,
            Time::from_str("06:00:00-05:00")?
        );
        assert_eq!(
            Time::from_str("24:00:00+01:00")?,
            Time::from_str("00:00:00+01:00")?
        );

        assert_eq!(
            Time::from_str("05:00:00-03:00")?,
            Time::from_str("10:00:00+02:00")?
        );
        assert_ne!(
            Time::from_str("23:00:00-03:00")?,
            Time::from_str("02:00:00Z")?
        );

        assert_ne!(
            GYearMonth::from_str("1986-02")?,
            GYearMonth::from_str("1986-03")?
        );
        assert_ne!(
            GYearMonth::from_str("1978-03")?,
            GYearMonth::from_str("1978-03Z")?
        );

        assert_ne!(
            GYear::from_str("2005-12:00")?,
            GYear::from_str("2005+12:00")?
        );
        assert_ne!(GYear::from_str("1976-05:00")?, GYear::from_str("1976")?);

        assert_eq!(
            GMonthDay::from_str("--12-25-14:00")?,
            GMonthDay::from_str("--12-26+10:00")?
        );
        assert_ne!(
            GMonthDay::from_str("--12-25")?,
            GMonthDay::from_str("--12-26Z")?
        );

        assert_ne!(
            GMonth::from_str("--12-14:00")?,
            GMonth::from_str("--12+10:00")?
        );
        assert_ne!(GMonth::from_str("--12")?, GMonth::from_str("--12Z")?);

        assert_ne!(
            GDay::from_str("---25-14:00")?,
            GDay::from_str("---25+10:00")?
        );
        assert_ne!(GDay::from_str("---12")?, GDay::from_str("---12Z")?);
        Ok(())
    }

    #[test]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn cmp() -> Result<(), XsdParseError> {
        assert!(Date::from_str("2004-12-25Z")? < Date::from_str("2004-12-25-05:00")?);
        assert!(!(Date::from_str("2004-12-25-12:00")? < Date::from_str("2004-12-26+12:00")?));

        assert!(Date::from_str("2004-12-25Z")? > Date::from_str("2004-12-25+07:00")?);
        assert!(!(Date::from_str("2004-12-25-12:00")? > Date::from_str("2004-12-26+12:00")?));

        assert!(!(Time::from_str("12:00:00")? < Time::from_str("23:00:00+06:00")?));
        assert!(Time::from_str("11:00:00-05:00")? < Time::from_str("17:00:00Z")?);
        assert!(!(Time::from_str("23:59:59")? < Time::from_str("24:00:00")?));

        assert!(!(Time::from_str("08:00:00+09:00")? > Time::from_str("17:00:00-06:00")?));

        assert!(GMonthDay::from_str("--12-12+13:00")? < GMonthDay::from_str("--12-12+11:00")?);
        assert!(GDay::from_str("---15")? < GDay::from_str("---16")?);
        assert!(GDay::from_str("---15-13:00")? > GDay::from_str("---16+13:00")?);
        assert_eq!(
            GDay::from_str("---15-11:00")?,
            GDay::from_str("---16+13:00")?
        );
        assert!(GDay::from_str("---15-13:00")?
            .partial_cmp(&GDay::from_str("---16")?)
            .is_none());
        Ok(())
    }

    #[test]
    fn year() -> Result<(), XsdParseError> {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")?.year(),
            1999
        );
        assert_eq!(
            DateTime::from_str("1999-05-31T21:30:00-05:00")?.year(),
            1999
        );
        assert_eq!(DateTime::from_str("1999-12-31T19:20:00")?.year(), 1999);
        assert_eq!(DateTime::from_str("1999-12-31T24:00:00")?.year(), 2000);
        assert_eq!(DateTime::from_str("-0002-06-06T00:00:00")?.year(), -2);

        assert_eq!(Date::from_str("1999-05-31")?.year(), 1999);
        assert_eq!(Date::from_str("2000-01-01+05:00")?.year(), 2000);
        assert_eq!(Date::from_str("-0002-06-01")?.year(), -2);

        assert_eq!(GYear::from_str("-0002")?.year(), -2);
        assert_eq!(GYearMonth::from_str("-0002-02")?.year(), -2);
        Ok(())
    }

    #[test]
    fn month() -> Result<(), XsdParseError> {
        assert_eq!(DateTime::from_str("1999-05-31T13:20:00-05:00")?.month(), 5);
        assert_eq!(DateTime::from_str("1999-12-31T19:20:00-05:00")?.month(), 12);

        assert_eq!(Date::from_str("1999-05-31-05:00")?.month(), 5);
        assert_eq!(Date::from_str("2000-01-01+05:00")?.month(), 1);

        assert_eq!(GMonth::from_str("--02")?.month(), 2);
        assert_eq!(GYearMonth::from_str("-0002-02")?.month(), 2);
        assert_eq!(GMonthDay::from_str("--02-03")?.month(), 2);
        Ok(())
    }

    #[test]
    fn day() -> Result<(), XsdParseError> {
        assert_eq!(DateTime::from_str("1999-05-31T13:20:00-05:00")?.day(), 31);
        assert_eq!(DateTime::from_str("1999-12-31T20:00:00-05:00")?.day(), 31);

        assert_eq!(Date::from_str("1999-05-31-05:00")?.day(), 31);
        assert_eq!(Date::from_str("2000-01-01+05:00")?.day(), 1);

        assert_eq!(GDay::from_str("---03")?.day(), 3);
        assert_eq!(GMonthDay::from_str("--02-03")?.day(), 3);
        Ok(())
    }

    #[test]
    fn hour() -> Result<(), XsdParseError> {
        assert_eq!(DateTime::from_str("1999-05-31T08:20:00-05:00")?.hour(), 8);
        assert_eq!(DateTime::from_str("1999-12-31T21:20:00-05:00")?.hour(), 21);
        assert_eq!(DateTime::from_str("1999-12-31T12:00:00")?.hour(), 12);
        assert_eq!(DateTime::from_str("1999-12-31T24:00:00")?.hour(), 0);

        assert_eq!(Time::from_str("11:23:00-05:00")?.hour(), 11);
        assert_eq!(Time::from_str("21:23:00-05:00")?.hour(), 21);
        assert_eq!(Time::from_str("01:23:00+05:00")?.hour(), 1);
        assert_eq!(Time::from_str("24:00:00")?.hour(), 0);
        Ok(())
    }

    #[test]
    fn minute() -> Result<(), XsdParseError> {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")?.minute(),
            20
        );
        assert_eq!(
            DateTime::from_str("1999-05-31T13:30:00+05:30")?.minute(),
            30
        );

        assert_eq!(Time::from_str("13:00:00Z")?.minute(), 0);
        Ok(())
    }

    #[test]
    fn second() -> Result<(), XsdParseError> {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")?.second(),
            Decimal::from(0)
        );

        assert_eq!(
            Time::from_str("13:20:10.5")?.second(),
            Decimal::from_str("10.5")?
        );
        Ok(())
    }

    #[test]
    fn timezone() -> Result<(), XsdParseError> {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")?.timezone(),
            Some(DayTimeDuration::from_str("-PT5H")?)
        );
        assert_eq!(
            DateTime::from_str("2000-06-12T13:20:00Z")?.timezone(),
            Some(DayTimeDuration::from_str("PT0S")?)
        );
        assert_eq!(DateTime::from_str("2004-08-27T00:00:00")?.timezone(), None);

        assert_eq!(
            Date::from_str("1999-05-31-05:00")?.timezone(),
            Some(DayTimeDuration::from_str("-PT5H")?)
        );
        assert_eq!(
            Date::from_str("2000-06-12Z")?.timezone(),
            Some(DayTimeDuration::from_str("PT0S")?)
        );

        assert_eq!(
            Time::from_str("13:20:00-05:00")?.timezone(),
            Some(DayTimeDuration::from_str("-PT5H")?)
        );
        assert_eq!(Time::from_str("13:20:00")?.timezone(), None);
        Ok(())
    }

    #[test]
    fn sub() -> Result<(), XsdParseError> {
        assert_eq!(
            DateTime::from_str("2000-10-30T06:12:00-05:00")?
                .checked_sub(DateTime::from_str("1999-11-28T09:00:00Z")?),
            Some(DayTimeDuration::from_str("P337DT2H12M")?)
        );

        assert_eq!(
            Date::from_str("2000-10-30")?.checked_sub(Date::from_str("1999-11-28")?),
            Some(DayTimeDuration::from_str("P337D")?)
        );
        assert_eq!(
            Date::from_str("2000-10-30+05:00")?.checked_sub(Date::from_str("1999-11-28Z")?),
            Some(DayTimeDuration::from_str("P336DT19H")?)
        );
        assert_eq!(
            Date::from_str("2000-10-15-05:00")?.checked_sub(Date::from_str("2000-10-10+02:00")?),
            Some(DayTimeDuration::from_str("P5DT7H")?)
        );

        assert_eq!(
            Time::from_str("11:12:00Z")?.checked_sub(Time::from_str("04:00:00-05:00")?),
            Some(DayTimeDuration::from_str("PT2H12M")?)
        );
        assert_eq!(
            Time::from_str("11:00:00-05:00")?.checked_sub(Time::from_str("21:30:00+05:30")?),
            Some(DayTimeDuration::from_str("PT0S")?)
        );
        assert_eq!(
            Time::from_str("17:00:00-06:00")?.checked_sub(Time::from_str("08:00:00+09:00")?),
            Some(DayTimeDuration::from_str("P1D")?)
        );
        assert_eq!(
            Time::from_str("24:00:00")?.checked_sub(Time::from_str("23:59:59")?),
            Some(DayTimeDuration::from_str("-PT23H59M59S")?)
        );
        Ok(())
    }

    #[test]
    fn add_duration() -> Result<(), XsdParseError> {
        assert_eq!(
            DateTime::from_str("2000-01-12T12:13:14Z")?
                .checked_add_duration(Duration::from_str("P1Y3M5DT7H10M3.3S")?),
            Some(DateTime::from_str("2001-04-17T19:23:17.3Z")?)
        );
        assert_eq!(
            Date::from_str("2000-01-01")?.checked_add_duration(Duration::from_str("-P3M")?),
            Some(Date::from_str("1999-10-01")?)
        );
        assert_eq!(
            Date::from_str("2000-01-12")?.checked_add_duration(Duration::from_str("PT33H")?),
            Some(Date::from_str("2000-01-13")?)
        );
        assert_eq!(
            Date::from_str("2000-03-30")?.checked_add_duration(Duration::from_str("P1D")?),
            Some(Date::from_str("2000-03-31")?)
        );
        assert_eq!(
            Date::from_str("2000-03-31")?.checked_add_duration(Duration::from_str("P1M")?),
            Some(Date::from_str("2000-04-30")?)
        );
        assert_eq!(
            Date::from_str("2000-03-30")?.checked_add_duration(Duration::from_str("P1M")?),
            Some(Date::from_str("2000-04-30")?)
        );
        assert_eq!(
            Date::from_str("2000-04-30")?.checked_add_duration(Duration::from_str("P1D")?),
            Some(Date::from_str("2000-05-01")?)
        );

        assert_eq!(
            DateTime::from_str("2000-10-30T11:12:00")?
                .checked_add_duration(Duration::from_str("P1Y2M")?),
            Some(DateTime::from_str("2001-12-30T11:12:00")?)
        );
        assert_eq!(
            DateTime::from_str("2000-10-30T11:12:00")?
                .checked_add_duration(Duration::from_str("P3DT1H15M")?),
            Some(DateTime::from_str("2000-11-02T12:27:00")?)
        );

        assert_eq!(
            Date::from_str("2000-10-30")?.checked_add_duration(Duration::from_str("P1Y2M")?),
            Some(Date::from_str("2001-12-30")?)
        );
        assert_eq!(
            Date::from_str("2004-10-30Z")?.checked_add_duration(Duration::from_str("P2DT2H30M0S")?),
            Some(Date::from_str("2004-11-01Z")?)
        );

        assert_eq!(
            Time::from_str("11:12:00")?.checked_add_duration(Duration::from_str("P3DT1H15M")?),
            Some(Time::from_str("12:27:00")?)
        );
        assert_eq!(
            Time::from_str("23:12:00+03:00")?
                .checked_add_duration(Duration::from_str("P1DT3H15M")?),
            Some(Time::from_str("02:27:00+03:00")?)
        );
        Ok(())
    }

    #[test]
    fn sub_duration() -> Result<(), XsdParseError> {
        assert_eq!(
            DateTime::from_str("2000-10-30T11:12:00")?
                .checked_sub_duration(Duration::from_str("P1Y2M")?),
            Some(DateTime::from_str("1999-08-30T11:12:00")?)
        );
        assert_eq!(
            DateTime::from_str("2000-10-30T11:12:00")?
                .checked_sub_duration(Duration::from_str("P3DT1H15M")?),
            Some(DateTime::from_str("2000-10-27T09:57:00")?)
        );

        assert_eq!(
            Date::from_str("2000-10-30")?.checked_sub_duration(Duration::from_str("P1Y2M")?),
            Some(Date::from_str("1999-08-30")?)
        );
        assert_eq!(
            Date::from_str("2000-02-29Z")?.checked_sub_duration(Duration::from_str("P1Y")?),
            Some(Date::from_str("1999-02-28Z")?)
        );
        assert_eq!(
            Date::from_str("2000-10-31-05:00")?.checked_sub_duration(Duration::from_str("P1Y1M")?),
            Some(Date::from_str("1999-09-30-05:00")?)
        );
        assert_eq!(
            Date::from_str("2000-10-30")?.checked_sub_duration(Duration::from_str("P3DT1H15M")?),
            Some(Date::from_str("2000-10-26")?)
        );

        assert_eq!(
            Time::from_str("11:12:00")?.checked_sub_duration(Duration::from_str("P3DT1H15M")?),
            Some(Time::from_str("09:57:00")?)
        );
        assert_eq!(
            Time::from_str("08:20:00-05:00")?
                .checked_sub_duration(Duration::from_str("P23DT10H10M")?),
            Some(Time::from_str("22:10:00-05:00")?)
        );
        Ok(())
    }

    #[test]
    fn adjust() -> Result<(), XsdParseError> {
        assert_eq!(
            DateTime::from_str("2002-03-07T10:00:00-07:00")?.adjust(Some(
                DayTimeDuration::from_str("PT10H")?.try_into().unwrap()
            )),
            Some(DateTime::from_str("2002-03-08T03:00:00+10:00")?)
        );
        assert_eq!(
            DateTime::from_str("2002-03-07T00:00:00+01:00")?.adjust(Some(
                DayTimeDuration::from_str("-PT8H")?.try_into().unwrap()
            )),
            Some(DateTime::from_str("2002-03-06T15:00:00-08:00")?)
        );
        assert_eq!(
            DateTime::from_str("2002-03-07T10:00:00")?.adjust(None),
            Some(DateTime::from_str("2002-03-07T10:00:00")?)
        );
        assert_eq!(
            DateTime::from_str("2002-03-07T10:00:00-07:00")?.adjust(None),
            Some(DateTime::from_str("2002-03-07T10:00:00")?)
        );

        assert_eq!(
            Date::from_str("2002-03-07")?.adjust(Some(
                DayTimeDuration::from_str("-PT10H")?.try_into().unwrap()
            )),
            Some(Date::from_str("2002-03-07-10:00")?)
        );
        assert_eq!(
            Date::from_str("2002-03-07-07:00")?.adjust(Some(
                DayTimeDuration::from_str("-PT10H")
                    .unwrap()
                    .try_into()
                    .unwrap()
            )),
            Some(Date::from_str("2002-03-06-10:00")?)
        );
        assert_eq!(
            Date::from_str("2002-03-07")?.adjust(None),
            Some(Date::from_str("2002-03-07")?)
        );
        assert_eq!(
            Date::from_str("2002-03-07-07:00")?.adjust(None),
            Some(Date::from_str("2002-03-07")?)
        );

        assert_eq!(
            Time::from_str("10:00:00")?.adjust(Some(
                DayTimeDuration::from_str("-PT10H")?.try_into().unwrap()
            )),
            Some(Time::from_str("10:00:00-10:00")?)
        );
        assert_eq!(
            Time::from_str("10:00:00-07:00")?.adjust(Some(
                DayTimeDuration::from_str("-PT10H")?.try_into().unwrap()
            )),
            Some(Time::from_str("07:00:00-10:00")?)
        );
        assert_eq!(
            Time::from_str("10:00:00")?.adjust(None),
            Some(Time::from_str("10:00:00")?)
        );
        assert_eq!(
            Time::from_str("10:00:00-07:00")?.adjust(None),
            Some(Time::from_str("10:00:00")?)
        );
        assert_eq!(
            Time::from_str("10:00:00-07:00")?.adjust(Some(
                DayTimeDuration::from_str("PT10H")?.try_into().unwrap()
            )),
            Some(Time::from_str("03:00:00+10:00")?)
        );
        Ok(())
    }

    #[test]
    fn now() -> Result<(), XsdParseError> {
        let now = DateTime::now().unwrap();
        assert!(DateTime::from_str("2022-01-01T00:00:00Z")? < now);
        assert!(now < DateTime::from_str("2100-01-01T00:00:00Z")?);
        Ok(())
    }
}
