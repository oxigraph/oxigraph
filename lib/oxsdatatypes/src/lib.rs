#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod boolean;
mod date_time;
mod decimal;
mod double;
mod duration;
mod float;
mod integer;

pub use self::boolean::Boolean;
pub use self::date_time::{
    Date, DateTime, DateTimeOverflowError, GDay, GMonth, GMonthDay, GYear, GYearMonth,
    InvalidTimezoneError, ParseDateTimeError, Time, TimezoneOffset,
};
pub use self::decimal::{Decimal, ParseDecimalError, TooLargeForDecimalError};
pub use self::double::Double;
pub use self::duration::{
    DayTimeDuration, Duration, DurationOverflowError, OppositeSignInDurationComponentsError,
    ParseDurationError, YearMonthDuration,
};
pub use self::float::Float;
pub use self::integer::{Integer, TooLargeForIntegerError};
