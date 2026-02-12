//! Date-range helpers for the transactions page.

use rusqlite::Connection;
use serde::Deserialize;
use time::{Date, Duration, Month};

use crate::Error;

#[derive(Deserialize)]
pub struct RangeQuery {
    /// The range preset to display.
    pub range: Option<RangePreset>,
    /// The interval preset to group transactions by.
    pub interval: Option<IntervalPreset>,
    /// Whether category summaries should be shown.
    pub summary: Option<bool>,
    /// The anchor date that determines the current range.
    pub anchor: Option<Date>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RangePreset {
    Week,
    Fortnight,
    Month,
    Quarter,
    HalfYear,
    Year,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntervalPreset {
    Week,
    Fortnight,
    Month,
    Quarter,
    HalfYear,
    Year,
}

impl IntervalPreset {
    pub fn default_preset() -> Self {
        Self::Week
    }

    pub fn as_query_value(self) -> &'static str {
        match self {
            Self::Week => "week",
            Self::Fortnight => "fortnight",
            Self::Month => "month",
            Self::Quarter => "quarter",
            Self::HalfYear => "half-year",
            Self::Year => "year",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Week => "Week",
            Self::Fortnight => "Fortnight",
            Self::Month => "Month",
            Self::Quarter => "Quarter",
            Self::HalfYear => "Half-year",
            Self::Year => "Year",
        }
    }

    fn size_rank(self) -> u8 {
        match self {
            Self::Week => 1,
            Self::Fortnight => 2,
            Self::Month => 3,
            Self::Quarter => 4,
            Self::HalfYear => 5,
            Self::Year => 6,
        }
    }
}

impl RangePreset {
    pub fn default_preset() -> Self {
        Self::Month
    }

    pub fn as_query_value(self) -> &'static str {
        match self {
            Self::Week => "week",
            Self::Fortnight => "fortnight",
            Self::Month => "month",
            Self::Quarter => "quarter",
            Self::HalfYear => "half-year",
            Self::Year => "year",
        }
    }

