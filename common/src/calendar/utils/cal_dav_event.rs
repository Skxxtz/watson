use std::{cell::Cell, sync::Arc};

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, Utc, Weekday};
use ical::parser::ical::component::IcalEvent;
use serde::{Deserialize, Serialize};

use crate::{
    calendar::utils::{
        funcs::{last_day_of_month, parse_exdate, parse_rdate, parse_until, parse_utc},
        structs::{Attendee, DateTimeSpec, RecurrenceRule},
    },
    utils::errors::{WatsonError, WatsonErrorKind},
    watson_err,
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarInfo {
    pub href: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalDavEvent {
    pub uid: String,

    pub title: String,
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

    pub calendar_info: Arc<CalendarInfo>,

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
                        watson_err!(WatsonErrorKind::Deserialize, "ICalEvent missing UID")
                    })?;
                }

                "SUMMARY" => out.title = prop.value.unwrap_or(out.title),
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
                WatsonErrorKind::Deserialize,
                "Failed to deserialize ICalEvent into CalDavEvent. (Missing start event)"
            ));
        };

        if out.uid.is_empty() {
            Err(watson_err!(
                WatsonErrorKind::Deserialize,
                "Failed to deserialize ICalEvent into CalDavEvent. (Empty UID)"
            ))
        } else {
            Ok(out)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub fn occurs_on_day(&self, day_to_check: &NaiveDate) -> bool {
        let Some(start) = self.start.as_ref() else {
            return false;
        };

        let start_local = start.utc_time().with_timezone(&Local).date_naive();
        let mut end_local = self
            .end
            .as_ref()
            .map(|e| e.utc_time().with_timezone(&Local).date_naive())
            .unwrap_or(start_local);

        if let Some(recurrence) = &self.recurrence {
            let handler = RecurrenceHandler::from_raw(&recurrence.raw, &self.rdates, &self.exdates);
            let active = handler.is_active_on(&start_local, day_to_check);
            return active;
        }

        if self.event_type == CalEventType::AllDay {
            if let Some(new_end) = end_local.checked_sub_days(Days::new(1)) {
                end_local = new_end;
            }
        }

        *day_to_check >= start_local && *day_to_check <= end_local
    }
}

