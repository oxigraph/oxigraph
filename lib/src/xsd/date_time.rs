use super::parser::{date_lexical_rep, date_time_lexical_rep, parse_value, time_lexical_rep};
use super::{DayTimeDuration, Decimal, Duration, XsdParseError, YearMonthDuration};
use crate::xsd::parser::{
    g_day_lexical_rep, g_month_day_lexical_rep, g_month_lexical_rep, g_year_lexical_rep,
    g_year_month_lexical_rep,
};
use std::cmp::{min, Ordering};
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::time::SystemTimeError;

/// [XML Schema `dateTime` datatype](https://www.w3.org/TR/xmlschema11-2/#dateTime) implementation.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct DateTime {
    timestamp: Timestamp,
}

impl DateTime {
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

    pub fn now() -> Result<Self, DateTimeError> {
        Ok(Self {
            timestamp: Timestamp::now()?,
        })
    }

    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    /// [fn:year-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-year-from-dateTime)
    pub fn year(&self) -> i64 {
        self.timestamp.year()
    }

    /// [fn:month-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-month-from-dateTime)
    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    /// [fn:day-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-day-from-dateTime)
    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    /// [fn:hour-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-hour-from-dateTime)
    pub fn hour(&self) -> u8 {
        self.timestamp.hour()
    }

    /// [fn:minute-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-minute-from-dateTime)
    pub fn minute(&self) -> u8 {
        self.timestamp.minute()
    }

    /// [fn:second-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-second-from-dateTime)
    pub fn second(&self) -> Decimal {
        self.timestamp.second()
    }

    /// [fn:timezone-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-timezone-from-dateTime)
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

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

    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// [op:subtract-dateTimes](https://www.w3.org/TR/xpath-functions/#func-subtract-dateTimes)
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Duration> {
        self.timestamp.checked_sub(rhs.into().timestamp)
    }

    /// [op:add-yearMonthDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDuration-to-dateTime)
    pub fn checked_add_year_month_duration(
        &self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-dateTime)
    pub fn checked_add_day_time_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            timestamp: self.timestamp.checked_add_seconds(rhs.all_seconds())?,
        })
    }

    /// [op:add-yearMonthDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDuration-to-dateTime) and [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-dateTime)
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
    pub fn checked_sub_year_month_duration(
        &self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-dayTimeDuration-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-dateTime)
    pub fn checked_sub_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            timestamp: self.timestamp.checked_sub_seconds(rhs.all_seconds())?,
        })
    }

    /// [op:subtract-yearMonthDuration-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDuration-from-dateTime) and [op:subtract-dayTimeDuration-from-dateTime](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-dateTime)
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

    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for DateTime {
    type Error = DateTimeError;

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
            write!(f, "{}", timezone_offset)?;
        }
        Ok(())
    }
}

/// [XML Schema `time` datatype](https://www.w3.org/TR/xmlschema11-2/#time) implementation.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct Time {
    timestamp: Timestamp,
}

impl Time {
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

    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    /// [fn:hour-from-time](https://www.w3.org/TR/xpath-functions/#func-hour-from-time)
    pub fn hour(&self) -> u8 {
        self.timestamp.hour()
    }

    /// [fn:minute-from-time](https://www.w3.org/TR/xpath-functions/#func-minute-from-time)
    pub fn minute(&self) -> u8 {
        self.timestamp.minute()
    }

    /// [fn:second-from-time](https://www.w3.org/TR/xpath-functions/#func-second-from-time)
    pub fn second(&self) -> Decimal {
        self.timestamp.second()
    }

    /// [fn:timezone-from-time](https://www.w3.org/TR/xpath-functions/#func-timezone-from-time)
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// [op:subtract-times](https://www.w3.org/TR/xpath-functions/#func-subtract-times)
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Duration> {
        self.timestamp.checked_sub(rhs.into().timestamp)
    }

    /// [op:add-dayTimeDuration-to-time](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-time)
    pub fn checked_add_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-dayTimeDuration-to-time](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-time)
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
    pub fn checked_sub_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-dayTimeDuration-from-time](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-time)
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

    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for Time {
    type Error = DateTimeError;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02}:{:02}:{:02}",
            self.hour(),
            self.minute(),
            self.second()
        )?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{}", timezone_offset)?;
        }
        Ok(())
    }
}

/// [XML Schema `date` datatype](https://www.w3.org/TR/xmlschema11-2/#date) implementation.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct Date {
    timestamp: Timestamp,
}

impl Date {
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

    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    /// [fn:year-from-date](https://www.w3.org/TR/xpath-functions/#func-year-from-date)
    pub fn year(&self) -> i64 {
        self.timestamp.year()
    }

