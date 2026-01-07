use super::structs::*;
use chrono::{DateTime, Datelike, NaiveDate, Utc};

pub fn last_day_of_month(year: i32, month: u32) -> u32 {
    // Handle December specially
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };

    // First day of next month
    let first_of_next_month =
        NaiveDate::from_ymd_opt(next_year, next_month, 1).expect("Invalid date");

    // Subtract one day
    let last_day = first_of_next_month.pred_opt().unwrap();

    last_day.day()
}

pub fn parse_until(s: &str) -> Option<DateTimeSpec> {
    if s.len() == 8 {
        // YYYYMMDD -> date-only
        NaiveDate::parse_from_str(s, "%Y%m%d")
            .ok()
            .map(DateTimeSpec::Date)
    } else if s.ends_with('Z') {
        // UTC datetime
        DateTime::parse_from_str(s, "%Y%m%dT%H%M%SZ")
            .ok()
            .map(|dt| DateTimeSpec::DateTime {
                value: dt.with_timezone(&Utc),
                tzid: Some("UTC".into()),
            })
    } else {
        // naive/floating datetime
        DateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
            .ok()
            .map(|dt| DateTimeSpec::DateTime {
                value: dt.with_timezone(&Utc),
                tzid: Some("floating".into()),
            })
    }
}

pub use parse_rdate as parse_exdate;
pub fn parse_rdate(prop: ical::property::Property) -> Option<Vec<DateTimeSpec>> {
    let val = &prop.value?;
    Some(
        val.split(',')
            .filter_map(|p| parse_until(p))
            .collect::<Vec<DateTimeSpec>>(),
    )
}

pub fn parse_utc(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_str(s, "%Y%m%d%T%H%M%S")
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}
