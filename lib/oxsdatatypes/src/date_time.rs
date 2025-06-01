#![expect(clippy::expect_used)]

use crate::{DayTimeDuration, Decimal, Duration, YearMonthDuration};
use std::cmp::{Ordering, min};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// [XML Schema `dateTime` datatype](https://www.w3.org/TR/xmlschema11-2/#dateTime)
///
/// It encodes the value using a number of seconds from the Gregorian calendar era using a [`Decimal`]
/// and an optional timezone offset in minutes.
#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct DateTime {
    timestamp: Timestamp,
}

impl DateTime {
    pub const MAX: Self = Self {
        timestamp: Timestamp::MAX,
    };
    pub const MIN: Self = Self {
        timestamp: Timestamp::MIN,
    };

    #[inline]
    pub(super) fn new(
        year: i64,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: Decimal,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeOverflowError> {
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

    /// [fn:current-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-current-dateTime)
    #[inline]
    pub fn now() -> Self {
        Self {
            timestamp: Timestamp::now(),
        }
    }

    #[inline]
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    /// [fn:year-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-year-from-dateTime)
    #[inline]
    #[must_use]
    pub fn year(self) -> i64 {
        self.timestamp.year()
    }

    /// [fn:month-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-month-from-dateTime)
    #[inline]
    #[must_use]
    pub fn month(self) -> u8 {
        self.timestamp.month()
    }

    /// [fn:day-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-day-from-dateTime)
    #[inline]
    #[must_use]
    pub fn day(self) -> u8 {
        self.timestamp.day()
    }

    /// [fn:hour-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-hours-from-dateTime)
    #[inline]
    #[must_use]
    pub fn hour(self) -> u8 {
        self.timestamp.hour()
    }

    /// [fn:minute-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-minutes-from-dateTime)
    #[inline]
    #[must_use]
    pub fn minute(self) -> u8 {
        self.timestamp.minute()
    }

    /// [fn:second-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-seconds-from-dateTime)
    #[inline]
    #[must_use]
    pub fn second(self) -> Decimal {
        self.timestamp.second()
    }

    /// [fn:timezone-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-timezone-from-dateTime)
    #[inline]
    #[must_use]
    pub fn timezone(self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    #[must_use]
    pub fn timezone_offset(self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    fn properties(self) -> DateTimeSevenPropertyModel {
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
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// [op:subtract-dateTimes](https://www.w3.org/TR/xpath-functions-31/#func-subtract-dateTimes)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub(self, rhs: impl Into<Self>) -> Option<DayTimeDuration> {
        self.timestamp.checked_sub(rhs.into().timestamp)
    }

    /// [op:add-yearMonthDuration-to-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-add-yearMonthDuration-to-dateTime)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_add_year_month_duration(
        self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-add-dayTimeDuration-to-dateTime)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_add_day_time_duration(self, rhs: impl Into<Duration>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            timestamp: self.timestamp.checked_add_seconds(rhs.all_seconds())?,
        })
    }

    /// [op:add-yearMonthDuration-to-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-add-yearMonthDuration-to-dateTime) and [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-add-dayTimeDuration-to-dateTime)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_add_duration(self, rhs: impl Into<Duration>) -> Option<Self> {
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

    /// [op:subtract-yearMonthDuration-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-subtract-yearMonthDuration-from-dateTime)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub_year_month_duration(
        self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-dayTimeDuration-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-subtract-dayTimeDuration-from-dateTime)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub_day_time_duration(self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        let rhs = rhs.into();
        Some(Self {
            timestamp: self.timestamp.checked_sub_seconds(rhs.as_seconds())?,
        })
    }

    /// [op:subtract-yearMonthDuration-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-subtract-yearMonthDuration-from-dateTime) and [op:subtract-dayTimeDuration-from-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-subtract-dayTimeDuration-from-dateTime)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub_duration(self, rhs: impl Into<Duration>) -> Option<Self> {
        let rhs = rhs.into();
        if let Ok(rhs) = DayTimeDuration::try_from(rhs) {
            self.checked_sub_day_time_duration(rhs)
        } else {
            Some(Self {
                timestamp: Timestamp::new(&date_time_plus_duration(
                    rhs.checked_neg()?,
                    &self.properties(),
                )?)
                .ok()?,
            })
        }
    }

    /// [fn:adjust-dateTime-to-timezone](https://www.w3.org/TR/xpath-functions-31/#func-adjust-dateTime-to-timezone)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn adjust(self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.timestamp.is_identical_with(other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl TryFrom<Date> for DateTime {
    type Error = DateTimeOverflowError;

    #[inline]
    fn try_from(date: Date) -> Result<Self, Self::Error> {
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
    type Err = ParseDateTimeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure_complete(input, date_time_lexical_rep)
    }
}

