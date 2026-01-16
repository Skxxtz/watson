use crate::ui::{
    g_templates::event_details::EventDetails,
    widgets::calendar::{context::CalendarContext, types::EventHitbox},
};

use gtk4::{DrawingArea, Stack, glib::WeakRef};

mod builder;
mod cache;
mod context;
mod data_store;
mod renderer;
pub mod types;

pub use builder::CalendarBuilder;
use renderer::CalendarRenderer;

#[derive(Debug, Clone)]
pub struct Calendar {
    pub area: WeakRef<DrawingArea>,
    pub stack: WeakRef<Stack>,
    pub details: WeakRef<EventDetails>,
}
impl Calendar {
    pub fn builder() -> CalendarBuilder {
        CalendarBuilder::new()
    }
}
