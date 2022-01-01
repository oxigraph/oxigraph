use super::date_time::{DateTimeError, GDay, GMonth, GMonthDay, GYear, GYearMonth, TimezoneOffset};
use super::decimal::ParseDecimalError;
use super::duration::{DayTimeDuration, YearMonthDuration};
use super::*;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while, take_while_m_n};
use nom::character::complete::{char, digit0, digit1};
use nom::combinator::{map, opt, recognize};
use nom::error::{ErrorKind, ParseError};
use nom::multi::many1;
use nom::sequence::{preceded, terminated, tuple};
use nom::Err;
use nom::{IResult, Needed};
use std::error::Error;
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct XsdParseError {
    kind: XsdParseErrorKind,
}

#[derive(Debug, Clone)]
enum XsdParseErrorKind {
    NomKind(ErrorKind),
    NomChar(char),
    MissingData(Needed),
    TooMuchData { count: usize },
    Overflow,
    ParseInt(ParseIntError),
    ParseDecimal(ParseDecimalError),
    OutOfIntegerRange { value: u8, min: u8, max: u8 },
    DateTime(DateTimeError),
}

impl fmt::Display for XsdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            XsdParseErrorKind::NomKind(kind) => {
                write!(f, "Invalid XML Schema value: {}", kind.description())
            }
            XsdParseErrorKind::NomChar(c) => {
                write!(f, "Unexpected character in XML Schema value: '{}'", c)
            }
            XsdParseErrorKind::MissingData(Needed::Unknown) => {
                write!(f, "Too small XML Schema value")
            }
            XsdParseErrorKind::MissingData(Needed::Size(size)) => {
                write!(f, "Too small XML Schema value: missing {} chars", size)
            }
            XsdParseErrorKind::TooMuchData { count } => {
                write!(f, "Too long XML Schema value: {} extra chars", count)
            }
            XsdParseErrorKind::Overflow => write!(f, "Computation overflow or underflow"),
            XsdParseErrorKind::ParseInt(error) => {
                write!(f, "Error while parsing integer: {}", error)
            }
            XsdParseErrorKind::ParseDecimal(error) => {
                write!(f, "Error while parsing decimal: {}", error)
            }
            XsdParseErrorKind::OutOfIntegerRange { value, min, max } => write!(
                f,
                "The integer {} is not between {} and {}",
                value, min, max
            ),
            XsdParseErrorKind::DateTime(error) => error.fmt(f),
        }
    }
}

impl Error for XsdParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            XsdParseErrorKind::ParseInt(error) => Some(error),
            XsdParseErrorKind::ParseDecimal(error) => Some(error),
            XsdParseErrorKind::DateTime(error) => Some(error),
            _ => None,
        }
    }
}

impl ParseError<&str> for XsdParseError {
    fn from_error_kind(_input: &str, kind: ErrorKind) -> Self {
        Self {
            kind: XsdParseErrorKind::NomKind(kind),
        }
    }

    fn append(_input: &str, _kind: ErrorKind, other: Self) -> Self {
        other
    }

    fn from_char(_input: &str, c: char) -> Self {
        Self {
            kind: XsdParseErrorKind::NomChar(c),
        }
    }

    fn or(self, other: Self) -> Self {
        other
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

impl From<Err<Self>> for XsdParseError {
    fn from(err: Err<Self>) -> Self {
        match err {
            Err::Incomplete(needed) => Self {
                kind: XsdParseErrorKind::MissingData(needed),
            },
            Err::Error(e) | Err::Failure(e) => e,
        }
    }
}

type XsdResult<'a, T> = IResult<&'a str, T, XsdParseError>;

const OVERFLOW_ERROR: XsdParseError = XsdParseError {
    kind: XsdParseErrorKind::Overflow,
};

pub fn parse_value<'a, T>(
    mut f: impl FnMut(&'a str) -> XsdResult<'a, T>,
    input: &'a str,
) -> Result<T, XsdParseError> {
    let (left, result) = f(input)?;
    if left.is_empty() {
        Ok(result)
    } else {
        Err(XsdParseError {
            kind: XsdParseErrorKind::TooMuchData { count: left.len() },
        })
    }
}

//TODO: check every computation

// [6]   duYearFrag ::= unsignedNoDecimalPtNumeral 'Y'
fn du_year_frag(input: &str) -> XsdResult<'_, i64> {
    terminated(unsigned_no_decimal_pt_numeral, char('Y'))(input)
}

// [7]   duMonthFrag ::= unsignedNoDecimalPtNumeral 'M'
fn du_month_frag(input: &str) -> XsdResult<'_, i64> {
    terminated(unsigned_no_decimal_pt_numeral, char('M'))(input)
}

