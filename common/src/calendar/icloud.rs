use std::{collections::HashMap, env, io::Cursor};

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use ical::{IcalParser, parser::ical::component::IcalEvent};
use quick_xml::{Reader, events::Event};
use reqwest::{Client, header::{CONTENT_TYPE, HeaderMap, HeaderValue}};

// Go to icloud.com
// Signin
// Click your profile picture
// Click "Manage Apple Account →"
// Go to "Sign-In and Security"
// Click "App-Specific Passwords"
// Create a new password (name it watson or so)
//
struct ICloudCalendarInterface {
    principal: String,
}
impl ICloudCalendarInterface {
    async fn new() -> Option<()> {
        let username = env::var("ICLOUD_USERNAME").unwrap_or_default();
        let password = env::var("ICLOUD_APP_SPECIFIC_PASSWORD").unwrap_or_default();

        if username.is_empty() || password.is_empty() {
            return None;
        }

        let client = Client::new();

        if let Ok(Some(principal)) = Self::get_principal(&client, &username, &password).await {
            let calendar_url = principal.replace("principal", "calendars");
            println!("{:?}", calendar_url);
            if let Ok(calendar_handles) =
                Self::get_calendars(&client, &username, &password, &calendar_url).await
            {
                let x = Self::get_calendar_events(&client, &username, &password, calendar_handles)
                    .await;
            }
        }

        Some(())
    }

    async fn get_principal(
        client: &Client,
        username: &str,
        password: &str,
    ) -> Result<Option<String>, String> {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );
        headers.insert("Depth", HeaderValue::from_static("0"));

        let url = "https://caldav.icloud.com/";
        let body = r#"
            <d:propfind xmlns:d="DAV:">
                <d:prop>
                    <d:current-user-principal/>
                </d:prop>
            </d:propfind>
        "#;

