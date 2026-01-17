use std::str::FromStr;

use chrono::{Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use gtk4::{DrawingArea, prelude::WidgetExt};

use crate::{
    config::WidgetSpec,
    ui::widgets::{
        calendar::{
            cache::CalendarCache,
            types::{CalendarConfig, CalendarHMFormat},
        },
        utils::render::Rgba,
    },
};

pub struct CalendarContext {
    pub font: String,
    pub padding: f64,
    pub padding_top: f64,

    pub text: Rgba,
    pub accent: Rgba,

    pub inner_width: f64,
    pub inner_height: f64,
    pub line_offset: f64,

    pub todate: NaiveDate,
    pub window_start: NaiveDateTime,
    pub window_end: NaiveDateTime,
    pub hours_to_show: u32,
    pub hours_past: u8,
    pub total_seconds: f64,

    pub hm_format: Option<CalendarHMFormat>,

    pub cache: CalendarCache,
    pub needs_init: bool,
}
impl Default for CalendarContext {
    fn default() -> Self {
        Self {
            font: String::from("Sans"),
            padding: 0.0,
            padding_top: 0.0,
            text: Rgba::default(),
            accent: Rgba::default(),
            inner_width: 0.0,
            inner_height: 0.0,
            line_offset: 0.0,
            todate: Default::default(),
            window_start: Default::default(),
            window_end: Default::default(),
            hours_to_show: 8,
            hours_past: 4,
            total_seconds: 8.0 * 3600.0,
            hm_format: None,
            cache: CalendarCache::default(),
            needs_init: true,
        }
    }
}
impl CalendarContext {
    fn new_time_window(
        hours_to_show: u32,
        hours_past: u8,
    ) -> (NaiveDate, NaiveDateTime, NaiveDateTime) {
        let today = Local::now();
        let todate = today.date_naive();
        let now = today.time();

        // Determine window start/end
        let now_hour = now.hour();
        let start_hour = if now_hour + hours_to_show > 24 {
            24 - hours_to_show
        } else {
            now_hour.saturating_sub(hours_past as u32)
        }
        .min(23);

        let window_start = todate.and_time(NaiveTime::from_hms_opt(start_hour, 0, 0).unwrap());
        let window_end = window_start + Duration::hours(hours_to_show as i64);

        (todate, window_start, window_end)
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn for_specs(&mut self, spec: &WidgetSpec) {
        let CalendarConfig {
            accent_color,
            font,
            hm_format,
            hours_past,
            hours_future,
        } = spec.as_calendar();

        // Calculations
        self.accent = Rgba::from_str(accent_color).unwrap_or_default();
        self.font = font.to_string();
        self.hours_to_show = (hours_past + hours_future).clamp(1, 24) as u32;
        self.hours_past = hours_past;
        self.total_seconds = (self.hours_to_show * 3600) as f64;
        self.hm_format = hm_format.cloned();
    }
    pub fn update(&mut self, area: &DrawingArea, width: f64, height: f64, num_events: usize) {
        self.text = area.color().into();

        self.padding = (width as f64 * 0.05).min(20.0);
        self.padding_top = if num_events != 0 { 120.0 } else { 100.0 };
        self.inner_width = width - 2.0 * self.padding;
        self.inner_height = height - self.padding - self.padding_top;

        // Date Calulations
        let (todate, window_start, window_end) =
            Self::new_time_window(self.hours_to_show, self.hours_past);
        self.todate = todate;
        self.window_start = window_start;
        self.window_end = window_end;
        self.needs_init = false;
    }
    pub fn is_dirty(&self, width: f64, height: f64) -> bool {
        self.cache.hitboxes.is_empty()
            || self.cache.last_width != width
            || self.cache.last_height != height
            || self.cache.last_window_start != self.window_start
    }
}