impl fmt::Display for DateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            f.write_str("-")?;
        }
        let second = self.second();
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{}{}",
            year.abs(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            if Decimal::from(-10) < second && second < Decimal::from(10) {
                "0"
            } else {
                ""
            },
            second
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
    #[cfg(test)]
    const MAX: Self = Self {
        timestamp: Timestamp {
            value: Decimal::new_from_i128_unchecked(62_230_255_200),
            timezone_offset: Some(TimezoneOffset::MIN),
        },
    };
    #[cfg(test)]
    const MIN: Self = Self {
        timestamp: Timestamp {
            value: Decimal::new_from_i128_unchecked(62_230_154_400),
            timezone_offset: Some(TimezoneOffset::MAX),
        },
    };

    #[inline]
    fn new(
        mut hour: u8,
        minute: u8,
        second: Decimal,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeOverflowError> {
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
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    /// [fn:current-time](https://www.w3.org/TR/xpath-functions-31/#func-current-time)
    #[inline]
    pub fn now() -> Self {
        Self {
            timestamp: Timestamp::now(),
        }
    }

    /// [fn:hour-from-time](https://www.w3.org/TR/xpath-functions-31/#func-hours-from-time)
    #[inline]
    #[must_use]
    pub fn hour(self) -> u8 {
        self.timestamp.hour()
    }

    /// [fn:minute-from-time](https://www.w3.org/TR/xpath-functions-31/#func-minutes-from-time)
    #[inline]
    #[must_use]
    pub fn minute(self) -> u8 {
        self.timestamp.minute()
    }

    /// [fn:second-from-time](https://www.w3.org/TR/xpath-functions-31/#func-seconds-from-time)
    #[inline]
    #[must_use]
    pub fn second(self) -> Decimal {
        self.timestamp.second()
    }

    /// [fn:timezone-from-time](https://www.w3.org/TR/xpath-functions-31/#func-timezone-from-time)
    #[inline]
    #[must_use]
    pub fn timezone(self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    #[must_use]
    pub fn timezone_offset(self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// [op:subtract-times](https://www.w3.org/TR/xpath-functions-31/#func-subtract-times)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub(self, rhs: impl Into<Self>) -> Option<DayTimeDuration> {
        self.timestamp.checked_sub(rhs.into().timestamp)
    }

    /// [op:add-dayTimeDuration-to-time](https://www.w3.org/TR/xpath-functions-31/#func-add-dayTimeDuration-to-time)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_add_day_time_duration(self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-dayTimeDuration-to-time](https://www.w3.org/TR/xpath-functions-31/#func-add-dayTimeDuration-to-time)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_add_duration(self, rhs: impl Into<Duration>) -> Option<Self> {
        Some(
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
            .into(),
        )
    }

    /// [op:subtract-dayTimeDuration-from-time](https://www.w3.org/TR/xpath-functions-31/#func-subtract-dayTimeDuration-from-time)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub_day_time_duration(self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-dayTimeDuration-from-time](https://www.w3.org/TR/xpath-functions-31/#func-subtract-dayTimeDuration-from-time)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub_duration(self, rhs: impl Into<Duration>) -> Option<Self> {
        Some(
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
            .into(),
        )
    }

    // [fn:adjust-time-to-timezone](https://www.w3.org/TR/xpath-functions-31/#func-adjust-time-to-timezone)
    #[inline]
    #[must_use]
    pub fn adjust(self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(
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
            .into(),
        )
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.timestamp.is_identical_with(other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl From<DateTime> for Time {
    #[inline]
    fn from(date_time: DateTime) -> Self {
        Self::new(
            date_time.hour(),
            date_time.minute(),
            date_time.second(),
            date_time.timezone_offset(),
        )
        .expect("Casting from xsd:dateTime to xsd:date can't fail")
    }
}

impl FromStr for Time {
    type Err = ParseDateTimeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure_complete(input, time_lexical_rep)
    }
}

impl fmt::Display for Time {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let second = self.second();
        write!(
            f,
            "{:02}:{:02}:{}{}",
            self.hour(),
            self.minute(),
            if Decimal::from(-10) < second && second < Decimal::from(10) {
                "0"
            } else {
                ""
            },
            second
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
    pub const MAX: Self = Self {
        timestamp: Timestamp {
            value: Decimal::new_from_i128_unchecked(170_141_183_460_469_216_800),
            timezone_offset: Some(TimezoneOffset::MAX),
        },
    };
    pub const MIN: Self = Self {
        timestamp: Timestamp {
            value: Decimal::new_from_i128_unchecked(-170_141_183_460_469_216_800),
            timezone_offset: Some(TimezoneOffset::MIN),
        },
    };

    #[inline]
    fn new(
        year: i64,
        month: u8,
        day: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeOverflowError> {
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
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    /// [fn:current-date](https://www.w3.org/TR/xpath-functions-31/#func-current-date)
    #[inline]
    pub fn now() -> Self {
        DateTime::now()
            .try_into()
            .expect("The current time seems way in the future, it's strange")
    }

    /// [fn:year-from-date](https://www.w3.org/TR/xpath-functions-31/#func-year-from-date)
    #[inline]
    #[must_use]
    pub fn year(self) -> i64 {
        self.timestamp.year()
    }

    /// [fn:month-from-date](https://www.w3.org/TR/xpath-functions-31/#func-month-from-date)
    #[inline]
    #[must_use]
    pub fn month(self) -> u8 {
        self.timestamp.month()
    }

    /// [fn:day-from-date](https://www.w3.org/TR/xpath-functions-31/#func-day-from-date)
    #[inline]
    #[must_use]
    pub fn day(self) -> u8 {
        self.timestamp.day()
    }

    /// [fn:timezone-from-date](https://www.w3.org/TR/xpath-functions-31/#func-timezone-from-date)
    #[inline]
    #[must_use]
    pub fn timezone(self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    #[must_use]
    pub fn timezone_offset(self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// [op:subtract-dates](https://www.w3.org/TR/xpath-functions-31/#func-subtract-dates)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub(self, rhs: impl Into<Self>) -> Option<DayTimeDuration> {
        self.timestamp.checked_sub(rhs.into().timestamp)
    }

    /// [op:add-yearMonthDuration-to-date](https://www.w3.org/TR/xpath-functions-31/#func-add-yearMonthDuration-to-date)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_add_year_month_duration(
        self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-add-dayTimeDuration-to-date)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_add_day_time_duration(self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_add_duration(Duration::from(rhs.into()))
    }

    /// [op:add-yearMonthDuration-to-date](https://www.w3.org/TR/xpath-functions-31/#func-add-yearMonthDuration-to-date) and [op:add-dayTimeDuration-to-dateTime](https://www.w3.org/TR/xpath-functions-31/#func-add-dayTimeDuration-to-date)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_add_duration(self, rhs: impl Into<Duration>) -> Option<Self> {
        DateTime::try_from(self)
            .ok()?
            .checked_add_duration(rhs)?
            .try_into()
            .ok()
    }

    /// [op:subtract-yearMonthDuration-from-date](https://www.w3.org/TR/xpath-functions-31/#func-subtract-yearMonthDuration-from-date)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub_year_month_duration(
        self,
        rhs: impl Into<YearMonthDuration>,
    ) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-dayTimeDuration-from-date](https://www.w3.org/TR/xpath-functions-31/#func-subtract-dayTimeDuration-from-date)
    ///
    /// Returns `None` in case of overflow ([`FODT0001`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001)).
    #[inline]
    #[must_use]
    pub fn checked_sub_day_time_duration(self, rhs: impl Into<DayTimeDuration>) -> Option<Self> {
        self.checked_sub_duration(Duration::from(rhs.into()))
    }

    /// [op:subtract-yearMonthDuration-from-date](https://www.w3.org/TR/xpath-functions-31/#func-subtract-yearMonthDuration-from-date) and [op:subtract-dayTimeDuration-from-date](https://www.w3.org/TR/xpath-functions-31/#func-subtract-dayTimeDuration-from-date)
    #[inline]
    #[must_use]
    pub fn checked_sub_duration(self, rhs: impl Into<Duration>) -> Option<Self> {
        DateTime::try_from(self)
            .ok()?
            .checked_sub_duration(rhs)?
            .try_into()
            .ok()
    }

    // [fn:adjust-date-to-timezone](https://www.w3.org/TR/xpath-functions-31/#func-adjust-date-to-timezone)
    #[inline]
    #[must_use]
    pub fn adjust(self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
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
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.timestamp.is_identical_with(other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl TryFrom<DateTime> for Date {
    type Error = DateTimeOverflowError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, Self::Error> {
        Self::new(
            date_time.year(),
            date_time.month(),
            date_time.day(),
            date_time.timezone_offset(),
        )
    }
}

impl FromStr for Date {
    type Err = ParseDateTimeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure_complete(input, date_lexical_rep)
    }
}

impl fmt::Display for Date {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            f.write_str("-")?;
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
    pub const MAX: Self = Self {
        timestamp: Timestamp {
            value: Decimal::new_from_i128_unchecked(170_141_183_460_469_216_800),
            timezone_offset: Some(TimezoneOffset::MAX),
        },
    };
    pub const MIN: Self = Self {
        timestamp: Timestamp {
            value: Decimal::new_from_i128_unchecked(-170_141_183_460_466_970_400),
            timezone_offset: Some(TimezoneOffset::MIN),
        },
    };

    #[inline]
    fn new(
        year: i64,
        month: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeOverflowError> {
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
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    #[must_use]
    pub fn year(self) -> i64 {
        self.timestamp.year()
    }

    #[inline]
    #[must_use]
    pub fn month(self) -> u8 {
        self.timestamp.month()
    }

    #[inline]
    #[must_use]
    pub fn timezone(self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    #[must_use]
    pub fn timezone_offset(self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    #[must_use]
    pub fn adjust(self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.timestamp.is_identical_with(other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl TryFrom<DateTime> for GYearMonth {
    type Error = DateTimeOverflowError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, Self::Error> {
        Self::new(
            date_time.year(),
            date_time.month(),
            date_time.timezone_offset(),
        )
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl From<Date> for GYearMonth {
    #[inline]
    fn from(date: Date) -> Self {
        Self::new(date.year(), date.month(), date.timezone_offset())
            .expect("Casting from xsd:date to xsd:gYearMonth can't fail")
    }
}

impl FromStr for GYearMonth {
    type Err = ParseDateTimeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure_complete(input, g_year_month_lexical_rep)
    }
}

impl fmt::Display for GYearMonth {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            f.write_str("-")?;
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
    pub const MAX: Self = Self {
        timestamp: Timestamp {
            value: Decimal::new_from_i128_unchecked(170_141_183_460_461_440_800),
            timezone_offset: Some(TimezoneOffset::MAX),
        },
    };
    pub const MIN: Self = Self {
        timestamp: Timestamp {
            value: Decimal::new_from_i128_unchecked(-170_141_183_460_461_700_000),
            timezone_offset: Some(TimezoneOffset::MIN),
        },
    };

    #[inline]
    fn new(
        year: i64,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeOverflowError> {
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
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    #[must_use]
    pub fn year(self) -> i64 {
        self.timestamp.year()
    }

    #[inline]
    #[must_use]
    pub fn timezone(self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    #[must_use]
    pub fn timezone_offset(self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    #[must_use]
    pub fn adjust(self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.timestamp.is_identical_with(other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl TryFrom<DateTime> for GYear {
    type Error = DateTimeOverflowError;

    #[inline]
    fn try_from(date_time: DateTime) -> Result<Self, Self::Error> {
        Self::new(date_time.year(), date_time.timezone_offset())
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl TryFrom<Date> for GYear {
    type Error = DateTimeOverflowError;

    #[inline]
    fn try_from(date: Date) -> Result<Self, Self::Error> {
        Self::new(date.year(), date.timezone_offset())
    }
}

impl TryFrom<GYearMonth> for GYear {
    type Error = DateTimeOverflowError;

    #[inline]
    fn try_from(year_month: GYearMonth) -> Result<Self, Self::Error> {
        Self::new(year_month.year(), year_month.timezone_offset())
    }
}

impl FromStr for GYear {
    type Err = ParseDateTimeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure_complete(input, g_year_lexical_rep)
    }
}

impl fmt::Display for GYear {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.year();
        if year < 0 {
            f.write_str("-")?;
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
    fn new(
        month: u8,
        day: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeOverflowError> {
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
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    #[must_use]
    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    #[inline]
    #[must_use]
    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    #[inline]
    #[must_use]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    #[must_use]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    #[must_use]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.timestamp.is_identical_with(other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl From<DateTime> for GMonthDay {
    #[inline]
    fn from(date_time: DateTime) -> Self {
        Self::new(
            date_time.month(),
            date_time.day(),
            date_time.timezone_offset(),
        )
        .expect("Casting from xsd:dateTime to xsd:gMonthDay can't fail")
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl From<Date> for GMonthDay {
    #[inline]
    fn from(date: Date) -> Self {
        Self::new(date.month(), date.day(), date.timezone_offset())
            .expect("Casting from xsd:date to xsd:gMonthDay can't fail")
    }
}

impl FromStr for GMonthDay {
    type Err = ParseDateTimeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure_complete(input, g_month_day_lexical_rep)
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
    fn new(
        month: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeOverflowError> {
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
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    #[must_use]
    pub fn month(&self) -> u8 {
        self.timestamp.month()
    }

    #[inline]
    #[must_use]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    #[must_use]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    #[must_use]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.timestamp.is_identical_with(other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl From<DateTime> for GMonth {
    #[inline]
    fn from(date_time: DateTime) -> Self {
        Self::new(date_time.month(), date_time.timezone_offset())
            .expect("Casting from xsd:dateTime to xsd:gMonth can't fail")
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl From<Date> for GMonth {
    #[inline]
    fn from(date: Date) -> Self {
        Self::new(date.month(), date.timezone_offset())
            .expect("Casting from xsd:date to xsd:gMonth can't fail")
    }
}

impl From<GYearMonth> for GMonth {
    #[inline]
    fn from(year_month: GYearMonth) -> Self {
        Self::new(year_month.month(), year_month.timezone_offset())
            .expect("Casting from xsd:gYearMonth to xsd:gMonth can't fail")
    }
}

impl From<GMonthDay> for GMonth {
    #[inline]
    fn from(month_day: GMonthDay) -> Self {
        Self::new(month_day.month(), month_day.timezone_offset())
            .expect("Casting from xsd:gMonthDay to xsd:gMonth can't fail")
    }
}

impl FromStr for GMonth {
    type Err = ParseDateTimeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure_complete(input, g_month_lexical_rep)
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
    fn new(
        day: u8,
        timezone_offset: Option<TimezoneOffset>,
    ) -> Result<Self, DateTimeOverflowError> {
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
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 18]) -> Self {
        Self {
            timestamp: Timestamp::from_be_bytes(bytes),
        }
    }

    #[inline]
    #[must_use]
    pub fn day(&self) -> u8 {
        self.timestamp.day()
    }

    #[inline]
    #[must_use]
    pub fn timezone(&self) -> Option<DayTimeDuration> {
        Some(self.timezone_offset()?.into())
    }

    #[inline]
    #[must_use]
    pub fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timestamp.timezone_offset()
    }

    #[inline]
    #[must_use]
    pub fn adjust(&self, timezone_offset: Option<TimezoneOffset>) -> Option<Self> {
        Some(Self {
            timestamp: self.timestamp.adjust(timezone_offset)?,
        })
    }

    #[inline]
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 18] {
        self.timestamp.to_be_bytes()
    }

    /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
    #[inline]
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.timestamp.is_identical_with(other.timestamp)
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl From<DateTime> for GDay {
    #[inline]
    fn from(date_time: DateTime) -> Self {
        Self::new(date_time.day(), date_time.timezone_offset())
            .expect("Casting from xsd:dateTime to xsd:gDay can't fail")
    }
}

/// Conversion according to [XPath cast rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-datetimes).
impl From<Date> for GDay {
    #[inline]
    fn from(date: Date) -> Self {
        Self::new(date.day(), date.timezone_offset())
            .expect("Casting from xsd:date to xsd:gDay can't fail")
    }
}

impl From<GMonthDay> for GDay {
    #[inline]
    fn from(month_day: GMonthDay) -> Self {
        Self::new(month_day.day(), month_day.timezone_offset())
            .expect("Casting from xsd:gMonthDay to xsd:gDay can't fail")
    }
}

impl FromStr for GDay {
    type Err = ParseDateTimeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure_complete(input, g_day_lexical_rep)
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
    pub const MAX: Self = Self { offset: 14 * 60 };
    pub const MIN: Self = Self { offset: -14 * 60 };
    pub const UTC: Self = Self { offset: 0 };

    /// From offset in minute with respect to UTC
    #[inline]
    pub fn new(offset_in_minutes: i16) -> Result<Self, InvalidTimezoneError> {
        let value = Self {
            offset: offset_in_minutes,
        };
        if Self::MIN <= value && value <= Self::MAX {
            Ok(value)
        } else {
            Err(InvalidTimezoneError {
                offset_in_minutes: offset_in_minutes.into(),
            })
        }
    }

    #[inline]
    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 2]) -> Self {
        Self {
            offset: i16::from_be_bytes(bytes),
        }
    }

    #[inline]
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 2] {
        self.offset.to_be_bytes()
    }
}

impl TryFrom<DayTimeDuration> for TimezoneOffset {
    type Error = InvalidTimezoneError;

    #[inline]
    fn try_from(value: DayTimeDuration) -> Result<Self, Self::Error> {
        let offset_in_minutes = value.minutes() + value.hours() * 60;
        let result = Self::new(
            offset_in_minutes
                .try_into()
                .map_err(|_| Self::Error { offset_in_minutes })?,
        )?;
        if DayTimeDuration::from(result) == value {
            Ok(result)
        } else {
            // The value is not an integral number of minutes or overflow problems
            Err(Self::Error { offset_in_minutes })
        }
    }
}

impl TryFrom<Duration> for TimezoneOffset {
    type Error = InvalidTimezoneError;

    #[inline]
    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        DayTimeDuration::try_from(value)
            .map_err(|_| Self::Error {
                offset_in_minutes: 0,
            })?
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
            0 => f.write_str("Z"),
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
            _ => false, // TODO: implicit timezone
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
    pub const MAX: Self = Self {
        value: Decimal::MAX,
        timezone_offset: Some(TimezoneOffset::MAX),
    };
    pub const MIN: Self = Self {
        value: Decimal::MIN,
        timezone_offset: Some(TimezoneOffset::MIN),
    };

    #[inline]
    fn new(props: &DateTimeSevenPropertyModel) -> Result<Self, DateTimeOverflowError> {
        Ok(Self {
            timezone_offset: props.timezone_offset,
            value: time_on_timeline(props).ok_or(DateTimeOverflowError)?,
        })
    }

    #[inline]
    fn now() -> Self {
        Self::new(
            &date_time_plus_duration(
                since_unix_epoch(),
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
            .expect("The current time seems way in the future, it's strange"),
        )
        .expect("The current time seems way in the future, it's strange")
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

    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[inline]
    #[must_use]
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
    #[must_use]
    fn year(&self) -> i64 {
        let (year, _, _) = self.year_month_day();
        year
    }

    #[inline]
    #[must_use]
    fn month(&self) -> u8 {
        let (_, month, _) = self.year_month_day();
        month
    }

    #[inline]
    #[must_use]
    fn day(&self) -> u8 {
        let (_, _, day) = self.year_month_day();
        day
    }

    #[expect(clippy::cast_possible_truncation)]
    #[inline]
    #[must_use]
    fn hour(&self) -> u8 {
        (((self.value.as_i128()
            + i128::from(self.timezone_offset.unwrap_or(TimezoneOffset::UTC).offset) * 60)
            .rem_euclid(86400))
            / 3600) as u8
    }

    #[expect(clippy::cast_possible_truncation)]
    #[inline]
    #[must_use]
    fn minute(&self) -> u8 {
        (((self.value.as_i128()
            + i128::from(self.timezone_offset.unwrap_or(TimezoneOffset::UTC).offset) * 60)
            .rem_euclid(3600))
            / 60) as u8
    }

    #[inline]
    #[must_use]
    fn second(&self) -> Decimal {
        self.value
            .checked_rem_euclid(60)
            .unwrap()
            .checked_abs()
            .unwrap()
    }

    #[inline]
    #[must_use]
    const fn timezone_offset(&self) -> Option<TimezoneOffset> {
        self.timezone_offset
    }

    #[inline]
    #[must_use]
    fn checked_add_seconds(&self, seconds: impl Into<Decimal>) -> Option<Self> {
        Some(Self {
            value: self.value.checked_add(seconds.into())?,
            timezone_offset: self.timezone_offset,
        })
    }

    #[inline]
    #[must_use]
    fn checked_sub(&self, rhs: Self) -> Option<DayTimeDuration> {
        match (self.timezone_offset, rhs.timezone_offset) {
            (Some(_), Some(_)) | (None, None) => {
                Some(DayTimeDuration::new(self.value.checked_sub(rhs.value)?))
            }
            _ => None, // TODO: implicit timezone
        }
    }

    #[inline]
    #[must_use]
    fn checked_sub_seconds(&self, seconds: Decimal) -> Option<Self> {
        Some(Self {
            value: self.value.checked_sub(seconds)?,
            timezone_offset: self.timezone_offset,
        })
    }

    #[inline]
    #[must_use]
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
                        .checked_add(i64::from(from_timezone.offset) * 60)?, /* We keep the literal value */
                    timezone_offset: None,
                }
            }
        } else if let Some(to_timezone) = timezone_offset {
            Self {
                value: self.value.checked_sub(i64::from(to_timezone.offset) * 60)?, /* We keep the literal value */
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
    #[must_use]
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
    #[must_use]
    pub fn is_identical_with(self, other: Self) -> bool {
        self.value == other.value && self.timezone_offset == other.timezone_offset
    }
}

#[cfg(feature = "custom-now")]
#[expect(unsafe_code)]
pub fn since_unix_epoch() -> Duration {
    unsafe extern "Rust" {
        fn custom_ox_now() -> Duration;
    }

    // SAFETY: Must be defined, if not compilation fails
    unsafe { custom_ox_now() }
}

#[cfg(all(not(feature = "custom-now"), target_os = "zkvm"))]
fn since_unix_epoch() -> Duration {
    DayTimeDuration::new(0).into()
}

#[cfg(all(
    feature = "js",
    not(feature = "custom-now"),
    target_family = "wasm",
    target_os = "unknown"
))]
fn since_unix_epoch() -> Duration {
    DayTimeDuration::new(
        Decimal::try_from(crate::Double::from(js_sys::Date::now() / 1000.))
            .expect("The current time seems way in the future, it's strange"),
    )
    .into()
}

#[cfg(not(any(
    feature = "custom-now",
    target_os = "zkvm",
    all(feature = "js", target_family = "wasm", target_os = "unknown")
)))]
fn since_unix_epoch() -> Duration {
    use std::time::SystemTime;

    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("System time before UNIX epoch")
        .try_into()
        .expect("The current time seems way in the future, it's strange")
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
    let mi = mi.checked_add(i64::try_from(se.as_i128().checked_div(60)?).ok()?)?; // TODO: good idea?
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

/// A parsing error
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ParseDateTimeError(#[from] ParseDateTimeErrorKind);

#[derive(Debug, Clone, thiserror::Error)]
enum ParseDateTimeErrorKind {
    #[error("{day} is not a valid day of {month}")]
    InvalidDayOfMonth { day: u8, month: u8 },
    #[error(transparent)]
    Overflow(#[from] DateTimeOverflowError),
    #[error(transparent)]
    InvalidTimezone(InvalidTimezoneError),
    #[error("{0}")]
    Message(&'static str),
}

impl ParseDateTimeError {
    const fn msg(message: &'static str) -> Self {
        Self(ParseDateTimeErrorKind::Message(message))
    }
}

// [16]   dateTimeLexicalRep ::= yearFrag '-' monthFrag '-' dayFrag 'T' ((hourFrag ':' minuteFrag ':' secondFrag) | endOfDayFrag) timezoneFrag?
fn date_time_lexical_rep(input: &str) -> Result<(DateTime, &str), ParseDateTimeError> {
    let (year, input) = year_frag(input)?;
    let input = expect_char(input, '-', "The year and month must be separated by '-'")?;
    let (month, input) = month_frag(input)?;
    let input = expect_char(input, '-', "The month and day must be separated by '-'")?;
    let (day, input) = day_frag(input)?;
    let input = expect_char(input, 'T', "The date and time must be separated by 'T'")?;
    let (hour, input) = hour_frag(input)?;
    let input = expect_char(input, ':', "The hours and minutes must be separated by ':'")?;
    let (minute, input) = minute_frag(input)?;
    let input = expect_char(
        input,
        ':',
        "The minutes and seconds must be separated by ':'",
    )?;
    let (second, input) = second_frag(input)?;
    // We validate 24:00:00
    if hour == 24 && minute != 0 && second != Decimal::from(0) {
        return Err(ParseDateTimeError::msg(
            "Times are not allowed to be after 24:00:00",
        ));
    }
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    validate_day_of_month(Some(year), month, day)?;
    Ok((
        DateTime::new(year, month, day, hour, minute, second, timezone_offset)?,
        input,
    ))
}

// [17]   timeLexicalRep ::= ((hourFrag ':' minuteFrag ':' secondFrag) | endOfDayFrag) timezoneFrag?
fn time_lexical_rep(input: &str) -> Result<(Time, &str), ParseDateTimeError> {
    let (hour, input) = hour_frag(input)?;
    let input = expect_char(input, ':', "The hours and minutes must be separated by ':'")?;
    let (minute, input) = minute_frag(input)?;
    let input = expect_char(
        input,
        ':',
        "The minutes and seconds must be separated by ':'",
    )?;
    let (second, input) = second_frag(input)?;
    // We validate 24:00:00
    if hour == 24 && minute != 0 && second != Decimal::from(0) {
        return Err(ParseDateTimeError::msg(
            "Times are not allowed to be after 24:00:00",
        ));
    }
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((Time::new(hour, minute, second, timezone_offset)?, input))
}

// [18]   dateLexicalRep ::= yearFrag '-' monthFrag '-' dayFrag timezoneFrag?   Constraint:  Day-of-month Representations
fn date_lexical_rep(input: &str) -> Result<(Date, &str), ParseDateTimeError> {
    let (year, input) = year_frag(input)?;
    let input = expect_char(input, '-', "The year and month must be separated by '-'")?;
    let (month, input) = month_frag(input)?;
    let input = expect_char(input, '-', "The month and day must be separated by '-'")?;
    let (day, input) = day_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    validate_day_of_month(Some(year), month, day)?;
    Ok((Date::new(year, month, day, timezone_offset)?, input))
}

// [19]   gYearMonthLexicalRep ::= yearFrag '-' monthFrag timezoneFrag?
fn g_year_month_lexical_rep(input: &str) -> Result<(GYearMonth, &str), ParseDateTimeError> {
    let (year, input) = year_frag(input)?;
    let input = expect_char(input, '-', "The year and month must be separated by '-'")?;
    let (month, input) = month_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((GYearMonth::new(year, month, timezone_offset)?, input))
}

// [20]   gYearLexicalRep ::= yearFrag timezoneFrag?
fn g_year_lexical_rep(input: &str) -> Result<(GYear, &str), ParseDateTimeError> {
    let (year, input) = year_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((GYear::new(year, timezone_offset)?, input))
}

// [21]   gMonthDayLexicalRep ::= '--' monthFrag '-' dayFrag timezoneFrag?   Constraint:  Day-of-month Representations
fn g_month_day_lexical_rep(input: &str) -> Result<(GMonthDay, &str), ParseDateTimeError> {
    let input = expect_char(input, '-', "gMonthDay values must start with '--'")?;
    let input = expect_char(input, '-', "gMonthDay values must start with '--'")?;
    let (month, input) = month_frag(input)?;
    let input = expect_char(input, '-', "The month and day must be separated by '-'")?;
    let (day, input) = day_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    validate_day_of_month(None, month, day)?;
    Ok((GMonthDay::new(month, day, timezone_offset)?, input))
}

// [22]   gDayLexicalRep ::= '---' dayFrag timezoneFrag?
fn g_day_lexical_rep(input: &str) -> Result<(GDay, &str), ParseDateTimeError> {
    let input = expect_char(input, '-', "gDay values must start with '---'")?;
    let input = expect_char(input, '-', "gDay values must start with '---'")?;
    let input = expect_char(input, '-', "gDay values must start with '---'")?;
    let (day, input) = day_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((GDay::new(day, timezone_offset)?, input))
}

// [23]   gMonthLexicalRep ::= '--' monthFrag timezoneFrag?
fn g_month_lexical_rep(input: &str) -> Result<(GMonth, &str), ParseDateTimeError> {
    let input = expect_char(input, '-', "gMonth values must start with '--'")?;
    let input = expect_char(input, '-', "gMonth values must start with '--'")?;
    let (month, input) = month_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((GMonth::new(month, timezone_offset)?, input))
}

// [56]   yearFrag ::= '-'? (([1-9] digit digit digit+)) | ('0' digit digit digit))
fn year_frag(input: &str) -> Result<(i64, &str), ParseDateTimeError> {
    let (sign, input) = if let Some(left) = input.strip_prefix('-') {
        (-1, left)
    } else {
        (1, input)
    };
    let (number_str, input) = integer_prefix(input);
    if number_str.len() < 4 {
        return Err(ParseDateTimeError::msg(
            "The year should be encoded on 4 digits",
        ));
    }
    if number_str.len() > 4 && number_str.starts_with('0') {
        return Err(ParseDateTimeError::msg(
            "The years value must not start with 0 if it can be encoded in at least 4 digits",
        ));
    }
    let number = i64::from_str(number_str).expect("valid integer");
    Ok((sign * number, input))
}

// [57]   monthFrag ::= ('0' [1-9]) | ('1' [0-2])
fn month_frag(input: &str) -> Result<(u8, &str), ParseDateTimeError> {
    let (number_str, input) = integer_prefix(input);
    if number_str.len() != 2 {
        return Err(ParseDateTimeError::msg(
            "Month must be encoded with two digits",
        ));
    }
    let number = u8::from_str(number_str).expect("valid integer");
    if !(1..=12).contains(&number) {
        return Err(ParseDateTimeError::msg("Month must be between 01 and 12"));
    }
    Ok((number, input))
}

// [58]   dayFrag ::= ('0' [1-9]) | ([12] digit) | ('3' [01])
fn day_frag(input: &str) -> Result<(u8, &str), ParseDateTimeError> {
    let (number_str, input) = integer_prefix(input);
    if number_str.len() != 2 {
        return Err(ParseDateTimeError::msg(
            "Day must be encoded with two digits",
        ));
    }
    let number = u8::from_str(number_str).expect("valid integer");
    if !(1..=31).contains(&number) {
        return Err(ParseDateTimeError::msg("Day must be between 01 and 31"));
    }
    Ok((number, input))
}

// [59]   hourFrag ::= ([01] digit) | ('2' [0-3])
// We also allow 24 for ease of parsing
fn hour_frag(input: &str) -> Result<(u8, &str), ParseDateTimeError> {
    let (number_str, input) = integer_prefix(input);
    if number_str.len() != 2 {
        return Err(ParseDateTimeError::msg(
            "Hours must be encoded with two digits",
        ));
    }
    let number = u8::from_str(number_str).expect("valid integer");
    if !(0..=24).contains(&number) {
        return Err(ParseDateTimeError::msg("Hours must be between 00 and 24"));
    }
    Ok((number, input))
}

// [60]   minuteFrag ::= [0-5] digit
fn minute_frag(input: &str) -> Result<(u8, &str), ParseDateTimeError> {
    let (number_str, input) = integer_prefix(input);
    if number_str.len() != 2 {
        return Err(ParseDateTimeError::msg(
            "Minutes must be encoded with two digits",
        ));
    }
    let number = u8::from_str(number_str).expect("valid integer");
    if !(0..=59).contains(&number) {
        return Err(ParseDateTimeError::msg("Minutes must be between 00 and 59"));
    }
    Ok((number, input))
}

// [61]   secondFrag ::= ([0-5] digit) ('.' digit+)?
fn second_frag(input: &str) -> Result<(Decimal, &str), ParseDateTimeError> {
    let (number_str, input) = decimal_prefix(input);
    let (before_dot_str, _) = number_str.split_once('.').unwrap_or((number_str, ""));
    if before_dot_str.len() != 2 {
        return Err(ParseDateTimeError::msg(
            "Seconds must be encoded with two digits",
        ));
    }
    let number = Decimal::from_str(number_str)
        .map_err(|_| ParseDateTimeError::msg("The second precision is too large"))?;
    if number < Decimal::from(0) || number >= Decimal::from(60) {
        return Err(ParseDateTimeError::msg("Seconds must be between 00 and 60"));
    }
    if number_str.ends_with('.') {
        return Err(ParseDateTimeError::msg(
            "Seconds are not allowed to end with a dot",
        ));
    }
    Ok((number, input))
}

// [63]   timezoneFrag ::= 'Z' | ('+' | '-') (('0' digit | '1' [0-3]) ':' minuteFrag | '14:00')
fn timezone_frag(input: &str) -> Result<(TimezoneOffset, &str), ParseDateTimeError> {
    if let Some(left) = input.strip_prefix('Z') {
        return Ok((TimezoneOffset::UTC, left));
    }
    let (sign, input) = if let Some(left) = input.strip_prefix('-') {
        (-1, left)
    } else if let Some(left) = input.strip_prefix('+') {
        (1, left)
    } else {
        (1, input)
    };

    let (hour_str, input) = integer_prefix(input);
    if hour_str.len() != 2 {
        return Err(ParseDateTimeError::msg(
            "The timezone hours must be encoded with two digits",
        ));
    }
    let hours = i16::from_str(hour_str).expect("valid integer");

    let input = expect_char(
        input,
        ':',
        "The timezone hours and minutes must be separated by ':'",
    )?;
    let (minutes, input) = minute_frag(input)?;

    if hours > 13 && !(hours == 14 && minutes == 0) {
        return Err(ParseDateTimeError::msg(
            "The timezone hours must be between 00 and 13",
        ));
    }

    Ok((
        TimezoneOffset::new(sign * (hours * 60 + i16::from(minutes)))
            .map_err(|e| ParseDateTimeError(ParseDateTimeErrorKind::InvalidTimezone(e)))?,
        input,
    ))
}

fn ensure_complete<T>(
    input: &str,
    parse: impl FnOnce(&str) -> Result<(T, &str), ParseDateTimeError>,
) -> Result<T, ParseDateTimeError> {
    let (result, left) = parse(input)?;
    if !left.is_empty() {
        return Err(ParseDateTimeError::msg("Unrecognized value suffix"));
    }
    Ok(result)
}

fn expect_char<'a>(
    input: &'a str,
    constant: char,
    error_message: &'static str,
) -> Result<&'a str, ParseDateTimeError> {
    if let Some(left) = input.strip_prefix(constant) {
        Ok(left)
    } else {
        Err(ParseDateTimeError::msg(error_message))
    }
}

fn integer_prefix(input: &str) -> (&str, &str) {
    let mut end = input.len();
    for (i, c) in input.char_indices() {
        if !c.is_ascii_digit() {
            end = i;
            break;
        }
    }
    input.split_at(end)
}

fn decimal_prefix(input: &str) -> (&str, &str) {
    let mut end = input.len();
    let mut dot_seen = false;
    for (i, c) in input.char_indices() {
        if c.is_ascii_digit() {
            // Ok
        } else if c == '.' && !dot_seen {
            dot_seen = true;
        } else {
            end = i;
            break;
        }
    }
    input.split_at(end)
}

fn optional_end<T>(
    input: &str,
    parse: impl FnOnce(&str) -> Result<(T, &str), ParseDateTimeError>,
) -> Result<(Option<T>, &str), ParseDateTimeError> {
    Ok(if input.is_empty() {
        (None, input)
    } else {
        let (result, input) = parse(input)?;
        (Some(result), input)
    })
}

fn validate_day_of_month(year: Option<i64>, month: u8, day: u8) -> Result<(), ParseDateTimeError> {
    // Constraint: Day-of-month Values
    if day > days_in_month(year, month) {
        return Err(ParseDateTimeError(
            ParseDateTimeErrorKind::InvalidDayOfMonth { day, month },
        ));
    }
    Ok(())
}

/// An overflow during [`DateTime`]-related operations.
///
/// Matches XPath [`FODT0001` error](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0001).
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("overflow during xsd:dateTime computation")]
pub struct DateTimeOverflowError;

impl From<DateTimeOverflowError> for ParseDateTimeError {
    fn from(error: DateTimeOverflowError) -> Self {
        Self(ParseDateTimeErrorKind::Overflow(error))
    }
}

/// The value provided as timezone is not valid.
///
/// Matches XPath [`FODT0003` error](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0003).
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("invalid timezone offset {}:{}",
        self.offset_in_minutes / 60,
        self.offset_in_minutes.abs() % 60)]
pub struct InvalidTimezoneError {
    offset_in_minutes: i64,
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn from_str() -> Result<(), ParseDateTimeError> {
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
            DateTime::from_str("-0899-03-01T00:00:00")?.to_string(),
            "-0899-03-01T00:00:00"
        );
        assert_eq!(
            DateTime::from_str("2000-01-01T00:00:00.1234567")?.to_string(),
            "2000-01-01T00:00:00.1234567"
        );
        assert_eq!(
            DateTime::from_str("2000-01-01T00:00:12.1234567")?.to_string(),
            "2000-01-01T00:00:12.1234567"
        );
        assert_eq!(
            Time::from_str("01:02:03.1234567")?.to_string(),
            "01:02:03.1234567"
        );
        assert_eq!(
            Time::from_str("01:02:13.1234567")?.to_string(),
            "01:02:13.1234567"
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

        GYear::from_str("02020").unwrap_err();
        GYear::from_str("+2020").unwrap_err();
        GYear::from_str("33").unwrap_err();

        assert_eq!(Time::from_str("00:00:00+14:00")?, Time::MIN);
        assert_eq!(Time::from_str("24:00:00-14:00")?, Time::MAX);
        Ok(())
    }

    #[test]
    fn to_be_bytes() -> Result<(), ParseDateTimeError> {
        assert_eq!(
            DateTime::from_be_bytes(DateTime::MIN.to_be_bytes()),
            DateTime::MIN
        );
        assert_eq!(
            DateTime::from_be_bytes(DateTime::MAX.to_be_bytes()),
            DateTime::MAX
        );
        assert_eq!(
            DateTime::from_be_bytes(DateTime::from_str("2022-01-03T01:02:03")?.to_be_bytes()),
            DateTime::from_str("2022-01-03T01:02:03")?
        );
        assert_eq!(Date::from_be_bytes(Date::MIN.to_be_bytes()), Date::MIN);
        assert_eq!(Date::from_be_bytes(Date::MAX.to_be_bytes()), Date::MAX);
        assert_eq!(
            Date::from_be_bytes(Date::from_str("2022-01-03")?.to_be_bytes()),
            Date::from_str("2022-01-03")?
        );
        assert_eq!(Time::from_be_bytes(Time::MIN.to_be_bytes()), Time::MIN);
        assert_eq!(Time::from_be_bytes(Time::MAX.to_be_bytes()), Time::MAX);
        assert_eq!(
            Time::from_be_bytes(Time::from_str("01:02:03")?.to_be_bytes()),
            Time::from_str("01:02:03")?
        );
        assert_eq!(
            Time::from_be_bytes(Time::from_str("01:02:03")?.to_be_bytes()),
            Time::from_str("01:02:03")?
        );
        assert_eq!(
            GYearMonth::from_be_bytes(GYearMonth::MIN.to_be_bytes()),
            GYearMonth::MIN
        );
        assert_eq!(
            GYearMonth::from_be_bytes(GYearMonth::MAX.to_be_bytes()),
            GYearMonth::MAX
        );
        assert_eq!(GYear::from_be_bytes(GYear::MIN.to_be_bytes()), GYear::MIN);
        assert_eq!(GYear::from_be_bytes(GYear::MAX.to_be_bytes()), GYear::MAX);
        Ok(())
    }

    #[test]
    fn equals() -> Result<(), ParseDateTimeError> {
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
    #[expect(clippy::neg_cmp_op_on_partial_ord)]
    fn cmp() -> Result<(), ParseDateTimeError> {
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
        assert!(
            GDay::from_str("---15-13:00")?
                .partial_cmp(&GDay::from_str("---16")?)
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn year() -> Result<(), ParseDateTimeError> {
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
    fn month() -> Result<(), ParseDateTimeError> {
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
    fn day() -> Result<(), ParseDateTimeError> {
        assert_eq!(DateTime::from_str("1999-05-31T13:20:00-05:00")?.day(), 31);
        assert_eq!(DateTime::from_str("1999-12-31T20:00:00-05:00")?.day(), 31);

        assert_eq!(Date::from_str("1999-05-31-05:00")?.day(), 31);
        assert_eq!(Date::from_str("2000-01-01+05:00")?.day(), 1);

        assert_eq!(GDay::from_str("---03")?.day(), 3);
        assert_eq!(GMonthDay::from_str("--02-03")?.day(), 3);
        Ok(())
    }

    #[test]
    fn hour() -> Result<(), ParseDateTimeError> {
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
    fn minute() -> Result<(), ParseDateTimeError> {
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
    fn second() -> Result<(), Box<dyn Error>> {
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
    fn timezone() -> Result<(), Box<dyn Error>> {
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
    fn sub() -> Result<(), Box<dyn Error>> {
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
    fn add_duration() -> Result<(), Box<dyn Error>> {
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
    fn sub_duration() -> Result<(), Box<dyn Error>> {
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
    fn adjust() -> Result<(), Box<dyn Error>> {
        assert_eq!(
            DateTime::from_str("2002-03-07T10:00:00-07:00")?
                .adjust(Some(DayTimeDuration::from_str("PT10H")?.try_into()?)),
            Some(DateTime::from_str("2002-03-08T03:00:00+10:00")?)
        );
        assert_eq!(
            DateTime::from_str("2002-03-07T00:00:00+01:00")?
                .adjust(Some(DayTimeDuration::from_str("-PT8H")?.try_into()?)),
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
            Date::from_str("2002-03-07")?
                .adjust(Some(DayTimeDuration::from_str("-PT10H")?.try_into()?)),
            Some(Date::from_str("2002-03-07-10:00")?)
        );
        assert_eq!(
            Date::from_str("2002-03-07-07:00")?
                .adjust(Some(DayTimeDuration::from_str("-PT10H")?.try_into()?)),
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
            Time::from_str("10:00:00")?
                .adjust(Some(DayTimeDuration::from_str("-PT10H")?.try_into()?)),
            Some(Time::from_str("10:00:00-10:00")?)
        );
        assert_eq!(
            Time::from_str("10:00:00-07:00")?
                .adjust(Some(DayTimeDuration::from_str("-PT10H")?.try_into()?)),
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
            Time::from_str("10:00:00-07:00")?
                .adjust(Some(DayTimeDuration::from_str("PT10H")?.try_into()?)),
            Some(Time::from_str("03:00:00+10:00")?)
        );
        Ok(())
    }

    #[test]
    fn time_from_datetime() -> Result<(), ParseDateTimeError> {
        assert_eq!(
            Time::from(DateTime::MIN),
            Time::from_str("19:51:08.312696284115894272-14:00")?
        );
        assert_eq!(
            Time::from(DateTime::MAX),
            Time::from_str("04:08:51.687303715884105727+14:00")?
        );
        Ok(())
    }

    #[test]
    fn date_from_datetime() -> Result<(), Box<dyn Error>> {
        assert_eq!(
            Date::try_from(
                DateTime::MIN
                    .checked_add_day_time_duration(DayTimeDuration::from_str("P1D")?)
                    .unwrap()
            )?,
            Date::MIN
        );
        assert_eq!(Date::try_from(DateTime::MAX)?, Date::MAX);
        Ok(())
    }

    #[test]
    fn g_year_month_from_date() {
        assert_eq!(GYearMonth::from(Date::MIN), GYearMonth::MIN);
        assert_eq!(GYearMonth::from(Date::MAX), GYearMonth::MAX);
    }

    #[test]
    fn g_year_from_g_year_month() -> Result<(), ParseDateTimeError> {
        assert_eq!(GYear::try_from(GYearMonth::MIN)?, GYear::MIN);
        assert_eq!(
            GYear::try_from(GYearMonth::from_str("5391559471918-12+14:00")?)?,
            GYear::MAX
        );
        Ok(())
    }

    #[cfg(feature = "custom-now")]
    #[test]
    fn custom_now() {
        #[expect(unsafe_code)]
        #[unsafe(no_mangle)]
        extern "Rust" fn custom_ox_now() -> Duration {
            Duration::default()
        }
        DateTime::now();
    }

    #[cfg(not(feature = "custom-now"))]
    #[test]
    fn now() -> Result<(), ParseDateTimeError> {
        let now = DateTime::now();
        assert!(DateTime::from_str("2022-01-01T00:00:00Z")? < now);
        assert!(now < DateTime::from_str("2100-01-01T00:00:00Z")?);
        Ok(())
    }

    #[test]
    fn minimally_conformant() -> Result<(), ParseDateTimeError> {
        // All minimally conforming processors must support nonnegative year values less than 10000
        // (i.e., those expressible with four digits) in all datatypes which
        // use the seven-property model defined in The Seven-property Model (§D.2.1)
        // and have a non-absent value for year (i.e. dateTime, dateTimeStamp, date, gYearMonth, and gYear).
        assert_eq!(GYear::from_str("9999")?.to_string(), "9999");
        assert_eq!(
            DateTime::from_str("9999-12-31T23:59:59Z")?.to_string(),
            "9999-12-31T23:59:59Z"
        );

        // All minimally conforming processors must support second values to milliseconds
        // (i.e. those expressible with three fraction digits) in all datatypes
        // which use the seven-property model defined in The Seven-property Model (§D.2.1)
        // and have a non-absent value for second (i.e. dateTime, dateTimeStamp, and time).
        assert_eq!(
            Time::from_str("00:00:00.678Z")?.to_string(),
            "00:00:00.678Z"
        );
        assert_eq!(
            DateTime::from_str("2000-01-01T00:00:00.678Z")?.to_string(),
            "2000-01-01T00:00:00.678Z"
        );
        Ok(())
    }
}
