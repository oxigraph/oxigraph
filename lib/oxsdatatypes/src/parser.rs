use super::date_time::{DateTimeError, GDay, GMonth, GMonthDay, GYear, GYearMonth, TimezoneOffset};
use super::decimal::ParseDecimalError;
use super::duration::{DayTimeDuration, YearMonthDuration};
use super::*;
use std::error::Error;
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

/// A parsing error
#[derive(Debug, Clone)]
pub struct XsdParseError {
    kind: XsdParseErrorKind,
}

#[derive(Debug, Clone)]
enum XsdParseErrorKind {
    ParseInt(ParseIntError),
    ParseDecimal(ParseDecimalError),
    DateTime(DateTimeError),
    Message(&'static str),
}

const OVERFLOW_ERROR: XsdParseError = XsdParseError {
    kind: XsdParseErrorKind::Message("Overflow error"),
};

impl fmt::Display for XsdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            XsdParseErrorKind::ParseInt(error) => {
                write!(f, "Error while parsing integer: {error}")
            }
            XsdParseErrorKind::ParseDecimal(error) => {
                write!(f, "Error while parsing decimal: {error}")
            }
            XsdParseErrorKind::DateTime(error) => error.fmt(f),
            XsdParseErrorKind::Message(msg) => write!(f, "{msg}"),
        }
    }
}

impl XsdParseError {
    const fn msg(message: &'static str) -> Self {
        Self {
            kind: XsdParseErrorKind::Message(message),
        }
    }
}

impl Error for XsdParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            XsdParseErrorKind::ParseInt(error) => Some(error),
            XsdParseErrorKind::ParseDecimal(error) => Some(error),
            XsdParseErrorKind::DateTime(error) => Some(error),
            XsdParseErrorKind::Message(_) => None,
        }
    }
}

impl From<ParseIntError> for XsdParseError {
    fn from(error: ParseIntError) -> Self {
        Self {
            kind: XsdParseErrorKind::ParseInt(error),
        }
    }
}

impl From<ParseDecimalError> for XsdParseError {
    fn from(error: ParseDecimalError) -> Self {
        Self {
            kind: XsdParseErrorKind::ParseDecimal(error),
        }
    }
}

impl From<DateTimeError> for XsdParseError {
    fn from(error: DateTimeError) -> Self {
        Self {
            kind: XsdParseErrorKind::DateTime(error),
        }
    }
}

// [6]   duYearFrag ::= unsignedNoDecimalPtNumeral 'Y'
// [7]   duMonthFrag ::= unsignedNoDecimalPtNumeral 'M'
// [8]   duDayFrag ::= unsignedNoDecimalPtNumeral 'D'
// [9]   duHourFrag ::= unsignedNoDecimalPtNumeral 'H'
// [10]   duMinuteFrag ::= unsignedNoDecimalPtNumeral 'M'
// [11]   duSecondFrag ::= (unsignedNoDecimalPtNumeral | unsignedDecimalPtNumeral) 'S'
// [12]   duYearMonthFrag ::= (duYearFrag duMonthFrag?) | duMonthFrag
// [13]   duTimeFrag ::= 'T' ((duHourFrag duMinuteFrag? duSecondFrag?) | (duMinuteFrag duSecondFrag?) | duSecondFrag)
// [14]   duDayTimeFrag ::= (duDayFrag duTimeFrag?) | duTimeFrag
// [15]   durationLexicalRep ::= '-'? 'P' ((duYearMonthFrag duDayTimeFrag?) | duDayTimeFrag)
struct DurationParts {
    year_month: Option<i64>,
    day_time: Option<Decimal>,
}