    /// [fn:month-from-date](https://www.w3.org/TR/xpath-functions/#func-month-from-date)
    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    /// [fn:day-from-date](https://www.w3.org/TR/xpath-functions/#func-day-from-date)
    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    /// [fn:timezone-from-date](https://www.w3.org/TR/xpath-functions/#func-timezone-from-date)
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// [op:subtract-dates](https://www.w3.org/TR/xpath-functions/#func-subtract-dates)
    pub fn checked_sub(&self, rhs: impl Into<Self>) -> Option<Duration> {
        self.timestamp.checked_sub(rhs.into().timestamp)
    }

    /// [op:add-yearMonthDuration-to-date](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDuration-to-date)
    pub fn checked_add_year_month_duration(
        &self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-date)
    pub fn checked_add_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-yearMonthDuration-to-date](https://www.w3.org/TR/xpath-functions/#func-add-yearMonthDuration-to-date) and [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions/#func-add-dayTimeDuration-to-date)
    pub fn checked_add_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        DateTime::try_from(*self)
            .ok()?
            .checked_add_duration(rhs)?
            .try_into()
            .ok()
    }

    /// [op:subtract-yearMonthDuration-from-date](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDuration-from-date)
    pub fn checked_sub_year_month_duration(
        &self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-dayTimeDuration-from-date](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-date)
    pub fn checked_sub_day_time_duration(&self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-yearMonthDuration-from-date](https://www.w3.org/TR/xpath-functions/#func-subtract-yearMonthDuration-from-date) and [op:subtract-dayTimeDuration-from-date](https://www.w3.org/TR/xpath-functions/#func-subtract-dayTimeDuration-from-date)
    pub fn checked_sub_duration(&self, rhs: impl Into<Duration>) -> Option<Self> {
        DateTime::try_from(*self)
            .ok()?
            .checked_sub_duration(rhs)?
            .try_into()
            .ok()
    }

    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for Date {
    type Error = DateTimeError;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            write!(f, "-")?;
        }
        write!(f, "{:04}-{:02}-{:02}", year.abs(), self.month(), self.day())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{}", timezone_offset)?;
        }
        Ok(())
    }
}

/// [XML Schema `gYearMonth` datatype](https://www.w3.org/TR/xmlschema11-2/#gYearMonth) implementation.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GYearMonth {
    timestamp: Timestamp,
}

impl GYearMonth {
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

    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    pub fn year(&self) -> i64 {
        self.timestamp.year()
    }

    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GYearMonth {
    type Error = DateTimeError;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            write!(f, "-")?;
        }
        write!(f, "{:04}-{:02}", year.abs(), self.month())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{}", timezone_offset)?;
        }
        Ok(())
    }
}

/// [XML Schema `gYear` datatype](https://www.w3.org/TR/xmlschema11-2/#gYear) implementation.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GYear {
    timestamp: Timestamp,
}

impl GYear {
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

    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    pub fn year(&self) -> i64 {
        self.timestamp.year()
    }

    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GYear {
    type Error = DateTimeError;

    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(date_time.year(), date_time.timezone_offset())
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for GYear {
    type Error = DateTimeError;

    fn try_from(date: Date) -> Result<Self, DateTimeError> {
        Self::new(date.year(), date.timezone_offset())
    }
}

impl TryFrom<GYearMonth> for GYear {
    type Error = DateTimeError;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            write!(f, "-")?;
        }
        write!(f, "{:04}", year.abs())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{}", timezone_offset)?;
        }
        Ok(())
    }
}

/// [XML Schema `gMonthDay` datatype](https://www.w3.org/TR/xmlschema11-2/#gMonthDay) implementation.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GMonthDay {
    timestamp: Timestamp,
}

impl GMonthDay {
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

    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GMonthDay {
    type Error = DateTimeError;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "--{:02}-{:02}", self.month(), self.day())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{}", timezone_offset)?;
        }
        Ok(())
    }
}

/// [XML Schema `gMonth` datatype](https://www.w3.org/TR/xmlschema11-2/#gMonth) implementation.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GMonth {
    timestamp: Timestamp,
}

impl GMonth {
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

    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GMonth {
    type Error = DateTimeError;

    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(date_time.month(), date_time.timezone_offset())
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for GMonth {
    type Error = DateTimeError;

    fn try_from(date: Date) -> Result<Self, DateTimeError> {
        Self::new(date.month(), date.timezone_offset())
    }
}

impl TryFrom<GYearMonth> for GMonth {
    type Error = DateTimeError;

    fn try_from(year_month: GYearMonth) -> Result<Self, DateTimeError> {
        Self::new(year_month.month(), year_month.timezone_offset())
    }
}

impl TryFrom<GMonthDay> for GMonth {
    type Error = DateTimeError;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "--{:02}", self.month())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{}", timezone_offset)?;
        }
        Ok(())
    }
}