#[derive(Debug)]
pub struct RecurrenceHandler<'d> {
    interval: i64,
    freq: Freq,
    until: Option<i64>,

    // R/EXDATES
    rdates: &'d Vec<DateTimeSpec>,
    exdates: &'d Vec<DateTimeSpec>,

    // Masks
    byday_mask: u8,
    bymonth_mask: u16,
    bymonthday_mask: u32,
    byweekno_mask: u64,
    byyearday_mask: [u64; 6], // > 365

    // Negs
    neg_bymonthday: Vec<i8>,
    neg_byweekno: Vec<i8>,
    neg_byyearday: Vec<i16>,
}
impl<'d> RecurrenceHandler<'d> {
    pub fn from_raw(
        raw: &str,
        rdates: &'d Vec<DateTimeSpec>,
        exdates: &'d Vec<DateTimeSpec>,
    ) -> Self {
        let mut freq = Freq::Daily;
        let mut interval = 1;
        let mut until = None;

        let mut byday_mask = 0;
        let mut bymonth_mask = 0;
        let mut bymonthday_mask = 0;
        let mut byweekno_mask = 0;
        let mut byyearday_mask = [0, 0, 0, 0, 0, 0];

        let mut neg_bymonthday = Vec::new();
        let mut neg_byweekno = Vec::new();
        let mut neg_byyearday = Vec::new();

        let clean_raw = raw.strip_prefix("RRULE:").unwrap_or(raw);
        for part in clean_raw.split(';') {
            if let Some((key, val)) = part.split_once('=') {
                match key {
                    "FREQ" => {
                        freq = match val {
                            "WEEKLY" => Freq::Weekly,
                            "MONTHLY" => Freq::Monthly,
                            "YEARLY" => Freq::Yearly,
                            _ => Freq::Daily,
                        }
                    }
                    "INTERVAL" => interval = val.parse().unwrap_or(1),
                    "BYDAY" => {
                        for day_str in val.split(',') {
                            byday_mask |= match day_str {
                                "MO" => 1 << 0,
                                "TU" => 1 << 1,
                                "WE" => 1 << 2,
                                "TH" => 1 << 3,
                                "FR" => 1 << 4,
                                "SA" => 1 << 5,
                                "SU" => 1 << 6,
                                _ => 0,
                            }
                        }
                    }
                    "BYMONTH" => {
                        for m_str in val.split(',') {
                            if let Ok(m_num) = m_str.parse::<u32>() {
                                // RFC 5545 months are 1-12
                                if m_num >= 1 && m_num <= 12 {
                                    bymonth_mask |= 1 << (m_num - 1);
                                }
                            }
                        }
                    }
                    "BYMONTHDAY" => {
                        for d in val.split(',') {
                            if let Ok(day_num) = d.parse::<i32>() {
                                if day_num >= 1 && day_num <= 31 {
                                    bymonthday_mask |= 1 << (day_num - 1);
                                } else if day_num <= -1 && day_num >= -31 {
                                    neg_bymonthday.push(day_num as i8);
                                }
                            }
                        }
                    }
                    "BYWEEKNO" => {
                        for w_str in val.split(',') {
                            if let Ok(w_num) = w_str.parse::<i32>() {
                                if w_num >= 1 && w_num <= 53 {
                                    byweekno_mask |= 1 << (w_num - 1);
                                } else if w_num <= -1 && w_num >= -53 {
                                    neg_byweekno.push(w_num as i8);
                                }
                            }
                        }
                    }
                    "BYYEARDAY" => {
                        for y_str in val.split(',') {
                            if let Ok(y_num) = y_str.parse::<i32>() {
                                if y_num >= 1 && y_num <= 366 {
                                    let idx = ((y_num - 1) / 64) as usize;
                                    let bit = (y_num - 1) % 64;
                                    byyearday_mask[idx] |= 1 << bit;
                                } else if y_num <= -1 && y_num >= -366 {
                                    neg_byyearday.push(y_num as i16);
                                }
                            }
                        }
                    }

                    "UNTIL" => until = parse_until(val).map(|u| u.utc_time().timestamp()),
                    _ => {}
                }
            }
        }

        Self {
            // Core
            freq,
            interval,
            until,

            // R/EXDATES
            rdates,
            exdates,

            // Masks
            byday_mask,
            bymonth_mask,
            bymonthday_mask,
            byweekno_mask,
            byyearday_mask,

            // Negs
            neg_bymonthday,
            neg_byweekno,
            neg_byyearday,
        }
    }

