use crate::{
    calendar::utils::{CalDavEvent, CalendarInfo},
    utils::errors::WatsonError,
};
use async_trait::async_trait;

#[async_trait]
pub trait CalendarProvider {
    // Init required parameters
    async fn init(&mut self) -> Result<(), WatsonError>;

    /// Refresh credentials / sessions if needed
    async fn refresh(&mut self) -> Result<(), WatsonError>;

    /// Retrieve all available calendars
    async fn get_calendars(&mut self) -> Result<Vec<CalendarInfo>, WatsonError>;

    /// Retrieve events for given calendars
    async fn get_events(
        &mut self,
        calendars: Vec<CalendarInfo>,
    ) -> Result<Vec<CalDavEvent>, WatsonError>;
}
