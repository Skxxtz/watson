use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    rc::Rc,
};

use chrono::{DateTime, Datelike, Duration, Utc, Weekday};
use ical::{IcalParser, parser::ical::component::IcalEvent};
use quick_xml::{Reader, events::Event};
use reqwest::{
    Client,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};

use crate::{
    auth::Credential,
    calendar::{
        icloud::protocol::PropfindRequest,
        utils::{
            funcs::{last_day_of_month, parse_exdate, parse_rdate, parse_until, parse_utc},
            structs::{Attendee, DateTimeSpec, RecurrenceRule},
        },
    },
    errors::{WatsonError, WatsonErrorType},
};

// Go to icloud.com
// Signin
// Click your profile picture
// Click "Manage Apple Account →"
// Go to "Sign-In and Security"
// Click "App-Specific Passwords"
// Create a new password (name it watson or so)
//

pub struct PropfindInterface {
    client: Client,
    headers: HeaderMap,
    username: String,
    password: String,
    principal: Option<String>,
}
impl PropfindInterface {
    pub fn new(credential: Credential) -> Self {
        let Credential {
            username,
            secret: password,
            ..
        } = credential;

        let client = Client::new();

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );

        Self {
            client,
            headers,
            username,
            password,
            principal: None,
        }
    }
    pub async fn make_request(&mut self, request: PropfindRequest) -> Result<String, WatsonError> {
        let mut headers = self.headers.clone();
        let params = request.params();
        headers.insert("Depth", HeaderValue::from_static(params.depth));
        let body = request.body();

        let resp = self
            .client
            .request(
                reqwest::Method::from_bytes(params.method).unwrap(),
                params.url,
            )
            .basic_auth(&self.username, Some(&self.password))
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|e| WatsonError {
                r#type: WatsonErrorType::HttpGetRequest,
                error: e.to_string(),
            })?;

        let text = resp.text().await.map_err(|e| WatsonError {
            r#type: WatsonErrorType::Deserialization,
            error: e.to_string(),
        })?;

        Ok(text)
    }
    pub async fn get_principal(&mut self) -> Result<(), WatsonError> {
        let request = PropfindRequest::Principal;
        let text = self.make_request(request).await?;

        if text.is_empty() {
            return Err(WatsonError {
                r#type: WatsonErrorType::HttpGetRequest,
                error: "Request parameters are wrong.".into(),
            });
        }

        // Read Principal
        let mut reader = Reader::from_str(&text);
        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"current-user-principal" => {
                    if let Ok(Event::Start(ref e2)) = reader.read_event() {
                        if e2.name().as_ref() == b"href" {
                            if let Ok(Event::Text(e_text)) = reader.read_event() {
                                self.principal = e_text
                                    .decode()
                                    .ok()
                                    .map(|s| {
                                        s.trim_start_matches('/')
                                            .split('/')
                                            .next()
                                            .map(|s| s.to_string())
                                    })
                                    .flatten();
                            }
                        }
                    }
                    break;
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => (),
            }
        }
        Ok(())
    }
    pub async fn get_calendars(&mut self) -> Result<Vec<IcloudCalendarInfo>, WatsonError> {
        let Some(principal) = &self.principal else {
            return Err(WatsonError {
                r#type: WatsonErrorType::UndefinedAttribute,
                error: "Principal is not defined.Principal is not defined.".into(),
            });
        };
        let request = PropfindRequest::Calendars {
            principal: principal.to_string(),
        };
        let text = self.make_request(request).await?;

        let mut reader = Reader::from_str(&text);
        let mut buf = Vec::new();
        let mut calendars = Vec::new();
        let mut current_href = None;
        let mut current_name = None;
        let mut color = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"response" => {
                        current_href = None;
                        current_name = None;
                    }
                    b"href" => {
                        if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                            current_href = Some(t.decode().unwrap().to_string());
                        }
                    }
                    b"displayname" => {
                        if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                            current_name = Some(t.decode().unwrap().to_string());
                        }
                    }
                    b"calendar-color" => {
                        if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                            color = Some(t.decode().unwrap().to_string());
                        }
                    }
                    _ => {}
                },
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == b"response" {
                        if let (Some(href), Some(name)) = (current_href.take(), current_name.take())
                        {
                            calendars.push(IcloudCalendarInfo {
                                href,
                                name,
                                color: color.take(),
                            });
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    eprintln!("Error parsing XML: {:?}", e);
                    break;
                }
                _ => {}
            }
            buf.clear();
        }

        Ok(calendars)
    }

    pub async fn get_events(
        &mut self,
        calendar_info: Vec<IcloudCalendarInfo>,
    ) -> Result<Vec<CalDavEvent>, WatsonError> {
        let mut out = Vec::new();
        for info in calendar_info {
            let request = PropfindRequest::Events {
                url: info.href.clone(),
            };
            let info = Rc::new(info);
            let text = self.make_request(request).await?;

            let mut reader = Reader::from_str(&text);
            let mut buf = Vec::new();
            let mut data_buf = HashMap::new();

            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(ref e)) => match e.name().as_ref() {
                        b"calendar-data" => {
                            if let Ok(Event::CData(t)) = reader.read_event_into(&mut buf) {
                                data_buf.insert(
                                    "calendar-data".to_string(),
                                    t.decode().unwrap().to_string(),
                                );
                            }
                        }
                        _ => {}
                    },
                    Ok(Event::End(ref e)) => {
                        match e.name().as_ref() {
                            b"calendar-data" => {
                                if let Some(ics) = data_buf.remove("calendar-data") {
                                    // Clean up ics
                                    let ics = unfold_ics(&ics);
                                    let events = parse_ical(ics, info.clone());
                                    out.extend(events);
                                }
                                data_buf = HashMap::new();
                            }
                            _ => {}
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }

        Ok(out)
    }
}

