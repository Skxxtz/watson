use std::{cell::Cell, rc::Rc, str::FromStr};

use crate::{
    config::WidgetSpec,
    ui::widgets::utils::{
        WidgetOption,
        render::{CairoShapesExt, Rgba},
    },
};
use common::{
    protocol::BatteryState,
    utils::errors::{WatsonError, WatsonErrorKind},
    watson_err,
};
use gtk4::{
    Align, Box, DrawingArea,
    cairo::{Context, LineCap},
    glib::{WeakRef, object::ObjectExt},
    prelude::{BoxExt, DrawingAreaExtManual, WidgetExt},
};

#[derive(Clone, Debug)]
pub struct Battery {
    pub weak: WeakRef<DrawingArea>,
    pub status: Rc<Cell<BatteryStatus>>,
}
impl Battery {
    pub fn poll_state(&self) {
        self.status.set(BatteryStatus::poll());
    }
    pub fn update_state(&self, state: BatteryState, percentage: u32) {
        let status = match state {
            BatteryState::Full => BatteryStatus::Full(percentage),
            BatteryState::Charging => BatteryStatus::Charging(percentage),
            BatteryState::Discharging => BatteryStatus::Discharging(percentage),
            _ => BatteryStatus::Invalid,
        };
        self.status.set(status)
    }
    pub fn queue_draw(&self) {
        if let Some(strong) = self.weak.upgrade() {
            strong.queue_draw();
        }
    }
}

pub struct BatteryBuilder {
    ui: WidgetOption<DrawingArea>,
    status: Rc<Cell<BatteryStatus>>,
}
impl BatteryBuilder {
    pub fn new(specs: WidgetSpec, in_holder: bool) -> Self {
        let base = specs.base();

        let builder = DrawingArea::builder().css_classes(["widget", "battery"]);

        let bat_area = if in_holder {
            builder
                .vexpand(true)
                .hexpand(true)
                .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Fill))
                .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Fill))
                .height_request(10)
                .width_request(10)
        } else {
            builder
                .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Start))
                .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Start))
                .height_request(100)
                .width_request(100)
        }
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
            let status = Rc::clone(&status);
            move |area, ctx, width, height| {
                let tooltip = status
                    .get()
                    .to_percentage()
                    .map(|s| format!("{}%", s * 100.0).to_string());
                area.set_tooltip_markup(tooltip.as_deref());
                Battery::draw(area, ctx, width, height, &specs, Rc::clone(&status));
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

        Self {
            ui: WidgetOption::Owned(bat_area),
            status,
        }
    }
    pub fn for_box(mut self, container: &Box) -> Self {
        if let Some(wid) = self.ui.take() {
            container.append(&wid)
        }
        self
    }
    pub fn build(self) -> Battery {
        let weak = self.ui.downgrade();
        Battery {
            weak,
            status: self.status,
        }
    }
}

impl Battery {
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
        let Some(percentage) = status.get().to_percentage() else {
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

        // Background Track
        ctx.set_line_cap(LineCap::Round);
        ctx.set_line_width(bat.line_width);
        ctx.set_source_rgba(color.r, color.g, color.b, 0.15);
        CairoShapesExt::circle_path(
            ctx,
            bat.center,
            bat.center,
            bat.height / 2.0 - bat.line_width / 2.0,
            1.0,
        );
        ctx.close_path();
        ctx.stroke().unwrap();

        // Progress arc
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        CairoShapesExt::circle_path(
            ctx,
            bat.center,
            bat.center,
            bat.height / 2.0 - bat.line_width / 2.0,
            percentage,
        );
        ctx.stroke().unwrap();

        ctx.save().unwrap();
        ctx.translate(bat.center, bat.center);
        ctx.set_line_width(0.1);
        ctx.set_line_cap(LineCap::Round);
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);

