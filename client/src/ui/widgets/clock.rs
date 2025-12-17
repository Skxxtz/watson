use std::{f64::consts::PI, fs, rc::Rc, str::FromStr};

use crate::{config::WidgetSpec, ui::widgets::utils::{CairoShapesExt, Rgba}};
use chrono::{DateTime, Local, Timelike};
use chrono_tz::Tz;
use gtk4::{
    DrawingArea, cairo::Context, glib::object::ObjectExt, prelude::{DrawingAreaExtManual, WidgetExt}
};
use serde::{Deserialize, Serialize};

pub struct Clock;
impl Clock {
    pub fn new(specs: &WidgetSpec) -> DrawingArea {
        let specs = Rc::new(specs.clone());

        let clock_area = DrawingArea::builder()
            .vexpand(false)
            .hexpand(false)
            .css_classes(["widget", "clock"])
            .halign(gtk4::Align::Start)
            .build();

        clock_area.set_size_request(200, 200);


        clock_area.set_draw_func({
            let specs = Rc::clone(&specs);
            move |area, ctx, width, height| {
                Clock::draw(area, ctx, width, height, &specs);
            }
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
    pub fn draw(_area: &DrawingArea, ctx: &Context, width: i32, height: i32, specs: &WidgetSpec) {
        let WidgetSpec::Clock { head_style, time_zone } = specs else {
            return
        };
        
        let tz: Tz = match time_zone {
            Some(tz_str) => tz_str.parse::<Tz>().unwrap_or(Tz::UTC),
            None => {
                fs::read_link("/etc/localtime")
                    .ok()
                    .and_then(|path| {
                        // Extract "Europe/Berlin" from "/usr/share/zoneinfo/Europe/Berlin"
                        let path_str = path.to_str()?;
                        let parts: Vec<&str> = path_str.split("zoneinfo/").collect();
                        parts.get(1).map(|&name| name.to_string())
                    })
                .and_then(|name| name.parse::<Tz>().ok())
                    // 3. Absolute fallback if symlink is missing or unparseable
                    .unwrap_or(Tz::UTC)
            }
        };

        let now_full: DateTime<Tz> = Local::now().with_timezone(&tz);
        let now = now_full.time();

        let padding = (width as f64 * 0.03).max(5.0);
        let inner_height = height as f64 - 2.0 * padding;
        let inner_width = width as f64 - 2.0 * padding;

        let clock = ClockContext {
            center: inner_height / 2.0 + padding,
            head_margin: 12.0,
            radius: (inner_width.min(inner_height) / 2.0) as f64,

            hour: now.hour() as f64,
            minute: now.minute() as f64,
            second: now.second() as f64,
        };

        ctx.select_font_face(
            "Sans",
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Bold,
        );
        ctx.set_font_size(15.0);
        ctx.set_line_cap(gtk4::cairo::LineCap::Round);

        // Background
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        ctx.set_operator(gtk4::cairo::Operator::Source);
        ctx.paint().unwrap();

        // Clock Frame
        let Rgba { r, g, b, a } = Rgba::from_str("#2E3035").unwrap_or_default();
        CairoShapesExt::rounded_rectangle(&ctx, 0.0, 0.0, width as f64, height as f64, 20.0);
        ctx.set_source_rgba(r, g, b, a);
        ctx.fill().unwrap();

        // Clock Face
        ctx.set_source_rgb(1.0, 1.0, 1.0);
        CairoShapesExt::circle(ctx, clock.center, clock.center, inner_height / 2.0);

        CairoShapesExt::circle(ctx, clock.center, clock.center, 5.0);

        // Draw hour marks
        let line_length = 10.0;
        let line_offset = 4.0;
        for i in 1..=12 {
            ctx.set_source_rgb(0.0, 0.0, 0.0);
            let angle = i as f64 * (2.0 * std::f64::consts::PI / 12.0);
            let x1 = clock.center + (clock.radius - line_length - line_offset) * angle.sin();
            let y1 = clock.center - (clock.radius - line_length - line_offset) * angle.cos();
            let x2 = clock.center + (clock.radius - line_offset) * angle.sin();
            let y2 = clock.center - (clock.radius - line_offset) * angle.cos();
            let x3 = clock.center + (clock.radius - line_length * 2.7) * angle.sin();
            let y3 = clock.center - (clock.radius - line_length * 2.7) * angle.cos();

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
            let x1 = clock.center + (clock.radius - line_length - line_offset) * angle.sin();
            let y1 = clock.center - (clock.radius - line_length - line_offset) * angle.cos();
            let x2 = clock.center + (clock.radius - line_offset) * angle.sin();
            let y2 = clock.center - (clock.radius - line_offset) * angle.cos();

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
        if let Some(tz_str) = tz.name().split('/').last().map(|s| s.replace('_', " ")) {
            let time_offset = now_full.offset().to_string();
            ctx.set_source_rgb(0.8, 0.8, 0.8);

            CairoShapesExt::centered_text(ctx, &time_offset, clock.center, clock.center + 35.0);

            ctx.set_font_size(18.0);
            CairoShapesExt::centered_text(ctx, &tz_str, clock.center, clock.center - 35.0);
        }

        // Draw Hour Hand
        head_style.hour_head(ctx, &clock);

        // Draw Minute Hand
        head_style.minute_hand(ctx, &clock);

        // Draw Second Hand
        HandStyle::Modern {
            color: "#bf4759".into(),
        }
        .second_head(&ctx, &clock);

        // Draw screws
        ctx.set_source_rgb(0.0, 0.0, 0.0);
        CairoShapesExt::circle(ctx, clock.center, clock.center, 4.5);

        let Rgba { r, g, b, a } = Rgba::from_str("#bf4759").unwrap_or_default();
        ctx.set_source_rgba(r, g, b, a);
        CairoShapesExt::circle(ctx, clock.center, clock.center, 3.0);

        ctx.set_source_rgb(1.0, 1.0, 1.0);
        CairoShapesExt::circle(ctx, clock.center, clock.center, 1.5);
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HandStyle {
    Modern { color: String },
    Sharp { color: String },
}
impl Default for HandStyle {
    fn default() -> Self {
        Self::Modern {
            color: "#000000".into(),
        }
    }
}
impl HandStyle {
    fn hour_head(&self, ctx: &Context, clock: &ClockContext) {
        let angle = ((clock.hour % 12.0 * 5.0) + clock.minute / 12.0) * (PI / 30.0);
        let line_length = clock.radius * 0.5;

        match self {
            Self::Modern { color } => {
                // Draw hour head
                let Rgba { r, g, b, a } = Rgba::from_str(&color).unwrap_or_default();
                ctx.set_source_rgba(r, g, b, a);
                ctx.set_line_width(3.0);
                let x1 = clock.center + clock.head_margin * angle.sin();
                let y1 = clock.center - clock.head_margin * angle.cos();

                ctx.move_to(clock.center, clock.center);
                ctx.line_to(x1, y1);
                ctx.stroke().unwrap();

                ctx.set_line_width(6.0);
                let x2 = clock.center + line_length * angle.sin();
                let y2 = clock.center - line_length * angle.cos();

                ctx.move_to(x1, y1);
                ctx.line_to(x2, y2);
                ctx.stroke().unwrap();
            }
            Self::Sharp { color } => {
                let Rgba { r, g, b, a } = Rgba::from_str(&color).unwrap_or_default();
                ctx.set_source_rgba(r, g, b, a);
                ctx.set_line_width(1.0);

                let thickness = 8.0;
                let half_t = thickness / 2.0;
                let k = 0.4;

                let dx = angle.sin();
                let dy = -angle.cos();

                let px = -dy;
                let py = dx;

                // End point point
                let cx = clock.center + dx * line_length;
                let cy = clock.center + dy * line_length;

                // Perpendicular offset vector
                let ox = px * half_t;
                let oy = py * half_t;

                // Fractual point on center → end
                let mx = clock.center + dx * line_length * k;
                let my = clock.center + dy * line_length * k;

                let bx = mx + ox;
                let by = my + oy;

                let bx2 = mx - ox;
                let by2 = my - oy;

                ctx.move_to(clock.center, clock.center);
                ctx.line_to(bx, by);
                ctx.line_to(cx, cy);
                ctx.line_to(bx2, by2);
                ctx.close_path();
                ctx.fill().unwrap();
            }
        }
    }
    fn minute_hand(&self, ctx: &Context, clock: &ClockContext) {
        let angle = (clock.minute + clock.second / 60.0) * 6.0 * (PI / 180.0);
        let line_length = clock.radius * 0.9;

        match self {
            Self::Modern { color } => {
                // Draw minute head
                let Rgba { r, g, b, a } = Rgba::from_str(&color).unwrap_or_default();
                ctx.set_source_rgba(r, g, b, a);
                ctx.set_line_width(3.0);
                let x1 = clock.center + clock.head_margin * angle.sin();
                let y1 = clock.center - clock.head_margin * angle.cos();

                ctx.move_to(clock.center, clock.center);
                ctx.line_to(x1, y1);
                ctx.stroke().unwrap();

                ctx.set_line_width(6.0);
                let x2 = clock.center + line_length * angle.sin();
                let y2 = clock.center - line_length * angle.cos();

                ctx.move_to(x1, y1);
                ctx.line_to(x2, y2);
                ctx.stroke().unwrap();
            }
            Self::Sharp { color } => {
                let Rgba {r, g, b, a} = Rgba::from_str(&color).unwrap_or_default();
                ctx.set_source_rgba(r, g, b, a);
                ctx.set_line_width(1.0);

                let thickness = 9.0;
                let half_t = thickness / 2.0;
                let k = 0.3;

                let dx = angle.sin();
                let dy = -angle.cos();

                let px = -dy;
                let py = dx;

                // End point point
                let cx = clock.center + dx * line_length;
                let cy = clock.center + dy * line_length;

                // Perpendicular offset vector
                let ox = px * half_t;
                let oy = py * half_t;

                // Fractual point on center → end
                let mx = clock.center + dx * line_length * k;
                let my = clock.center + dy * line_length * k;

                let bx = mx + ox;
                let by = my + oy;

                let bx2 = mx - ox;
                let by2 = my - oy;

                ctx.move_to(clock.center, clock.center);
                ctx.line_to(bx, by);
                ctx.line_to(cx, cy);
                ctx.line_to(bx2, by2);
                ctx.close_path();
                ctx.fill().unwrap();
            }
        }
    }

    fn second_head(&self, ctx: &Context, clock: &ClockContext) {
        match self {
            Self::Modern { color } => {
                // Draw second head
                let Rgba { r,g,b,a } = Rgba::from_str(&color).unwrap_or_default();
                ctx.set_source_rgba(r, g, b, a);
                ctx.set_line_width(2.0);
                let line_length = clock.radius * 0.8;
                let angle = clock.second as f64 * (PI / 30.0);
                let x1 = clock.center + line_length * angle.sin();
                let y1 = clock.center - line_length * angle.cos();

                ctx.move_to(clock.center, clock.center);
                ctx.line_to(x1, y1);
                ctx.stroke().unwrap();

                let angle = (clock.second as f64 - 30.0) * (PI / 30.0);
                let x1 = clock.center + 1.3 * clock.head_margin * angle.sin();
                let y1 = clock.center - 1.3 * clock.head_margin * angle.cos();

                ctx.move_to(clock.center, clock.center);
                ctx.line_to(x1, y1);
                ctx.stroke().unwrap();
            }
            _ => {}
        }
    }
}

struct ClockContext {
    center: f64,
    head_margin: f64,
    radius: f64,

    hour: f64,
    minute: f64,
    second: f64,
}
