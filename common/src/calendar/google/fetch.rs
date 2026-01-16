use std::{cell::Cell, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::{
    auth::{Credential, CredentialData},
    calendar::{
        google::auth::GoogleAuth,
        protocol::CalendarProvider,
        utils::{
            CalDavEvent, CalEventType, CalendarInfo,
            structs::{Attendee, DateTimeSpec, RecurrenceRule},
        },
    },
    utils::errors::{WatsonError, WatsonErrorKind},
    watson_err,
};

//--------------------
//------ Errors ------
//--------------------
#[derive(Deserialize)]
pub struct GoogleApiErrorResponse {
    pub error: GoogleError,
}

#[derive(Deserialize)]
pub struct GoogleError {
    message: String,
}

//--------------------
//---- Calendars -----
//--------------------
#[derive(Deserialize)]
struct GoogleCalendarList {
    pub items: Vec<GoogleCalendarEntry>,
}

#[derive(Deserialize)]
struct GoogleCalendarEntry {
    pub id: String,
    pub summary: String,

    #[serde(rename = "timeZone")]
    pub _time_zone: Option<String>,

    #[serde(rename = "backgroundColor")]
    pub color: Option<String>,
}
impl From<GoogleCalendarEntry> for CalendarInfo {
    fn from(value: GoogleCalendarEntry) -> Self {
        Self {
            href: value.id,
            name: value.summary,
            color: value.color,
        }
    }
}

//--------------------
//------ Events ------
//--------------------
#[derive(Deserialize)]
struct GoogleCalendarEventList {
    pub items: Vec<GoogleCalendarEvent>,
}

#[derive(Debug, Deserialize)]
pub struct GoogleCalendarEvent {
    pub id: String,

    #[serde(rename = "summary")]
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,

    pub start: Option<GoogleEventDateTime>,
    pub end: Option<GoogleEventDateTime>,

    pub recurrence: Option<Vec<String>>,
    #[serde(rename = "updated")]
    pub last_modified: Option<DateTime<Utc>>,

    pub organizer: Option<GoogleEventUser>,
    pub attendees: Option<Vec<GoogleEventUser>>,
}
impl GoogleCalendarEvent {
    fn to_cal_dav_event(self, calendar_info: Arc<CalendarInfo>) -> CalDavEvent {
        let start = self.start.map(|v| v.into());
        let end = self.end.map(|v| v.into());
        let event_type = match (start.as_ref(), end.as_ref()) {
            (Some(DateTimeSpec::DateTime { .. }), Some(DateTimeSpec::DateTime { .. })) => {
                CalEventType::Timed
            }
            _ => CalEventType::AllDay,
        };
        CalDavEvent {
            uid: self.id,
            title: self.title.unwrap_or("Untitled Event".into()),
            description: self.description,
            location: self.location,
            start,
            end,
            recurrence: self
                .recurrence
                .and_then(|v| v.into_iter().next())
                .map(RecurrenceRule::new),
            recurrence_id: None,
            rdates: Vec::new(),
            exdates: Vec::new(),
            last_modified: self.last_modified,
            sequence: None,
            url: None,
            organizer: self.organizer.and_then(|v| v.display_name),
            attendees: self
                .attendees
                .map(|a| a.into_iter().map(|v| v.into()).collect())
                .unwrap_or_default(),
            calendar_info,
            event_type,
            seen: Cell::new(false),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum GoogleEventDateTime {
    DateTime {
        #[serde(rename = "dateTime")]
        date_time: DateTime<FixedOffset>,
    },
    Date {
        date: NaiveDate,
    },
}
impl From<GoogleEventDateTime> for DateTimeSpec {
    fn from(v: GoogleEventDateTime) -> Self {
        match v {
            GoogleEventDateTime::Date { date } => Self::Date(date),
            GoogleEventDateTime::DateTime { date_time } => Self::DateTime {
                value: date_time.to_utc(),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GoogleEventUser {
    pub email: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub organizer: Option<bool>,
    #[serde(rename = "responseStatus")]
    pub partstat: Option<String>,
}
impl From<GoogleEventUser> for Attendee {
    fn from(v: GoogleEventUser) -> Self {
        let role = if v.organizer.unwrap_or(false) {
            Some("Organizer".to_string())
        } else {
            Some("Attendee".to_string())
        };
        Self {
            email: Some(v.email),
            display_name: v.display_name,
            role,
            partstat: v.partstat,
        }
    }
}

//--------------------
//------ Client ------
//--------------------
#[derive(Debug, Clone)]
pub struct GoogleCalendarClient {
    client: Client,
    credential: Credential,
}
impl GoogleCalendarClient {
    pub fn new(credential: Credential) -> Self {
        Self {
            client: Client::new(),
            credential,
        }
    }
}

#[async_trait]
impl CalendarProvider for GoogleCalendarClient {
    async fn init(&mut self) -> Result<(), WatsonError> {
        Ok(())
    }
    async fn refresh(&mut self) -> Result<(), WatsonError> {
        if let CredentialData::OAuth {
            expires_at,
            refresh_token,
            access_token,
            ..
        } = &mut self.credential.data
        {
            if *expires_at <= Utc::now().timestamp() + 120 {
                let new_token = GoogleAuth::refresh_credential(&refresh_token.take()).await?;
                *access_token = crate::auth::CredentialSecret::Decrypted(new_token);
                *expires_at = Utc::now().timestamp() + 3600;
                self.credential.save()?;
            }
        }
        Ok(())
    }

    async fn get_calendars(&mut self) -> Result<Vec<CalendarInfo>, WatsonError> {
        self.refresh().await?;

        let CredentialData::OAuth { access_token, .. } = &self.credential.data else {
            return Err(watson_err!(
                WatsonErrorKind::GoogleAuth,
                "Invalid auth type provided."
            ));
        };

        let url = "https://www.googleapis.com/calendar/v3/users/me/calendarList";
        let resp = self
            .client
            .get(url)
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::HttpGetRequest, e.to_string()))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::Deserialize, e.to_string()))?;

        if !status.is_success() {
            let error: GoogleApiErrorResponse = serde_json::from_str(&text)
                .map_err(|e| watson_err!(WatsonErrorKind::Deserialize, e.to_string()))?;
            return Err(watson_err!(
                WatsonErrorKind::GoogleCalendar,
                error.error.message
            ));
        }

        let list: GoogleCalendarList = serde_json::from_str(&text)
            .map_err(|e| watson_err!(WatsonErrorKind::Deserialize, e.to_string()))?;

        Ok(list.items.into_iter().map(|i| i.into()).collect())
    }

    async fn get_events(
        &mut self,
        calendars: Vec<CalendarInfo>,
    ) -> Result<Vec<CalDavEvent>, WatsonError> {
        self.refresh().await?;

        let mut events = Vec::new();

        let CredentialData::OAuth { access_token, .. } = &self.credential.data else {
            return Err(watson_err!(
                WatsonErrorKind::GoogleAuth,
                "Invalid auth type provided."
            ));
        };

        for calendar in calendars {
            let url = format!(
                "https://www.googleapis.com/calendar/v3/calendars/{}/events",
                calendar.href
            );

            let resp = self
                .client
                .get(&url)
                .bearer_auth(access_token)
                .send()
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::HttpGetRequest, e.to_string()))?;

            let status = resp.status();
            let text = resp
                .text()
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::Deserialize, e.to_string()))?;

            if !status.is_success() {
                continue;
            }

            let calendar_rc = Arc::new(calendar);
            let tmp_events: Vec<CalDavEvent> =
                serde_json::from_str::<GoogleCalendarEventList>(&text)
                    .map_err(|e| watson_err!(WatsonErrorKind::Deserialize, e.to_string()))?
                    .items
                    .into_iter()
                    .map(|v| v.to_cal_dav_event(calendar_rc.clone()))
                    .collect();

            events.extend(tmp_events);
        }

        Ok(events)
    }
}
