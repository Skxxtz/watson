use std::{cell::RefCell, rc::Rc};

use chrono::{Local, NaiveTime, Timelike};
use gtk4::{
    DrawingArea, cairo::{Context, LinearGradient}, glib::object::ObjectExt, prelude::{DrawingAreaExtManual, WidgetExt}
};

use crate::ui::widgets::utils::{CairoShapesExt, Conversions};

pub struct CalendarEvent {
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub label: &'static str,
}

pub struct Calendar {
    events: Rc<RefCell<Vec<CalendarEvent>>>,
}
impl Calendar {
    pub fn new(events: Vec<CalendarEvent>) -> (DrawingArea, Self) {
        let calendar_area = DrawingArea::builder()
            .vexpand(false)
            .hexpand(false)
            .valign(gtk4::Align::Start)
            .css_classes(["widget", "calendar"])
            .build();
        calendar_area.set_size_request(400, 400);

        let events = Rc::new(RefCell::new(events));

        // Draw function
        calendar_area.set_draw_func({
            let events = Rc::clone(&events);
            move |area, ctx, width, height| {
                Calendar::draw(area, ctx, width, height, &events.borrow());
            }
        });

        // Minute interval redraw
        gtk4::glib::timeout_add_seconds_local(60, {
            let calendar_ref = calendar_area.downgrade();
            move || {
                if let Some(cal) = calendar_ref.upgrade() {
                    cal.queue_draw();
                }
                gtk4::glib::ControlFlow::Continue
            }
        });


        (calendar_area, Self { events })
    }
    pub fn draw(
        _area: &DrawingArea,
        ctx: &Context,
        width: i32,
        height: i32,
        events: &[CalendarEvent],
    ) {
        // Create timeline
        let hours_to_show = 8;
        let now = Local::now().time();

        // Tentative start/end centered on now
        let now_hour = now.hour();
        let window_start;
        let window_end;
        if now_hour + hours_to_show > 24 {
            // Window extends past the end of the day
            window_start = NaiveTime::from_hms_opt(24 - hours_to_show, 0, 0).unwrap();
            window_end = NaiveTime::from_hms_opt(23, 59, 59).unwrap();
        } else if now_hour < hours_to_show {
            // Window starts at the beginning of the day
            window_start = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
            window_end = NaiveTime::from_hms_opt(hours_to_show, 0, 0).unwrap();
        } else {
            // Normal sliding window
            window_start = NaiveTime::from_hms_opt(now_hour - hours_to_show / 2, 0, 0).unwrap();
            window_end = NaiveTime::from_hms_opt(now_hour + hours_to_show / 2, 0, 0).unwrap();
        }
        let total_seconds = (window_end - window_start).num_seconds() as f64;

        // Initialize the area and frame
        let padding = (width as f64 * 0.04).max(20.0);
        let inner_width = width as f64 - 2.0 * padding;
        let inner_height = height as f64 - 2.0 * padding;

        // Background
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        ctx.set_operator(gtk4::cairo::Operator::Source);
        ctx.paint().unwrap();

        CairoShapesExt::rounded_rectangle(&ctx, 0.0, 0.0, width as f64, height as f64, 20.0);
        let gradient = LinearGradient::new(0.0, 0.0, 0.0, height as f64);
        gradient.add_color_stop_rgb(0.0, 0.18, 0.19, 0.21);
        gradient.add_color_stop_rgb(1.0, 0.14, 0.15, 0.17);
        ctx.set_source(&gradient).unwrap();
        ctx.fill().unwrap();

        // Draw hour lines
        for offset in 0..=hours_to_show {
            ctx.set_source_rgb(0.537, 0.545, 0.575);
            ctx.set_line_cap(gtk4::cairo::LineCap::Round);
            ctx.set_line_width(0.5);

            let hour = window_start.hour() + offset as u32;
            let y = (offset as f64 / hours_to_show as f64) * inner_height;
            ctx.move_to(padding + 40.0, y + padding);
            ctx.line_to(inner_width + padding, y + padding);
            ctx.stroke().unwrap();

            // Draw hour text
            ctx.set_source_rgb(0.537, 0.545, 0.575);
            ctx.select_font_face(
                "Sans",
                gtk4::cairo::FontSlant::Normal,
                gtk4::cairo::FontWeight::Bold,
            );
            ctx.set_font_size(12.0);
            let time_label = format!("{:02}:00", hour);
            ctx.move_to(padding, y + padding + 4.0);
            ctx.show_text(&time_label).unwrap();
        }

        // Draw mock events
        ctx.set_operator(gtk4::cairo::Operator::Over);
        ctx.select_font_face(
            "Sans",
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Normal,
        );
        for event in events {
            if event.end <= window_start || event.start >= window_end {
                continue;
            }

            let visible_start = event.start.max(window_start);
            let visible_end = event.end.min(window_end);

            let start_secs = (visible_start - window_start).num_seconds() as f64;
            let end_secs = (visible_end - window_start).num_seconds() as f64;

            let start_y = (start_secs / total_seconds) * inner_height + padding;
            let end_y = (end_secs / total_seconds) * inner_height + padding;
            let rect_height = (end_y - start_y).max(1.0);

            ctx.set_source_rgba(0.3, 0.6, 0.9, 0.9);
            // Colors:
            // Blue: #4D99E6
            // Orange: #e8a849
            let (r, g, b) = Conversions::hex_to_rgb("#e8a849");
            ctx.set_source_rgba(r, g, b, 0.9);
            CairoShapesExt::rounded_rectangle(
                ctx,
                padding + 40.0,
                start_y,
                inner_width - 40.0,
                rect_height,
                5.0,
            );
            ctx.fill().unwrap();

            // Event label
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            ctx.move_to(padding + 45.0, start_y + 15.0);
            ctx.show_text(event.label).unwrap();
        }

        // Draw current time line
        let now = Local::now().time();
        let current_y = (now - window_start).num_seconds() as f64 / total_seconds * inner_height;
        let (r, g, b) = Conversions::hex_to_rgb("#bf4759");
        ctx.set_source_rgba(r, g, b, 1.0); // Red line
        ctx.set_line_width(2.0);
        ctx.move_to(padding, current_y + padding);
        ctx.line_to(inner_width + padding, current_y + padding);
        ctx.stroke().unwrap();

        let rad = 3.0;
        CairoShapesExt::circle(ctx, padding - rad, current_y + padding, rad);
    }
}