//  [8]   duDayFrag ::= unsignedNoDecimalPtNumeral 'D'
fn du_day_frag(input: &str) -> XsdResult<'_, i64> {
    terminated(unsigned_no_decimal_pt_numeral, char('D'))(input)
}

// [9]   duHourFrag ::= unsignedNoDecimalPtNumeral 'H'
fn du_hour_frag(input: &str) -> XsdResult<'_, i64> {
    terminated(unsigned_no_decimal_pt_numeral, char('H'))(input)
}

// [10]   duMinuteFrag ::= unsignedNoDecimalPtNumeral 'M'
fn du_minute_frag(input: &str) -> XsdResult<'_, i64> {
    terminated(unsigned_no_decimal_pt_numeral, char('M'))(input)
}

// [11]   duSecondFrag ::= (unsignedNoDecimalPtNumeral | unsignedDecimalPtNumeral) 'S'
fn du_second_frag(input: &str) -> XsdResult<'_, Decimal> {
    terminated(
        map_res(
            recognize(tuple((digit0, opt(preceded(char('.'), digit0))))),
            Decimal::from_str,
        ),
        char('S'),
    )(input)
}

// [12]   duYearMonthFrag ::= (duYearFrag duMonthFrag?) | duMonthFrag
fn du_year_month_frag(input: &str) -> XsdResult<'_, i64> {
    alt((
        map(tuple((du_year_frag, opt(du_month_frag))), |(y, m)| {
            12 * y + m.unwrap_or(0)
        }),
        du_month_frag,
    ))(input)
}

// [13]   duTimeFrag ::= 'T' ((duHourFrag duMinuteFrag? duSecondFrag?) | (duMinuteFrag duSecondFrag?) | duSecondFrag)
fn du_time_frag(input: &str) -> XsdResult<'_, Decimal> {
    preceded(
        char('T'),
        alt((
            map_res(
                tuple((du_hour_frag, opt(du_minute_frag), opt(du_second_frag))),
                |(h, m, s)| {
                    Decimal::from(3600 * h + 60 * m.unwrap_or(0))
                        .checked_add(s.unwrap_or_default())
                        .ok_or(OVERFLOW_ERROR)
                },
            ),
            map_res(tuple((du_minute_frag, opt(du_second_frag))), |(m, s)| {
                Decimal::from(m * 60)
                    .checked_add(s.unwrap_or_default())
                    .ok_or(OVERFLOW_ERROR)
            }),
            du_second_frag,
        )),
    )(input)
}

// [14]   duDayTimeFrag ::= (duDayFrag duTimeFrag?) | duTimeFrag
fn du_day_time_frag(input: &str) -> XsdResult<'_, Decimal> {
    alt((
        map_res(tuple((du_day_frag, opt(du_time_frag))), |(d, t)| {
            Decimal::from(d)
                .checked_mul(Decimal::from(86400))
                .ok_or(OVERFLOW_ERROR)?
                .checked_add(t.unwrap_or_default())
                .ok_or(OVERFLOW_ERROR)
        }),
        du_time_frag,
    ))(input)
}

// [15]   durationLexicalRep ::= '-'? 'P' ((duYearMonthFrag duDayTimeFrag?) | duDayTimeFrag)
pub fn duration_lexical_rep(input: &str) -> XsdResult<'_, Duration> {
    map(
        tuple((
            opt(char('-')),
            preceded(
                char('P'),
                alt((
                    map(
                        tuple((du_year_month_frag, opt(du_day_time_frag))),
                        |(y, d)| Duration::new(y, d.unwrap_or_default()),
                    ),
                    map(du_day_time_frag, |d| Duration::new(0, d)),
                )),
            ),
        )),
        |(sign, duration)| {
            if sign == Some('-') {
                -duration
            } else {
                duration
            }
        },
    )(input)
}

// [16]   dateTimeLexicalRep ::= yearFrag '-' monthFrag '-' dayFrag 'T' ((hourFrag ':' minuteFrag ':' secondFrag) | endOfDayFrag) timezoneFrag?
pub fn date_time_lexical_rep(input: &str) -> XsdResult<'_, DateTime> {
    map_res(
        tuple((
            year_frag,
            char('-'),
            month_frag,
            char('-'),
            day_frag,
            char('T'),
            alt((
                map(
                    tuple((hour_frag, char(':'), minute_frag, char(':'), second_frag)),
                    |(h, _, m, _, s)| (h, m, s),
                ),
                end_of_day_frag,
            )),
            opt(timezone_frag),
        )),
        |(year, _, month, _, day, _, (hours, minutes, seconds), timezone)| {
            DateTime::new(year, month, day, hours, minutes, seconds, timezone)
        },
    )(input)
}

