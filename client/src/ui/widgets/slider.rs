use crate::{
    config::{WidgetBase, WidgetOrientation, WidgetSpec},
    ui::widgets::utils::{
        animation::*,
        interactives::WidgetBehavior,
        render::{CairoShapesExt, Rgba},
    },
};
use common::protocol::AtomicSystemState;
use gtk4::{
    Box as GtkBox, DrawingArea, GestureDrag, Image, Overlay, Widget,
    cairo::Context,
    glib::{
        WeakRef,
        object::{Cast, CastNone, ObjectExt},
    },
    prelude::{
        BoxExt, DrawingAreaExtManual, EventControllerExt, GestureDragExt, WidgetExt,
        WidgetExtManual,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::Instant,
};

pub struct Slider {
    pub weak: WeakRef<Widget>,
    pub func: Box<dyn WidgetBehavior>,
    pub edit_lock: Rc<Cell<bool>>,
}
impl Slider {
    pub fn queue_draw(&self) {
        if let Some(strong) = self.weak.upgrade() {
            strong.queue_draw();
        }
    }
}

pub struct SliderBuilder {
    area: DrawingArea,
    overlay: Overlay,
    func: Box<dyn WidgetBehavior>,
    edit_lock: Rc<Cell<bool>>,
}
impl SliderBuilder {
    pub fn new(specs: WidgetSpec, system_state: Arc<AtomicSystemState>, in_holder: bool) -> Self {
        let (base, func_spec, _range, orientation) = specs.as_slider().unwrap();
        let func = func_spec.build();

        let (overlay, area, icon) = match orientation {
            WidgetOrientation::Vertical => Self::vertical_ui(&base, &func, in_holder),
            WidgetOrientation::Horizontal => Self::horizontal_ui(&base, &func, in_holder),
        };

        if let Some(id) = base.id {
            area.set_widget_name(&id);
        }
        if let Some(class) = base.class {
            area.add_css_class(&class);
        }

        let animation_state = Rc::new(AnimationState::new());
        let edit_lock = Rc::new(Cell::new(false));

        Slider::connect_drag(
            &area,
            Arc::clone(&system_state),
            &func,
            orientation,
            icon,
            Rc::clone(&animation_state),
            Rc::clone(&edit_lock),
        );

        area.set_draw_func({
            let system_state = Arc::clone(&system_state);
            let func = func.clone();
            let animation_state = Rc::clone(&animation_state);
            move |area, ctx, w, h| match orientation {
                WidgetOrientation::Vertical => Slider::draw_vert(
                    area,
                    ctx,
                    w,
                    h,
                    Arc::clone(&system_state),
                    &func,
                    Rc::clone(&animation_state),
                ),
                WidgetOrientation::Horizontal => Slider::draw_horz(
                    area,
                    ctx,
                    w,
                    h,
                    Arc::clone(&system_state),
                    &func,
                    Rc::clone(&animation_state),
                ),
            }
        });
        area.add_tick_callback({
            let animation_state = Rc::clone(&animation_state);
            move |widget, frame_clock| {
                if !animation_state.running.get() {
                    return gtk4::glib::ControlFlow::Continue;
                }
                animation_state.update(frame_clock);
                widget.queue_draw();
                gtk4::glib::ControlFlow::Continue
            }
        });

        Self {
            area,
            overlay,
            func,
            edit_lock,
        }
    }
    pub fn vertical_ui(
        base: &WidgetBase,
        func: &Box<dyn WidgetBehavior>,
        in_holder: bool,
    ) -> (Overlay, DrawingArea, WeakRef<Image>) {
        let icon = func.icon_name(50).to_string();

        let builder = Overlay::builder()
            .css_classes(["widget", "slider", "vertical"])
            .overflow(gtk4::Overflow::Hidden)
            .height_request(200)
            .width_request(50);

        let overlay = if in_holder {
            builder
                .vexpand(true)
                .valign(base.valign.map(|d| d.into()).unwrap_or(gtk4::Align::Fill))
                .halign(base.halign.map(|d| d.into()).unwrap_or(gtk4::Align::Start))
        } else {
            builder
                .valign(base.valign.map(|d| d.into()).unwrap_or(gtk4::Align::Start))
                .halign(base.halign.map(|d| d.into()).unwrap_or(gtk4::Align::Start))
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

        (overlay, area, svg_icon.downgrade())
    }
    fn horizontal_ui(
        base: &WidgetBase,
        func: &Box<dyn WidgetBehavior>,
        in_holder: bool,
    ) -> (Overlay, DrawingArea, WeakRef<Image>) {
        let builder = Overlay::builder()
            .css_classes(["widget", "slider", "horizontal"])
            .hexpand(true)
            .height_request(50)
            .width_request(100)
            .valign(base.valign.map(|d| d.into()).unwrap_or(gtk4::Align::Center))
            .halign(base.halign.map(|d| d.into()).unwrap_or(gtk4::Align::Fill));

        // TODO: Check styling in and out of holder
        let overlay = if in_holder { builder } else { builder }.build();

        let icon_left = Image::builder()
            .icon_name(func.icon_name(10))
            .can_target(false)
            .build();

        let icon_right = Image::builder()
            .icon_name(func.icon_name(90))
            .can_target(false)
            .build();

        let area = DrawingArea::builder()
            .css_classes(["slider-obj"])
            .hexpand(true)
            .vexpand(true)
            .build();

        let content = GtkBox::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(5)
            .hexpand(true)
            .build();
        content.append(&icon_left);
        content.append(&area);
        content.append(&icon_right);

        overlay.set_child(Some(&content));

        (overlay, area, WeakRef::new())
    }
    pub fn for_box(self, container: &GtkBox) -> Self {
        container.append(&self.overlay);
        self
    }
    pub fn build(self) -> Slider {
        let weak = self.area.upcast::<Widget>().downgrade();
        Slider {
            weak,
            func: self.func,
            edit_lock: self.edit_lock,
        }
    }
}

impl Slider {
    fn draw_vert(
        _area: &DrawingArea,
        ctx: &Context,
        width: i32,
        height: i32,
        state: Arc<AtomicSystemState>,
        func: &Box<dyn WidgetBehavior>,
        _animation_state: Rc<AnimationState>,
    ) {
        let color: Rgba = _area.color().into();
        let percentage = func.get_percentage(&state) as f64 / 100.0;

        let h = height as f64;
        let w = width as f64;
        let fill_height = h * percentage;
        let y_start = h - fill_height;

        // Background
        ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
        ctx.rectangle(0.0, y_start, w, fill_height);
        ctx.fill().unwrap();
    }
    fn draw_horz(
        _area: &DrawingArea,
        ctx: &Context,
        width: i32,
        height: i32,
        state: Arc<AtomicSystemState>,
        func: &Box<dyn WidgetBehavior>,
        animation_state: Rc<AnimationState>,
    ) {
        let color: Rgba = _area.color().into();
        let percentage = func.get_percentage(&state) as f64 / 100.0;

        let progress = animation_state.progress.get();

        let thickness = 5.0 + 2.0 * progress;
        let h = height as f64;
        let w = width as f64;
        let fill_width = w * percentage;

        let bg_height = 5.0;
        let bg_y = (h - bg_height) / 2.0;

        let progress_y = (h - thickness) / 2.0;

        ctx.set_source_rgba(color.r, color.g, color.b, 0.3);
        CairoShapesExt::rounded_rectangle(
            ctx,
            0.0,
            bg_y,
            w,
            bg_height,
            (
                bg_height / 2.0,
                bg_height / 2.0,
                bg_height / 2.0,
                bg_height / 2.0,
            ),
        );
        ctx.fill().unwrap();

        ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
        CairoShapesExt::rounded_rectangle(
            ctx,
            0.0,
            progress_y,
            fill_width,
            thickness,
            (
                thickness / 2.0,
                thickness / 2.0,
                thickness / 2.0,
                thickness / 2.0,
            ),
        );
        ctx.fill().unwrap();
    }

    fn connect_drag(
        target: &DrawingArea,
        system_state: Arc<AtomicSystemState>,
        func: &Box<dyn WidgetBehavior>,
        orientation: WidgetOrientation,
        icon: WeakRef<Image>,
        animation_state: Rc<AnimationState>,
        edit_lock: Rc<Cell<bool>>,
    ) {
        let drag = GestureDrag::new();
        let perc = func.get_percentage(&system_state);

        drag.connect_drag_begin({
            let system_state = Arc::clone(&system_state);
            let icon = icon.clone();
            let animation_state = Rc::clone(&animation_state);
            let edit_lock = Rc::clone(&edit_lock);
            let func = func.clone();
            move |gesture, x, y| {
                edit_lock.set(true);
                let target = gesture.widget().and_downcast::<DrawingArea>().unwrap();
                target.add_css_class("moving");
                animation_state.start(AnimationDirection::Forward {
                    duration: 0.05,
                    function: EaseFunction::EaseIn,
                });

                let new_p = match orientation {
                    WidgetOrientation::Vertical => {
                        let height = target.height() as f64;

                        (1.0 - (y / height)).clamp(0.0, 1.0) as f32
                    }
                    WidgetOrientation::Horizontal => {
                        let width = target.width() as f64;
                        (x / width).clamp(0.0, 1.0) as f32
                    }
                };

                let new_percent = (new_p * 100.0) as u8;
                let next_icon = func.icon_name(new_percent);
                if let Some(icon_widget) = icon.upgrade() {
                    icon_widget.set_icon_name(Some(&next_icon));
                }

                func.set_percentage(&system_state, new_percent);
                func.execute(&system_state);
                target.queue_draw();
            }
        });

        drag.connect_drag_update({
            let system_state = Arc::clone(&system_state);
            let last_seen_percent = Rc::new(Cell::new(0u8));
            let last_sent_time = Rc::new(Cell::new(Instant::now()));
            let last_seen_icon = Rc::new(RefCell::new(func.icon_name(perc)));
            let func = func.clone();
            move |gesture, x, y| {
                let target = gesture.widget().and_downcast::<DrawingArea>().unwrap();

                if let Some((x_start, y_start)) = gesture.start_point() {
                    let new_p = match orientation {
                        WidgetOrientation::Vertical => {
                            let current_y = y_start + y;
                            let height = target.height() as f64;

                            (1.0 - (current_y / height)).clamp(0.0, 1.0) as f32
                        }
                        WidgetOrientation::Horizontal => {
                            let current_x = x_start + x;
                            let width = target.width() as f64;
                            (current_x / width).clamp(0.0, 1.0) as f32
                        }
                    };

                    let new_percent = (new_p * 100.0) as u8;
                    func.set_percentage(&system_state, new_percent);
                    let now = Instant::now();

                    let elapsed = now.duration_since(last_sent_time.get()).as_millis();
                    let diff = last_seen_percent.get().abs_diff(new_percent);
                    if diff >= 1 {
                        // efficient icon replace logic
                        let next_icon = func.icon_name(new_percent);
                        if *last_seen_icon.borrow() != next_icon {
                            if let Some(icon_widget) = icon.upgrade() {
                                icon_widget.set_icon_name(Some(&next_icon));
                            }
                            last_seen_icon.replace(next_icon);
                        }
                        target.queue_draw();

                        if elapsed > 100 {
                            last_seen_percent.set(new_percent);
                            last_sent_time.set(now);
                            func.execute(&system_state);
                        }
                    }
                }
            }
        });
        drag.connect_drag_end({
            let system_state = Arc::clone(&system_state);
            let animation_state = Rc::clone(&animation_state);
            let edit_lock = Rc::clone(&edit_lock);
            let func = func.clone();
            move |gesture, _, _| {
                edit_lock.set(false);
                let target = gesture.widget().and_downcast::<DrawingArea>().unwrap();
                target.remove_css_class("moving");

                animation_state.start(AnimationDirection::Backward {
                    duration: 0.1,
                    function: EaseFunction::EaseOut,
                });
                func.as_request(&system_state);
            }
        });

        target.add_controller(drag);
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
