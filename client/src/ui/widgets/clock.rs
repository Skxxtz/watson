use std::fs;

use crate::ui::widgets::utils::CairoShapesExt;
use chrono::{DateTime, Local, Timelike};
use gtk4::{
    DrawingArea,
    cairo::Context,
    glib::object::ObjectExt,
    prelude::{DrawingAreaExtManual, WidgetExt},
};

use super::utils::Conversions;

pub struct Clock;
impl Clock {
    pub fn new() -> DrawingArea {
        let clock_area = DrawingArea::builder()
            .vexpand(false)
            .hexpand(false)
            .css_classes(["widget", "clock"])
            .halign(gtk4::Align::Start)
            .build();

        clock_area.set_size_request(200, 200);

        clock_area.set_draw_func(|area, ctx, width, height| {
            Clock::draw(area, ctx, width, height);
        });

        // Make update every second
        let clock_area_clone = clock_area.downgrade();
        gtk4::glib::timeout_add_seconds_local(1, move || {
            if let Some(clock) = clock_area_clone.upgrade() {
                clock.queue_draw();
            }
            gtk4::glib::ControlFlow::Continue
        });

        clock_area
    }
    pub fn draw(_area: &DrawingArea, ctx: &Context, width: i32, height: i32) {
        let padding = (width as f64 * 0.03).max(5.0);
        let inner_height = height as f64 - 2.0 * padding;
        let inner_width = width as f64 - 2.0 * padding;

        let head_margin = 12.0;

        ctx.select_font_face(
            "Sans",
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Bold,
        );
        ctx.set_font_size(15.0);
        ctx.set_line_cap(gtk4::cairo::LineCap::Round);

        let now_full: DateTime<Local> = Local::now();
        let now = now_full.time();

        // Background
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        ctx.set_operator(gtk4::cairo::Operator::Source);
        ctx.paint().unwrap();

        // Clock Frame
        let (r, g, b) = Conversions::hex_to_rgb("##2E3035");
        CairoShapesExt::rounded_rectangle(&ctx, 0.0, 0.0, width as f64, height as f64, 20.0);
        ctx.set_source_rgba(r, g, b, 1.0);
        ctx.fill().unwrap();

        // Clock Face
        let center = inner_height / 2.0 + padding;
        ctx.set_source_rgb(1.0, 1.0, 1.0);
        CairoShapesExt::circle(ctx, center, center, inner_height / 2.0);

        CairoShapesExt::circle(ctx, center, center, 5.0);

        let radius = (inner_width.min(inner_height) / 2.0) as f64;

        // Draw hour marks
        let line_length = 10.0;
        let line_offset = 4.0;
        for i in 1..=12 {
            ctx.set_source_rgb(0.0, 0.0, 0.0);
            let angle = i as f64 * (2.0 * std::f64::consts::PI / 12.0);
            let x1 = center + (radius - line_length - line_offset) * angle.sin();
            let y1 = center - (radius - line_length - line_offset) * angle.cos();
            let x2 = center + (radius - line_offset) * angle.sin();
            let y2 = center - (radius - line_offset) * angle.cos();
            let x3 = center + (radius - line_length * 2.7) * angle.sin();
            let y3 = center - (radius - line_length * 2.7) * angle.cos();

            ctx.move_to(x1, y1);
            ctx.line_to(x2, y2);
            ctx.stroke().unwrap();

            // Draw text
            ctx.set_source_rgb(0.3, 0.3, 0.3);
            CairoShapesExt::centered_text(ctx, &i.to_string(), x3, y3);
        }

        // Draw minute marks
        ctx.set_source_rgb(0.5, 0.5, 0.5);
        let line_length = 5.0;
        let line_offset = 4.0;
        for i in 1..=60 {
            if i % 5 == 0 {
                continue;
            }

            let angle = i as f64 * (2.0 * std::f64::consts::PI / 60.0);
            let x1 = center + (radius - line_length - line_offset) * angle.sin();
            let y1 = center - (radius - line_length - line_offset) * angle.cos();
            let x2 = center + (radius - line_offset) * angle.sin();
            let y2 = center - (radius - line_offset) * angle.cos();

            ctx.move_to(x1, y1);
            ctx.line_to(x2, y2);
            ctx.stroke().unwrap();
        }

        // Draw Zimezone
        ctx.select_font_face(
            "Sans",
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Normal,
        );
        if let Ok(tz) = fs::read_link("/etc/localtime") {
            if let Some(place) = tz.to_str().and_then(|s| s.split('/').last()) {
                let time_offset = now_full.offset().to_string();
                ctx.set_source_rgb(0.8, 0.8, 0.8);

                CairoShapesExt::centered_text(ctx, &time_offset, center, center + 35.0);

                ctx.set_font_size(18.0);
                CairoShapesExt::centered_text(ctx, place, center, center - 35.0);
            }
        }

        ctx.set_source_rgb(0.0, 0.0, 0.0);

        // Draw minute head
        ctx.set_line_width(3.0);
        let minute = now.minute();
        let angle = minute as f64 * (2.0 * std::f64::consts::PI / 60.0);
        let x1 = center + head_margin * angle.sin();
        let y1 = center - head_margin * angle.cos();

        ctx.move_to(center, center);
        ctx.line_to(x1, y1);
        ctx.stroke().unwrap();

        ctx.set_line_width(6.0);
        let line_length = radius * 0.9;
        let x2 = center + line_length * angle.sin();
        let y2 = center - line_length * angle.cos();

        ctx.move_to(x1, y1);
        ctx.line_to(x2, y2);
        ctx.stroke().unwrap();

        // Draw hour head
        ctx.set_line_width(3.0);
        let hour = now.hour() % 12 * 5;
        let minute = minute as f64 / 60.0 * 5.0;
        let angle = (hour as f64 + minute) * (2.0 * std::f64::consts::PI / 60.0);
        let x1 = center + head_margin * angle.sin();
        let y1 = center - head_margin * angle.cos();

        ctx.move_to(center, center);
        ctx.line_to(x1, y1);
        ctx.stroke().unwrap();

        ctx.set_line_width(6.0);
        let line_length = radius * 0.5;
        let x2 = center + line_length * angle.sin();
        let y2 = center - line_length * angle.cos();

        ctx.move_to(x1, y1);
        ctx.line_to(x2, y2);
        ctx.stroke().unwrap();

        // Draw minute head
        let (r, g, b) = Conversions::hex_to_rgb("#bf4759");
        ctx.set_source_rgba(r, g, b, 1.0);
        ctx.set_line_width(2.0);
        let line_length = radius * 0.8;
        let minute = now.second();
        let angle = minute as f64 * (2.0 * std::f64::consts::PI / 60.0);
        let x1 = center + line_length * angle.sin();
        let y1 = center - line_length * angle.cos();

        ctx.move_to(center, center);
        ctx.line_to(x1, y1);
        ctx.stroke().unwrap();

        let angle = (minute as f64 - 30.0) * (2.0 * std::f64::consts::PI / 60.0);
        let x1 = center + 1.3 * head_margin * angle.sin();
        let y1 = center - 1.3 * head_margin * angle.cos();

        ctx.move_to(center, center);
        ctx.line_to(x1, y1);
        ctx.stroke().unwrap();

        // Draw screws
        ctx.set_source_rgb(0.0, 0.0, 0.0);
        CairoShapesExt::circle(ctx, center, center, 4.5);

        let (r, g, b) = Conversions::hex_to_rgb("#bf4759");
        ctx.set_source_rgba(r, g, b, 1.0);
        CairoShapesExt::circle(ctx, center, center, 3.0);

        ctx.set_source_rgb(1.0, 1.0, 1.0);
        CairoShapesExt::circle(ctx, center, center, 1.5);
    }
}