fn duration_parts(input: &str) -> Result<(DurationParts, &str), XsdParseError> {
    // States
    const START: u32 = 0;
    const AFTER_YEAR: u32 = 1;
    const AFTER_MONTH: u32 = 2;
    const AFTER_DAY: u32 = 3;
    const AFTER_T: u32 = 4;
    const AFTER_HOUR: u32 = 5;
    const AFTER_MINUTE: u32 = 6;
    const AFTER_SECOND: u32 = 7;

    let (is_negative, input) = if let Some(left) = input.strip_prefix('-') {
        (true, left)
    } else {
        (false, input)
    };
    let mut input = expect_char(input, 'P', "Durations must start with 'P'")?;
    let mut state = START;
    let mut year_month: Option<i64> = None;
    let mut day_time: Option<Decimal> = None;
    while !input.is_empty() {
        if let Some(left) = input.strip_prefix('T') {
            if state >= AFTER_T {
                return Err(XsdParseError::msg("Duplicated time separator 'T'"));
            }
            state = AFTER_T;
            input = left;
        } else {
            let (number_str, left) = decimal_prefix(input);
            match left.chars().next() {
                Some('Y') if state < AFTER_YEAR => {
                    year_month = Some(
                        year_month
                            .unwrap_or_default()
                            .checked_add(
                                apply_i64_neg(i64::from_str(number_str)?, is_negative)?
                                    .checked_mul(12)
                                    .ok_or(OVERFLOW_ERROR)?,
                            )
                            .ok_or(OVERFLOW_ERROR)?,
                    );
                    state = AFTER_YEAR;
                }
                Some('M') if state < AFTER_MONTH => {
                    year_month = Some(
                        year_month
                            .unwrap_or_default()
                            .checked_add(apply_i64_neg(i64::from_str(number_str)?, is_negative)?)
                            .ok_or(OVERFLOW_ERROR)?,
                    );
                    state = AFTER_MONTH;
                }
                Some('D') if state < AFTER_DAY => {
                    if number_str.contains('.') {
                        return Err(XsdParseError::msg(
                            "Decimal numbers are not allowed for days",
                        ));
                    }
                    day_time = Some(
                        day_time
                            .unwrap_or_default()
                            .checked_add(
                                apply_decimal_neg(Decimal::from_str(number_str)?, is_negative)?
                                    .checked_mul(86400)
                                    .ok_or(OVERFLOW_ERROR)?,
                            )
                            .ok_or(OVERFLOW_ERROR)?,
                    );
                    state = AFTER_DAY;
                }
                Some('H') if state == AFTER_T => {
                    if number_str.contains('.') {
                        return Err(XsdParseError::msg(
                            "Decimal numbers are not allowed for hours",
                        ));
                    }
                    day_time = Some(
                        day_time
                            .unwrap_or_default()
                            .checked_add(
                                apply_decimal_neg(Decimal::from_str(number_str)?, is_negative)?
                                    .checked_mul(3600)
                                    .ok_or(OVERFLOW_ERROR)?,
                            )
                            .ok_or(OVERFLOW_ERROR)?,
                    );
                    state = AFTER_HOUR;
                }
                Some('M') if (AFTER_T..AFTER_MINUTE).contains(&state) => {
                    if number_str.contains('.') {
                        return Err(XsdParseError::msg(
                            "Decimal numbers are not allowed for minutes",
                        ));
                    }
                    day_time = Some(
                        day_time
                            .unwrap_or_default()
                            .checked_add(
                                apply_decimal_neg(Decimal::from_str(number_str)?, is_negative)?
                                    .checked_mul(60)
                                    .ok_or(OVERFLOW_ERROR)?,
                            )
                            .ok_or(OVERFLOW_ERROR)?,
                    );
                    state = AFTER_MINUTE;
                }
                Some('S') if (AFTER_T..AFTER_SECOND).contains(&state) => {
                    day_time = Some(
                        day_time
                            .unwrap_or_default()
                            .checked_add(apply_decimal_neg(
                                Decimal::from_str(number_str)?,
                                is_negative,
                            )?)
                            .ok_or(OVERFLOW_ERROR)?,
                    );
                    state = AFTER_SECOND;
                }
                Some(_) => return Err(XsdParseError::msg("Unexpected type character")),
                None => {
                    return Err(XsdParseError::msg(
                        "Numbers in durations must be followed by a type character",
                    ))
                }
            }
            input = &left[1..];
        }
    }

    Ok((
        DurationParts {
            year_month,
            day_time,
        },
        input,
    ))
}

fn apply_i64_neg(value: i64, is_negative: bool) -> Result<i64, XsdParseError> {
    if is_negative {
        value.checked_neg().ok_or(OVERFLOW_ERROR)
    } else {
        Ok(value)
    }
}