/// [XML Schema `date` datatype](https://www.w3.org/TR/xmlschema11-2/#date) implementation.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct GDay {
    timestamp: Timestamp,
}

impl GDay {
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

    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.timestamp.is_identical_with(&other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<DateTime> for GDay {
    type Error = DateTimeError;

    fn try_from(date_time: DateTime) -> Result<Self, DateTimeError> {
        Self::new(date_time.day(), date_time.timezone_offset())
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions/#casting-to-datetimes).
impl TryFrom<Date> for GDay {
    type Error = DateTimeError;

    fn try_from(date: Date) -> Result<Self, DateTimeError> {
        Self::new(date.day(), date.timezone_offset())
    }
}

impl TryFrom<GMonthDay> for GDay {
    type Error = DateTimeError;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "---{:02}", self.day())?;
        if let Some(timezone_offset) = self.timezone_offset() {
            write!(f, "{}", timezone_offset)?;
        }
        Ok(())
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct TimezoneOffset {
    offset: i16, // in minute with respect to UTC
}

impl TimezoneOffset {
    pub const fn utc() -> Self {
        Self { offset: 0 }
    }

    /// From offset in minute with respect to UTC
    pub(super) const fn new(offset: i16) -> Self {
        Self { offset }
    }

    pub fn from_be_bytes(bytes: [u8; 2]) -> Self {
        Self {
            offset: i16::from_be_bytes(bytes),
        }
    }

    pub fn to_be_bytes(self) -> [u8; 2] {
        self.offset.to_be_bytes()
    }
}

impl From<i16> for TimezoneOffset {
    fn from(offset: i16) -> Self {
        Self { offset }
    }
}

impl From<TimezoneOffset> for DayTimeDuration {
    fn from(value: TimezoneOffset) -> Self {
        Self::new(i32::from(value.offset) * 60)
    }
}

impl From<TimezoneOffset> for Duration {
    fn from(value: TimezoneOffset) -> Self {
        DayTimeDuration::from(value).into()
    }
}

impl fmt::Display for TimezoneOffset {
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
    fn eq(&self, other: &Self) -> bool {
        match (self.timezone_offset, other.timezone_offset) {
            (Some(_), Some(_)) | (None, None) => self.value.eq(&other.value),
            _ => false, //TODO: implicit timezone
        }
    }
}

impl Eq for Timestamp {}

impl PartialOrd for Timestamp {
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
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state)
    }
}

impl Timestamp {
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
            value: time_on_timeline(props).ok_or(DateTimeError {
                kind: DateTimeErrorKind::Overflow,
            })?,
        })
    }

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
                    timezone_offset: Some(TimezoneOffset::utc()),
                },
            )
            .ok_or(DateTimeError {
                kind: DateTimeErrorKind::Overflow,
            })?,
        )
    }

    fn from_be_bytes(bytes: [u8; 18]) -> Self {
        let mut value = [0; 16];
        value.copy_from_slice(&bytes[0..16]);
        let mut timezone_offset = [0; 2];
        timezone_offset.copy_from_slice(&bytes[16..18]);

        Self {
            value: Decimal::from_be_bytes(value),
            timezone_offset: if timezone_offset == [u8::MAX; 2] {
                None
            } else {
                Some(TimezoneOffset::from_be_bytes(timezone_offset))
            },
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn year_month_day(&self) -> (i64, u8, u8) {
        let mut days = (self.value.as_i128()
            + i128::from(
                self.timezone_offset
                    .unwrap_or_else(TimezoneOffset::utc)
                    .offset,
            ) * 60)
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

        let leap_year_offset = if (year_mul_100 == 0 || year_mul_4 != 0) && year_mod_4 == 0 {
            1
        } else {
            0
        };
        days += leap_year_offset;

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

    fn year(&self) -> i64 {
        let (year, _, _) = self.year_month_day();
        year
    }

    fn month(&self) -> u8 {
        let (_, month, _) = self.year_month_day();
        month
    }

    fn day(&self) -> u8 {
        let (_, _, day) = self.year_month_day();
        day
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn hour(&self) -> u8 {
        (((self.value.as_i128()
            + i128::from(
                self.timezone_offset
                    .unwrap_or_else(TimezoneOffset::utc)
                    .offset,
            ) * 60)
            .rem_euclid(86400))
            / 3600) as u8
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn minute(&self) -> u8 {
        (((self.value.as_i128()
            + i128::from(
                self.timezone_offset
                    .unwrap_or_else(TimezoneOffset::utc)
                    .offset,
            ) * 60)
            .rem_euclid(3600))
            / 60) as u8
    }

    fn second(&self) -> Decimal {
        self.value.checked_rem_euclid(60).unwrap().abs()
    }

    const fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timezone_offset
    }

    fn checked_add_seconds(&self, seconds: Decimal) -> Option<Self> {
        Some(Self {
            value: self.value.checked_add(seconds)?,
            timezone_offset: self.timezone_offset,
        })
    }

    fn checked_sub(&self, rhs: Self) -> Option<Duration> {
        match (self.timezone_offset, rhs.timezone_offset) {
            (Some(_), Some(_)) | (None, None) => {
                Some(Duration::new(0, self.value.checked_sub(rhs.value)?))
            }
            _ => None, //TODO: implicit timezone
        }
    }

    fn checked_sub_seconds(&self, seconds: Decimal) -> Option<Self> {
        Some(Self {
            value: self.value.checked_sub(seconds)?,
            timezone_offset: self.timezone_offset,
        })
    }

    fn to_be_bytes(self) -> [u8; 18] {
        let mut bytes = [0; 18];
        bytes[0..16].copy_from_slice(&self.value.to_be_bytes());
        bytes[16..18].copy_from_slice(&match &self.timezone_offset {
            Some(timezone_offset) => timezone_offset.to_be_bytes(),
            None => [u8::MAX; 2],
        });
        bytes
    }

    pub fn is_identical_with(&self, other: &Self) -> bool {
        self.value == other.value && self.timezone_offset == other.timezone_offset
    }
}

#[allow(clippy::unnecessary_wraps)]
#[cfg(target_arch = "wasm32")]
fn since_unix_epoch() -> Result<Duration, DateTimeError> {
    Ok(Duration::new(
        0,
        Decimal::from_double((js_sys::Date::now() / 1000.).into()),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn since_unix_epoch() -> Result<Duration, DateTimeError> {
    use std::time::SystemTime;

    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .try_into()
        .map_err(|_| DateTimeError {
            kind: DateTimeErrorKind::Overflow,
        })
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
        - i128::from(
            props
                .timezone_offset
                .unwrap_or_else(TimezoneOffset::utc)
                .offset,
        );
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DateTimeErrorKind::InvalidDayOfMonth { day, month } => {
                write!(f, "{} is not a valid day of {}", day, month)
            }
            DateTimeErrorKind::Overflow => write!(f, "Overflow during date time normalization"),
            DateTimeErrorKind::SystemTime(error) => error.fmt(f),
        }
    }
}

impl Error for DateTimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            DateTimeErrorKind::SystemTime(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SystemTimeError> for DateTimeError {
    fn from(error: SystemTimeError) -> Self {
        Self {
            kind: DateTimeErrorKind::SystemTime(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str() {
        assert_eq!(
            Time::from_str("00:00:00Z").unwrap().to_string(),
            "00:00:00Z"
        );
        assert_eq!(Time::from_str("00:00:00").unwrap().to_string(), "00:00:00");
        assert_eq!(
            Time::from_str("00:00:00+02:00").unwrap().to_string(),
            "00:00:00+02:00"
        );
        assert_eq!(
            Time::from_str("00:00:00+14:00").unwrap().to_string(),
            "00:00:00+14:00"
        );
        assert_eq!(Time::from_str("24:00:00").unwrap().to_string(), "00:00:00");
        assert_eq!(
            Time::from_str("24:00:00.00").unwrap().to_string(),
            "00:00:00"
        );
        assert_eq!(
            Time::from_str("23:59:59.9999999999").unwrap().to_string(),
            "23:59:59.9999999999"
        );

        assert_eq!(
            Date::from_str("0001-01-01Z").unwrap().to_string(),
            "0001-01-01Z"
        );
        assert_eq!(
            Date::from_str("0001-01-01").unwrap().to_string(),
            "0001-01-01"
        );
        assert_eq!(
            DateTime::from_str("0001-01-01T00:00:00Z")
                .unwrap()
                .to_string(),
            "0001-01-01T00:00:00Z"
        );
        assert_eq!(
            DateTime::from_str("0001-01-01T00:00:00")
                .unwrap()
                .to_string(),
            "0001-01-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("1000000000-01-01T00:00:00")
                .unwrap()
                .to_string(),
            "1000000000-01-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("2001-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "2001-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2004-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "2004-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("1900-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "1900-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2000-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "2000-12-31T23:59:59",
        );
        assert_eq!(
            DateTime::from_str("1899-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "1899-12-31T23:59:59"
        );

        assert_eq!(
            DateTime::from_str("2001-02-28T23:59:59")
                .unwrap()
                .to_string(),
            "2001-02-28T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2004-02-29T23:59:59")
                .unwrap()
                .to_string(),
            "2004-02-29T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("1900-02-28T23:59:59")
                .unwrap()
                .to_string(),
            "1900-02-28T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2000-02-29T23:59:59")
                .unwrap()
                .to_string(),
            "2000-02-29T23:59:59",
        );
        assert_eq!(
            DateTime::from_str("1899-02-28T23:59:59")
                .unwrap()
                .to_string(),
            "1899-02-28T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("2001-03-01T00:00:00")
                .unwrap()
                .to_string(),
            "2001-03-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("2004-03-01T00:00:00")
                .unwrap()
                .to_string(),
            "2004-03-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("1900-03-01T00:00:00")
                .unwrap()
                .to_string(),
            "1900-03-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("2000-03-01T00:00:00")
                .unwrap()
                .to_string(),
            "2000-03-01T00:00:00",
        );
        assert_eq!(
            DateTime::from_str("1899-03-01T00:00:00")
                .unwrap()
                .to_string(),
            "1899-03-01T00:00:00"
        );

        assert_eq!(
            DateTime::from_str("-1000000000-01-01T00:00:00")
                .unwrap()
                .to_string(),
            "-1000000000-01-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("-2001-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "-2001-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("-2004-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "-2004-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("-1900-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "-1900-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("-2000-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "-2000-12-31T23:59:59"
        );
        assert_eq!(
            DateTime::from_str("-1899-12-31T23:59:59")
                .unwrap()
                .to_string(),
            "-1899-12-31T23:59:59"
        );

        assert_eq!(
            GYearMonth::from_str("-1899-12+01:00").unwrap().to_string(),
            "-1899-12+01:00"
        );
        assert_eq!(
            GYear::from_str("-1899+01:00").unwrap().to_string(),
            "-1899+01:00"
        );
        assert_eq!(
            GMonthDay::from_str("--01-01+01:00").unwrap().to_string(),
            "--01-01+01:00"
        );
        assert_eq!(
            GDay::from_str("---01+01:00").unwrap().to_string(),
            "---01+01:00"
        );
        assert_eq!(
            GMonth::from_str("--01+01:00").unwrap().to_string(),
            "--01+01:00"
        );
    }

    #[test]
    fn equals() {
        assert_eq!(
            DateTime::from_str("2002-04-02T12:00:00-01:00").unwrap(),
            DateTime::from_str("2002-04-02T17:00:00+04:00").unwrap()
        );
        assert_eq!(
            DateTime::from_str("2002-04-02T12:00:00-05:00").unwrap(),
            DateTime::from_str("2002-04-02T23:00:00+06:00").unwrap()
        );
        assert_ne!(
            DateTime::from_str("2002-04-02T12:00:00-05:00").unwrap(),
            DateTime::from_str("2002-04-02T17:00:00-05:00").unwrap()
        );
        assert_eq!(
            DateTime::from_str("2002-04-02T12:00:00-05:00").unwrap(),
            DateTime::from_str("2002-04-02T12:00:00-05:00").unwrap()
        );
        assert_eq!(
            DateTime::from_str("2002-04-02T23:00:00-04:00").unwrap(),
            DateTime::from_str("2002-04-03T02:00:00-01:00").unwrap()
        );
        assert_eq!(
            DateTime::from_str("1999-12-31T24:00:00-05:00").unwrap(),
            DateTime::from_str("2000-01-01T00:00:00-05:00").unwrap()
        );
        assert_ne!(
            DateTime::from_str("2005-04-04T24:00:00-05:00").unwrap(),
            DateTime::from_str("2005-04-04T00:00:00-05:00").unwrap()
        );

        assert_ne!(
            Date::from_str("2004-12-25Z").unwrap(),
            Date::from_str("2004-12-25+07:00").unwrap()
        );
        assert_eq!(
            Date::from_str("2004-12-25-12:00").unwrap(),
            Date::from_str("2004-12-26+12:00").unwrap()
        );

        assert_ne!(
            Time::from_str("08:00:00+09:00").unwrap(),
            Time::from_str("17:00:00-06:00").unwrap()
        );
        assert_eq!(
            Time::from_str("21:30:00+10:30").unwrap(),
            Time::from_str("06:00:00-05:00").unwrap()
        );
        assert_eq!(
            Time::from_str("24:00:00+01:00").unwrap(),
            Time::from_str("00:00:00+01:00").unwrap()
        );

        assert_eq!(
            Time::from_str("05:00:00-03:00").unwrap(),
            Time::from_str("10:00:00+02:00").unwrap()
        );
        assert_ne!(
            Time::from_str("23:00:00-03:00").unwrap(),
            Time::from_str("02:00:00Z").unwrap()
        );

        assert_ne!(
            GYearMonth::from_str("1986-02").unwrap(),
            GYearMonth::from_str("1986-03").unwrap()
        );
        assert_ne!(
            GYearMonth::from_str("1978-03").unwrap(),
            GYearMonth::from_str("1978-03Z").unwrap()
        );

        assert_ne!(
            GYear::from_str("2005-12:00").unwrap(),
            GYear::from_str("2005+12:00").unwrap()
        );
        assert_ne!(
            GYear::from_str("1976-05:00").unwrap(),
            GYear::from_str("1976").unwrap()
        );

        assert_eq!(
            GMonthDay::from_str("--12-25-14:00").unwrap(),
            GMonthDay::from_str("--12-26+10:00").unwrap()
        );
        assert_ne!(
            GMonthDay::from_str("--12-25").unwrap(),
            GMonthDay::from_str("--12-26Z").unwrap()
        );

        assert_ne!(
            GMonth::from_str("--12-14:00").unwrap(),
            GMonth::from_str("--12+10:00").unwrap()
        );
        assert_ne!(
            GMonth::from_str("--12").unwrap(),
            GMonth::from_str("--12Z").unwrap()
        );

        assert_ne!(
            GDay::from_str("---25-14:00").unwrap(),
            GDay::from_str("---25+10:00").unwrap()
        );
        assert_ne!(
            GDay::from_str("---12").unwrap(),
            GDay::from_str("---12Z").unwrap()
        );
    }

    #[test]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn cmp() {
        assert!(
            Date::from_str("2004-12-25Z").unwrap() < Date::from_str("2004-12-25-05:00").unwrap()
        );
        assert!(
            !(Date::from_str("2004-12-25-12:00").unwrap()
                < Date::from_str("2004-12-26+12:00").unwrap())
        );

        assert!(
            Date::from_str("2004-12-25Z").unwrap() > Date::from_str("2004-12-25+07:00").unwrap()
        );
        assert!(
            !(Date::from_str("2004-12-25-12:00").unwrap()
                > Date::from_str("2004-12-26+12:00").unwrap())
        );

        assert!(!(Time::from_str("12:00:00").unwrap() < Time::from_str("23:00:00+06:00").unwrap()));
        assert!(Time::from_str("11:00:00-05:00").unwrap() < Time::from_str("17:00:00Z").unwrap());
        assert!(!(Time::from_str("23:59:59").unwrap() < Time::from_str("24:00:00").unwrap()));

        assert!(
            !(Time::from_str("08:00:00+09:00").unwrap()
                > Time::from_str("17:00:00-06:00").unwrap())
        );

        assert!(
            GMonthDay::from_str("--12-12+13:00").unwrap()
                < GMonthDay::from_str("--12-12+11:00").unwrap()
        );
        assert!(GDay::from_str("---15").unwrap() < GDay::from_str("---16").unwrap());
        assert!(GDay::from_str("---15-13:00").unwrap() > GDay::from_str("---16+13:00").unwrap());
        assert_eq!(
            GDay::from_str("---15-11:00").unwrap(),
            GDay::from_str("---16+13:00").unwrap()
        );
        assert!(GDay::from_str("---15-13:00")
            .unwrap()
            .partial_cmp(&GDay::from_str("---16").unwrap())
            .is_none());
    }

    #[test]
    fn year() {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")
                .unwrap()
                .year(),
            1999
        );
        assert_eq!(
            DateTime::from_str("1999-05-31T21:30:00-05:00")
                .unwrap()
                .year(),
            1999
        );
        assert_eq!(
            DateTime::from_str("1999-12-31T19:20:00").unwrap().year(),
            1999
        );
        assert_eq!(
            DateTime::from_str("1999-12-31T24:00:00").unwrap().year(),
            2000
        );
        assert_eq!(
            DateTime::from_str("-0002-06-06T00:00:00").unwrap().year(),
            -2
        );

        assert_eq!(Date::from_str("1999-05-31").unwrap().year(), 1999);
        assert_eq!(Date::from_str("2000-01-01+05:00").unwrap().year(), 2000);
        assert_eq!(Date::from_str("-0002-06-01").unwrap().year(), -2);
    }

    #[test]
    fn month() {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")
                .unwrap()
                .month(),
            5
        );
        assert_eq!(
            DateTime::from_str("1999-12-31T19:20:00-05:00")
                .unwrap()
                .month(),
            12
        );

        assert_eq!(Date::from_str("1999-05-31-05:00").unwrap().month(), 5);
        assert_eq!(Date::from_str("2000-01-01+05:00").unwrap().month(), 1);
    }

    #[test]
    fn day() {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")
                .unwrap()
                .day(),
            31
        );
        assert_eq!(
            DateTime::from_str("1999-12-31T20:00:00-05:00")
                .unwrap()
                .day(),
            31
        );

        assert_eq!(Date::from_str("1999-05-31-05:00").unwrap().day(), 31);
        assert_eq!(Date::from_str("2000-01-01+05:00").unwrap().day(), 1);
    }

    #[test]
    fn hour() {
        assert_eq!(
            DateTime::from_str("1999-05-31T08:20:00-05:00")
                .unwrap()
                .hour(),
            8
        );
        assert_eq!(
            DateTime::from_str("1999-12-31T21:20:00-05:00")
                .unwrap()
                .hour(),
            21
        );
        assert_eq!(
            DateTime::from_str("1999-12-31T12:00:00").unwrap().hour(),
            12
        );
        assert_eq!(DateTime::from_str("1999-12-31T24:00:00").unwrap().hour(), 0);

        assert_eq!(Time::from_str("11:23:00-05:00").unwrap().hour(), 11);
        assert_eq!(Time::from_str("21:23:00-05:00").unwrap().hour(), 21);
        assert_eq!(Time::from_str("01:23:00+05:00").unwrap().hour(), 1);
        assert_eq!(Time::from_str("24:00:00").unwrap().hour(), 0);
    }

    #[test]
    fn minute() {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")
                .unwrap()
                .minute(),
            20
        );
        assert_eq!(
            DateTime::from_str("1999-05-31T13:30:00+05:30")
                .unwrap()
                .minute(),
            30
        );

        assert_eq!(Time::from_str("13:00:00Z").unwrap().minute(), 0);
    }

    #[test]
    fn second() {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")
                .unwrap()
                .second(),
            Decimal::from(0)
        );

        assert_eq!(
            Time::from_str("13:20:10.5").unwrap().second(),
            Decimal::from_str("10.5").unwrap()
        );
    }

    #[test]
    fn timezone() {
        assert_eq!(
            DateTime::from_str("1999-05-31T13:20:00-05:00")
                .unwrap()
                .timezone(),
            Some(DayTimeDuration::from_str("-PT5H").unwrap())
        );
        assert_eq!(
            DateTime::from_str("2000-06-12T13:20:00Z")
                .unwrap()
                .timezone(),
            Some(DayTimeDuration::from_str("PT0S").unwrap())
        );
        assert_eq!(
            DateTime::from_str("2004-08-27T00:00:00")
                .unwrap()
                .timezone(),
            None
        );

        assert_eq!(
            Date::from_str("1999-05-31-05:00").unwrap().timezone(),
            Some(DayTimeDuration::from_str("-PT5H").unwrap())
        );
        assert_eq!(
            Date::from_str("2000-06-12Z").unwrap().timezone(),
            Some(DayTimeDuration::from_str("PT0S").unwrap())
        );

        assert_eq!(
            Time::from_str("13:20:00-05:00").unwrap().timezone(),
            Some(DayTimeDuration::from_str("-PT5H").unwrap())
        );
        assert_eq!(Time::from_str("13:20:00").unwrap().timezone(), None);
    }

    #[test]
    fn sub() {
        assert_eq!(
            DateTime::from_str("2000-10-30T06:12:00-05:00")
                .unwrap()
                .checked_sub(DateTime::from_str("1999-11-28T09:00:00Z").unwrap())
                .unwrap(),
            Duration::from_str("P337DT2H12M").unwrap()
        );

        assert_eq!(
            Date::from_str("2000-10-30")
                .unwrap()
                .checked_sub(Date::from_str("1999-11-28").unwrap())
                .unwrap(),
            Duration::from_str("P337D").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-10-30+05:00")
                .unwrap()
                .checked_sub(Date::from_str("1999-11-28Z").unwrap())
                .unwrap(),
            Duration::from_str("P336DT19H").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-10-15-05:00")
                .unwrap()
                .checked_sub(Date::from_str("2000-10-10+02:00").unwrap())
                .unwrap(),
            Duration::from_str("P5DT7H").unwrap()
        );

        assert_eq!(
            Time::from_str("11:12:00Z")
                .unwrap()
                .checked_sub(Time::from_str("04:00:00-05:00").unwrap())
                .unwrap(),
            Duration::from_str("PT2H12M").unwrap()
        );
        assert_eq!(
            Time::from_str("11:00:00-05:00")
                .unwrap()
                .checked_sub(Time::from_str("21:30:00+05:30").unwrap())
                .unwrap(),
            Duration::from_str("PT0S").unwrap()
        );
        assert_eq!(
            Time::from_str("17:00:00-06:00")
                .unwrap()
                .checked_sub(Time::from_str("08:00:00+09:00").unwrap())
                .unwrap(),
            Duration::from_str("P1D").unwrap()
        );
        assert_eq!(
            Time::from_str("24:00:00")
                .unwrap()
                .checked_sub(Time::from_str("23:59:59").unwrap())
                .unwrap(),
            Duration::from_str("-PT23H59M59S").unwrap()
        );
    }

    #[test]
    fn add_duration() {
        assert_eq!(
            DateTime::from_str("2000-01-12T12:13:14Z")
                .unwrap()
                .checked_add_duration(Duration::from_str("P1Y3M5DT7H10M3.3S").unwrap())
                .unwrap(),
            DateTime::from_str("2001-04-17T19:23:17.3Z").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-01-01")
                .unwrap()
                .checked_add_duration(Duration::from_str("-P3M").unwrap())
                .unwrap()
                .to_string(),
            "1999-10-01"
        );
        assert_eq!(
            Date::from_str("2000-01-12")
                .unwrap()
                .checked_add_duration(Duration::from_str("PT33H").unwrap())
                .unwrap(),
            Date::from_str("2000-01-13").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-03-30")
                .unwrap()
                .checked_add_duration(Duration::from_str("P1D").unwrap())
                .unwrap(),
            Date::from_str("2000-03-31").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-03-31")
                .unwrap()
                .checked_add_duration(Duration::from_str("P1M").unwrap())
                .unwrap(),
            Date::from_str("2000-04-30").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-03-30")
                .unwrap()
                .checked_add_duration(Duration::from_str("P1M").unwrap())
                .unwrap(),
            Date::from_str("2000-04-30").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-04-30")
                .unwrap()
                .checked_add_duration(Duration::from_str("P1D").unwrap())
                .unwrap(),
            Date::from_str("2000-05-01").unwrap()
        );

        assert_eq!(
            DateTime::from_str("2000-10-30T11:12:00")
                .unwrap()
                .checked_add_duration(Duration::from_str("P1Y2M").unwrap())
                .unwrap(),
            DateTime::from_str("2001-12-30T11:12:00").unwrap()
        );
        assert_eq!(
            DateTime::from_str("2000-10-30T11:12:00")
                .unwrap()
                .checked_add_duration(Duration::from_str("P3DT1H15M").unwrap())
                .unwrap(),
            DateTime::from_str("2000-11-02T12:27:00").unwrap()
        );

        assert_eq!(
            Date::from_str("2000-10-30")
                .unwrap()
                .checked_add_duration(Duration::from_str("P1Y2M").unwrap())
                .unwrap(),
            Date::from_str("2001-12-30").unwrap()
        );
        assert_eq!(
            Date::from_str("2004-10-30Z")
                .unwrap()
                .checked_add_duration(Duration::from_str("P2DT2H30M0S").unwrap())
                .unwrap(),
            Date::from_str("2004-11-01Z").unwrap()
        );

        assert_eq!(
            Time::from_str("11:12:00")
                .unwrap()
                .checked_add_duration(Duration::from_str("P3DT1H15M").unwrap())
                .unwrap(),
            Time::from_str("12:27:00").unwrap()
        );
        assert_eq!(
            Time::from_str("23:12:00+03:00")
                .unwrap()
                .checked_add_duration(Duration::from_str("P1DT3H15M").unwrap())
                .unwrap(),
            Time::from_str("02:27:00+03:00").unwrap()
        );
    }

    #[test]
    fn sub_duration() {
        assert_eq!(
            DateTime::from_str("2000-10-30T11:12:00")
                .unwrap()
                .checked_sub_duration(Duration::from_str("P1Y2M").unwrap())
                .unwrap(),
            DateTime::from_str("1999-08-30T11:12:00").unwrap()
        );
        assert_eq!(
            DateTime::from_str("2000-10-30T11:12:00")
                .unwrap()
                .checked_sub_duration(Duration::from_str("P3DT1H15M").unwrap())
                .unwrap(),
            DateTime::from_str("2000-10-27T09:57:00").unwrap()
        );

        assert_eq!(
            Date::from_str("2000-10-30")
                .unwrap()
                .checked_sub_duration(Duration::from_str("P1Y2M").unwrap())
                .unwrap(),
            Date::from_str("1999-08-30").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-02-29Z")
                .unwrap()
                .checked_sub_duration(Duration::from_str("P1Y").unwrap())
                .unwrap(),
            Date::from_str("1999-02-28Z").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-10-31-05:00")
                .unwrap()
                .checked_sub_duration(Duration::from_str("P1Y1M").unwrap())
                .unwrap(),
            Date::from_str("1999-09-30-05:00").unwrap()
        );
        assert_eq!(
            Date::from_str("2000-10-30")
                .unwrap()
                .checked_sub_duration(Duration::from_str("P3DT1H15M").unwrap())
                .unwrap(),
            Date::from_str("2000-10-26").unwrap()
        );

        assert_eq!(
            Time::from_str("11:12:00")
                .unwrap()
                .checked_sub_duration(Duration::from_str("P3DT1H15M").unwrap())
                .unwrap(),
            Time::from_str("09:57:00").unwrap()
        );
        assert_eq!(
            Time::from_str("08:20:00-05:00")
                .unwrap()
                .checked_sub_duration(Duration::from_str("P23DT10H10M").unwrap())
                .unwrap(),
            Time::from_str("22:10:00-05:00").unwrap()
        );
    }
}