        match status.get() {
            BatteryStatus::Charging(_) => {
                Self::draw_plug(ctx, &bat);
            }
            _ => {
                Self::draw_bolt(ctx, &bat);
            }
        }

        ctx.fill().unwrap();
        ctx.restore().unwrap();
    }
    fn draw_bolt(ctx: &Context, bat: &BatteryContext) {
        let bolt_size = bat.height * 0.25;
        ctx.scale(bolt_size, bolt_size);
        // top point
        ctx.move_to(0.3, -1.0);
        ctx.line_to(0.05, -0.15);
        ctx.line_to(0.65, -0.15);
        ctx.line_to(-0.3, 1.0);
        ctx.line_to(-0.1, 0.15);
        ctx.line_to(-0.6, 0.15);
        ctx.close_path();
    }
    fn draw_plug(ctx: &Context, bat: &BatteryContext) {
        let size = bat.height * 0.420;
        ctx.scale(size, size);

        let body_width = 0.6;
        let body_height = 0.6;
        let body_x = -body_width / 2.0;
        let body_y = -body_height / 2.0;

        CairoShapesExt::rounded_rectangle(
            ctx,
            body_x,
            body_y,
            body_width,
            body_height,
            (0.05, 0.05, 0.4, 0.4),
        );
        ctx.fill().unwrap();

        // Prongs
        let prong_width = 0.1;
        let prong_height = 0.25;
        let prong_y = body_y - prong_height;

        // Left prong
        CairoShapesExt::rounded_rectangle(
            ctx,
            body_x + 0.1,
            prong_y,
            prong_width,
            prong_height + 0.1,
            (0.1, 0.1, 0.0, 0.0),
        );
        ctx.fill().unwrap();

        // Right prong
        CairoShapesExt::rounded_rectangle(
            ctx,
            body_x + body_width - prong_width - 0.1,
            prong_y,
            prong_width,
            prong_height + 0.1,
            (0.1, 0.1, 0.0, 0.0),
        );
        ctx.fill().unwrap();

        // Optional: cord
        ctx.set_line_width(0.09);
        ctx.move_to(0.0, body_y + body_height);
        ctx.line_to(0.0, body_y + body_height + 0.3);
        ctx.stroke().unwrap();
    }
}
struct BatteryContext {
    center: f64,
    height: f64,
    line_width: f64,
}

#[derive(Copy, Clone, Debug)]
pub enum BatteryStatus {
    Discharging(u32),
    Full(u32),
    Charging(u32),
    Invalid,
}
impl BatteryStatus {
    fn poll() -> Self {
        let status_path = "/sys/class/power_supply/BAT0/status";
        let status = {
            let status_opt = std::fs::read_to_string(status_path);

            match status_opt {
                Ok(c) => c.trim().to_lowercase(),
                _ => return Self::Invalid,
            }
        };

        let capacity = match Self::capacity() {
            Ok(cap) => cap,
            Err(_) => return Self::Invalid,
        };

        match status.as_str() {
            "discharging" => Self::Discharging(capacity),
            "full" => Self::Full(capacity),
            "charging" => Self::Charging(capacity),
            _ => Self::Invalid,
        }
    }
    fn capacity() -> Result<u32, WatsonError> {
        let capacity_path = "/sys/class/power_supply/BAT0/capacity";
        let capacity = {
            let capacity_opt = std::fs::read_to_string(capacity_path)
                .expect("Failed to read capacity")
                .trim()
                .parse::<u32>();

            match capacity_opt {
                Ok(c) => c,
                Err(e) => return Err(watson_err!(WatsonErrorKind::Deserialize, e.to_string())),
            }
        };

        Ok(capacity)
    }
    fn to_percentage(&self) -> Option<f64> {
        match self {
            Self::Full(d) => Some(*d as f64 / 100.0),
            Self::Charging(d) => Some(*d as f64 / 100.0),
            Self::Discharging(d) => Some(*d as f64 / 100.0),
            Self::Invalid => None,
        }
    }
}
