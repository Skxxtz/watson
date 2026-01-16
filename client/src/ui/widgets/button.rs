use crate::{
    config::WidgetSpec,
    ui::widgets::utils::{BackendFunc, Rgba, WidgetOption},
};
use common::protocol::AtomicSystemState;
use gtk4::{
    Box, DrawingArea, GestureClick, Image, Overlay,
    cairo::Context,
    glib::{WeakRef, object::ObjectExt},
    prelude::{BoxExt, DrawingAreaExtManual, WidgetExt},
};
use std::{rc::Rc, sync::Arc};

#[derive(Clone, Debug)]
pub struct Button {
    pub weak: WeakRef<DrawingArea>,
    pub func: BackendFunc,
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
    func: BackendFunc,
}
impl ButtonBuilder {
    pub fn new(specs: &WidgetSpec, system_state: Arc<AtomicSystemState>, in_holder: bool) -> Self {
        let specs = Rc::new(specs.clone());
        let (base, func, icon) = specs.as_button().unwrap();

        let perc = func.percentage(&system_state);
        let icon = icon.unwrap_or(func.icon_name(perc as f32 / 100.0));

        let initial_class = if perc == 1 { "active" } else { "inactive" };
        let builder = Overlay::builder().css_classes(["button", initial_class]);

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

        let button_holder = Box::builder()
            .hexpand(true)
            .vexpand(true)
            .can_target(false)
            .overflow(gtk4::Overflow::Visible)
            .valign(gtk4::Align::Center)
            .halign(gtk4::Align::Center)
            .build();

        let area = DrawingArea::builder()
            .css_classes(["button-obj"])
            .overflow(gtk4::Overflow::Hidden)
            .build();
        button_holder.append(&area);

        let svg_icon = Image::builder().icon_name(icon).can_target(false).build();

        overlay.set_child(Some(&button_holder));
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
        let weak = self.ui.downgrade();
        Button {
            weak,
            func: self.func,
        }
    }
}

impl Button {
    fn draw(_area: &DrawingArea, ctx: &Context, width: i32, height: i32) {
        let color: Rgba = _area.color().into();

        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        ctx.rectangle(0.0, 0.0, width as f64, height as f64);
        ctx.fill().unwrap();
    }

    fn connect_clicked(target: &Overlay, func: BackendFunc, system_state: Arc<AtomicSystemState>) {
        let click = GestureClick::new();
        let times = std::cell::Cell::new(func.percentage(&system_state));
        click.connect_pressed({
            let target = target.downgrade();
            let state = Arc::clone(&system_state);
            move |_gesture, _, _, _| {
                func.execute(Arc::clone(&state));
                times.set(times.get() ^ 1);
                if let Some(target) = target.upgrade() {
                    if times.get() == 1 {
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
