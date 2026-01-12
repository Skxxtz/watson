use crate::{
    DAEMON_TX, SystemState,
    config::WidgetSpec,
    ui::{
        g_templates::main_window::MainWindow,
        widgets::utils::{CairoShapesExt, Rgba, WidgetOption},
    },
};
use common::protocol::PowerMode;
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
    pub fn new(specs: &WidgetSpec, system_state: Rc<SystemState>, in_holder: bool) -> Self {
        let specs = Rc::new(specs.clone());
        let (base, func, icon) = specs.as_button().unwrap();
        let icon = icon.unwrap_or(func.icon_name());

        let initial_class = if func.is_active(&system_state) {
            "active"
        } else {
            "inactive"
        };
        let builder = Overlay::builder().css_classes(["widget", "button", initial_class]);

        let overlay = if in_holder {
            builder
                .vexpand(true)
                .hexpand(true)
                .valign(base.valign.map(|d| d.into()).unwrap_or(gtk4::Align::Fill))
                .halign(base.halign.map(|d| d.into()).unwrap_or(gtk4::Align::Fill))
                .height_request(10)
                .width_request(10)
        } else {
            builder
                .valign(base.valign.map(|d| d.into()).unwrap_or(gtk4::Align::Start))
                .halign(base.halign.map(|d| d.into()).unwrap_or(gtk4::Align::Start))
                .height_request(100)
                .width_request(100)
        }
        .build();

        let area = DrawingArea::new();
        let svg_icon = Image::builder().icon_name(icon).can_target(false).build();

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

        Button::connect_clicked(&overlay, *func, system_state);

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

        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        CairoShapesExt::circle(ctx, cx, cy, radius);
    }

    fn connect_clicked(target: &Overlay, func: ButtonFunc, system_state: Rc<SystemState>) {
        let click = GestureClick::new();
        let times = std::cell::Cell::new(func.is_active(&system_state));
        click.connect_pressed({
            let target = target.downgrade();
            let state = Rc::clone(&system_state);
            move |_gesture, _, _, _| {
                func.execute(Rc::clone(&state));
                times.set(!times.get());
                if let Some(target) = target.upgrade() {
                    if times.get() {
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
    Dnd,
    PowerSave,
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
            Self::PowerSave => "power-profile-power-saver-symbolic",
            Self::Dnd => "weather-clear-night-symbolic",
            Self::None => "none",
        }
        .into()
    }
    pub fn execute(&self, state: Rc<SystemState>) {
        match self {
            ButtonFunc::Wifi => {
                let target = !state.wifi.get();
                DAEMON_TX
                    .get()
                    .map(|d| d.send(common::protocol::Request::SetWifi(target)));
                state.wifi.set(target);
            }
            ButtonFunc::Bluetooth => {
                let target = !state.bluetooth.get();
                DAEMON_TX
                    .get()
                    .map(|d| d.send(common::protocol::Request::SetBluetooth(target)));
                state.wifi.set(target);
            }
            ButtonFunc::PowerSave => {
                let target = !state.powermode.get();
                DAEMON_TX
                    .get()
                    .map(|d| d.send(common::protocol::Request::SetPowerMode(target)));
                state.powermode.set(target);
            }
            _ => {}
        }
    }
    pub fn is_active(&self, state: &Rc<SystemState>) -> bool {
        match self {
            ButtonFunc::Wifi => state.wifi.get(),
            ButtonFunc::Bluetooth => state.bluetooth.get(),
            ButtonFunc::PowerSave => state.powermode.get() == PowerMode::PowerSave,
            _ => false,
        }
    }
}
