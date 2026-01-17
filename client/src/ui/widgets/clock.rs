use std::{f64::consts::PI, fs, str::FromStr};

use crate::{
    config::WidgetSpec,
    ui::widgets::utils::render::{CairoShapesExt, Rgba},
};
use chrono::{DateTime, Local, Timelike};
use chrono_tz::Tz;
use gtk4::{
    DrawingArea,
    cairo::Context,
    glib::object::ObjectExt,
    prelude::{DrawingAreaExtManual, WidgetExt},
};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct ClockConfig {
    time_zone: Option<String>,
    hand_style: HandStyle,
    accent_color: String,
    font: String,
}
impl From<&WidgetSpec> for ClockConfig {
    fn from(value: &WidgetSpec) -> Self {
        if let WidgetSpec::Clock {
            time_zone,
            hand_style,
            accent_color,
            font,
            ..
        } = value
        {
            Self {
                time_zone: time_zone.clone(),
                hand_style: hand_style.clone(),
                accent_color: accent_color.clone(),
                font: font.clone(),
            }
        } else {
            Default::default()
        }
    }
}
pub struct Clock;
impl Clock {
    pub fn new(specs: WidgetSpec) -> DrawingArea {
        let config = ClockConfig::from(&specs);
        let base = specs.base();

        let clock_area = DrawingArea::builder()
            .vexpand(false)
            .hexpand(false)
            .css_classes(["widget", "clock"])
            .valign(base.valign.map(|d| d.into()).unwrap_or(gtk4::Align::Start))
            .halign(base.halign.map(|d| d.into()).unwrap_or(gtk4::Align::Start))
            .build();

        if let Some(id) = specs.id() {
            clock_area.set_widget_name(id);
        }
        if let Some(class) = specs.class() {
            clock_area.add_css_class(class);
        }

        clock_area.set_size_request(200, 200);

        clock_area.set_draw_func({
            let config = config;
            move |area, ctx, width, height| {
                Clock::draw(area, ctx, width, height, &config);
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
    pub fn draw(_area: &DrawingArea, ctx: &Context, width: i32, height: i32, config: &ClockConfig) {
        let ClockConfig {
            time_zone,
            hand_style: head_style,
            accent_color,
            font,
        } = config;

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
            color: Rgba::from(_area.color()),
            center: inner_height / 2.0 + padding,
            head_margin: 12.0,
            radius: (inner_width.min(inner_height) / 2.0) as f64,

            hour: now.hour() as f64,
            minute: now.minute() as f64,
            second: now.second() as f64,
        };

        ctx.select_font_face(
            &font,
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Bold,
        );
        ctx.set_font_size(15.0);
        ctx.set_line_cap(gtk4::cairo::LineCap::Round);

        // Clock Face
        let inverse = clock.color.invert();
        ctx.set_source_rgb(inverse.r, inverse.g, inverse.b);
        CairoShapesExt::circle(ctx, clock.center, clock.center, inner_height / 2.0);

        CairoShapesExt::circle(ctx, clock.center, clock.center, 5.0);

        // Draw hour marks
        let line_length = 10.0;
        let line_offset = 4.0;
        let muted3 = clock.color.lerp(&inverse, 0.3);
        for i in 1..=12 {
            ctx.set_source_rgb(clock.color.r, clock.color.g, clock.color.b);
            let angle = i as f64 * (2.0 * std::f64::consts::PI / 12.0);
            let x1 = clock.center + (clock.radius - line_length - line_offset) * angle.sin();
            let y1 = clock.center - (clock.radius - line_length - line_offset) * angle.cos();
            let x2 = clock.center + (clock.radius - line_offset) * angle.sin();
            let y2 = clock.center - (clock.radius - line_offset) * angle.cos();
            let x3 = clock.center + (clock.radius - line_length * 2.5) * angle.sin();
            let y3 = clock.center - (clock.radius - line_length * 2.5) * angle.cos();

            ctx.move_to(x1, y1);
            ctx.line_to(x2, y2);
            ctx.stroke().unwrap();

            // Draw text
            ctx.set_source_rgb(muted3.r, muted3.g, muted3.b);
            CairoShapesExt::centered_text(ctx, &i.to_string(), x3, y3);
        }

        // Draw minute marks
        let muted6 = clock.color.lerp(&inverse, 0.6);
        ctx.set_source_rgb(muted6.r, muted6.g, muted6.b);
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
            &font,
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
            color: accent_color.clone(),
            width: 6.0,
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
    Modern { color: String, width: f64 },
    Sharp { color: String, width: f64 },
}
impl Default for HandStyle {
    fn default() -> Self {
        Self::Modern {
            color: "#000000".into(),
            width: 6.0,
        }
    }
}
impl HandStyle {
    fn hour_head(&self, ctx: &Context, clock: &ClockContext) {
        let angle = ((clock.hour % 12.0 * 5.0) + clock.minute / 12.0) * (PI / 30.0);
        let line_length = clock.radius * 0.5;

        match self {
            Self::Modern { color, width } => {
                // Draw hour head
                let Rgba { r, g, b, a } = Rgba::from_str(&color).unwrap_or_default();
                ctx.set_source_rgba(r, g, b, a);
                ctx.set_line_width(3.0);
                let x1 = clock.center + clock.head_margin * angle.sin();
                let y1 = clock.center - clock.head_margin * angle.cos();

                ctx.move_to(clock.center, clock.center);
                ctx.line_to(x1, y1);
                ctx.stroke().unwrap();

                ctx.set_line_width(*width);
                let x2 = clock.center + line_length * angle.sin();
                let y2 = clock.center - line_length * angle.cos();

                ctx.move_to(x1, y1);
                ctx.line_to(x2, y2);
                ctx.stroke().unwrap();
            }
            Self::Sharp { color, width } => {
                let Rgba { r, g, b, a } = Rgba::from_str(&color).unwrap_or_default();
                ctx.set_source_rgba(r, g, b, a);
                ctx.set_line_width(1.0);

                let thickness = width; // default 8.0
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
        let line_length = clock.radius * 0.76;

        match self {
            Self::Modern { color, width } => {
                // Draw minute head
                let Rgba { r, g, b, a } = Rgba::from_str(&color).unwrap_or_default();
                ctx.set_source_rgba(r, g, b, a);
                ctx.set_line_width(3.0);
                let x1 = clock.center + clock.head_margin * angle.sin();
                let y1 = clock.center - clock.head_margin * angle.cos();

                ctx.move_to(clock.center, clock.center);
                ctx.line_to(x1, y1);
                ctx.stroke().unwrap();

                ctx.set_line_width(*width);
                let x2 = clock.center + line_length * angle.sin();
                let y2 = clock.center - line_length * angle.cos();

                ctx.move_to(x1, y1);
                ctx.line_to(x2, y2);
                ctx.stroke().unwrap();
            }
            Self::Sharp { color, width } => {
                let Rgba { r, g, b, a } = Rgba::from_str(&color).unwrap_or_default();
                ctx.set_source_rgba(r, g, b, a);
                ctx.set_line_width(1.0);

                let thickness = width; // default 9.0;
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
            Self::Modern { color, .. } => {
                // Draw second head
                let Rgba { r, g, b, a } = Rgba::from_str(&color).unwrap_or_default();
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
    color: Rgba,
    center: f64,
    head_margin: f64,
    radius: f64,

    hour: f64,
    minute: f64,
    second: f64,
}