    fn size_rank(self) -> u8 {
        match self {
            Self::Week => 1,
            Self::Fortnight => 2,
            Self::Month => 3,
            Self::Quarter => 4,
            Self::HalfYear => 5,
            Self::Year => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    pub start: Date,
    pub end: Date,
}

#[derive(Debug, Clone)]
pub struct RangeNavigation {
    pub range: DateRange,
    pub prev: Option<RangeNavLink>,
    pub next: Option<RangeNavLink>,
}

impl RangeNavigation {
    pub fn new(preset: RangePreset, range: DateRange, bounds: Option<DateRange>) -> Self {
        let prev_anchor = range.start - Duration::days(1);
        let next_anchor = range.end + Duration::days(1);
        let prev_range = compute_range(preset, prev_anchor);
        let next_range = compute_range(preset, next_anchor);

        let (prev, next) = match bounds {
            Some(bounds) => {
                let prev = if prev_range.end >= bounds.start {
                    Some(RangeNavLink::new(preset, prev_range))
                } else {
                    None
                };
                let next = if next_range.start <= bounds.end {
                    Some(RangeNavLink::new(preset, next_range))
                } else {
                    None
                };
                (prev, next)
            }
            None => (None, None),
        };

        Self { range, prev, next }
    }
}

#[derive(Debug, Clone)]
pub struct RangeNavLink {
    pub range: DateRange,
    pub href: String,
}

impl RangeNavLink {
    pub fn new(preset: RangePreset, range: DateRange) -> Self {
        Self {
            range,
            href: range_anchor_query(preset, range.end),
        }
    }
}

pub fn get_transaction_date_bounds(connection: &Connection) -> Result<Option<DateRange>, Error> {
    let mut stmt = connection
        .prepare("SELECT MIN(date) as min_date, MAX(date) as max_date FROM \"transaction\"")?;
    let mut rows = stmt.query([])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let min_date: Option<Date> = row.get(0)?;
    let max_date: Option<Date> = row.get(1)?;

    match (min_date, max_date) {
        (Some(start), Some(end)) => Ok(Some(DateRange { start, end })),
        _ => Ok(None),
    }
}

pub fn compute_range(preset: RangePreset, anchor_date: Date) -> DateRange {
    match preset {
        RangePreset::Week => week_bounds(anchor_date),
        RangePreset::Fortnight => fortnight_bounds(anchor_date),
        RangePreset::Month => month_bounds(anchor_date.year(), anchor_date.month()),
        RangePreset::Quarter => quarter_bounds(anchor_date.year(), anchor_date.month()),
        RangePreset::HalfYear => half_year_bounds(anchor_date.year(), anchor_date.month()),
        RangePreset::Year => year_bounds(anchor_date.year()),
    }
}

pub fn compute_interval_range(preset: IntervalPreset, anchor_date: Date) -> DateRange {
    match preset {
        IntervalPreset::Week => week_bounds(anchor_date),
        IntervalPreset::Fortnight => fortnight_bounds(anchor_date),
        IntervalPreset::Month => month_bounds(anchor_date.year(), anchor_date.month()),
        IntervalPreset::Quarter => quarter_bounds(anchor_date.year(), anchor_date.month()),
        IntervalPreset::HalfYear => half_year_bounds(anchor_date.year(), anchor_date.month()),
        IntervalPreset::Year => year_bounds(anchor_date.year()),
    }
}

pub fn range_preset_can_contain_interval(range: RangePreset, interval: IntervalPreset) -> bool {
    range.size_rank() >= interval.size_rank()
}

pub fn smallest_range_for_interval(interval: IntervalPreset) -> RangePreset {
    match interval {
        IntervalPreset::Week => RangePreset::Week,
        IntervalPreset::Fortnight => RangePreset::Fortnight,
        IntervalPreset::Month => RangePreset::Month,
        IntervalPreset::Quarter => RangePreset::Quarter,
        IntervalPreset::HalfYear => RangePreset::HalfYear,
        IntervalPreset::Year => RangePreset::Year,
    }
}

pub fn range_label(range: DateRange) -> String {
    let start = format_date_label(range.start);
    let end = format_date_label(range.end);

    format!("{start} - {end}")
}

pub fn range_anchor_query(preset: RangePreset, anchor: Date) -> String {
    format!("range={}&anchor={}", preset.as_query_value(), anchor)
}

fn week_bounds(anchor_date: Date) -> DateRange {
    let weekday_number = anchor_date.weekday().number_from_monday() as i64;
    let start = anchor_date - Duration::days(weekday_number - 1);
    let end = start + Duration::days(6);

    DateRange { start, end }
}

fn fortnight_bounds(anchor_date: Date) -> DateRange {
    let year = anchor_date.year();
    let month = anchor_date.month();
    let day = anchor_date.day();
    let end_day = if day <= 14 {
        14
    } else {
        last_day_of_month(year, month)
    };
    let start_day = if day <= 14 { 1 } else { 15 };

    DateRange {
        start: Date::from_calendar_date(year, month, start_day)
            .expect("invalid fortnight start date"),
        end: Date::from_calendar_date(year, month, end_day).expect("invalid fortnight end date"),
    }
}

fn month_bounds(year: i32, month: Month) -> DateRange {
    let start = Date::from_calendar_date(year, month, 1).expect("invalid month start date");
    let end = Date::from_calendar_date(year, month, last_day_of_month(year, month))
        .expect("invalid month end date");

    DateRange { start, end }
}

fn quarter_bounds(year: i32, month: Month) -> DateRange {
    let month_number = month_number(month);
    let quarter_start = ((month_number - 1) / 3) * 3 + 1;
    let quarter_end = quarter_start + 2;

    let start_month = month_from_number(quarter_start);
    let end_month = month_from_number(quarter_end);

    DateRange {
        start: Date::from_calendar_date(year, start_month, 1).expect("invalid quarter start date"),
        end: Date::from_calendar_date(year, end_month, last_day_of_month(year, end_month))
            .expect("invalid quarter end date"),
    }
}

fn half_year_bounds(year: i32, month: Month) -> DateRange {
    let month_number = month_number(month);
    let (start_month, end_month) = if month_number <= 6 {
        (Month::January, Month::June)
    } else {
        (Month::July, Month::December)
    };

    DateRange {
        start: Date::from_calendar_date(year, start_month, 1)
            .expect("invalid half-year start date"),
        end: Date::from_calendar_date(year, end_month, last_day_of_month(year, end_month))
            .expect("invalid half-year end date"),
    }
}

fn year_bounds(year: i32) -> DateRange {
    DateRange {
        start: Date::from_calendar_date(year, Month::January, 1).expect("invalid year start date"),
        end: Date::from_calendar_date(year, Month::December, 31).expect("invalid year end date"),
    }
}

fn last_day_of_month(year: i32, month: Month) -> u8 {
    match month {
        Month::January
        | Month::March
        | Month::May
        | Month::July
        | Month::August
        | Month::October
        | Month::December => 31,
        Month::April | Month::June | Month::September | Month::November => 30,
        Month::February => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn month_number(month: Month) -> u8 {
    match month {
        Month::January => 1,
        Month::February => 2,
        Month::March => 3,
        Month::April => 4,
        Month::May => 5,
        Month::June => 6,
        Month::July => 7,
        Month::August => 8,
        Month::September => 9,
        Month::October => 10,
        Month::November => 11,
        Month::December => 12,
    }
}

fn month_from_number(month: u8) -> Month {
    match month {
        1 => Month::January,
        2 => Month::February,
        3 => Month::March,
        4 => Month::April,
        5 => Month::May,
        6 => Month::June,
        7 => Month::July,
        8 => Month::August,
        9 => Month::September,
        10 => Month::October,
        11 => Month::November,
        12 => Month::December,
        _ => panic!("invalid month number {month}"),
    }
}

fn format_date_label(date: Date) -> String {
    format!(
        "{} {} {}",
        date.day(),
        month_abbrev(date.month()),
        date.year()
    )
}

fn month_abbrev(month: Month) -> &'static str {
    match month {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}
