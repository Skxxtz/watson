use std::{cell::Cell, rc::Rc, str::FromStr};

use crate::{
    config::WidgetSpec,
    ui::widgets::utils::{CairoShapesExt, Rgba},
};
use gtk4::{
    DrawingArea,
    cairo::{Context, LineCap},
    glib::object::ObjectExt,
    prelude::{DrawingAreaExtManual, WidgetExt},
};

pub struct Battery;
impl Battery {
    pub fn new(specs: &WidgetSpec) -> DrawingArea {
        let specs = Rc::new(specs.clone());

        let bat_area = DrawingArea::builder()
            .vexpand(false)
            .hexpand(false)
            .css_classes(["widget", "battery"])
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::Start)
            .width_request(100)
            .height_request(100)
            .build();

        if let Some(id) = specs.id() {
            bat_area.set_widget_name(id);
        }
        if let Some(class) = specs.class() {
            bat_area.add_css_class(class);
        }

        let status = BatteryStatus::poll();
        let status = Rc::new(Cell::new(status));

        bat_area.set_draw_func({
            let specs = Rc::clone(&specs);
            let status = Rc::clone(&status);
            move |area, ctx, width, height| {
                Self::draw(area, ctx, width, height, &specs, Rc::clone(&status));
            }
        });

        let clock_area_clone = bat_area.downgrade();
        gtk4::glib::timeout_add_seconds_local(30, {
            let status = Rc::clone(&status);
            move || {
                status.set(BatteryStatus::poll());
                if let Some(clock) = clock_area_clone.upgrade() {
                    clock.queue_draw();
                }
                gtk4::glib::ControlFlow::Continue
            }
        });

        bat_area
    }
    fn draw(
        _area: &DrawingArea,
        ctx: &Context,
        width: i32,
        height: i32,
        specs: &WidgetSpec,
        status: Rc<Cell<BatteryStatus>>,
    ) {
        let WidgetSpec::Battery {
            base: _,
            colors,
            threshold,
        } = specs
        else {
            return;
        };
        let Some(percentage) = status.get().percentage() else {
            return;
        };

        let threshold = *threshold as f64 / 100.0;
        let color = if percentage > threshold {
            let t = (percentage - threshold) / threshold;
            let mid_color = Rgba::from_str(&colors[1]).unwrap_or_default();
            let start_color = Rgba::from_str(&colors[0]).unwrap_or_default();
            mid_color.lerp(&start_color, t)
        } else {
            let t = percentage / threshold;
            let end_color = Rgba::from_str(&colors[2]).unwrap_or_default();
            let mid_color = Rgba::from_str(&colors[1]).unwrap_or_default();
            end_color.lerp(&mid_color, t)
        };

        let bat = {
            let padding = (width as f64 * 0.03).max(5.0);
            let width = width as f64 - 2.0 * padding;
            let height = height as f64 - 2.0 * padding;
            BatteryContext {
                center: width as f64 / 2.0 + padding,
                height,
                line_width: height * 0.1,
            }
        };
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        ctx.set_line_cap(LineCap::Round);
        ctx.set_line_width(bat.line_width);
        CairoShapesExt::circle_path(
            ctx,
            bat.center,
            bat.center,
            bat.height / 2.0 - bat.line_width / 2.0,
            percentage,
        );
        ctx.stroke().unwrap()
    }
}
struct BatteryContext {
    center: f64,
    height: f64,
    line_width: f64,
}

#[derive(Copy, Clone)]
pub enum BatteryStatus {
    Discharging(u32),
    Full(u32),
    Charging(u32),
    Invalid,
}
impl BatteryStatus {
    fn poll() -> Self {
        let capacity_path = "/sys/class/power_supply/BAT0/capacity";
        let status_path = "/sys/class/power_supply/BAT0/status";

        let capacity = {
            let capacity_opt = std::fs::read_to_string(capacity_path)
                .expect("Failed to read capacity")
                .trim()
                .parse::<u32>();

            match capacity_opt {
                Ok(c) => c,
                _ => return Self::Invalid,
            }
        };
        let status = {
            let status_opt = std::fs::read_to_string(status_path);

            match status_opt {
                Ok(c) => c.trim().to_lowercase(),
                _ => return Self::Invalid,
            }
        };

        match status.as_str() {
            "discharging" => Self::Discharging(capacity),
            "full" => Self::Full(capacity),
            "charging" => Self::Charging(capacity),
            _ => Self::Invalid,
        }
    }
    fn percentage(&self) -> Option<f64> {
        match self {
            Self::Full(d) => Some(*d as f64 / 100.0),
            Self::Charging(d) => Some(*d as f64 / 100.0),
            Self::Discharging(d) => Some(*d as f64 / 100.0),
            Self::Invalid => None,
        }
    }
}