// [17]   timeLexicalRep ::= ((hourFrag ':' minuteFrag ':' secondFrag) | endOfDayFrag) timezoneFrag?
pub fn time_lexical_rep(input: &str) -> XsdResult<'_, Time> {
    map_res(
        tuple((
            alt((
                map(
                    tuple((hour_frag, char(':'), minute_frag, char(':'), second_frag)),
                    |(h, _, m, _, s)| (h, m, s),
                ),
                end_of_day_frag,
            )),
            opt(timezone_frag),
        )),
        |((hours, minutes, seconds), timezone)| Time::new(hours, minutes, seconds, timezone),
    )(input)
}

// [18]   dateLexicalRep ::= yearFrag '-' monthFrag '-' dayFrag timezoneFrag?   Constraint:  Day-of-month Representations
pub fn date_lexical_rep(input: &str) -> XsdResult<'_, Date> {
    map_res(
        tuple((
            year_frag,
            char('-'),
            month_frag,
            char('-'),
            day_frag,
            opt(timezone_frag),
        )),
        |(year, _, month, _, day, timezone)| Date::new(year, month, day, timezone),
    )(input)
}

// [19]   gYearMonthLexicalRep ::= yearFrag '-' monthFrag timezoneFrag?
pub fn g_year_month_lexical_rep(input: &str) -> XsdResult<'_, GYearMonth> {
    map_res(
        tuple((year_frag, char('-'), month_frag, opt(timezone_frag))),
        |(year, _, month, timezone)| GYearMonth::new(year, month, timezone),
    )(input)
}

// [20]   gYearLexicalRep ::= yearFrag timezoneFrag?
pub fn g_year_lexical_rep(input: &str) -> XsdResult<'_, GYear> {
    map_res(
        tuple((year_frag, opt(timezone_frag))),
        |(year, timezone)| GYear::new(year, timezone),
    )(input)
}

// [21]   gMonthDayLexicalRep ::= '--' monthFrag '-' dayFrag timezoneFrag?   Constraint:  Day-of-month Representations
pub fn g_month_day_lexical_rep(input: &str) -> XsdResult<'_, GMonthDay> {
    map_res(
        tuple((
            char('-'),
            char('-'),
            month_frag,
            char('-'),
            day_frag,
            opt(timezone_frag),
        )),
        |(_, _, month, _, day, timezone)| GMonthDay::new(month, day, timezone),
    )(input)
}

// [22]   gDayLexicalRep ::= '---' dayFrag timezoneFrag?
pub fn g_day_lexical_rep(input: &str) -> XsdResult<'_, GDay> {
    map_res(
        tuple((
            char('-'),
            char('-'),
            char('-'),
            day_frag,
            opt(timezone_frag),
        )),
        |(_, _, _, day, timezone)| GDay::new(day, timezone),
    )(input)
}

// [23]   gMonthLexicalRep ::= '--' monthFrag timezoneFrag?
pub fn g_month_lexical_rep(input: &str) -> XsdResult<'_, GMonth> {
    map_res(
        tuple((char('-'), char('-'), month_frag, opt(timezone_frag))),
        |(_, _, month, timezone)| GMonth::new(month, timezone),
    )(input)
}

// [42]   yearMonthDurationLexicalRep ::= '-'? 'P' duYearMonthFrag
pub fn year_month_duration_lexical_rep(input: &str) -> XsdResult<'_, YearMonthDuration> {
    map(
        tuple((opt(char('-')), preceded(char('P'), du_year_month_frag))),
        |(sign, duration)| {
            YearMonthDuration::new(if sign == Some('-') {
                -duration
            } else {
                duration
            })
        },
    )(input)
}

// [43]   dayTimeDurationLexicalRep ::= '-'? 'P' duDayTimeFrag
pub fn day_time_duration_lexical_rep(input: &str) -> XsdResult<'_, DayTimeDuration> {
    map(
        tuple((opt(char('-')), preceded(char('P'), du_day_time_frag))),
        |(sign, duration)| {
            DayTimeDuration::new(if sign == Some('-') {
                -duration
            } else {
                duration
            })
        },
    )(input)
}

// [46]   unsignedNoDecimalPtNumeral ::= digit+
fn unsigned_no_decimal_pt_numeral(input: &str) -> XsdResult<'_, i64> {
    map_res(digit1, i64::from_str)(input)
}