    #[inline(always)]
    pub fn is_active_on(&self, dt_start: &NaiveDate, target: &NaiveDate) -> bool {
        // UNTIL early return
        if let Some(u) = &self.until {
            let until_date = DateTime::from_timestamp(*u, 0)
                .map(|dt| dt.date_naive())
                .unwrap_or(NaiveDate::MAX);
            if *target > until_date {
                return false;
            }
        }

        // RDATE and EXDATE check
        if self
            .exdates
            .iter()
            .any(|ex| ex.utc_time().date_naive() == *target)
        {
            return false;
        }
        if self
            .rdates
            .iter()
            .any(|ex| ex.utc_time().date_naive() == *target)
        {
            return true;
        }

        // Restraint check
        let has_day_constraints = self.byday_mask != 0
            || self.bymonthday_mask != 0
            || !self.neg_bymonthday.is_empty()
            || !self.neg_byyearday.is_empty()
            || self.byyearday_mask.iter().any(|&m| m != 0);

        // Restraint early return
        if !has_day_constraints {
            match self.freq {
                Freq::Weekly => {
                    if target.weekday() != dt_start.weekday() {
                        return false;
                    }
                }
                Freq::Monthly => {
                    if target.day() != dt_start.day() {
                        return false;
                    }
                }
                Freq::Yearly => {
                    if target.month() != dt_start.month() || target.day() != dt_start.day() {
                        return false;
                    }
                }
                Freq::Daily => {}
            }
        }

        // BYDAY check
        if self.byday_mask != 0 {
            if !self.matches_day(target.weekday()) {
                return false;
            }
        }

        // BYMONTH check
        if self.bymonth_mask != 0 {
            let month_bit = 1 << (target.month() - 1);
            if (self.bymonth_mask & month_bit) == 0 {
                return false;
            }
        }

        // BYMONTHDAY Check
        if self.bymonthday_mask != 0 || !self.neg_bymonthday.is_empty() {
            let mut matches = false;

            let day_bit = 1 << (target.day() - 1);
            if (self.bymonthday_mask & day_bit) != 0 {
                matches = true;
            }

            if !matches && !self.neg_bymonthday.is_empty() {
                let total_days = last_day_of_month(target.year(), target.month()) as i32;
                let target_neg = (target.day() as i32) - total_days - 1;

                if self.neg_bymonthday.iter().any(|&d| d as i32 == target_neg) {
                    matches = true;
                }
            }

            if !matches {
                return false;
            }
        }

        // BYWEEKNO check
        if self.byweekno_mask != 0 || !self.neg_byweekno.is_empty() {
            let mut matches = false;
            let week_obj = target.iso_week();
            let w_num = week_obj.week();

            if (self.byweekno_mask & (1 << (w_num - 1))) != 0 {
                matches = true;
            }

            if !matches && !self.neg_byweekno.is_empty() {
                let total_weeks = Self::weeks_in_year(week_obj.year());
                let w_neg = (w_num as i32) - (total_weeks as i32) - 1;

                if self.neg_byweekno.iter().any(|&w| w as i32 == w_neg) {
                    matches = true;
                }

                if !matches {
                    return false;
                }
            }
        }

        if !self.neg_byyearday.is_empty() || self.byyearday_mask.iter().any(|&m| m != 0) {
            let mut matches = false;

            let y_day = target.ordinal();

            let idx = ((y_day - 1) / 64) as usize;
            let bit = (y_day - 1) % 64;
            if self.byyearday_mask[idx] & (1 << bit) != 0 {
                matches = true;
            }

            if !matches && !self.neg_byyearday.is_empty() {
                let total_year_days = if Self::is_leap_year(target.year()) {
                    366
                } else {
                    365
                };

                let y_neg = (y_day as i32) - total_year_days - 1;
                if self.neg_byyearday.iter().any(|&d| d as i32 == y_neg) {
                    matches = true;
                }

                if !matches {
                    return false;
                }
            }
        }

        // Frequency Interval Logic
        match self.freq {
            Freq::Daily => {
                let diff = (*target - *dt_start).num_days();
                diff >= 0 && diff % self.interval == 0
            }
            Freq::Weekly => {
                let s_monday =
                    dt_start.num_days_from_ce() - dt_start.weekday().num_days_from_monday() as i32;
                let t_monday =
                    target.num_days_from_ce() - target.weekday().num_days_from_monday() as i32;
                ((t_monday - s_monday) / 7) % self.interval as i32 == 0
            }
            Freq::Monthly => {
                let months_since = (target.year() - dt_start.year()) * 12
                    + (target.month() as i32 - dt_start.month() as i32);
                months_since >= 0 && months_since as i64 % self.interval == 0
            }
            Freq::Yearly => {
                let years_since = target.year() - dt_start.year();
                years_since >= 0 && years_since as i64 % self.interval == 0
            }
        }
    }

    #[inline(always)]
    fn weekday_to_mask(wd: Weekday) -> u8 {
        // Sets 1 at index of u8
        // 1 << 0 => 00000001 Monday
        // 1 << 3 => 00001000 Thursday
        1 << (wd.num_days_from_monday())
    }
    #[inline(always)]
    fn matches_day(&self, day: Weekday) -> bool {
        // Bitwise check:
        //   00000101  (Mask for MO and WE)
        // & 00000010  (Bit for Tuesday)
        // ----------
        //   00000000  (Result is 0) -> FALSE
        //
        //   00000101  (Mask for MO and WE)
        // & 00000100  (Bit for Wednesday)
        // ----------
        //   00000100  (Result is 4, which is != 0) -> TRUE

        (self.byday_mask & Self::weekday_to_mask(day)) != 0
    }
    #[inline(always)]
    fn weeks_in_year(iso_year: i32) -> u32 {
        NaiveDate::from_ymd_opt(iso_year, 12, 28)
            .unwrap()
            .iso_week()
            .week()
    }
    #[inline(always)]
    fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}
