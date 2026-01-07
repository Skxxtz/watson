use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use chrono::{DateTime, Datelike, Duration, NaiveTime, TimeZone, Utc, Weekday};
use ical::parser::ical::component::IcalEvent;

use crate::{
    calendar::utils::{
        funcs::{last_day_of_month, parse_exdate, parse_rdate, parse_until, parse_utc},
        structs::{Attendee, DateTimeSpec, RecurrenceRule},
    },
    errors::{WatsonError, WatsonErrorKind},
    watson_err,
};

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CalendarInfo {
    pub href: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CalDavEvent {
    pub uid: String,

    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,

    pub start: Option<DateTimeSpec>,
    pub end: Option<DateTimeSpec>,

    pub recurrence: Option<RecurrenceRule>,
    pub recurrence_id: Option<DateTimeSpec>,

    pub rdates: Vec<DateTimeSpec>,
    pub exdates: Vec<DateTimeSpec>,

    pub last_modified: Option<DateTime<Utc>>,
    pub sequence: Option<i32>,

    pub url: Option<String>,

    pub organizer: Option<String>,
    pub attendees: Vec<Attendee>,

    pub calendar_info: Rc<CalendarInfo>,

    pub event_type: CalEventType,

    pub seen: Cell<bool>,
}
impl TryFrom<IcalEvent> for CalDavEvent {
    type Error = WatsonError;
    fn try_from(value: IcalEvent) -> Result<Self, Self::Error> {
        let mut out = Self::default();

        for prop in value.properties {
            match prop.name.as_str() {
                "UID" => {
                    out.uid = prop.value.ok_or_else(|| {
                        watson_err!(WatsonErrorKind::Deserialization, "ICalEvent missing UID")
                    })?;
                }

                "SUMMARY" => out.summary = prop.value,
                "DESCRIPTION" => out.description = prop.value,
                "LOCATION" => out.location = prop.value,

                "DTSTART" => out.start = Some(DateTimeSpec::try_from(prop)?),
                "DTEND" => out.end = Some(DateTimeSpec::try_from(prop)?),

                "RECURRENCE-ID" => {
                    out.recurrence_id = Some(DateTimeSpec::try_from(prop)?);
                }

                "RRULE" => out.recurrence = prop.value.map(|v| RecurrenceRule::new(v)),
                "RDATE" => out.rdates = parse_rdate(prop).unwrap_or_default(),
                "EXDATE" => out.exdates = parse_exdate(prop).unwrap_or_default(),

                "LAST-MODIFIED" => out.last_modified = prop.value.and_then(|v| parse_utc(&v)),

                "SEQUENCE" => out.sequence = prop.value.and_then(|v| v.parse().ok()),

                "URL" => out.url = prop.value,

                "ORGANIZER" => out.organizer = prop.value,

                "ATTENDEE" => {
                    if let Ok(attendee) = Attendee::try_from(prop) {
                        out.attendees.push(attendee);
                    }
                }

                _ => {}
            }
        }

        out.event_type = if let Some(start) = out.start.as_ref() {
            match (start, out.end.as_ref()) {
                (DateTimeSpec::DateTime { .. }, Some(DateTimeSpec::DateTime { .. })) => {
                    CalEventType::Timed
                }
                _ => CalEventType::AllDay,
            }
        } else {
            return Err(watson_err!(
                WatsonErrorKind::Deserialization,
                "Failed to deserialize ICalEvent into CalDavEvent. (Missing start event)"
            ));
        };

        if out.uid.is_empty() {
            Err(watson_err!(
                WatsonErrorKind::Deserialization,
                "Failed to deserialize ICalEvent into CalDavEvent. (Empty UID)"
            ))
        } else {
            Ok(out)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CalEventType {
    Timed,
    AllDay,
}
impl Default for CalEventType {
    fn default() -> Self {
        CalEventType::AllDay
    }
}

impl CalDavEvent {
    pub fn occurs_on_day(&self, day: &DateTime<Utc>) -> bool {
        let Some(start) = self.start.as_ref() else {
            return false;
        };

        if self.summary.is_none() {
            println!("{:?}", self);
        }
        // If the event is recurring, delegate
        if self.recurrence.is_some() {
            return self.handle_recurrence(day);
        }

        // Compute start and end as UTC
        let start_utc = start.utc_time();
        let end_utc = self.end.as_ref().map(|e| e.utc_time()).unwrap_or(start_utc);

        // Compare day ignoring time (assuming day is at midnight UTC)
        let day = day.date_naive();
        let day_start =
            Utc.from_utc_datetime(&day.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()));
        let day_end = day_start + Duration::days(1) - Duration::seconds(1); // 23:59:59 of that day

        start_utc <= day_end && end_utc >= day_start
    }

    fn handle_recurrence(&self, target: &DateTime<Utc>) -> bool {
        let dt_start = match self.start.as_ref() {
            Some(d) => d.utc_time(),
            _ => return false,
        };
        let recurrence = match self.recurrence.as_ref() {
            Some(r) => r,
            _ => return false,
        };

        // Gather all rules
        let map: HashMap<String, String> = recurrence
            .raw
            .split(';')
            .filter_map(|p| p.split_once('='))
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();

        // Early return if same day
        if *target == dt_start {
            return true;
        }

        // If date is explicitely named, return true
        if self.rdates.iter().any(|d| d.utc_time() == *target) {
            return true;
        }
        // If date is explicitely named, return false
        if self.exdates.iter().any(|d| d.utc_time() == *target) {
            return false;
        }

        // If rule does not apply anymore
        if let Some(until) = map.get("UNTIL").and_then(|u| parse_until(u)) {
            match until {
                DateTimeSpec::Date(d) => {
                    if target.date_naive() > d {
                        return false;
                    }
                }
                DateTimeSpec::DateTime { .. } => {
                    let until_dt = until.utc_time();
                    if *target > until_dt {
                        return false;
                    }
                }
            }
        }

        // Early return if BYDAY does not match
        if let Some(by_day) = map.get("BYDAY") {
            let allowed_days: HashSet<Weekday> = by_day
                .split(',')
                .map(|d| d.trim())
                .filter_map(|d| match d {
                    "MO" => Some(Weekday::Mon),
                    "TU" => Some(Weekday::Tue),
                    "WE" => Some(Weekday::Wed),
                    "TH" => Some(Weekday::Thu),
                    "FR" => Some(Weekday::Fri),
                    "SA" => Some(Weekday::Sat),
                    "SU" => Some(Weekday::Sun),
                    _ => None,
                })
                .collect();
            if allowed_days.is_empty() {
                if target.weekday() != dt_start.weekday() {
                    return false;
                }
            }
            if !allowed_days.contains(&target.weekday()) {
                return false;
            }
        }

        // Early return if BYMONTHDAY is not true
        if let Some(bymonthday) = map.get("BYMONTHDAY") {
            let day = target.day();
            let last = last_day_of_month(target.year(), target.month());

            let bymonth_matches = bymonthday
                .split(',')
                .filter_map(|d| d.parse::<i32>().ok())
                .any(|d| {
                    if d > 0 && d as u32 == day {
                        true
                    } else if d < 0 && last as i32 + d == day as i32 {
                        true
                    } else {
                        false
                    }
                });

            if !bymonth_matches {
                return false;
            }
        }

        // Early return if BYWEEKNO is not true
        if let Some(byweekno) = map.get("BYWEEKNO") {
            let weekno = target.iso_week().week();
            let byweekno_matches = byweekno
                .split(',')
                .filter_map(|d| d.parse::<u32>().ok())
                .any(|d| weekno == d);
            if !byweekno_matches {
                return false;
            }
        }

        // Early return if BYMONTH is not true
        if let Some(bymonth) = map.get("BYMONTH") {
            let month = target.month();
            let bymonth_matches = bymonth
                .split(',')
                .filter_map(|d| d.parse::<u32>().ok())
                .any(|d| d == month);

            if !bymonth_matches {
                return false;
            }
        }

        // Check if frequency matches
        if let Some(freq) = map.get("FREQ") {
            let interval = map
                .get("INTERVAL")
                .and_then(|i| i.parse::<i64>().ok())
                .unwrap_or(1);
            let delta_time = *target - dt_start;

            // Check invalid time since
            let num_days = delta_time.num_days();
            if num_days == 0 {
                return true;
            } else if num_days < 0 {
                return false;
            }

            match freq.as_str() {
                "SECONDLY" | "MINUTELY" | "HOURLY" => {
                    return false; // No support yet, maybe in the future
                }

                "DAILY" => return num_days % interval == 0,
                "WEEKLY" => {
                    // Early return if day does not match
                    if map.get("BYDAY").is_some() {
                        if target.weekday() != dt_start.weekday() {
                            return false;
                        }
                    }

                    let days = (target.date_naive() - dt_start.date_naive()).num_days();
                    if days < 0 {
                        return false;
                    }

                    if days % 7 != 0 {
                        return false;
                    }

                    let weeks_since = days / 7;
                    return (weeks_since as i64) % interval == 0;
                }
                "MONTHLY" => {
                    let months_since = (target.year() - dt_start.year()) * 12
                        + (target.month() - dt_start.month()) as i32;

                    if map.get("BYMONTHDAY").is_none() {
                        if target.day() != dt_start.day() {
                            return false;
                        }
                    }

                    return months_since as i64 % interval == 0;
                }
                "YEARLY" => {
                    let months_since = (target.year() - dt_start.year()) * 12
                        + (target.month() - dt_start.month()) as i32;

                    return months_since as i64 % (interval * 12) == 0;
                }
                _ => return false,
            }
        } else {
            return false;
        }
    }
}
