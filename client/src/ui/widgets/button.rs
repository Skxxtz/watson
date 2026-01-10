use crate::{
    config::WidgetSpec,
    ui::{
        g_templates::main_window::MainWindow,
        widgets::utils::{CairoShapesExt, Rgba, WidgetOption},
    },
};
use gtk4::{
    Box, DrawingArea, GestureClick, Image, Overlay,
    cairo::Context,
    glib::{
        WeakRef,
        object::{CastNone, ObjectExt},
        subclass::types::ObjectSubclassIsExt,
    },
    prelude::{BoxExt, DrawingAreaExtManual, GtkApplicationExt, WidgetExt},
};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Button {
    pub weak: WeakRef<DrawingArea>,
    pub func: ButtonFunc,
}
impl Button {
    pub fn queue_draw(&self) {
        if let Some(strong) = self.weak.upgrade() {
            strong.queue_draw();
        }
    }
}

pub struct ButtonBuilder {
    ui: WidgetOption<DrawingArea>,
    overlay: WidgetOption<Overlay>,
    func: ButtonFunc,
}
impl ButtonBuilder {
    pub fn new(specs: &WidgetSpec, in_holder: bool) -> Self {
        let specs = Rc::new(specs.clone());
        let (_, func, icon) = specs.as_button().unwrap();
        let icon = icon.unwrap_or(func.icon_name());

        let builder = Overlay::builder().css_classes(["widget", "button"]);

        let overlay = if in_holder {
            builder
                .vexpand(true)
                .hexpand(true)
                .halign(gtk4::Align::Fill)
                .valign(gtk4::Align::Fill)
                .height_request(10)
                .width_request(10)
        } else {
            builder
                .halign(gtk4::Align::Start)
                .valign(gtk4::Align::Start)
                .height_request(100)
                .width_request(100)
        }
        .build();

        let area = DrawingArea::new();
        let svg_icon = Image::builder()
            .icon_name(icon)
            .css_classes(["active"])
            .build();

        overlay.set_child(Some(&area));
        overlay.add_overlay(&svg_icon);

        if let Some(id) = specs.id() {
            area.set_widget_name(id);
        }
        if let Some(class) = specs.class() {
            area.add_css_class(class);
        }

        area.set_draw_func({
            move |area, ctx, width, height| {
                Button::draw(area, ctx, width, height);
            }
        });

        let area_clone = area.downgrade();
        gtk4::glib::timeout_add_seconds_local(30, {
            move || {
                if let Some(clock) = area_clone.upgrade() {
                    clock.queue_draw();
                }
                gtk4::glib::ControlFlow::Continue
            }
        });

        Button::connect_clicked(&svg_icon, *func);

        Self {
            ui: WidgetOption::Owned(area),
            overlay: WidgetOption::Owned(overlay),
            func: *func,
        }
    }
    pub fn for_box(mut self, container: &Box) -> Self {
        if let Some(wid) = self.overlay.take() {
            container.append(&wid);
        }
        self
    }
    pub fn build(self) -> Button {
        let weak = self.ui.weak();
        Button {
            weak,
            func: self.func,
        }
    }
}

impl Button {
    fn draw(_area: &DrawingArea, ctx: &Context, width: i32, height: i32) {
        let color: Rgba = _area.color().into();

        let side = (width as f64).min(height as f64);
        let padding = side * 0.1;
        let radius = (side / 2.0) - padding;
        let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);

        // Background
        let grad =
            gtk4::cairo::LinearGradient::new(cx - radius, cy - radius, cx + radius, cy + radius);
        grad.add_color_stop_rgba(0.0, color.r * 1.2, color.g * 1.2, color.b * 1.2, 0.15);
        grad.add_color_stop_rgba(1.0, color.r * 0.7, color.g * 0.7, color.b * 0.7, 0.4);

        ctx.set_source(&grad).unwrap();
        CairoShapesExt::circle(ctx, cx, cy, radius);
    }

    fn connect_clicked(target: &Image, func: ButtonFunc) {
        let click = GestureClick::new();
        let times = std::cell::Cell::new(0);
        click.connect_pressed({
            let target = target.downgrade();
            move |_gesture, _, _, _| {
                times.set(times.get() + 1);
                if let Some(target) = target.upgrade() {
                    if times.get() % 2 == 0 {
                        target.remove_css_class("inactive");
                        target.add_css_class("active");
                    } else {
                        target.remove_css_class("active");
                        target.add_css_class("inactive");
                    }
                }
                func.update_widgets();
            }
        });
        target.add_controller(click);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ButtonFunc {
    #[default]
    None,
    Wifi,
    Bluetooth,
    Anonymous,
    Dnd,
}
impl ButtonFunc {
    pub fn update_widgets(&self) {
        let app = gtk4::Application::default();
        if let Some(window) = app.active_window().and_downcast::<MainWindow>() {
            let state = window.imp().state.borrow();
            let buttons = state.button(*self);
            buttons.for_each(|b| b.queue_draw());
        }
    }
    pub fn icon_name(&self) -> String {
        match self {
            Self::Wifi => "network-wireless-signal-excellent-symbolic",
            Self::Bluetooth => "bluetooth-symbolic",
            Self::Dnd => "weather-clear-night-symbolic",
            Self::Anonymous => "security-high-symbolic",
            Self::None => "none",
        }
        .into()
    }
}