// [56]   yearFrag ::= '-'? (([1-9] digit digit digit+)) | ('0' digit digit digit))
fn year_frag(input: &str) -> XsdResult<'_, i64> {
    map_res(
        recognize(tuple((
            opt(char('-')),
            take_while_m_n(4, usize::MAX, |c: char| c.is_ascii_digit()),
        ))),
        i64::from_str,
    )(input)
}

// [57]   monthFrag ::= ('0' [1-9]) | ('1' [0-2])
fn month_frag(input: &str) -> XsdResult<'_, u8> {
    map_res(take_while_m_n(2, 2, |c: char| c.is_ascii_digit()), |v| {
        parsed_u8_range(v, 1, 12)
    })(input)
}

// [58]   dayFrag ::= ('0' [1-9]) | ([12] digit) | ('3' [01])
fn day_frag(input: &str) -> XsdResult<'_, u8> {
    map_res(take_while_m_n(2, 2, |c: char| c.is_ascii_digit()), |v| {
        parsed_u8_range(v, 1, 31)
    })(input)
}

// [59]   hourFrag ::= ([01] digit) | ('2' [0-3])
fn hour_frag(input: &str) -> XsdResult<'_, u8> {
    map_res(take_while_m_n(2, 2, |c: char| c.is_ascii_digit()), |v| {
        parsed_u8_range(v, 0, 23)
    })(input)
}

// [60]   minuteFrag ::= [0-5] digit
fn minute_frag(input: &str) -> XsdResult<'_, u8> {
    map_res(take_while_m_n(2, 2, |c: char| c.is_ascii_digit()), |v| {
        parsed_u8_range(v, 0, 59)
    })(input)
}

// [61]   secondFrag ::= ([0-5] digit) ('.' digit+)?
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn second_frag(input: &str) -> XsdResult<'_, Decimal> {
    map_res(
        recognize(tuple((
            take_while_m_n(2, 2, |c: char| c.is_ascii_digit()),
            opt(preceded(
                char('.'),
                take_while(|c: char| c.is_ascii_digit()),
            )),
        ))),
        |v| {
            let value = Decimal::from_str(v)?;
            if Decimal::from(0) <= value && value < Decimal::from(60) {
                Ok(value)
            } else {
                Err(XsdParseError {
                    kind: XsdParseErrorKind::OutOfIntegerRange {
                        value: value.as_i128() as u8,
                        min: 0,
                        max: 60,
                    },
                })
            }
        },
    )(input)
}

// [62]   endOfDayFrag ::= '24:00:00' ('.' '0'+)?
fn end_of_day_frag(input: &str) -> XsdResult<'_, (u8, u8, Decimal)> {
    map(
        recognize(tuple((
            tag("24:00:00"),
            opt(preceded(char('.'), many1(char('0')))),
        ))),
        |_| (24, 0, 0.into()),
    )(input)
}

// [63]   timezoneFrag ::= 'Z' | ('+' | '-') (('0' digit | '1' [0-3]) ':' minuteFrag | '14:00')
fn timezone_frag(input: &str) -> XsdResult<'_, TimezoneOffset> {
    alt((
        map(char('Z'), |_| TimezoneOffset::utc()),
        map(
            tuple((
                alt((map(char('+'), |_| 1), map(char('-'), |_| -1))),
                alt((
                    map(
                        tuple((
                            map_res(take_while_m_n(2, 2, |c: char| c.is_ascii_digit()), |v| {
                                parsed_u8_range(v, 0, 13)
                            }),
                            char(':'),
                            minute_frag,
                        )),
                        |(hours, _, minutes)| i16::from(hours) * 60 + i16::from(minutes),
                    ),
                    map(tag("14:00"), |_| 14 * 60),
                )),
            )),
            |(sign, value)| TimezoneOffset::new(sign * value),
        ),
    ))(input)
}

fn parsed_u8_range(input: &str, min: u8, max: u8) -> Result<u8, XsdParseError> {
    let value = u8::from_str(input)?;
    if min <= value && value <= max {
        Ok(value)
    } else {
        Err(XsdParseError {
            kind: XsdParseErrorKind::OutOfIntegerRange { value, min, max },
        })
    }
}

fn map_res<'a, O1, O2, E2: Into<XsdParseError>>(
    mut first: impl FnMut(&'a str) -> XsdResult<'a, O1>,
    mut second: impl FnMut(O1) -> Result<O2, E2>,
) -> impl FnMut(&'a str) -> XsdResult<'a, O2> {
    move |input| {
        let (input, o1) = first(input)?;
        Ok((input, second(o1).map_err(|e| Err::Error(e.into()))?))
    }
}
