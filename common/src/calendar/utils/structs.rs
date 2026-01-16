use chrono::{
    DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc, offset::LocalResult,
};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::{
    utils::errors::{WatsonError, WatsonErrorKind},
    watson_err,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DateTimeSpec {
    Date(NaiveDate),
    DateTime { value: DateTime<Utc> },
}
impl DateTimeSpec {
    pub fn utc_time(&self) -> DateTime<Utc> {
        match self {
            Self::Date(d) => Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap()),
            Self::DateTime { value, .. } => value.clone(),
        }
    }
    pub fn local(&self) -> DateTime<Local> {
        match self {
            Self::Date(d) => {
                let naive = d.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap());

                match Local.from_local_datetime(&naive) {
                    LocalResult::Single(dt) => dt,
                    LocalResult::Ambiguous(a, _) => a,
                    LocalResult::None => Local.from_utc_datetime(&naive),
                }
            }

            Self::DateTime { value, .. } => value.with_timezone(&Local),
        }
    }
}
pub struct InvalidDateTimeSpec;
impl TryFrom<ical::property::Property> for DateTimeSpec {
    type Error = WatsonError;
    fn try_from(value: ical::property::Property) -> Result<Self, Self::Error> {
        let Some(inner) = value.value.as_ref() else {
            return Err(watson_err!(
                WatsonErrorKind::InvalidAttribute,
                "Cannot parse DateTimeSpec: property `{}` is missing a value.",
                value.name
            ));
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
            // It's a date-only value: "YYYYMMDD"
            Ok(DateTimeSpec::Date(
                NaiveDate::parse_from_str(inner, "%Y%m%d").map_err(|e| {
                    watson_err!(
                        WatsonErrorKind::DateParse,
                        "Failed to parse NaiveDate from `{inner}` using `%Y%m%d` format. Error: {}",
                        e.to_string()
                    )
                })?,
            ))
        } else {
            // It's a date-time value: "YYYYMMDDTHHMMSS"
            let is_utc = inner.ends_with('Z');
            let inner = inner.strip_suffix('Z').unwrap_or(inner);

            let naive = NaiveDateTime::parse_from_str(inner, "%Y%m%dT%H%M%S")
                .map_err(|e| watson_err!(
                    WatsonErrorKind::DateParse,
                    "Failed to parse NaiveDateTime from `{inner}` using `%Y%m%dT%H%M%S` format. Error: {}",
                    e.to_string()
                ))?;

            let dt_utc = if is_utc {
                Utc.from_utc_datetime(&naive)
            } else if let Some(tzid) = &tzid {
                let tzid = windows_to_iana(&tzid);
                let tz: Tz = tzid.parse().map_err(|_| {
                    watson_err!(
                        WatsonErrorKind::InvalidAttribute,
                        "Failed to parse TZID `{}` into a valid timezone",
                        tzid
                    )
                })?;

                tz.from_local_datetime(&naive)
                    .single()
                    .ok_or(watson_err!(
                        WatsonErrorKind::InvalidAttribute,
                        "Ambiguous or non-existent local datetime `{}` in timezone `{}`",
                        naive,
                        tzid
                    ))?
                    .with_timezone(&Utc)
            } else {
                Utc.from_utc_datetime(&naive)
            };

            Ok(DateTimeSpec::DateTime { value: dt_utc })
        }
    }
}

fn windows_to_iana(tzid: &str) -> String {
    match tzid {
        "W. Europe Standard Time" => "Europe/Berlin".into(),
        "Central European Standard Time" => "Europe/Paris".into(),
        "Eastern Standard Time" => "America/New_York".into(),
        _ => tzid.into(),
    }
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Attendee {
    pub email: Option<String>,
    #[serde(rename = "cn")]
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub partstat: Option<String>,
}
impl Attendee {
    pub fn is_valid(&self) -> bool {
        self.email.is_some()
    }
}
pub struct InvalidAttendee;
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
            display_name: cn,
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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecurrenceRule {
    // e.g. "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU"
    pub raw: String,
}
impl RecurrenceRule {
    pub fn new(raw: String) -> Self {
        Self { raw }
    }
}
