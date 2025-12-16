pub enum PropfindRequest {
    Principal,
    Calendars { principal: String },
    Events { url: String },
}
pub struct PropfindParams {
    pub url: String,
    pub depth: &'static str,
    pub method: &'static [u8],
}
impl PropfindRequest {
    pub fn params(&self) -> PropfindParams {
        match self {
            Self::Principal => PropfindParams {
                url: "https://caldav.icloud.com/".into(),
                depth: "0",
                method: b"PROPFIND",
            },
            Self::Calendars { principal } => PropfindParams {
                url: format!("https://caldav.icloud.com/{}/calendars", principal),
                depth: "1",
                method: b"PROPFIND",
            },
            Self::Events { url } => PropfindParams {
                url: format!("https://caldav.icloud.com{}", url),
                depth: "1",
                method: b"REPORT",
            },
        }
    }
    pub fn body(&self) -> &'static str {
        match self {
            Self::Principal => {
                r#"
                <d:propfind xmlns:d="DAV:">
                    <d:prop>
                        <d:current-user-principal/>
                    </d:prop>
                </d:propfind>
                "#
            }
            Self::Calendars { .. } => {
                r#"
                <propfind xmlns="DAV:" xmlns:cs="http://calendarserver.org/ns/" xmlns:apple="http://apple.com/ns/ical/">
                  <prop>
                    <displayname/>
                    <resourcetype/>
                    <apple:calendar-color/>
                  </prop>
                </propfind>
                "#
            }
            Self::Events { .. } => {
                r#"
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
                "#
            }
        }
    }
}
