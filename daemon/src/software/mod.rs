use crate::software::calendar::CalendarBackend;

mod calendar;

pub struct SoftwareController {
    pub events: CalendarBackend,
}

impl SoftwareController {
    pub async fn new() -> Self {
        let mut events = CalendarBackend::new();
        let _ = events.fetch_for_today().await;
        Self { events }
    }
}
