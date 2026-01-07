use std::{collections::HashMap, io::Cursor, rc::Rc};

use ical::IcalParser;
use quick_xml::{Reader, events::Event};
use reqwest::{
    Client,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};

use crate::{
    auth::{Credential, CredentialData},
    calendar::{
        icloud::protocol::PropfindRequest,
        utils::{CalDavEvent, CalendarInfo},
    },
    errors::{WatsonError, WatsonErrorKind},
    watson_err,
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
    data: CredentialData,
    principal: Option<String>,
}
impl PropfindInterface {
    pub fn new(credential: Credential) -> Self {
        let Credential { data, .. } = credential;

        let client = Client::new();

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );

        Self {
            client,
            headers,
            data,
            principal: None,
        }
    }
    pub async fn make_request(&mut self, request: PropfindRequest) -> Result<String, WatsonError> {
        let mut headers = self.headers.clone();
        let params = request.params();
        headers.insert("Depth", HeaderValue::from_static(params.depth));
        let body = request.body();

        let resp = match &self.data {
            CredentialData::Password { username, secret } => self
                .client
                .request(
                    reqwest::Method::from_bytes(params.method).unwrap(),
                    params.url,
                )
                .basic_auth(&username, Some(&secret))
                .headers(headers)
                .body(body)
                .send()
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::HttpGetRequest, e.to_string()))?,
            CredentialData::OAuth { access_token, .. } => self
                .client
                .request(
                    reqwest::Method::from_bytes(params.method).unwrap(),
                    params.url,
                )
                .bearer_auth(access_token)
                .headers(headers)
                .body(body)
                .send()
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::HttpGetRequest, e.to_string()))?,
            CredentialData::Empty => {
                return Err(watson_err!(
                    WatsonErrorKind::UndefinedAttribute,
                    "Undefined credential data."
                ));
            }
        };

        let text = resp
            .text()
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::Deserialization, e.to_string()))?;

        Ok(text)
    }
    pub async fn get_principal(&mut self) -> Result<(), WatsonError> {
        let request = PropfindRequest::Principal;
        let text = self.make_request(request).await?;

        if text.is_empty() {
            return Err(watson_err!(
                WatsonErrorKind::HttpGetRequest,
                "Request parameters are wrong."
            ));
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
    pub async fn get_calendars(&mut self) -> Result<Vec<CalendarInfo>, WatsonError> {
        let Some(principal) = &self.principal else {
            return Err(watson_err!(
                WatsonErrorKind::UndefinedAttribute,
                "Principal is not defined.Principal is not defined."
            ));
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
                            calendars.push(CalendarInfo {
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
        calendar_info: Vec<CalendarInfo>,
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

fn parse_ical(ics: String, calendar_info: Rc<CalendarInfo>) -> Vec<CalDavEvent> {
    let parser = IcalParser::new(Cursor::new(&ics));
    let mut out = Vec::new();

    for calendar in parser {
        let calendar = match calendar {
            Ok(c) => c,
            Err(_) => continue,
        };

        for event in calendar.events {
            match CalDavEvent::try_from(event) {
                Ok(mut ev) => {
                    ev.calendar_info = calendar_info.clone();
                    out.push(ev);
                }
                Err(e) => {
                    eprint!("{:?}", e)
                }
            }
        }
    }

    out
}
