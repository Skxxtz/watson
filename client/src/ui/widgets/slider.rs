use crate::{
    config::WidgetSpec,
    ui::{
        g_templates::main_window::MainWindow,
        widgets::utils::{Rgba, WidgetOption},
    },
};
use gtk4::{
    Box, DrawingArea, GestureClick, GestureDrag, Image, Overlay,
    cairo::Context,
    glib::{
        WeakRef,
        object::{CastNone, ObjectExt},
        subclass::types::ObjectSubclassIsExt,
    },
    prelude::{
        BoxExt, DrawingAreaExtManual, EventControllerExt, GestureDragExt, GtkApplicationExt,
        WidgetExt,
    },
};
use serde::{Deserialize, Serialize};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Debug)]
pub struct Slider {
    pub weak: WeakRef<DrawingArea>,
    pub func: SliderFunc,
}
impl Slider {
    pub fn queue_draw(&self) {
        if let Some(strong) = self.weak.upgrade() {
            strong.queue_draw();
        }
    }
}

pub struct SliderBuilder {
    ui: WidgetOption<DrawingArea>,
    overlay: WidgetOption<Overlay>,
    func: SliderFunc,
}
impl SliderBuilder {
    pub fn new(specs: &WidgetSpec, in_holder: bool) -> Self {
        let specs = Rc::new(specs.clone());
        let (_, func, icon, range) = specs.as_slider().unwrap();

        let perc = Rc::new(Cell::new(0.0));
        let icon = icon.unwrap_or(func.icon_name(0.5));

        let builder = Overlay::builder()
            .css_classes(["widget", "slider"])
            .overflow(gtk4::Overflow::Hidden)
            .height_request(200)
            .width_request(50);

        let overlay = if in_holder {
            builder.vexpand(true).valign(gtk4::Align::Fill)
        } else {
            builder
                .halign(gtk4::Align::Start)
                .valign(gtk4::Align::Start)
        }
        .build();

        let area = DrawingArea::new();
        let svg_icon = Image::builder()
            .icon_name(icon)
            .css_classes(["active"])
            .can_target(false)
            .build();

        overlay.set_child(Some(&area));
        overlay.add_overlay(&svg_icon);

        if let Some(id) = specs.id() {
            area.set_widget_name(id);
        }
        if let Some(class) = specs.class() {
            area.add_css_class(class);
        }

        Slider::connect_clicked(&area, Rc::clone(&perc), *func, svg_icon.downgrade());
        Slider::connect_drag(&area, Rc::clone(&perc), *func, svg_icon.downgrade());

        area.set_draw_func({
            move |area, ctx, width, height| {
                Slider::draw(area, ctx, width, height, perc.clone());
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
    pub fn build(self) -> Slider {
        let weak = self.ui.weak();
        Slider {
            weak,
            func: self.func,
        }
    }
}

impl Slider {
    fn draw(
        _area: &DrawingArea,
        ctx: &Context,
        width: i32,
        height: i32,
        percentage: Rc<Cell<f32>>,
    ) {
        let color: Rgba = _area.color().into();

        let p = percentage.get() as f64;

        let height = height as f64;
        let width = width as f64;

        // Background
        let grad = gtk4::cairo::LinearGradient::new(0.0, 0.0, 0.0, height);
        grad.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
        grad.add_color_stop_rgba(1.0 - p, 0.0, 0.0, 0.0, 0.0);
        grad.add_color_stop_rgba(1.0 - p, color.r, color.g, color.b, 1.0);
        grad.add_color_stop_rgba(1.0, color.r, color.g, color.b, 1.0);

        ctx.set_source(&grad).unwrap();
        ctx.rectangle(0.0, 0.0, width, height);
        ctx.fill().unwrap();
    }

    fn connect_clicked(
        target: &DrawingArea,
        percentage: Rc<Cell<f32>>,
        func: SliderFunc,
        icon: WeakRef<Image>,
    ) {
        let click = GestureClick::new();
        click.connect_pressed({
            let percentage = Rc::clone(&percentage);
            let icon = icon.clone();
            move |gesture, _, _, y| {
                let target = gesture.widget().and_downcast::<DrawingArea>().unwrap();
                let height = target.height() as f64;

                let new_p = (1.0 - (y / height)).clamp(0.0, 1.0) as f32;
                percentage.set(new_p);
                if let Some(icon) = icon.upgrade() {
                    icon.set_icon_name(Some(&func.icon_name(new_p)));
                }

                target.queue_draw();
            }
        });
        target.add_controller(click);
    }
    fn connect_drag(
        target: &DrawingArea,
        percentage: Rc<Cell<f32>>,
        func: SliderFunc,
        icon: WeakRef<Image>,
    ) {
        let drag = GestureDrag::new();

        drag.connect_drag_update({
            let percentage = Rc::clone(&percentage);
            move |gesture, _x, y| {
                let target = gesture.widget().and_downcast::<DrawingArea>().unwrap();
                if let Some((_, y_start)) = gesture.start_point() {
                    let current_y = y_start + y;
                    let height = target.height() as f64;

                    let new_p = (1.0 - (current_y / height)).clamp(0.0, 1.0) as f32;
                    percentage.set(new_p);

                    if let Some(icon) = icon.upgrade() {
                        icon.set_icon_name(Some(&func.icon_name(new_p)));
                    }

                    target.queue_draw();
                }
            }
        });

        target.add_controller(drag);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SliderFunc {
    #[default]
    None,
    Volume,
    Brightness,
}
impl SliderFunc {
    pub fn update_widgets(&self) {
        let app = gtk4::Application::default();
        if let Some(window) = app.active_window().and_downcast::<MainWindow>() {
            let state = window.imp().state.borrow();
            // let buttons = state.slider(*self);
            // buttons.for_each(|b| b.queue_draw());
        }
    }
    pub fn icon_name(&self, percentage: f32) -> String {
        match self {
            Self::Volume => match percentage {
                p if p > 0.67 => "audio-volume-high-symbolic",
                p if p > 0.34 => "audio-volume-medium-symbolic",
                p if p > 0.01 => "audio-volume-low-symbolic",
                _ => "audio-volume-muted-symbolic",
            },
            Self::Brightness => match percentage {
                p if p > 0.67 => "display-brightness-high-symbolic",
                p if p > 0.34 => "display-brightness-medium-symbolic",
                p if p > 0.01 => "display-brightness-low-symbolic",
                _ => "display-brightness-off-symbolic",
            },
            Self::None => "none",
        }
        .into()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SliderRange {
    min: i32,
    max: i32,
}
impl Default for SliderRange {
    fn default() -> Self {
        Self { min: 0, max: 100 }
    }
}