fn apply_decimal_neg(value: Decimal, is_negative: bool) -> Result<Decimal, XsdParseError> {
    if is_negative {
        value.checked_neg().ok_or(OVERFLOW_ERROR)
    } else {
        Ok(value)
    }
}

pub fn parse_duration(input: &str) -> Result<Duration, XsdParseError> {
    let parts = ensure_complete(input, duration_parts)?;
    if parts.year_month.is_none() && parts.day_time.is_none() {
        return Err(XsdParseError::msg("Empty duration"));
    }
    Ok(Duration::new(
        parts.year_month.unwrap_or(0),
        parts.day_time.unwrap_or_default(),
    ))
}

pub fn parse_year_month_duration(input: &str) -> Result<YearMonthDuration, XsdParseError> {
    let parts = ensure_complete(input, duration_parts)?;
    if parts.day_time.is_some() {
        return Err(XsdParseError::msg(
            "There must not be any day or time component in a yearMonthDuration",
        ));
    }
    Ok(YearMonthDuration::new(parts.year_month.ok_or(
        XsdParseError::msg("No year and month values found"),
    )?))
}

pub fn parse_day_time_duration(input: &str) -> Result<DayTimeDuration, XsdParseError> {
    let parts = ensure_complete(input, duration_parts)?;
    if parts.year_month.is_some() {
        return Err(XsdParseError::msg(
            "There must not be any year or month component in a dayTimeDuration",
        ));
    }
    Ok(DayTimeDuration::new(parts.day_time.ok_or(
        XsdParseError::msg("No day or time values found"),
    )?))
}

// [16]   dateTimeLexicalRep ::= yearFrag '-' monthFrag '-' dayFrag 'T' ((hourFrag ':' minuteFrag ':' secondFrag) | endOfDayFrag) timezoneFrag?
fn date_time_lexical_rep(input: &str) -> Result<(DateTime, &str), XsdParseError> {
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
        return Err(XsdParseError::msg(
            "Times are not allowed to be after 24:00:00",
        ));
    }
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((
        DateTime::new(year, month, day, hour, minute, second, timezone_offset)?,
        input,
    ))
}

pub fn parse_date_time(input: &str) -> Result<DateTime, XsdParseError> {
    ensure_complete(input, date_time_lexical_rep)
}

// [17]   timeLexicalRep ::= ((hourFrag ':' minuteFrag ':' secondFrag) | endOfDayFrag) timezoneFrag?
fn time_lexical_rep(input: &str) -> Result<(Time, &str), XsdParseError> {
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
        return Err(XsdParseError::msg(
            "Times are not allowed to be after 24:00:00",
        ));
    }
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((Time::new(hour, minute, second, timezone_offset)?, input))
}

pub fn parse_time(input: &str) -> Result<Time, XsdParseError> {
    ensure_complete(input, time_lexical_rep)
}

// [18]   dateLexicalRep ::= yearFrag '-' monthFrag '-' dayFrag timezoneFrag?   Constraint:  Day-of-month Representations
fn date_lexical_rep(input: &str) -> Result<(Date, &str), XsdParseError> {
    let (year, input) = year_frag(input)?;
    let input = expect_char(input, '-', "The year and month must be separated by '-'")?;
    let (month, input) = month_frag(input)?;
    let input = expect_char(input, '-', "The month and day must be separated by '-'")?;
    let (day, input) = day_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((Date::new(year, month, day, timezone_offset)?, input))
}

pub fn parse_date(input: &str) -> Result<Date, XsdParseError> {
    ensure_complete(input, date_lexical_rep)
}

// [19]   gYearMonthLexicalRep ::= yearFrag '-' monthFrag timezoneFrag?
fn g_year_month_lexical_rep(input: &str) -> Result<(GYearMonth, &str), XsdParseError> {
    let (year, input) = year_frag(input)?;
    let input = expect_char(input, '-', "The year and month must be separated by '-'")?;
    let (month, input) = month_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((GYearMonth::new(year, month, timezone_offset)?, input))
}