fn unfold_ics(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }

            match chars.peek() {
                Some(' ') | Some('\t') => {
                    chars.next(); // swallow folding whitespace
                }
                _ => out.push('\n'),
            }
        } else if c == '\n' {
            match chars.peek() {
                Some(' ') | Some('\t') => {
                    chars.next(); // swallow folding whitespace
                }
                _ => out.push('\n'),
            }
        } else {
            out.push(c);
        }
    }

    out
}

fn parse_ical(ics: String, calendar_info: Rc<IcloudCalendarInfo>) -> Vec<CalDavEvent> {
    let parser = IcalParser::new(Cursor::new(&ics));
    let mut out = Vec::new();

    for calendar in parser {
        let calendar = match calendar {
            Ok(c) => c,
            Err(_) => continue,
        };

        for event in calendar.events {
            if let Ok(mut ev) = CalDavEvent::try_from(event) {
                ev.calendar_info = calendar_info.clone();
                out.push(ev);
            }
        }
    }

    out
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct IcloudCalendarInfo {
    href: String,
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

    pub calendar_info: Rc<IcloudCalendarInfo>,

    pub rule_expired: bool,

    pub event_type: CalEventType,
}
impl TryFrom<IcalEvent> for CalDavEvent {
    type Error = WatsonError;
    fn try_from(value: IcalEvent) -> Result<Self, Self::Error> {
        let mut out = Self::default();

        for prop in value.properties {
            match prop.name.as_str() {
                "UID" => {
                    out.uid = prop.value.ok_or_else(|| WatsonError {
                        r#type: WatsonErrorType::Deserialization,
                        error: "ICalEvent missing UID".into(),
                    })?;
                }

                "SUMMARY" => out.summary = prop.value,
                "DESCRIPTION" => out.description = prop.value,
                "LOCATION" => out.location = prop.value,

                "DTSTART" => out.start = DateTimeSpec::try_from(prop).ok(),
                "DTEND" => out.end = DateTimeSpec::try_from(prop).ok(),

                "RECURRENCE-ID" => {
                    out.recurrence_id = DateTimeSpec::try_from(prop).ok();
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
            return Err(WatsonError {
                r#type: WatsonErrorType::Deserialization,
                error: "Failed to deserialize ICalEvent into CalDavEvent. (Missing start event)"
                    .into(),
            });
        };

        if out.uid.is_empty() {
            Err(WatsonError {
                r#type: WatsonErrorType::Deserialization,
                error: "Failed to deserialize ICalEvent into CalDavEvent. (Empty UID)".into(),
            })
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

        if self.recurrence.is_some() {
            self.handle_recurrence(day)
        } else {
            let (start_day, end_day) = match (start, self.end.as_ref()) {
                (DateTimeSpec::Date(s), Some(DateTimeSpec::Date(e))) => {
                    (*s, *e - Duration::days(-1))
                }
                (DateTimeSpec::Date(s), None) => (*s, *s),
                (
                    DateTimeSpec::DateTime { value, .. },
                    Some(DateTimeSpec::DateTime { value: e, .. }),
                ) => (value.date(), (*e - Duration::seconds(1)).date()),
                (DateTimeSpec::DateTime { value, .. }, None) => (value.date(), value.date()),
                _ => return false,
            };
            let day = day.date_naive();
            day >= start_day && day <= end_day
        }
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

        let todate = target.date_naive();
        // If date is explicitely named, return true
        if self
            .rdates
            .iter()
            .any(|d| d.utc_time().date_naive() == todate)
        {
            return true;
        }
        // If date is explicitely named, return false
        if self
            .exdates
            .iter()
            .any(|d| d.utc_time().date_naive() == todate)
        {
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
                    let start_of_week = dt_start.date_naive()
                        - Duration::days(dt_start.weekday().num_days_from_monday() as i64);
                    let target_start_of_week = target.date_naive()
                        - Duration::days(target.weekday().num_days_from_monday() as i64);

                    let weeks_since = (target_start_of_week - start_of_week).num_days() / 7;

                    if (weeks_since as i64).abs() % interval != 0 {
                        return false;
                    }

                    if map.get("BYDAY").is_none() {
                        if target.weekday() != dt_start.weekday() {
                            return false;
                        }
                    }

                    return true;
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
