use std::str::FromStr;
use wkt::Wkt;
// use std::time::Geo as StdDuration;

/// [XML Schema `duration` datatype](https://www.w3.org/TR/xmlschema11-2/#duration)
///
/// It stores the duration using a pair of a [`YearMonthDuration`] and a [`DayTimeDuration`].
#[derive(Debug, Clone)]
pub struct GeoPoint {
    geom: wkt::Wkt<f64>,
}

type WktError = &'static str;

impl GeoPoint {
    #[inline]
    pub fn new() -> Result<Self, WktError> {
        Self::from_str("POINT(0 0)")
    }
}

impl FromStr for GeoPoint {
    type Err = WktError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            geom: Wkt::from_str(input)?,
        })
    }
}
//
// impl fmt::Display for GeoPoint {
//     #[allow(clippy::many_single_char_names)]
//     #[inline]
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         let ym = self.year_month.months;
//         let ss = self.day_time.seconds;
//
//         if (ym < 0 && ss > 0.into()) || (ym > 0 && ss < 0.into()) {
//             return Err(fmt::Error); // Not able to format with only a part of the duration that is negative
//         }
//         if ym < 0 || ss < 0.into() {
//             write!(f, "-")?;
//         }
//         write!(f, "P")?;
//
//         if ym == 0 && ss == 0.into() {
//             return write!(f, "T0S");
//         }
//
//         {
//             let y = ym / 12;
//             let m = ym % 12;
//
//             if y != 0 {
//                 if m == 0 {
//                     write!(f, "{}Y", y.abs())?;
//                 } else {
//                     write!(f, "{}Y{}M", y.abs(), m.abs())?;
//                 }
//             } else if m != 0 || ss == 0.into() {
//                 write!(f, "{}M", m.abs())?;
//             }
//         }
//
//         {
//             let s_int = ss.as_i128();
//             let d = s_int / 86400;
//             let h = (s_int % 86400) / 3600;
//             let m = (s_int % 3600) / 60;
//             let s = ss
//                 .checked_sub(
//                     Decimal::try_from(d * 86400 + h * 3600 + m * 60).map_err(|_| fmt::Error)?,
//                 )
//                 .ok_or(fmt::Error)?;
//
//             if d != 0 {
//                 write!(f, "{}D", d.abs())?;
//             }
//
//             if h != 0 || m != 0 || s != 0.into() {
//                 write!(f, "T")?;
//                 if h != 0 {
//                     write!(f, "{}H", h.abs())?;
//                 }
//                 if m != 0 {
//                     write!(f, "{}M", m.abs())?;
//                 }
//                 if s != 0.into() {
//                     write!(f, "{}S", s.checked_abs().ok_or(fmt::Error)?)?;
//                 }
//             }
//         }
//         Ok(())
//     }
// }
//
// impl PartialOrd for GeoPoint {
//     #[inline]
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         let first = DateTime::new(1969, 9, 1, 0, 0, 0.into(), None).ok()?;
//         let first_result = first
//             .checked_add_duration(*self)?
//             .partial_cmp(&first.checked_add_duration(*other)?);
//         let second = DateTime::new(1697, 2, 1, 0, 0, 0.into(), None).ok()?;
//         let second_result = second
//             .checked_add_duration(*self)?
//             .partial_cmp(&second.checked_add_duration(*other)?);
//         let third = DateTime::new(1903, 3, 1, 0, 0, 0.into(), None).ok()?;
//         let third_result = third
//             .checked_add_duration(*self)?
//             .partial_cmp(&third.checked_add_duration(*other)?);
//         let fourth = DateTime::new(1903, 7, 1, 0, 0, 0.into(), None).ok()?;
//         let fourth_result = fourth
//             .checked_add_duration(*self)?
//             .partial_cmp(&fourth.checked_add_duration(*other)?);
//         if first_result == second_result
//             && second_result == third_result
//             && third_result == fourth_result
//         {
//             first_result
//         } else {
//             None
//         }
//     }
// }
//
// /// [XML Schema `yearMonthDuration` datatype](https://www.w3.org/TR/xmlschema11-2/#yearMonthDuration)
// ///
// /// It stores the duration as a number of months encoded using a [`i64`].
// #[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash, Default)]
// pub struct YearMonthDuration {
//     months: i64,
// }
//
// impl YearMonthDuration {
//     #[inline]
//     pub fn new(months: impl Into<i64>) -> Self {
//         Self {
//             months: months.into(),
//         }
//     }
//
//     #[inline]
//     pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
//         Self {
//             months: i64::from_be_bytes(bytes),
//         }
//     }
//
//     /// [fn:years-from-duration](https://www.w3.org/TR/xpath-functions-31/#func-years-from-duration)
//     #[inline]
//     pub fn years(self) -> i64 {
//         self.months / 12
//     }
//
//     /// [fn:months-from-duration](https://www.w3.org/TR/xpath-functions-31/#func-months-from-duration)
//     #[inline]
//     pub fn months(self) -> i64 {
//         self.months % 12
//     }
//
//     #[inline]
//     pub(crate) const fn all_months(self) -> i64 {
//         self.months
//     }
//
//     #[inline]
//     pub fn to_be_bytes(self) -> [u8; 8] {
//         self.months.to_be_bytes()
//     }
//
//     /// [op:add-yearMonthDurations](https://www.w3.org/TR/xpath-functions-31/#func-add-yearMonthDurations)
//     ///
//     /// Returns `None` in case of overflow ([`FODT0002`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0002)).
//     #[inline]
//     pub fn checked_add(self, rhs: impl Into<Self>) -> Option<Self> {
//         let rhs = rhs.into();
//         Some(Self {
//             months: self.months.checked_add(rhs.months)?,
//         })
//     }
//
//     /// [op:subtract-yearMonthDurations](https://www.w3.org/TR/xpath-functions-31/#func-subtract-yearMonthDurations)
//     ///
//     /// Returns `None` in case of overflow ([`FODT0002`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0002)).
//     #[inline]
//     pub fn checked_sub(self, rhs: impl Into<Self>) -> Option<Self> {
//         let rhs = rhs.into();
//         Some(Self {
//             months: self.months.checked_sub(rhs.months)?,
//         })
//     }
//
//     /// Unary negation.
//     ///
//     /// Returns `None` in case of overflow ([`FODT0002`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0002)).
//     #[inline]
//     pub fn checked_neg(self) -> Option<Self> {
//         Some(Self {
//             months: self.months.checked_neg()?,
//         })
//     }
//
//     /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
//     #[inline]
//     pub fn is_identical_with(self, other: Self) -> bool {
//         self == other
//     }
//
//     pub const MIN: Self = Self { months: i64::MIN };
//
//     pub const MAX: Self = Self { months: i64::MAX };
// }
//
// impl From<YearMonthDuration> for GeoPoint {
//     #[inline]
//     fn from(value: YearMonthDuration) -> Self {
//         Self {
//             year_month: value,
//             day_time: DayTimeDuration::default(),
//         }
//     }
// }
//
// impl TryFrom<GeoPoint> for YearMonthDuration {
//     type Error = DurationOverflowError;
//
//     #[inline]
//     fn try_from(value: GeoPoint) -> Result<Self, DurationOverflowError> {
//         if value.day_time == DayTimeDuration::default() {
//             Ok(value.year_month)
//         } else {
//             Err(DurationOverflowError)
//         }
//     }
// }
//
// impl FromStr for YearMonthDuration {
//     type Err = ParseDurationError;
//
//     fn from_str(input: &str) -> Result<Self, ParseDurationError> {
//         let parts = ensure_complete(input, duration_parts)?;
//         if parts.day_time.is_some() {
//             return Err(ParseDurationError::msg(
//                 "There must not be any day or time component in a yearMonthDuration",
//             ));
//         }
//         Ok(Self::new(parts.year_month.ok_or(
//             ParseDurationError::msg("No year and month values found"),
//         )?))
//     }
// }
//
// impl fmt::Display for YearMonthDuration {
//     #[inline]
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         if self.months == 0 {
//             write!(f, "P0M")
//         } else {
//             GeoPoint::from(*self).fmt(f)
//         }
//     }
// }
//
// impl PartialEq<GeoPoint> for YearMonthDuration {
//     #[inline]
//     fn eq(&self, other: &GeoPoint) -> bool {
//         GeoPoint::from(*self).eq(other)
//     }
// }
//
// impl PartialEq<YearMonthDuration> for GeoPoint {
//     #[inline]
//     fn eq(&self, other: &YearMonthDuration) -> bool {
//         self.eq(&Self::from(*other))
//     }
// }
//
// impl PartialOrd<GeoPoint> for YearMonthDuration {
//     #[inline]
//     fn partial_cmp(&self, other: &GeoPoint) -> Option<Ordering> {
//         GeoPoint::from(*self).partial_cmp(other)
//     }
// }
//
// impl PartialOrd<YearMonthDuration> for GeoPoint {
//     #[inline]
//     fn partial_cmp(&self, other: &YearMonthDuration) -> Option<Ordering> {
//         self.partial_cmp(&Self::from(*other))
//     }
// }
//
// /// [XML Schema `dayTimeDuration` datatype](https://www.w3.org/TR/xmlschema11-2/#dayTimeDuration)
// ///
// /// It stores the duration as a number of seconds encoded using a [`Decimal`].
// #[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash, Default)]
// pub struct DayTimeDuration {
//     seconds: Decimal,
// }
//
// impl DayTimeDuration {
//     #[inline]
//     pub fn new(seconds: impl Into<Decimal>) -> Self {
//         Self {
//             seconds: seconds.into(),
//         }
//     }
//
//     #[inline]
//     pub fn from_be_bytes(bytes: [u8; 16]) -> Self {
//         Self {
//             seconds: Decimal::from_be_bytes(bytes),
//         }
//     }
//
//     /// [fn:days-from-duration](https://www.w3.org/TR/xpath-functions-31/#func-days-from-duration)
//     #[allow(clippy::cast_possible_truncation)]
//     #[inline]
//     pub fn days(self) -> i64 {
//         (self.seconds.as_i128() / 86400) as i64
//     }
//
//     /// [fn:hours-from-duration](https://www.w3.org/TR/xpath-functions-31/#func-hours-from-duration)
//     #[allow(clippy::cast_possible_truncation)]
//     #[inline]
//     pub fn hours(self) -> i64 {
//         ((self.seconds.as_i128() % 86400) / 3600) as i64
//     }
//
//     /// [fn:minutes-from-duration](https://www.w3.org/TR/xpath-functions-31/#func-minutes-from-duration)
//     #[allow(clippy::cast_possible_truncation)]
//     #[inline]
//     pub fn minutes(self) -> i64 {
//         ((self.seconds.as_i128() % 3600) / 60) as i64
//     }
//
//     /// [fn:seconds-from-duration](https://www.w3.org/TR/xpath-functions-31/#func-seconds-from-duration)
//     #[inline]
//     pub fn seconds(self) -> Decimal {
//         self.seconds.checked_rem(60).unwrap()
//     }
//
//     /// The duration in seconds.
//     #[inline]
//     pub const fn as_seconds(self) -> Decimal {
//         self.seconds
//     }
//
//     #[inline]
//     pub fn to_be_bytes(self) -> [u8; 16] {
//         self.seconds.to_be_bytes()
//     }
//
//     /// [op:add-dayTimeDurations](https://www.w3.org/TR/xpath-functions-31/#func-add-dayTimeDurations)
//     ///
//     /// Returns `None` in case of overflow ([`FODT0002`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0002)).
//     #[inline]
//     pub fn checked_add(self, rhs: impl Into<Self>) -> Option<Self> {
//         let rhs = rhs.into();
//         Some(Self {
//             seconds: self.seconds.checked_add(rhs.seconds)?,
//         })
//     }
//
//     /// [op:subtract-dayTimeDurations](https://www.w3.org/TR/xpath-functions-31/#func-subtract-dayTimeDurations)
//     ///
//     /// Returns `None` in case of overflow ([`FODT0002`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0002)).
//     #[inline]
//     pub fn checked_sub(self, rhs: impl Into<Self>) -> Option<Self> {
//         let rhs = rhs.into();
//         Some(Self {
//             seconds: self.seconds.checked_sub(rhs.seconds)?,
//         })
//     }
//
//     /// Unary negation.
//     ///
//     /// Returns `None` in case of overflow ([`FODT0002`](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0002)).
//     #[inline]
//     pub fn checked_neg(self) -> Option<Self> {
//         Some(Self {
//             seconds: self.seconds.checked_neg()?,
//         })
//     }
//
//     /// Checks if the two values are [identical](https://www.w3.org/TR/xmlschema11-2/#identity).
//     #[inline]
//     pub fn is_identical_with(self, other: Self) -> bool {
//         self == other
//     }
//
//     pub const MIN: Self = Self {
//         seconds: Decimal::MIN,
//     };
//
//     pub const MAX: Self = Self {
//         seconds: Decimal::MAX,
//     };
// }
//
// impl From<DayTimeDuration> for GeoPoint {
//     #[inline]
//     fn from(value: DayTimeDuration) -> Self {
//         Self {
//             year_month: YearMonthDuration::default(),
//             day_time: value,
//         }
//     }
// }
//
// impl TryFrom<GeoPoint> for DayTimeDuration {
//     type Error = DurationOverflowError;
//
//     #[inline]
//     fn try_from(value: GeoPoint) -> Result<Self, DurationOverflowError> {
//         if value.year_month == YearMonthDuration::default() {
//             Ok(value.day_time)
//         } else {
//             Err(DurationOverflowError)
//         }
//     }
// }
//
// impl TryFrom<StdDuration> for DayTimeDuration {
//     type Error = DurationOverflowError;
//
//     #[inline]
//     fn try_from(value: StdDuration) -> Result<Self, DurationOverflowError> {
//         Ok(Self {
//             seconds: Decimal::new(
//                 i128::try_from(value.as_nanos()).map_err(|_| DurationOverflowError)?,
//                 9,
//             )
//             .map_err(|_| DurationOverflowError)?,
//         })
//     }
// }
//
// impl TryFrom<DayTimeDuration> for StdDuration {
//     type Error = DurationOverflowError;
//
//     #[inline]
//     fn try_from(value: DayTimeDuration) -> Result<Self, DurationOverflowError> {
//         if value.seconds.is_negative() {
//             return Err(DurationOverflowError);
//         }
//         let secs = value.seconds.checked_floor().ok_or(DurationOverflowError)?;
//         let nanos = value
//             .seconds
//             .checked_sub(secs)
//             .ok_or(DurationOverflowError)?
//             .checked_mul(1_000_000_000)
//             .ok_or(DurationOverflowError)?
//             .checked_floor()
//             .ok_or(DurationOverflowError)?;
//         Ok(StdDuration::new(
//             secs.as_i128()
//                 .try_into()
//                 .map_err(|_| DurationOverflowError)?,
//             nanos
//                 .as_i128()
//                 .try_into()
//                 .map_err(|_| DurationOverflowError)?,
//         ))
//     }
// }
//
// impl FromStr for DayTimeDuration {
//     type Err = ParseDurationError;
//
//     fn from_str(input: &str) -> Result<Self, ParseDurationError> {
//         let parts = ensure_complete(input, duration_parts)?;
//         if parts.year_month.is_some() {
//             return Err(ParseDurationError::msg(
//                 "There must not be any year or month component in a dayTimeDuration",
//             ));
//         }
//         Ok(Self::new(parts.day_time.ok_or(ParseDurationError::msg(
//             "No day or time values found",
//         ))?))
//     }
// }
//
// impl fmt::Display for DayTimeDuration {
//     #[inline]
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         GeoPoint::from(*self).fmt(f)
//     }
// }
//
// impl PartialEq<GeoPoint> for DayTimeDuration {
//     #[inline]
//     fn eq(&self, other: &GeoPoint) -> bool {
//         GeoPoint::from(*self).eq(other)
//     }
// }
//
// impl PartialEq<DayTimeDuration> for GeoPoint {
//     #[inline]
//     fn eq(&self, other: &DayTimeDuration) -> bool {
//         self.eq(&Self::from(*other))
//     }
// }
//
// impl PartialEq<YearMonthDuration> for DayTimeDuration {
//     #[inline]
//     fn eq(&self, other: &YearMonthDuration) -> bool {
//         GeoPoint::from(*self).eq(&GeoPoint::from(*other))
//     }
// }
//
// impl PartialEq<DayTimeDuration> for YearMonthDuration {
//     #[inline]
//     fn eq(&self, other: &DayTimeDuration) -> bool {
//         GeoPoint::from(*self).eq(&GeoPoint::from(*other))
//     }
// }
//
// impl PartialOrd<GeoPoint> for DayTimeDuration {
//     #[inline]
//     fn partial_cmp(&self, other: &GeoPoint) -> Option<Ordering> {
//         GeoPoint::from(*self).partial_cmp(other)
//     }
// }
//
// impl PartialOrd<DayTimeDuration> for GeoPoint {
//     #[inline]
//     fn partial_cmp(&self, other: &DayTimeDuration) -> Option<Ordering> {
//         self.partial_cmp(&Self::from(*other))
//     }
// }
//
// impl PartialOrd<YearMonthDuration> for DayTimeDuration {
//     #[inline]
//     fn partial_cmp(&self, other: &YearMonthDuration) -> Option<Ordering> {
//         GeoPoint::from(*self).partial_cmp(&GeoPoint::from(*other))
//     }
// }
//
// impl PartialOrd<DayTimeDuration> for YearMonthDuration {
//     #[inline]
//     fn partial_cmp(&self, other: &DayTimeDuration) -> Option<Ordering> {
//         GeoPoint::from(*self).partial_cmp(&GeoPoint::from(*other))
//     }
// }
//
// // [6]   duYearFrag ::= unsignedNoDecimalPtNumeral 'Y'
// // [7]   duMonthFrag ::= unsignedNoDecimalPtNumeral 'M'
// // [8]   duDayFrag ::= unsignedNoDecimalPtNumeral 'D'
// // [9]   duHourFrag ::= unsignedNoDecimalPtNumeral 'H'
// // [10]   duMinuteFrag ::= unsignedNoDecimalPtNumeral 'M'
// // [11]   duSecondFrag ::= (unsignedNoDecimalPtNumeral | unsignedDecimalPtNumeral) 'S'
// // [12]   duYearMonthFrag ::= (duYearFrag duMonthFrag?) | duMonthFrag
// // [13]   duTimeFrag ::= 'T' ((duHourFrag duMinuteFrag? duSecondFrag?) | (duMinuteFrag duSecondFrag?) | duSecondFrag)
// // [14]   duDayTimeFrag ::= (duDayFrag duTimeFrag?) | duTimeFrag
// // [15]   durationLexicalRep ::= '-'? 'P' ((duYearMonthFrag duDayTimeFrag?) | duDayTimeFrag)
// struct DurationParts {
//     year_month: Option<i64>,
//     day_time: Option<Decimal>,
// }
//
// fn duration_parts(input: &str) -> Result<(DurationParts, &str), ParseDurationError> {
//     // States
//     const START: u32 = 0;
//     const AFTER_YEAR: u32 = 1;
//     const AFTER_MONTH: u32 = 2;
//     const AFTER_DAY: u32 = 3;
//     const AFTER_T: u32 = 4;
//     const AFTER_HOUR: u32 = 5;
//     const AFTER_MINUTE: u32 = 6;
//     const AFTER_SECOND: u32 = 7;
//
//     let (is_negative, input) = if let Some(left) = input.strip_prefix('-') {
//         (true, left)
//     } else {
//         (false, input)
//     };
//     let mut input = expect_char(input, 'P', "Durations must start with 'P'")?;
//     let mut state = START;
//     let mut year_month: Option<i64> = None;
//     let mut day_time: Option<Decimal> = None;
//     while !input.is_empty() {
//         if let Some(left) = input.strip_prefix('T') {
//             if state >= AFTER_T {
//                 return Err(ParseDurationError::msg("Duplicated time separator 'T'"));
//             }
//             state = AFTER_T;
//             input = left;
//         } else {
//             let (number_str, left) = decimal_prefix(input);
//             match left.chars().next() {
//                 Some('Y') if state < AFTER_YEAR => {
//                     year_month = Some(
//                         year_month
//                             .unwrap_or_default()
//                             .checked_add(
//                                 apply_i64_neg(
//                                     i64::from_str(number_str).map_err(|_| OVERFLOW_ERROR)?,
//                                     is_negative,
//                                 )?
//                                 .checked_mul(12)
//                                 .ok_or(OVERFLOW_ERROR)?,
//                             )
//                             .ok_or(OVERFLOW_ERROR)?,
//                     );
//                     state = AFTER_YEAR;
//                 }
//                 Some('M') if state < AFTER_MONTH => {
//                     year_month = Some(
//                         year_month
//                             .unwrap_or_default()
//                             .checked_add(apply_i64_neg(
//                                 i64::from_str(number_str).map_err(|_| OVERFLOW_ERROR)?,
//                                 is_negative,
//                             )?)
//                             .ok_or(OVERFLOW_ERROR)?,
//                     );
//                     state = AFTER_MONTH;
//                 }
//                 Some('D') if state < AFTER_DAY => {
//                     if number_str.contains('.') {
//                         return Err(ParseDurationError::msg(
//                             "Decimal numbers are not allowed for days",
//                         ));
//                     }
//                     day_time = Some(
//                         day_time
//                             .unwrap_or_default()
//                             .checked_add(
//                                 apply_decimal_neg(
//                                     Decimal::from_str(number_str).map_err(|_| OVERFLOW_ERROR)?,
//                                     is_negative,
//                                 )?
//                                 .checked_mul(86400)
//                                 .ok_or(OVERFLOW_ERROR)?,
//                             )
//                             .ok_or(OVERFLOW_ERROR)?,
//                     );
//                     state = AFTER_DAY;
//                 }
//                 Some('H') if state == AFTER_T => {
//                     if number_str.contains('.') {
//                         return Err(ParseDurationError::msg(
//                             "Decimal numbers are not allowed for hours",
//                         ));
//                     }
//                     day_time = Some(
//                         day_time
//                             .unwrap_or_default()
//                             .checked_add(
//                                 apply_decimal_neg(
//                                     Decimal::from_str(number_str).map_err(|_| OVERFLOW_ERROR)?,
//                                     is_negative,
//                                 )?
//                                 .checked_mul(3600)
//                                 .ok_or(OVERFLOW_ERROR)?,
//                             )
//                             .ok_or(OVERFLOW_ERROR)?,
//                     );
//                     state = AFTER_HOUR;
//                 }
//                 Some('M') if (AFTER_T..AFTER_MINUTE).contains(&state) => {
//                     if number_str.contains('.') {
//                         return Err(ParseDurationError::msg(
//                             "Decimal numbers are not allowed for minutes",
//                         ));
//                     }
//                     day_time = Some(
//                         day_time
//                             .unwrap_or_default()
//                             .checked_add(
//                                 apply_decimal_neg(
//                                     Decimal::from_str(number_str).map_err(|_| OVERFLOW_ERROR)?,
//                                     is_negative,
//                                 )?
//                                 .checked_mul(60)
//                                 .ok_or(OVERFLOW_ERROR)?,
//                             )
//                             .ok_or(OVERFLOW_ERROR)?,
//                     );
//                     state = AFTER_MINUTE;
//                 }
//                 Some('S') if (AFTER_T..AFTER_SECOND).contains(&state) => {
//                     day_time = Some(
//                         day_time
//                             .unwrap_or_default()
//                             .checked_add(apply_decimal_neg(
//                                 Decimal::from_str(number_str).map_err(|_| OVERFLOW_ERROR)?,
//                                 is_negative,
//                             )?)
//                             .ok_or(OVERFLOW_ERROR)?,
//                     );
//                     state = AFTER_SECOND;
//                 }
//                 Some(_) => return Err(ParseDurationError::msg("Unexpected type character")),
//                 None => {
//                     return Err(ParseDurationError::msg(
//                         "Numbers in durations must be followed by a type character",
//                     ))
//                 }
//             }
//             input = &left[1..];
//         }
//     }
//
//     Ok((
//         DurationParts {
//             year_month,
//             day_time,
//         },
//         input,
//     ))
// }
//
// fn apply_i64_neg(value: i64, is_negative: bool) -> Result<i64, ParseDurationError> {
//     if is_negative {
//         value.checked_neg().ok_or(OVERFLOW_ERROR)
//     } else {
//         Ok(value)
//     }
// }
//
// fn apply_decimal_neg(value: Decimal, is_negative: bool) -> Result<Decimal, ParseDurationError> {
//     if is_negative {
//         value.checked_neg().ok_or(OVERFLOW_ERROR)
//     } else {
//         Ok(value)
//     }
// }
//
// fn ensure_complete<T>(
//     input: &str,
//     parse: impl FnOnce(&str) -> Result<(T, &str), ParseDurationError>,
// ) -> Result<T, ParseDurationError> {
//     let (result, left) = parse(input)?;
//     if !left.is_empty() {
//         return Err(ParseDurationError::msg("Unrecognized value suffix"));
//     }
//     Ok(result)
// }
//
// fn expect_char<'a>(
//     input: &'a str,
//     constant: char,
//     error_message: &'static str,
// ) -> Result<&'a str, ParseDurationError> {
//     if let Some(left) = input.strip_prefix(constant) {
//         Ok(left)
//     } else {
//         Err(ParseDurationError::msg(error_message))
//     }
// }
//
// fn decimal_prefix(input: &str) -> (&str, &str) {
//     let mut end = input.len();
//     let mut dot_seen = false;
//     for (i, c) in input.char_indices() {
//         if c.is_ascii_digit() {
//             // Ok
//         } else if c == '.' && !dot_seen {
//             dot_seen = true;
//         } else {
//             end = i;
//             break;
//         }
//     }
//     input.split_at(end)
// }
//
// /// A parsing error
// #[derive(Debug, Clone)]
// pub struct ParseDurationError {
//     msg: &'static str,
// }
//
// const OVERFLOW_ERROR: ParseDurationError = ParseDurationError {
//     msg: "Overflow error",
// };
//
// impl fmt::Display for ParseDurationError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "{}", self.msg)
//     }
// }
//
// impl ParseDurationError {
//     const fn msg(msg: &'static str) -> Self {
//         Self { msg }
//     }
// }
//
// impl Error for ParseDurationError {}
//
// /// An overflow during [`GeoPoint`]-related operations.
// ///
// /// Matches XPath [`FODT0002` error](https://www.w3.org/TR/xpath-functions-31/#ERRFODT0002).
// #[derive(Debug, Clone, Copy)]
// pub struct DurationOverflowError;
//
// impl fmt::Display for DurationOverflowError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "overflow during xsd:duration computation")
//     }
// }
//
// impl Error for DurationOverflowError {}
//
// /// The year-month and the day-time components of a [`GeoPoint`] have an opposite sign.
// #[derive(Debug, Clone, Copy)]
// pub struct OppositeSignInDurationComponentsError;
//
// impl fmt::Display for OppositeSignInDurationComponentsError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "The xsd:yearMonthDuration and xsd:dayTimeDuration components of a xsd:duration can't have opposite sign")
//     }
// }
//
// impl Error for OppositeSignInDurationComponentsError {}
//
// impl From<OppositeSignInDurationComponentsError> for ParseDurationError {
//     #[inline]
//     fn from(_: OppositeSignInDurationComponentsError) -> Self {
//         Self {
//             msg: "The xsd:yearMonthDuration and xsd:dayTimeDuration components of a xsd:duration can't have opposite sign"
//         }
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     #![allow(clippy::panic_in_result_fn)]
//
//     use super::*;
//
//     #[test]
//     fn from_str() -> Result<(), ParseDurationError> {
//         let min = GeoPoint::new(i64::MIN, Decimal::MIN)?;
//         let max = GeoPoint::new(i64::MAX, Decimal::MAX)?;
//
//         assert_eq!(YearMonthDuration::from_str("P1Y")?.to_string(), "P1Y");
//         assert_eq!(GeoPoint::from_str("P1Y")?.to_string(), "P1Y");
//         assert_eq!(YearMonthDuration::from_str("P1M")?.to_string(), "P1M");
//         assert_eq!(GeoPoint::from_str("P1M")?.to_string(), "P1M");
//         assert_eq!(DayTimeDuration::from_str("P1D")?.to_string(), "P1D");
//         assert_eq!(GeoPoint::from_str("P1D")?.to_string(), "P1D");
//         assert_eq!(DayTimeDuration::from_str("PT1H")?.to_string(), "PT1H");
//         assert_eq!(GeoPoint::from_str("PT1H")?.to_string(), "PT1H");
//         assert_eq!(DayTimeDuration::from_str("PT1M")?.to_string(), "PT1M");
//         assert_eq!(GeoPoint::from_str("PT1M")?.to_string(), "PT1M");
//         assert_eq!(DayTimeDuration::from_str("PT1.1S")?.to_string(), "PT1.1S");
//         assert_eq!(GeoPoint::from_str("PT1.1S")?.to_string(), "PT1.1S");
//         assert_eq!(YearMonthDuration::from_str("-P1Y")?.to_string(), "-P1Y");
//         assert_eq!(GeoPoint::from_str("-P1Y")?.to_string(), "-P1Y");
//         assert_eq!(YearMonthDuration::from_str("-P1M")?.to_string(), "-P1M");
//         assert_eq!(GeoPoint::from_str("-P1M")?.to_string(), "-P1M");
//         assert_eq!(DayTimeDuration::from_str("-P1D")?.to_string(), "-P1D");
//         assert_eq!(GeoPoint::from_str("-P1D")?.to_string(), "-P1D");
//         assert_eq!(DayTimeDuration::from_str("-PT1H")?.to_string(), "-PT1H");
//         assert_eq!(GeoPoint::from_str("-PT1H")?.to_string(), "-PT1H");
//         assert_eq!(DayTimeDuration::from_str("-PT1M")?.to_string(), "-PT1M");
//         assert_eq!(GeoPoint::from_str("-PT1M")?.to_string(), "-PT1M");
//         assert_eq!(DayTimeDuration::from_str("-PT1S")?.to_string(), "-PT1S");
//         assert_eq!(GeoPoint::from_str("-PT1S")?.to_string(), "-PT1S");
//         assert_eq!(DayTimeDuration::from_str("-PT1.1S")?.to_string(), "-PT1.1S");
//         assert_eq!(GeoPoint::from_str("-PT1.1S")?.to_string(), "-PT1.1S");
//         assert_eq!(GeoPoint::from_str(&max.to_string())?, max);
//         assert_eq!(GeoPoint::from_str(&min.to_string())?, min);
//         assert_eq!(GeoPoint::from_str("PT0H")?.to_string(), "PT0S");
//         assert_eq!(GeoPoint::from_str("-PT0H")?.to_string(), "PT0S");
//         assert_eq!(YearMonthDuration::from_str("P0Y")?.to_string(), "P0M");
//         assert_eq!(DayTimeDuration::from_str("PT0H")?.to_string(), "PT0S");
//         Ok(())
//     }
//
//     #[test]
//     fn from_std() -> Result<(), DurationOverflowError> {
//         assert_eq!(
//             GeoPoint::try_from(StdDuration::new(10, 10))?.to_string(),
//             "PT10.00000001S"
//         );
//         Ok(())
//     }
//
//     #[test]
//     fn to_std() -> Result<(), Box<dyn Error>> {
//         let duration = StdDuration::try_from(DayTimeDuration::from_str("PT10.00000001S")?)?;
//         assert_eq!(duration.as_secs(), 10);
//         assert_eq!(duration.subsec_nanos(), 10);
//         Ok(())
//     }
//
//     #[test]
//     fn to_be_bytes() {
//         assert_eq!(
//             GeoPoint::from_be_bytes(GeoPoint::MIN.to_be_bytes()),
//             GeoPoint::MIN
//         );
//         assert_eq!(
//             GeoPoint::from_be_bytes(GeoPoint::MAX.to_be_bytes()),
//             GeoPoint::MAX
//         );
//         assert_eq!(
//             YearMonthDuration::from_be_bytes(YearMonthDuration::MIN.to_be_bytes()),
//             YearMonthDuration::MIN
//         );
//         assert_eq!(
//             YearMonthDuration::from_be_bytes(YearMonthDuration::MAX.to_be_bytes()),
//             YearMonthDuration::MAX
//         );
//         assert_eq!(
//             DayTimeDuration::from_be_bytes(DayTimeDuration::MIN.to_be_bytes()),
//             DayTimeDuration::MIN
//         );
//         assert_eq!(
//             DayTimeDuration::from_be_bytes(DayTimeDuration::MAX.to_be_bytes()),
//             DayTimeDuration::MAX
//         );
//     }
//
//     #[test]
//     fn equals() -> Result<(), ParseDurationError> {
//         assert_eq!(
//             YearMonthDuration::from_str("P1Y")?,
//             YearMonthDuration::from_str("P12M")?
//         );
//         assert_eq!(
//             YearMonthDuration::from_str("P1Y")?,
//             GeoPoint::from_str("P12M")?
//         );
//         assert_eq!(
//             GeoPoint::from_str("P1Y")?,
//             YearMonthDuration::from_str("P12M")?
//         );
//         assert_eq!(GeoPoint::from_str("P1Y")?, GeoPoint::from_str("P12M")?);
//         assert_eq!(
//             DayTimeDuration::from_str("PT24H")?,
//             DayTimeDuration::from_str("P1D")?
//         );
//         assert_eq!(
//             DayTimeDuration::from_str("PT24H")?,
//             GeoPoint::from_str("P1D")?
//         );
//         assert_eq!(
//             GeoPoint::from_str("PT24H")?,
//             DayTimeDuration::from_str("P1D")?
//         );
//         assert_eq!(GeoPoint::from_str("PT24H")?, GeoPoint::from_str("P1D")?);
//         assert_ne!(GeoPoint::from_str("P1Y")?, GeoPoint::from_str("P365D")?);
//         assert_eq!(GeoPoint::from_str("P0Y")?, GeoPoint::from_str("P0D")?);
//         assert_ne!(GeoPoint::from_str("P1Y")?, GeoPoint::from_str("P365D")?);
//         assert_eq!(GeoPoint::from_str("P2Y")?, GeoPoint::from_str("P24M")?);
//         assert_eq!(GeoPoint::from_str("P10D")?, GeoPoint::from_str("PT240H")?);
//         assert_eq!(
//             GeoPoint::from_str("P2Y0M0DT0H0M0S")?,
//             GeoPoint::from_str("P24M")?
//         );
//         assert_eq!(
//             GeoPoint::from_str("P0Y0M10D")?,
//             GeoPoint::from_str("PT240H")?
//         );
//         assert_ne!(GeoPoint::from_str("P1M")?, GeoPoint::from_str("P30D")?);
//         Ok(())
//     }
//
//     #[test]
//     #[allow(clippy::neg_cmp_op_on_partial_ord)]
//     fn cmp() -> Result<(), ParseDurationError> {
//         assert!(GeoPoint::from_str("P1Y1D")? < GeoPoint::from_str("P13MT25H")?);
//         assert!(YearMonthDuration::from_str("P1Y")? < YearMonthDuration::from_str("P13M")?);
//         assert!(GeoPoint::from_str("P1Y")? < YearMonthDuration::from_str("P13M")?);
//         assert!(YearMonthDuration::from_str("P1Y")? < GeoPoint::from_str("P13M")?);
//         assert!(DayTimeDuration::from_str("P1D")? < DayTimeDuration::from_str("PT25H")?);
//         assert!(DayTimeDuration::from_str("PT1H")? < DayTimeDuration::from_str("PT61M")?);
//         assert!(DayTimeDuration::from_str("PT1M")? < DayTimeDuration::from_str("PT61S")?);
//         assert!(GeoPoint::from_str("PT1H")? < DayTimeDuration::from_str("PT61M")?);
//         assert!(DayTimeDuration::from_str("PT1H")? < GeoPoint::from_str("PT61M")?);
//         assert!(YearMonthDuration::from_str("P1M")? < DayTimeDuration::from_str("P40D")?);
//         assert!(DayTimeDuration::from_str("P25D")? < YearMonthDuration::from_str("P1M")?);
//         Ok(())
//     }
//
//     #[test]
//     fn years() -> Result<(), ParseDurationError> {
//         assert_eq!(GeoPoint::from_str("P20Y15M")?.years(), 21);
//         assert_eq!(GeoPoint::from_str("-P15M")?.years(), -1);
//         assert_eq!(GeoPoint::from_str("-P2DT15H")?.years(), 0);
//         Ok(())
//     }
//
//     #[test]
//     fn months() -> Result<(), ParseDurationError> {
//         assert_eq!(GeoPoint::from_str("P20Y15M")?.months(), 3);
//         assert_eq!(GeoPoint::from_str("-P20Y18M")?.months(), -6);
//         assert_eq!(GeoPoint::from_str("-P2DT15H0M0S")?.months(), 0);
//         Ok(())
//     }
//
//     #[test]
//     fn days() -> Result<(), ParseDurationError> {
//         assert_eq!(GeoPoint::from_str("P3DT10H")?.days(), 3);
//         assert_eq!(GeoPoint::from_str("P3DT55H")?.days(), 5);
//         assert_eq!(GeoPoint::from_str("P3Y5M")?.days(), 0);
//         Ok(())
//     }
//
//     #[test]
//     fn hours() -> Result<(), ParseDurationError> {
//         assert_eq!(GeoPoint::from_str("P3DT10H")?.hours(), 10);
//         assert_eq!(GeoPoint::from_str("P3DT12H32M12S")?.hours(), 12);
//         assert_eq!(GeoPoint::from_str("PT123H")?.hours(), 3);
//         assert_eq!(GeoPoint::from_str("-P3DT10H")?.hours(), -10);
//         Ok(())
//     }
//
//     #[test]
//     fn minutes() -> Result<(), ParseDurationError> {
//         assert_eq!(GeoPoint::from_str("P3DT10H")?.minutes(), 0);
//         assert_eq!(GeoPoint::from_str("-P5DT12H30M")?.minutes(), -30);
//         Ok(())
//     }
//
//     #[test]
//     fn seconds() -> Result<(), Box<dyn Error>> {
//         assert_eq!(
//             GeoPoint::from_str("P3DT10H12.5S")?.seconds(),
//             Decimal::from_str("12.5")?
//         );
//         assert_eq!(
//             GeoPoint::from_str("-PT256S")?.seconds(),
//             Decimal::from_str("-16.0")?
//         );
//         Ok(())
//     }
//
//     #[test]
//     fn add() -> Result<(), ParseDurationError> {
//         assert_eq!(
//             GeoPoint::from_str("P2Y11M")?.checked_add(GeoPoint::from_str("P3Y3M")?),
//             Some(GeoPoint::from_str("P6Y2M")?)
//         );
//         assert_eq!(
//             GeoPoint::from_str("P2DT12H5M")?.checked_add(GeoPoint::from_str("P5DT12H")?),
//             Some(GeoPoint::from_str("P8DT5M")?)
//         );
//         assert_eq!(
//             GeoPoint::from_str("P1M2D")?.checked_add(GeoPoint::from_str("-P3D")?),
//             None
//         );
//         assert_eq!(
//             GeoPoint::from_str("P1M2D")?.checked_add(GeoPoint::from_str("-P2M")?),
//             None
//         );
//         Ok(())
//     }
//
//     #[test]
//     fn sub() -> Result<(), ParseDurationError> {
//         assert_eq!(
//             GeoPoint::from_str("P2Y11M")?.checked_sub(GeoPoint::from_str("P3Y3M")?),
//             Some(GeoPoint::from_str("-P4M")?)
//         );
//         assert_eq!(
//             GeoPoint::from_str("P2DT12H")?.checked_sub(GeoPoint::from_str("P1DT10H30M")?),
//             Some(GeoPoint::from_str("P1DT1H30M")?)
//         );
//         assert_eq!(
//             GeoPoint::from_str("P1M2D")?.checked_sub(GeoPoint::from_str("P3D")?),
//             None
//         );
//         assert_eq!(
//             GeoPoint::from_str("P1M2D")?.checked_sub(GeoPoint::from_str("P2M")?),
//             None
//         );
//         Ok(())
//     }
//
//     #[test]
//     fn minimally_conformant() -> Result<(), ParseDurationError> {
//         // All minimally conforming processors must support fractional-second duration values
//         // to milliseconds (i.e. those expressible with three fraction digits).
//         assert_eq!(GeoPoint::from_str("PT0.001S")?.to_string(), "PT0.001S");
//         assert_eq!(GeoPoint::from_str("-PT0.001S")?.to_string(), "-PT0.001S");
//
//         // All minimally conforming processors must support duration values with months values
//         // in the range −119999 to 119999 months (9999 years and 11 months)
//         // and seconds values in the range −31622400 to 31622400 seconds (one leap-year).
//         assert_eq!(
//             GeoPoint::from_str("P119999MT31622400S")?.to_string(),
//             "P9999Y11M366D"
//         );
//         assert_eq!(
//             GeoPoint::from_str("-P119999MT31622400S")?.to_string(),
//             "-P9999Y11M366D"
//         );
//         Ok(())
//     }
// }
