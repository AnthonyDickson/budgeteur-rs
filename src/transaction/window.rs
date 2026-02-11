//! Windowed date-range helpers for the transactions page.

use rusqlite::Connection;
use serde::Deserialize;
use time::{Date, Duration, Month};

use crate::Error;

#[derive(Deserialize)]
pub struct WindowQuery {
    /// The window preset to display.
    pub window: Option<WindowPreset>,
    /// The bucket preset to group transactions by.
    pub bucket: Option<BucketPreset>,
    /// The anchor date that determines the current window.
    pub anchor: Option<Date>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowPreset {
    Week,
    Fortnight,
    Month,
    Quarter,
    HalfYear,
    Year,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BucketPreset {
    Week,
    Fortnight,
    Month,
    Quarter,
    HalfYear,
    Year,
}

impl BucketPreset {
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
}

impl WindowPreset {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRange {
    pub start: Date,
    pub end: Date,
}

#[derive(Debug, Clone)]
pub struct WindowNavigation {
    pub range: WindowRange,
    pub prev: Option<WindowNavLink>,
    pub next: Option<WindowNavLink>,
}

impl WindowNavigation {
    pub fn new(preset: WindowPreset, range: WindowRange, bounds: Option<WindowRange>) -> Self {
        let prev_anchor = range.start - Duration::days(1);
        let next_anchor = range.end + Duration::days(1);
        let prev_range = compute_window_range(preset, prev_anchor);
        let next_range = compute_window_range(preset, next_anchor);

        let (prev, next) = match bounds {
            Some(bounds) => {
                let prev = if prev_range.end >= bounds.start {
                    Some(WindowNavLink::new(preset, prev_range))
                } else {
                    None
                };
                let next = if next_range.start <= bounds.end {
                    Some(WindowNavLink::new(preset, next_range))
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
pub struct WindowNavLink {
    pub range: WindowRange,
    pub href: String,
}

impl WindowNavLink {
    pub fn new(preset: WindowPreset, range: WindowRange) -> Self {
        Self {
            range,
            href: window_anchor_query(preset, range.end),
        }
    }
}

pub fn get_transaction_date_bounds(connection: &Connection) -> Result<Option<WindowRange>, Error> {
    let mut stmt = connection
        .prepare("SELECT MIN(date) as min_date, MAX(date) as max_date FROM \"transaction\"")?;
    let mut rows = stmt.query([])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let min_date: Option<Date> = row.get(0)?;
    let max_date: Option<Date> = row.get(1)?;

    match (min_date, max_date) {
        (Some(start), Some(end)) => Ok(Some(WindowRange { start, end })),
        _ => Ok(None),
    }
}

pub fn compute_window_range(preset: WindowPreset, anchor_date: Date) -> WindowRange {
    match preset {
        WindowPreset::Week => week_bounds(anchor_date),
        WindowPreset::Fortnight => fortnight_bounds(anchor_date),
        WindowPreset::Month => month_bounds(anchor_date.year(), anchor_date.month()),
        WindowPreset::Quarter => quarter_bounds(anchor_date.year(), anchor_date.month()),
        WindowPreset::HalfYear => half_year_bounds(anchor_date.year(), anchor_date.month()),
        WindowPreset::Year => year_bounds(anchor_date.year()),
    }
}

pub fn compute_bucket_range(preset: BucketPreset, anchor_date: Date) -> WindowRange {
    match preset {
        BucketPreset::Week => week_bounds(anchor_date),
        BucketPreset::Fortnight => fortnight_bounds(anchor_date),
        BucketPreset::Month => month_bounds(anchor_date.year(), anchor_date.month()),
        BucketPreset::Quarter => quarter_bounds(anchor_date.year(), anchor_date.month()),
        BucketPreset::HalfYear => half_year_bounds(anchor_date.year(), anchor_date.month()),
        BucketPreset::Year => year_bounds(anchor_date.year()),
    }
}

pub fn window_range_label(range: WindowRange) -> String {
    let start = format_date_label(range.start);
    let end = format_date_label(range.end);

    format!("{start} - {end}")
}

pub fn window_anchor_query(preset: WindowPreset, anchor: Date) -> String {
    format!("window={}&anchor={}", preset.as_query_value(), anchor)
}

fn week_bounds(anchor_date: Date) -> WindowRange {
    let weekday_number = anchor_date.weekday().number_from_monday() as i64;
    let start = anchor_date - Duration::days(weekday_number - 1);
    let end = start + Duration::days(6);

    WindowRange { start, end }
}

fn fortnight_bounds(anchor_date: Date) -> WindowRange {
    let year = anchor_date.year();
    let month = anchor_date.month();
    let day = anchor_date.day();
    let end_day = if day <= 14 {
        14
    } else {
        last_day_of_month(year, month)
    };
    let start_day = if day <= 14 { 1 } else { 15 };

    WindowRange {
        start: Date::from_calendar_date(year, month, start_day)
            .expect("invalid fortnight start date"),
        end: Date::from_calendar_date(year, month, end_day).expect("invalid fortnight end date"),
    }
}

fn month_bounds(year: i32, month: Month) -> WindowRange {
    let start = Date::from_calendar_date(year, month, 1).expect("invalid month start date");
    let end = Date::from_calendar_date(year, month, last_day_of_month(year, month))
        .expect("invalid month end date");

    WindowRange { start, end }
}

fn quarter_bounds(year: i32, month: Month) -> WindowRange {
    let month_number = month_number(month);
    let quarter_start = ((month_number - 1) / 3) * 3 + 1;
    let quarter_end = quarter_start + 2;

    let start_month = month_from_number(quarter_start);
    let end_month = month_from_number(quarter_end);

    WindowRange {
        start: Date::from_calendar_date(year, start_month, 1).expect("invalid quarter start date"),
        end: Date::from_calendar_date(year, end_month, last_day_of_month(year, end_month))
            .expect("invalid quarter end date"),
    }
}

fn half_year_bounds(year: i32, month: Month) -> WindowRange {
    let month_number = month_number(month);
    let (start_month, end_month) = if month_number <= 6 {
        (Month::January, Month::June)
    } else {
        (Month::July, Month::December)
    };

    WindowRange {
        start: Date::from_calendar_date(year, start_month, 1)
            .expect("invalid half-year start date"),
        end: Date::from_calendar_date(year, end_month, last_day_of_month(year, end_month))
            .expect("invalid half-year end date"),
    }
}

fn year_bounds(year: i32) -> WindowRange {
    WindowRange {
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
