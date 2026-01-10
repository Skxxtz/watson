use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use quick_xml::{Reader, events::Event};
use reqwest::{
    Client,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};

use crate::{
    auth::{Credential, CredentialData},
    calendar::{
        icloud::{
            protocol::PropfindRequest,
            utils::{parse_ical, unfold_ics},
        },
        protocol::CalendarProvider,
        utils::{CalDavEvent, CalendarInfo},
    },
    errors::{WatsonError, WatsonErrorKind},
    watson_err,
};

pub struct ICloudCalendarClient {
    client: Client,
    headers: HeaderMap,
    data: CredentialData,
    principal: Option<String>,
}
impl Default for ICloudCalendarClient {
    fn default() -> Self {
        let client = Client::new();

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );

        Self {
            client,
            headers,
            data: CredentialData::Empty,
            principal: None,
        }
    }
}
impl ICloudCalendarClient {
    pub fn new(credential: Credential) -> Self {
        let mut obj = Self::default();
        let Credential { data, .. } = credential;

        obj.data = data;
        obj
    }
    pub async fn make_request(&mut self, request: PropfindRequest) -> Result<String, WatsonError> {
        let mut headers = self.headers.clone();
        let params = request.params();
        headers.insert("Depth", HeaderValue::from_static(params.depth));
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("text/xml; charset=utf-8"),
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        );

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
}

#[async_trait]
impl CalendarProvider for ICloudCalendarClient {
    async fn init(&mut self) -> Result<(), WatsonError> {
        self.get_principal().await
    }
    async fn refresh(&mut self) -> Result<(), WatsonError> {
        Ok(())
    }
    async fn get_calendars(&mut self) -> Result<Vec<CalendarInfo>, WatsonError> {
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

    async fn get_events(
        &mut self,
        calendar_info: Vec<CalendarInfo>,
    ) -> Result<Vec<CalDavEvent>, WatsonError> {
        let mut out = Vec::new();
        for info in calendar_info {
            let request = PropfindRequest::Events {
                url: info.href.clone(),
            };
            let info = Arc::new(info);
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
