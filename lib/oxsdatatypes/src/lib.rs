#![doc = include_str!("../README.md")]
#![deny(unsafe_code)]
#![doc(test(attr(deny(warnings))))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod boolean;
mod date_time;
mod decimal;
mod double;
mod duration;
mod float;
mod integer;
mod parser;

pub use self::boolean::Boolean;
pub use self::date_time::{
    Date, DateTime, DateTimeError, GDay, GMonth, GMonthDay, GYear, GYearMonth, Time, TimezoneOffset,
};
pub use self::decimal::{Decimal, DecimalOverflowError, ParseDecimalError};
pub use self::double::Double;
pub use self::duration::{DayTimeDuration, Duration, YearMonthDuration};
pub use self::float::Float;
pub use self::integer::Integer;
pub use self::parser::XsdParseError;