        let resp = client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), url)
            .basic_auth(username, Some(password))
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|e| "Failed to make request".to_string())?;

        let text = resp
            .text()
            .await
            .map_err(|e| "Failed to get response text".to_string())?;

        // Read Principal
        let mut reader = Reader::from_str(&text);
        let mut principal = None;
        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"current-user-principal" => {
                    if let Ok(Event::Start(ref e2)) = reader.read_event() {
                        if e2.name().as_ref() == b"href" {
                            if let Ok(Event::Text(e_text)) = reader.read_event() {
                                principal = e_text.decode().map(|s| s.to_string()).ok();
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

        Ok(principal)
    }

    async fn get_calendars(
        client: &Client,
        username: &str,
        password: &str,
        calendar_url: &str,
    ) -> Result<Vec<ICloudCalendarHandle>, Box<dyn std::error::Error>> {
        let url = format!("https://caldav.icloud.com{}", calendar_url);
        let body = r#"
        <propfind xmlns="DAV:" xmlns:cs="http://calendarserver.org/ns/">
          <prop>
            <displayname/>
            <resourcetype/>
          </prop>
        </propfind>
        "#;

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );
        headers.insert("Depth", HeaderValue::from_static("1"));

        let resp = client
            .request(reqwest::Method::from_bytes(b"PROPFIND")?, url)
            .basic_auth(username, Some(password))
            .headers(headers)
            .body(body)
            .send()
            .await?;

        // Logic to extract the
        let text = resp.text().await?;

        let mut reader = Reader::from_str(&text);
        let mut buf = Vec::new();
        let mut calendars = Vec::new();
        let mut current_href = None;
        let mut current_name = None;

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
                            println!("{:?}", current_href);
                        }
                    }
                    b"displayname" => {
                        if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                            current_name = Some(t.decode().unwrap().to_string());
                            println!("{:?}", current_name);
                        }
                    }
                    _ => {}
                },
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == b"response" {
                        if let (Some(href), Some(name)) = (&current_href, &current_name) {
                            calendars.push(ICloudCalendarHandle {
                                href: href.clone(),
                                name: name.clone(),
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

    async fn get_calendar_events(
        client: &Client,
        username: &str,
        password: &str,
        calendars: Vec<ICloudCalendarHandle>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for cal_handle in calendars {
            let url = format!("https://caldav.icloud.com{}", cal_handle.href);
            let body = r#"
                <calendar-query xmlns="urn:ietf:params:xml:ns:caldav" xmlns:D="DAV:">
                    <D:prop>
                        <D:getetag/>
                        <calendar-data/>
                    </D:prop>
                    <filter>
                        <comp-filter name="VCALENDAR">
                            <comp-filter name="VEVENT"/>
                        </comp-filter>
                    </filter>
                </calendar-query>
            "#;

            let mut headers = HeaderMap::new();
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static("application/xml; charset=utf-8"),
            );
            headers.insert("Depth", HeaderValue::from_static("1"));

            let resp = client
                .request(reqwest::Method::from_bytes(b"REPORT")?, url)
                .basic_auth(username, Some(password))
                .headers(headers)
                .body(body)
                .send()
                .await?;

            // Logic to extract the
            let text = resp.text().await?;
            let mut reader = Reader::from_str(&text);
            let mut buf = Vec::new();

            let mut data_buf = HashMap::new();
            let mut calendars = Vec::new();

            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(ref e)) => match e.name().as_ref() {
                        b"href" => {
                            if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                                data_buf
                                    .insert("href".to_string(), t.decode().unwrap().to_string());
                            }
                        }
                        b"getetag" => {
                            if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                                data_buf
                                    .insert("getetag".to_string(), t.decode().unwrap().to_string());
                            }
                        }
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
                                if let Some(href) = data_buf.remove("href") {
                                    if let Some(etag) = data_buf.remove("getetag") {
                                        if let Some(ics) = data_buf.remove("calendar-data") {
                                            // Clean up ics
                                            let ics = unfold_ics(&ics);
                                            let events = parse_ical(ics);
                                            calendars.push(CalDavCalendar { href, etag, events });
                                        }
                                    }
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

        Ok(())
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

fn parse_ical(ics: String) -> Vec<CalDavEvent> {
    let parser = IcalParser::new(Cursor::new(&ics));
    let mut out = Vec::new();

    for calendar in parser {
        let calendar = match calendar {
            Ok(c) => c,
            Err(_) => continue,
        };

        for event in calendar.events {
            if let Some(ev) = parse_event(event) {
                out.push(ev);
            }
        }
    }

    out
}

fn parse_event(event: IcalEvent) -> Option<CalDavEvent> {
    let mut out = CalDavEvent::default();

    for prop in event.properties {
        match prop.name.as_str() {
            "UID" => out.uid = prop.value?,

            "SUMMARY" => out.summary = prop.value,
            "DESCRIPTION" => out.description = prop.value,
            "LOCATION" => out.location = prop.value,

            "DTSTART" => out.start = DateTimeSpec::try_from(prop).ok(),
            "DTEND" => out.end = DateTimeSpec::try_from(prop).ok(),

            "RECURRENCE-ID" => {
                out.recurrence_id = DateTimeSpec::try_from(prop).ok();
            }

            "RRULE" => out.recurrence = prop.value.map(|v| RecurrenceRule::new(v)),

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

    if out.uid.is_empty() { None } else { Some(out) }
}

#[derive(Debug)]
struct ICloudCalendarHandle {
    href: String,
    name: String,
}

struct CalDavCalendar {
    href: String,
    etag: String,
    events: Vec<CalDavEvent>,
}

#[derive(Default)]
struct CalDavEvent {
    uid: String,

    summary: Option<String>,
    description: Option<String>,
    location: Option<String>,

    start: Option<DateTimeSpec>,
    end: Option<DateTimeSpec>,

    recurrence: Option<RecurrenceRule>,
    recurrence_id: Option<DateTimeSpec>,

    last_modified: Option<DateTime<Utc>>,
    sequence: Option<i32>,

    url: Option<String>,

    organizer: Option<String>,
    attendees: Vec<Attendee>,
}
impl CalDavEvent {
    pub fn is_today(&self) -> bool {

        true
    }
}

enum DateTimeSpec {
    Date(NaiveDate),
    DateTime {
        value: NaiveDateTime,
        tzid: Option<String>,
    },
}
struct InvalidDateTimeSpec;
impl TryFrom<ical::property::Property> for DateTimeSpec {
    type Error = InvalidDateTimeSpec;
    fn try_from(value: ical::property::Property) -> Result<Self, Self::Error> {
        let Some(inner) = value.value.as_ref() else {
            return Err(Self::Error {});
        };

        let mut tzid = None;
        if let Some(params) = value.params {
            for (k, v) in params {
                if k.as_str() == "TZID" {
                    tzid = v.into_iter().next();
                }
            }
        }

        if inner.len() == 8 {
            Ok(DateTimeSpec::Date(
                NaiveDate::parse_from_str(inner, "%Y%m%d").unwrap(),
            ))
        } else {
            Ok(DateTimeSpec::DateTime {
                value: NaiveDateTime::parse_from_str(inner, "%Y%m%d%T%H%M%S").unwrap(),
                tzid,
            })
        }
    }
}

#[derive(Default)]
struct Attendee {
    email: Option<String>,
    cn: Option<String>,
    role: Option<String>,
    partstat: Option<String>,
}
struct InvalidAttendee;
impl Attendee {
    pub fn is_valid(&self) -> bool {
        self.email.is_some()
    }
}
impl TryFrom<ical::property::Property> for Attendee {
    type Error = InvalidAttendee;
    fn try_from(value: ical::property::Property) -> Result<Self, Self::Error> {
        let mut cn = None;
        let mut role = None;
        let mut partstat = None;

        if let Some(params) = value.params {
            for (k, v) in params {
                match k.as_str() {
                    "CN" => cn = v.into_iter().next(),
                    "ROLE" => role = v.into_iter().next(),
                    "PARTSTAT" => partstat = v.into_iter().next(),
                    _ => {}
                }
            }
        }

        let attendee = Attendee {
            email: value.value,
            cn,
            role,
            partstat,
        };

        if attendee.is_valid() {
            Ok(attendee)
        } else {
            Err(Self::Error {})
        }
    }
}

#[derive(Default)]
struct RecurrenceRule {
    // e.g. "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU"
    raw: String,
}
impl RecurrenceRule {
    fn new(raw: String) -> Self {
        Self { raw }
    }
}

fn parse_utc(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_str(s, "%Y%m%d%T%H%M%S")
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}
