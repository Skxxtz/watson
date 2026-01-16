use crate::{
    config::{WidgetOrientation, WidgetSpec},
    ui::widgets::utils::{
        AnimationDirection, AnimationState, BackendFunc, CairoShapesExt, EaseFunction, Rgba,
        WidgetOption,
    },
};
use common::protocol::AtomicSystemState;
use gtk4::{
    Box, DrawingArea, GestureDrag, Image, Overlay,
    cairo::Context,
    glib::{
        WeakRef,
        object::{CastNone, ObjectExt},
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

#[derive(Clone, Debug)]
pub struct Slider {
    pub weak: WeakRef<DrawingArea>,
    pub func: BackendFunc,
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
    ui: WidgetOption<DrawingArea>,
    overlay: WidgetOption<Overlay>,
    func: BackendFunc,
    edit_lock: Rc<Cell<bool>>,
}
impl SliderBuilder {
    pub fn new(specs: &WidgetSpec, system_state: Arc<AtomicSystemState>, in_holder: bool) -> Self {
        let (_, _, _, _range, orientation) = specs.as_slider().unwrap();

        match orientation {
            WidgetOrientation::Vertical => Self::vertical(specs, system_state, in_holder),
            WidgetOrientation::Horizontal => Self::horizontal(specs, system_state, in_holder),
        }
    }
    pub fn vertical(
        specs: &WidgetSpec,
        system_state: Arc<AtomicSystemState>,
        in_holder: bool,
    ) -> Self {
        let (base, func, icon, _range, _) = specs.as_slider().unwrap();

        let icon = icon.unwrap_or(func.icon_name(0.5));

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

        if let Some(id) = specs.id() {
            area.set_widget_name(id);
        }
        if let Some(class) = specs.class() {
            area.add_css_class(class);
        }

        // Setup animations
        let animation_state = Rc::new(AnimationState::new());
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

        let edit_lock = Rc::new(Cell::new(false));
        Slider::connect_drag(
            &area,
            Arc::clone(&system_state),
            *func,
            WidgetOrientation::Vertical,
            svg_icon.downgrade(),
            Rc::clone(&animation_state),
            Rc::clone(&edit_lock),
        );

        area.set_draw_func({
            let func = func.clone();
            let system_state = Arc::clone(&system_state);
            move |area, ctx, width, height| {
                Slider::draw_vert(
                    area,
                    ctx,
                    width,
                    height,
                    Arc::clone(&system_state),
                    func,
                    Rc::clone(&animation_state),
                );
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
            edit_lock,
        }
    }
    pub fn horizontal(
        specs: &WidgetSpec,
        system_state: Arc<AtomicSystemState>,
        in_holder: bool,
    ) -> Self {
        let (base, func, _, _range, _) = specs.as_slider().unwrap();

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
            .icon_name(func.icon_name(0.1))
            .can_target(false)
            .build();

        let icon_right = Image::builder()
            .icon_name(func.icon_name(0.9))
            .can_target(false)
            .build();

        let area = DrawingArea::builder()
            .css_classes(["slider-obj"])
            .hexpand(true)
            .vexpand(true)
            .build();

        let content = Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(5)
            .hexpand(true)
            .build();
        content.append(&icon_left);
        content.append(&area);
        content.append(&icon_right);

        overlay.set_child(Some(&content));

        if let Some(id) = specs.id() {
            area.set_widget_name(id);
        }
        if let Some(class) = specs.class() {
            area.add_css_class(class);
        }

        // Setup animations
        let animation_state = Rc::new(AnimationState::new());
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

        let edit_lock = Rc::new(Cell::new(false));
        Slider::connect_drag(
            &area,
            Arc::clone(&system_state),
            *func,
            WidgetOrientation::Horizontal,
            WeakRef::default(),
            Rc::clone(&animation_state),
            Rc::clone(&edit_lock),
        );

        area.set_draw_func({
            let func = func.clone();
            let system_state = Arc::clone(&system_state);
            move |area, ctx, width, height| {
                Slider::draw_horz(
                    area,
                    ctx,
                    width,
                    height,
                    Arc::clone(&system_state),
                    func,
                    Rc::clone(&animation_state),
                );
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
            edit_lock,
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
        func: BackendFunc,
        _animation_state: Rc<AnimationState>,
    ) {
        let color: Rgba = _area.color().into();
        let percentage = func.percentage(&state) as f64 / 100.0;

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
        func: BackendFunc,
        animation_state: Rc<AnimationState>,
    ) {
        let color: Rgba = _area.color().into();
        let percentage = func.percentage(&state) as f64 / 100.0;

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
        func: BackendFunc,
        orientation: WidgetOrientation,
        icon: WeakRef<Image>,
        animation_state: Rc<AnimationState>,
        edit_lock: Rc<Cell<bool>>,
    ) {
        let drag = GestureDrag::new();
        let perc = func.percentage(&system_state) as f32 / 100.0;

        drag.connect_drag_begin({
            let system_state = Arc::clone(&system_state);
            let icon = icon.clone();
            let animation_state = Rc::clone(&animation_state);
            let edit_lock = Rc::clone(&edit_lock);
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

                let next_icon = func.icon_name(new_p);
                if let Some(icon_widget) = icon.upgrade() {
                    icon_widget.set_icon_name(Some(&next_icon));
                }

                let new_percent = (new_p * 100.0) as u8;
                func.set_percentage(&system_state, new_percent);
                func.execute(Arc::clone(&system_state));
                target.queue_draw();
            }
        });

        drag.connect_drag_update({
            let system_state = Arc::clone(&system_state);
            let last_seen_percent = Rc::new(Cell::new(0u8));
            let last_sent_time = Rc::new(Cell::new(Instant::now()));
            let last_seen_icon = Rc::new(RefCell::new(func.icon_name(perc)));
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
                        let next_icon = func.icon_name(new_p);
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
                            func.execute(Arc::clone(&system_state));
                        }
                    }
                }
            }
        });
        drag.connect_drag_end({
            let system_state = Arc::clone(&system_state);
            let animation_state = Rc::clone(&animation_state);
            let edit_lock = Rc::clone(&edit_lock);
            move |gesture, _, _| {
                edit_lock.set(false);
                let target = gesture.widget().and_downcast::<DrawingArea>().unwrap();
                target.remove_css_class("moving");

                animation_state.start(AnimationDirection::Backward {
                    duration: 0.1,
                    function: EaseFunction::EaseOut,
                });
                let final_percent = func.percentage(&system_state);
                func.set_percentage(&system_state, final_percent);
                func.execute(Arc::clone(&system_state));
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
