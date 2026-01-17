use gtk4::{
    Box, DrawingArea, EventControllerKey, GestureClick, Stack,
    glib::object::ObjectExt,
    prelude::{
        BoxExt, DrawingAreaExtManual, EventControllerExt, GestureSingleExt, WidgetExt,
        WidgetExtManual,
    },
};
use std::{cell::RefCell, rc::Rc};

use crate::{
    config::WidgetSpec,
    ui::{
        g_templates::event_details::EventDetails,
        widgets::{
            Calendar,
            calendar::{
                CalendarContext, CalendarRenderer, cache::CalendarCache,
                data_store::CalendarDataStore,
            },
            utils::animation::{AnimationDirection, AnimationState, EaseFunction},
        },
    },
};

#[derive(Default)]
pub struct CalendarBuilder {
    area: DrawingArea,
    stack: Stack,
    details: EventDetails,
    animation_state: Rc<AnimationState>,
    data_store: Rc<CalendarDataStore>,
    context: Rc<RefCell<CalendarContext>>,
}
impl CalendarBuilder {
    pub fn new() -> Self {
        let stack = Stack::builder()
            .vexpand(false)
            .hexpand(false)
            .overflow(gtk4::Overflow::Hidden)
            .css_classes(["widget", "calendar-holder"])
            .width_request(400)
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .transition_duration(250)
            .interpolate_size(true)
            .build();

        let area = DrawingArea::builder()
            .vexpand(true)
            .hexpand(true)
            .css_classes(["inner-widget", "calendar"])
            .build();

        stack.add_named(&area, Some("calendar"));

        let details = EventDetails::new();
        stack.add_named(&details, Some("details"));

        Self {
            stack,
            area,
            details,
            animation_state: Rc::new(AnimationState::new()),
            data_store: Rc::new(CalendarDataStore::new()),
            context: Rc::new(RefCell::new(CalendarContext::new())),
        }
    }
    pub fn for_spec(self, specs: &WidgetSpec) -> Self {
        let WidgetSpec::Calendar {
            base,
            hours_past,
            hours_future,
            ..
        } = specs
        else {
            return self;
        };
        self.context.borrow_mut().for_specs(specs);
        self.data_store.for_specs(specs);

        // Calculate height
        let span = (hours_past + hours_future).clamp(1, 24);
        let tmp = 200 + span as i32 * 50;
        let height = tmp - tmp % 100;

        // Initialize UI
        self.stack
            .set_valign(base.valign.map(|d| d.into()).unwrap_or(gtk4::Align::Start));
        self.stack
            .set_halign(base.halign.map(|d| d.into()).unwrap_or(gtk4::Align::Start));
        self.stack.set_height_request(height);

        if let Some(id) = specs.id() {
            self.area.set_widget_name(id);
        }
        if let Some(class) = specs.class() {
            self.area.add_css_class(class);
        }

        self
    }
    /// Attatches all drawing related functions:
    /// - set_draw_func
    /// - add_tick_callback
    /// - redrawing
    fn connect_draw(&self) {
        // Genral draw function
        self.area.set_draw_func({
            let data_store = Rc::clone(&self.data_store);
            let state = Rc::clone(&self.animation_state);
            let context = Rc::clone(&self.context);
            move |area, ctx, width, height| {
                let mut context = context.borrow_mut();

                if context.needs_init {
                    // Measure time label once for offset
                    ctx.select_font_face(
                        &context.font,
                        gtk4::cairo::FontSlant::Normal,
                        gtk4::cairo::FontWeight::Bold,
                    );
                    ctx.set_font_size(12.0);
                    let ext = ctx.text_extents("00:00").unwrap();
                    context.line_offset = ext.width() + 10.0;

                    let events_timed = data_store.timed.borrow();
                    context.update(area, width as f64, height as f64, events_timed.len());
                    context.cache.hitboxes =
                        CalendarCache::calculate_hitboxes(&*events_timed, &context);
                    context.cache.last_window_start = context.window_start;
                    area.queue_draw();
                }
                let renderer = CalendarRenderer::new(ctx, &context, state.progress.get());
                renderer.draw_all(Rc::clone(&data_store));
            }
        });

        // Tick callback for animations
        self.area.add_tick_callback({
            let state = Rc::clone(&self.animation_state);
            move |widget, frame_clock| {
                if !state.running.get() {
                    return gtk4::glib::ControlFlow::Continue;
                }
                state.update(frame_clock);
                widget.queue_draw();
                gtk4::glib::ControlFlow::Continue
            }
        });

        // Minute interval redraw
        gtk4::glib::timeout_add_seconds_local(60, {
            let calendar_ref = self.area.downgrade();
            let context = Rc::clone(&self.context);
            let data_store = Rc::clone(&self.data_store);
            move || {
                if let Some(area) = calendar_ref.upgrade() {
                    let mut context = context.borrow_mut();
                    let events_timed = data_store.timed.borrow();
                    let w = area.width() as f64;
                    let h = area.height() as f64;
                    context.update(&area, w, h, events_timed.len());

                    if context.is_dirty(w, h) {
                        context.cache.hitboxes =
                            CalendarCache::calculate_hitboxes(&*events_timed, &context);
                        area.queue_draw();
                    }

                    gtk4::glib::ControlFlow::Continue
                } else {
                    gtk4::glib::ControlFlow::Break
                }
            }
        });
    }
    fn connect_signals(&self) {
        // Create a GestureClick controller
        let click = GestureClick::new();
        click.set_button(0);

        // Connect to the clicked signal
        click.connect_pressed({
            let stack_weak = self.stack.downgrade();
            let details_weak = self.details.downgrade();
            let data_store = Rc::clone(&self.data_store);
            let context = Rc::clone(&self.context);
            move |_gesture, _n_press, x, y| {
                let Some(stack) = stack_weak.upgrade() else {
                    return;
                };
                let Some(details) = details_weak.upgrade() else {
                    return;
                };

                let ctx_borrow = context.borrow();
                let events_borrow = data_store.timed.borrow();
                let hit =
                    ctx_borrow.cache.hitboxes.iter().rev().find(|hb| {
                        x >= hb.x && x <= (hb.x + hb.w) && y >= hb.y && y <= (hb.y + hb.h)
                    });

                if let Some(hitbox) = hit {
                    if let Some(event) = events_borrow.get(hitbox.index) {
                        stack.set_visible_child_name("details");
                        details.grab_focus();
                        details.set_event(event);
                    }
                }
            }
        });
        self.area.add_controller(click);

        let click = GestureClick::new();
        click.set_button(0);
        click.connect_pressed({
            let stack_weak = self.stack.downgrade();
            move |_gesture, _n_press, _x, _y| {
                let Some(stack) = stack_weak.upgrade() else {
                    return;
                };

                stack.set_visible_child_name("calendar");
            }
        });
        self.details.add_controller(click);

        let controller = EventControllerKey::new();
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        controller.connect_key_pressed({
            let stack = self.stack.downgrade();
            move |_gesture, key, _keycode, _state| {
                if key == gtk4::gdk::Key::Escape {
                    if let Some(stack) = stack.upgrade() {
                        stack.set_visible_child_name("calendar");
                        return gtk4::glib::Propagation::Stop;
                    }
                }
                gtk4::glib::Propagation::Proceed
            }
        });
        self.details.add_controller(controller);
    }
    /// Loads first batch of events and handles async fetching of remote events
    /// WARING: Has to be called after drawing is attatched! Otherwise, drawing of cached events
    /// will fail.
    fn attatch_refresh(&self) {
        // Draw first events once the stack shows
        let _ = self.data_store.load_from_cache();
        self.animation_state.start(AnimationDirection::Forward {
            duration: 0.7,
            function: EaseFunction::EaseOutCubic,
        });

        // Get the calendar events async once the application finished starting up
        gtk4::glib::idle_add_local_full(gtk4::glib::Priority::LOW, {
            let animation_state = Rc::clone(&self.animation_state);
            let data_store = Rc::clone(&self.data_store);
            let context = Rc::clone(&self.context);
            move || {
                gtk4::glib::MainContext::default().spawn_local({
                    let animation_state = Rc::clone(&animation_state);
                    let data_store = Rc::clone(&data_store);
                    let context = Rc::clone(&context);
                    async move {
                        let num_changes = data_store.refresh().await;
                        if num_changes > 0 {
                            let mut context = context.borrow_mut();
                            context.cache.hitboxes = CalendarCache::calculate_hitboxes(
                                &*data_store.timed.borrow(),
                                &context,
                            );
                            context.cache.last_window_start = context.window_start;
                            // Internally ques draw
                            animation_state.start(AnimationDirection::Forward {
                                duration: 0.7,
                                function: EaseFunction::EaseOutCubic,
                            });
                        }
                    }
                });
                gtk4::glib::ControlFlow::Break
            }
        });
    }

    pub fn for_box(self, container: &Box) -> Self {
        container.append(&self.stack);
        self
    }

    pub fn build(self) -> Calendar {
        // Draw function
        self.connect_draw();

        // User event handlers
        self.connect_signals();

        // Handle event loading
        self.attatch_refresh();

        Calendar {
            area: self.area.downgrade(),
            stack: self.stack.downgrade(),
            details: self.details.downgrade(),
        }
    }
}