pub fn parse_g_year_month(input: &str) -> Result<GYearMonth, XsdParseError> {
    ensure_complete(input, g_year_month_lexical_rep)
}

// [20]   gYearLexicalRep ::= yearFrag timezoneFrag?
fn g_year_lexical_rep(input: &str) -> Result<(GYear, &str), XsdParseError> {
    let (year, input) = year_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((GYear::new(year, timezone_offset)?, input))
}

pub fn parse_g_year(input: &str) -> Result<GYear, XsdParseError> {
    ensure_complete(input, g_year_lexical_rep)
}

// [21]   gMonthDayLexicalRep ::= '--' monthFrag '-' dayFrag timezoneFrag?   Constraint:  Day-of-month Representations
fn g_month_day_lexical_rep(input: &str) -> Result<(GMonthDay, &str), XsdParseError> {
    let input = expect_char(input, '-', "gMonthDay values must start with '--'")?;
    let input = expect_char(input, '-', "gMonthDay values must start with '--'")?;
    let (month, input) = month_frag(input)?;
    let input = expect_char(input, '-', "The month and day must be separated by '-'")?;
    let (day, input) = day_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((GMonthDay::new(month, day, timezone_offset)?, input))
}

pub fn parse_g_month_day(input: &str) -> Result<GMonthDay, XsdParseError> {
    ensure_complete(input, g_month_day_lexical_rep)
}

// [22]   gDayLexicalRep ::= '---' dayFrag timezoneFrag?
fn g_day_lexical_rep(input: &str) -> Result<(GDay, &str), XsdParseError> {
    let input = expect_char(input, '-', "gDay values must start with '---'")?;
    let input = expect_char(input, '-', "gDay values must start with '---'")?;
    let input = expect_char(input, '-', "gDay values must start with '---'")?;
    let (day, input) = day_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((GDay::new(day, timezone_offset)?, input))
}

pub fn parse_g_day(input: &str) -> Result<GDay, XsdParseError> {
    ensure_complete(input, g_day_lexical_rep)
}

// [23]   gMonthLexicalRep ::= '--' monthFrag timezoneFrag?
fn g_month_lexical_rep(input: &str) -> Result<(GMonth, &str), XsdParseError> {
    let input = expect_char(input, '-', "gMonth values must start with '--'")?;
    let input = expect_char(input, '-', "gMonth values must start with '--'")?;
    let (month, input) = month_frag(input)?;
    let (timezone_offset, input) = optional_end(input, timezone_frag)?;
    Ok((GMonth::new(month, timezone_offset)?, input))
}

pub fn parse_g_month(input: &str) -> Result<GMonth, XsdParseError> {
    ensure_complete(input, g_month_lexical_rep)
}

// [56]   yearFrag ::= '-'? (([1-9] digit digit digit+)) | ('0' digit digit digit))
fn year_frag(input: &str) -> Result<(i64, &str), XsdParseError> {
    let (sign, input) = if let Some(left) = input.strip_prefix('-') {
        (-1, left)
    } else {
        (1, input)
    };
    let (number_str, input) = integer_prefix(input);
    if number_str.len() < 4 {
        return Err(XsdParseError::msg("The year should be encoded on 4 digits"));
    }
    if number_str.len() > 4 && number_str.starts_with('0') {
        return Err(XsdParseError::msg(
            "The years value must not start with 0 if it can be encoded in at least 4 digits",
        ));
    }
    let number = i64::from_str(number_str)?;
    Ok((sign * number, input))
}

// [57]   monthFrag ::= ('0' [1-9]) | ('1' [0-2])
fn month_frag(input: &str) -> Result<(u8, &str), XsdParseError> {
    let (number_str, input) = integer_prefix(input);
    if number_str.len() != 2 {
        return Err(XsdParseError::msg("Month must be encoded with two digits"));
    }
    let number = u8::from_str(number_str)?;
    if !(1..=12).contains(&number) {
        return Err(XsdParseError::msg("Month must be between 01 and 12"));
    }
    Ok((number, input))
}

