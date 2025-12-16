use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

#[derive(Debug, Clone, PartialEq)]
pub enum DateTimeSpec {
    Date(NaiveDate),
    DateTime {
        value: NaiveDateTime,
        tzid: Option<String>,
    },
}
impl DateTimeSpec {
    pub fn utc_time(&self) -> DateTime<Utc> {
        match self {
            Self::Date(d) => Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap()),
            Self::DateTime { value, tzid } => {
                let naive = *value;

                let tz: Tz = match tzid {
                    Some(tz_str) => tz_str.parse().unwrap_or(chrono_tz::UTC),
                    None => chrono_tz::UTC,
                };

                let dt_with_tz = tz.from_local_datetime(&naive).single().unwrap_or_else(|| {
                    tz.with_ymd_and_hms(
                        naive.year(),
                        naive.month(),
                        naive.day(),
                        naive.hour(),
                        naive.minute(),
                        naive.second(),
                    )
                    .unwrap()
                });
                dt_with_tz.with_timezone(&Utc)
            }
        }
    }
}
pub struct InvalidDateTimeSpec;
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
                value: NaiveDateTime::parse_from_str(inner, "%Y%m%dT%H%M%S").unwrap(),
                tzid,
            })
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Attendee {
    pub email: Option<String>,
    pub cn: Option<String>,
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

#[derive(Default, Debug, Clone, PartialEq)]
pub struct RecurrenceRule {
    // e.g. "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU"
    pub raw: String,
}
impl RecurrenceRule {
    pub fn new(raw: String) -> Self {
        Self { raw }
    }
}
