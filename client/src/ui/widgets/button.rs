use crate::{
    config::WidgetSpec,
    ui::widgets::{interactives::WidgetBehavior, utils::Rgba},
};
use common::protocol::AtomicSystemState;
use gtk4::{
    Box as GtkBox, DrawingArea, GestureClick, Image, Overlay, Widget,
    cairo::Context,
    glib::{
        WeakRef,
        object::{Cast, ObjectExt},
    },
    prelude::{BoxExt, DrawingAreaExtManual, WidgetExt},
};
use std::{rc::Rc, sync::Arc};

pub struct Button {
    pub weak: WeakRef<Widget>,
    pub func: Box<dyn WidgetBehavior>,
}
impl Button {
    pub fn queue_draw(&self) {
        if let Some(strong) = self.weak.upgrade() {
            strong.queue_draw();
        }
    }
}

pub struct ButtonBuilder {
    area: DrawingArea,
    overlay: Overlay,
    func: Box<dyn WidgetBehavior>,
}
impl ButtonBuilder {
    pub fn new(specs: &WidgetSpec, system_state: Arc<AtomicSystemState>, in_holder: bool) -> Self {
        let specs = Rc::new(specs.clone());
        let (base, func, icon) = specs.as_button().unwrap();
        let func = func.build();

        let perc = func.get_percentage(&system_state);

        let icon = icon.unwrap_or(func.icon_name(perc).to_string());

        let initial_class = format!("state-{perc}");
        let builder =
            Overlay::builder().css_classes(["button", &initial_class, &func.func().to_string()]);

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

        let button_holder = GtkBox::builder()
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

        Button::connect_clicked(&overlay, &func, system_state);

        Self {
            area,
            overlay,
            func,
        }
    }
    pub fn for_box(self, container: &GtkBox) -> Self {
        container.append(&self.overlay);
        self
    }
    pub fn build(self) -> Button {
        let weak = self.area.upcast::<Widget>().downgrade();
        Button {
            weak,
            func: self.func,
        }
    }
}

impl Button {
    fn draw(area: &DrawingArea, ctx: &Context, width: i32, height: i32) {
        let color: Rgba = area.color().into();
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        ctx.rectangle(0.0, 0.0, width as f64, height as f64);
        ctx.fill().unwrap();
    }

    fn connect_clicked(
        target: &Overlay,
        func: &Box<dyn WidgetBehavior>,
        system_state: Arc<AtomicSystemState>,
    ) {
        let click = GestureClick::new();
        let times = std::cell::Cell::new(func.get_percentage(&system_state));
        click.connect_pressed({
            let target = target.downgrade();
            let state = Arc::clone(&system_state);
            let func = func.clone();
            move |_gesture, _, _, _| {
                let new_state = func.execute(&state);
                times.set(times.get() ^ 1);
                if let Some(target) = target.upgrade() {
                    let state_class = target
                        .css_classes()
                        .iter()
                        .find(|s| s.starts_with("state-"))
                        .map(|v| v.to_string());
                    if let Some(class) = state_class {
                        target.remove_css_class(&class);
                    }
                    if let Some(class) = new_state {
                        target.add_css_class(&format!("state-{class}"));
                    }

                    if times.get() == 1 {
                        target.remove_css_class("inactive");
                        target.add_css_class("active");
                    } else {
                        target.remove_css_class("active");
                        target.add_css_class("inactive");
                    }
                }
            }
        });
        target.add_controller(click);
    }
}