// [58]   dayFrag ::= ('0' [1-9]) | ([12] digit) | ('3' [01])
fn day_frag(input: &str) -> Result<(u8, &str), XsdParseError> {
    let (number_str, input) = integer_prefix(input);
    if number_str.len() != 2 {
        return Err(XsdParseError::msg("Day must be encoded with two digits"));
    }
    let number = u8::from_str(number_str)?;
    if !(1..=31).contains(&number) {
        return Err(XsdParseError::msg("Day must be between 01 and 31"));
    }
    Ok((number, input))
}

// [59]   hourFrag ::= ([01] digit) | ('2' [0-3])
// We also allow 24 for ease of parsing
fn hour_frag(input: &str) -> Result<(u8, &str), XsdParseError> {
    let (number_str, input) = integer_prefix(input);
    if number_str.len() != 2 {
        return Err(XsdParseError::msg("Hours must be encoded with two digits"));
    }
    let number = u8::from_str(number_str)?;
    if !(0..=24).contains(&number) {
        return Err(XsdParseError::msg("Hours must be between 00 and 24"));
    }
    Ok((number, input))
}

// [60]   minuteFrag ::= [0-5] digit
fn minute_frag(input: &str) -> Result<(u8, &str), XsdParseError> {
    let (number_str, input) = integer_prefix(input);
    if number_str.len() != 2 {
        return Err(XsdParseError::msg(
            "Minutes must be encoded with two digits",
        ));
    }
    let number = u8::from_str(number_str)?;
    if !(0..=59).contains(&number) {
        return Err(XsdParseError::msg("Minutes must be between 00 and 59"));
    }
    Ok((number, input))
}

// [61]   secondFrag ::= ([0-5] digit) ('.' digit+)?
fn second_frag(input: &str) -> Result<(Decimal, &str), XsdParseError> {
    let (number_str, input) = decimal_prefix(input);
    let (before_dot_str, _) = number_str.split_once('.').unwrap_or((number_str, ""));
    if before_dot_str.len() != 2 {
        return Err(XsdParseError::msg(
            "Seconds must be encoded with two digits",
        ));
    }
    let number = Decimal::from_str(number_str)?;
    if number < Decimal::from(0) || number >= Decimal::from(60) {
        return Err(XsdParseError::msg("Seconds must be between 00 and 60"));
    }
    if number_str.ends_with('.') {
        return Err(XsdParseError::msg(
            "Seconds are not allowed to end with a dot",
        ));
    }
    Ok((number, input))
}

// [63]   timezoneFrag ::= 'Z' | ('+' | '-') (('0' digit | '1' [0-3]) ':' minuteFrag | '14:00')
fn timezone_frag(input: &str) -> Result<(TimezoneOffset, &str), XsdParseError> {
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
        return Err(XsdParseError::msg(
            "The timezone hours must be encoded with two digits",
        ));
    }
    let hours = i16::from_str(hour_str)?;

    let input = expect_char(
        input,
        ':',
        "The timezone hours and minutes must be separated by ':'",
    )?;
    let (minutes, input) = minute_frag(input)?;

    if hours > 13 && !(hours == 14 && minutes == 0) {
        return Err(XsdParseError::msg(
            "The timezone hours must be between 00 and 13",
        ));
    }

    Ok((
        TimezoneOffset::new(sign * (hours * 60 + i16::from(minutes)))?,
        input,
    ))
}

fn ensure_complete<T>(
    input: &str,
    parse: impl FnOnce(&str) -> Result<(T, &str), XsdParseError>,
) -> Result<T, XsdParseError> {
    let (result, left) = parse(input)?;
    if !left.is_empty() {
        return Err(XsdParseError::msg("Unrecognized value suffix"));
    }
    Ok(result)
}

fn expect_char<'a>(
    input: &'a str,
    constant: char,
    error_message: &'static str,
) -> Result<&'a str, XsdParseError> {
    if let Some(left) = input.strip_prefix(constant) {
        Ok(left)
    } else {
        Err(XsdParseError::msg(error_message))
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
    parse: impl FnOnce(&str) -> Result<(T, &str), XsdParseError>,
) -> Result<(Option<T>, &str), XsdParseError> {
    Ok(if input.is_empty() {
        (None, input)
    } else {
        let (result, input) = parse(input)?;
        (Some(result), input)
    })
}
