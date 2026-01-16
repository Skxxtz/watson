use crate::{
    config::{CalendarHMFormat, CalendarRule, WidgetSpec},
    ui::{
        g_templates::event_details::EventDetails,
        widgets::utils::{AnimationDirection, AnimationState, EaseFunction, WidgetOption},
    },
};
use std::{
    cell::RefCell,
    collections::HashSet,
    fs,
    io::{BufReader, BufWriter},
    rc::Rc,
    str::FromStr,
};

use chrono::{Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use gtk4::{
    Align, Box, DrawingArea, EventControllerKey, GestureClick, Stack,
    cairo::{Context, FontSlant, FontWeight},
    glib::{WeakRef, object::ObjectExt},
    prelude::{
        BoxExt, DrawingAreaExtManual, EventControllerExt, GestureSingleExt, WidgetExt,
        WidgetExtManual,
    },
};

use crate::ui::widgets::utils::{CairoShapesExt, Rgba};
use common::{
    auth::CredentialManager,
    calendar::utils::{CalDavEvent, CalEventType},
    utils::{
        errors::{WatsonError, WatsonErrorKind},
        paths::get_cache_dir,
    },
    watson_err,
};

#[derive(Debug)]
pub struct EventHitbox {
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub has_neighbor_above: bool,
}
#[derive(Default)]
pub struct CalendarCache {
    pub last_width: f64,
    pub last_height: f64,
    pub last_window_start: NaiveDateTime,
    pub hitboxes: Vec<EventHitbox>,
}
pub struct CalendarConfig<'w> {
    pub accent_color: &'w str,
    pub font: &'w str,
    pub hm_format: &'w CalendarHMFormat,
    pub hours_past: u8,
    pub hours_future: u8,
}
struct CalendarContext {
    font: String,
    padding: f64,
    padding_top: f64,

    text: Rgba,
    accent: Rgba,

    inner_width: f64,
    inner_height: f64,
    line_offset: f64,

    todate: NaiveDate,
    window_start: NaiveDateTime,
    window_end: NaiveDateTime,
    hours_to_show: u32,
    hours_past: u8,
    total_seconds: f64,

    hm_format: CalendarHMFormat,

    cache: CalendarCache,
}
impl CalendarContext {
    fn new_time_window(
        hours_to_show: u32,
        hours_past: u8,
    ) -> (NaiveDate, NaiveDateTime, NaiveDateTime) {
        let today = Local::now();
        let todate = today.date_naive();
        let now = today.time();

        // Determine window start/end
        let now_hour = now.hour();
        let start_hour = if now_hour + hours_to_show > 24 {
            24 - hours_to_show
        } else {
            now_hour.saturating_sub(hours_past as u32)
        }
        .min(23);

        let window_start = todate.and_time(NaiveTime::from_hms_opt(start_hour, 0, 0).unwrap());
        let window_end = window_start + Duration::hours(hours_to_show as i64);

        (todate, window_start, window_end)
    }
    pub fn new(spec: &Rc<WidgetSpec>) -> Self {
        let default_format = CalendarHMFormat {
            timeline: "%H:%M".to_string(),
            event: "H:%M".to_string(),
        };
        let CalendarConfig {
            accent_color,
            font,
            hm_format,
            hours_past,
            hours_future,
        } = spec.as_calendar(&default_format);

        // Calculations
        let hours_to_show = (hours_past + hours_future).clamp(1, 24) as u32;
        let (todate, window_start, window_end) = Self::new_time_window(hours_to_show, hours_past);

        Self {
            accent: Rgba::from_str(accent_color).unwrap_or_default(),
            text: Rgba::default(),
            font: font.to_string(),
            padding: 0.0,
            padding_top: 0.0,
            inner_width: 0.0,
            inner_height: 0.0,
            line_offset: 0.0,
            todate,
            window_start,
            window_end,
            hours_to_show,
            hours_past,
            total_seconds: (window_end - window_start).num_seconds() as f64,
            hm_format: hm_format.clone(),
            cache: CalendarCache::default(),
        }
    }
    fn update(
        &mut self,
        area: &DrawingArea,
        ctx: &Context,
        width: f64,
        height: f64,
        num_events: usize,
    ) {
        self.text = area.color().into();

        self.padding = (width as f64 * 0.05).min(20.0);
        self.padding_top = if num_events != 0 { 120.0 } else { 100.0 };
        self.inner_width = width - 2.0 * self.padding;
        self.inner_height = height - self.padding - self.padding_top;

        // Date Calulations
        let (todate, window_start, window_end) =
            Self::new_time_window(self.hours_to_show, self.hours_past);
        self.todate = todate;
        self.window_start = window_start;
        self.window_end = window_end;

        // Measure time label once for offset
        ctx.select_font_face(&self.font, FontSlant::Normal, FontWeight::Bold);
        ctx.set_font_size(12.0);
        let ext = ctx.text_extents("00:00").unwrap();
        self.line_offset = ext.width() + 10.0;
    }
    fn is_dirty(&self, width: f64, height: f64) -> bool {
        self.cache.hitboxes.is_empty()
            || self.cache.last_width != width
            || self.cache.last_height != height
            || self.cache.last_window_start != self.window_start
    }
}

#[derive(Default)]
pub struct CalendarBuilder {
    area: WidgetOption<DrawingArea>,
    stack: WidgetOption<Stack>,
    details: WidgetOption<EventDetails>,
}
impl CalendarBuilder {
    fn connect_signals(
        area: &DrawingArea,
        stack: &Stack,
        details: &EventDetails,
        context: Rc<RefCell<CalendarContext>>,
        data: Rc<CalendarDataStore>,
    ) {
        // Create a GestureClick controller
        let click = GestureClick::new();
        click.set_button(0);

        // Connect to the clicked signal
        click.connect_pressed({
            let stack_weak = stack.downgrade();
            let details_weak = details.downgrade();
            move |_gesture, _n_press, x, y| {
                let Some(stack) = stack_weak.upgrade() else {
                    return;
                };
                let Some(details) = details_weak.upgrade() else {
                    return;
                };

                let ctx_borrow = context.borrow();
                let events_borrow = data.timed.borrow();
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
        area.add_controller(click);

        let click = GestureClick::new();
        click.set_button(0);
        click.connect_pressed({
            let stack_weak = stack.downgrade();
            move |_gesture, _n_press, _x, _y| {
                let Some(stack) = stack_weak.upgrade() else {
                    return;
                };

                stack.set_visible_child_name("calendar");
            }
        });
        details.add_controller(click);

        let controller = EventControllerKey::new();
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        controller.connect_key_pressed({
            let stack = stack.downgrade();
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
        details.add_controller(controller);
    }
    pub fn for_spec(mut self, specs: &WidgetSpec) -> Self {
        let state = Rc::new(AnimationState::new());
        let specs = Rc::new(specs.clone());
        let data_store = Rc::new(CalendarDataStore::new(&specs));
        let context = Rc::new(RefCell::new(CalendarContext::new(&specs)));

        let base = specs.base();
        let _ = data_store.load_from_cache();

        let mut height = 400;

        if let WidgetSpec::Calendar {
            hours_past,
            hours_future,
            ..
        } = specs.as_ref()
        {
            let span = (hours_past + hours_future).clamp(1, 24);
            let tmp = 200 + span as i32 * 50;
            height = tmp - tmp % 100;
        };

        let stack = Stack::builder()
            .vexpand(false)
            .hexpand(false)
            .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Start))
            .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Start))
            .overflow(gtk4::Overflow::Hidden)
            .css_classes(["widget", "calendar-holder"])
            .width_request(400)
            .height_request(height)
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .transition_duration(250)
            .interpolate_size(true)
            .build();

        let calendar_area = DrawingArea::builder()
            .vexpand(true)
            .hexpand(true)
            .css_classes(["inner-widget", "calendar"])
            .build();
        stack.add_named(&calendar_area, Some("calendar"));

        let detail_box = EventDetails::new();
        stack.add_named(&detail_box, Some("details"));

        if let Some(id) = specs.id() {
            calendar_area.set_widget_name(id);
        }
        if let Some(class) = specs.class() {
            calendar_area.add_css_class(class);
        }

        // Draw function
        calendar_area.set_draw_func({
            let data_store = Rc::clone(&data_store);
            let state = Rc::clone(&state);
            let context = Rc::clone(&context);
            move |area, ctx, width, height| {
                let mut context = context.borrow_mut();
                let w = width as f64;
                let h = height as f64;

                {
                    let events_timed = data_store.timed.borrow();
                    context.update(area, ctx, w, h, events_timed.len());

                    if context.is_dirty(w, h) {
                        context.cache.hitboxes = compute_event_hitboxes(&*events_timed, &context);
                        context.cache.last_window_start = context.window_start;
                    }
                }

                Calendar::draw(
                    area,
                    ctx,
                    &context,
                    Rc::clone(&data_store),
                    Rc::clone(&state),
                );
            }
        });

        Self::connect_signals(
            &calendar_area,
            &stack,
            &detail_box,
            Rc::clone(&context),
            Rc::clone(&data_store),
        );

        // Minute interval redraw
        gtk4::glib::timeout_add_seconds_local(60, {
            let calendar_ref = calendar_area.downgrade();
            move || {
                if let Some(cal) = calendar_ref.upgrade() {
                    cal.queue_draw();
                }
                gtk4::glib::ControlFlow::Continue
            }
        });

        calendar_area.add_tick_callback({
            let state = Rc::clone(&state);
            move |widget, frame_clock| {
                if !state.running.get() {
                    return gtk4::glib::ControlFlow::Continue;
                }
                state.update(frame_clock);
                widget.queue_draw();
                gtk4::glib::ControlFlow::Continue
            }
        });
        state.start(AnimationDirection::Forward {
            duration: 0.0,
            function: EaseFunction::None,
        });

        // Get the calendar events async
        gtk4::glib::MainContext::default().spawn_local({
            let state = Rc::clone(&state);
            async move {
                let _ = data_store.refresh().await;

                // Internally ques draw
                state.start(AnimationDirection::Forward {
                    duration: 0.7,
                    function: EaseFunction::EaseOutCubic,
                });
            }
        });

        self.area = WidgetOption::Owned(calendar_area);
        self.stack = WidgetOption::Owned(stack);
        self.details = WidgetOption::Owned(detail_box);
        self
    }

    pub fn for_box(self, container: &Box) -> Self {
        match &self.stack {
            WidgetOption::Owned(stack) => {
                container.append(stack);
            }
            WidgetOption::Borrowed(weak) => {
                if let Some(stack) = weak.upgrade() {
                    container.append(&stack);
                }
            }
        }
        self
    }

    pub fn build(self) -> Calendar {
        Calendar {
            area: self.area.weak(),
            stack: self.stack.weak(),
            details: self.details.weak(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Calendar {
    pub area: WeakRef<DrawingArea>,
    pub stack: WeakRef<Stack>,
    pub details: WeakRef<EventDetails>,
}
impl Calendar {
    pub fn builder() -> CalendarBuilder {
        CalendarBuilder::default()
    }
    fn draw(
        _area: &DrawingArea,
        ctx: &Context,
        calendar_context: &CalendarContext,
        data_store: Rc<CalendarDataStore>,
        state: Rc<AnimationState>,
    ) {
        // Header: Date and Weekday
        ctx.set_source_rgb(
            calendar_context.text.r,
            calendar_context.text.g,
            calendar_context.text.b,
        );
        ctx.select_font_face(
            &calendar_context.font,
            FontSlant::Normal,
            FontWeight::Normal,
        );
        ctx.set_font_size(50.0);
        let today_string = calendar_context.todate.format("%b %-d").to_string();
        let ext1 = ctx.text_extents(&today_string).unwrap();
        ctx.move_to(
            calendar_context.padding,
            calendar_context.padding + ext1.height(),
        );
        ctx.show_text(&today_string).unwrap();

        ctx.set_source_rgba(
            calendar_context.accent.r,
            calendar_context.accent.g,
            calendar_context.accent.b,
            calendar_context.accent.a,
        );
        ctx.set_font_size(15.0);
        let weekday_string = calendar_context.todate.format("%A").to_string();
        ctx.move_to(
            calendar_context.padding,
            calendar_context.padding + ext1.height() + 20.0,
        );
        ctx.show_text(&weekday_string).unwrap();

        // Hour lines and timeline
        ctx.set_line_width(0.5);
        ctx.set_line_cap(gtk4::cairo::LineCap::Round);

        for offset in 0..=calendar_context.hours_to_show {
            let y = (offset as f64 / calendar_context.hours_to_show as f64)
                * calendar_context.inner_height
                + calendar_context.padding_top;
            let hour = (calendar_context.window_start.hour() + offset as u32) % 24;

            // Draw hour line
            ctx.set_source_rgba(
                calendar_context.text.r,
                calendar_context.text.g,
                calendar_context.text.b,
                0.2,
            ); // Low alpha for grid lines
            ctx.move_to(calendar_context.padding + calendar_context.line_offset, y);
            ctx.line_to(calendar_context.inner_width + calendar_context.padding, y);
            ctx.stroke().unwrap();

            // Draw hour label
            ctx.set_source_rgb(
                calendar_context.text.r,
                calendar_context.text.g,
                calendar_context.text.b,
            );
            ctx.set_font_size(12.0);
            let label = NaiveTime::from_hms_opt(hour, 0, 0)
                .unwrap()
                .format(&calendar_context.hm_format.timeline)
                .to_string();
            CairoShapesExt::vert_centered_text(ctx, &label, calendar_context.padding, y);
        }

        // Drawing events
        let mut allday_x = 0.0;
        for event in data_store.allday.borrow().iter() {
            draw_allday_event(ctx, event, &calendar_context, &mut allday_x);
        }

        for hitbox in calendar_context.cache.hitboxes.iter() {
            if let Some(event) = data_store.timed.borrow().get(hitbox.index) {
                draw_event(ctx, hitbox, event, calendar_context, state.progress.get());
            }
        }

        // Current time indicator
        let now_full = Local::now().naive_local();
        if now_full >= calendar_context.window_start && now_full <= calendar_context.window_end {
            let current_y = (now_full - calendar_context.window_start).num_seconds() as f64
                / calendar_context.total_seconds
                * calendar_context.inner_height
                + calendar_context.padding_top;
            let x_start = calendar_context.padding + calendar_context.line_offset - 6.0;

            ctx.set_source_rgba(
                calendar_context.accent.r,
                calendar_context.accent.g,
                calendar_context.accent.b,
                calendar_context.accent.a,
            );
            ctx.set_line_width(2.0);
            ctx.move_to(x_start, current_y);
            ctx.line_to(
                calendar_context.inner_width + calendar_context.padding,
                current_y,
            );
            ctx.stroke().unwrap();

            CairoShapesExt::circle(ctx, x_start, current_y, 3.0);
            ctx.fill().unwrap();
        }
    }
}

fn draw_event(
    ctx: &Context,
    hitbox: &EventHitbox,
    event: &CalDavEvent,
    context: &CalendarContext,
    progress: f64,
) -> Option<()> {
    let EventHitbox {
        x,
        y,
        w,
        h,
        has_neighbor_above,
        ..
    } = *hitbox;

    // Animation State
    let alpha = if event.seen.get() {
        1.0
    } else {
        if progress >= 1.0 {
            event.seen.set(true);
        }
        progress
    };

    // Coloring
    let color_str = event.calendar_info.color.as_deref().unwrap_or("#e9a949");
    let base_color = Rgba::from_str(color_str).unwrap_or_default();

    // Background
    let rad = if h > context.inner_height {
        (6.0, 6.0, 0.0, 0.0)
    } else {
        (6.0, 6.0, 6.0, 6.0)
    };
    let neightbor_offset = if has_neighbor_above { 2.5 } else { 0.0 };

    CairoShapesExt::rounded_rectangle(ctx, x, y + neightbor_offset, w, h - neightbor_offset, rad);

    // Set background
    let grad = gtk4::cairo::LinearGradient::new(x, y, x, y + h);
    grad.add_color_stop_rgba(0.0, base_color.r, base_color.g, base_color.b, 0.45 * alpha);
    grad.add_color_stop_rgba(1.0, base_color.r, base_color.g, base_color.b, 0.35 * alpha);

    ctx.set_source(&grad).unwrap();
    ctx.fill_preserve().unwrap();

    // Border
    ctx.save().unwrap();
    let rim_grad = gtk4::cairo::LinearGradient::new(x, y, x, y + h);
    rim_grad.add_color_stop_rgba(0.0, base_color.r, base_color.g, base_color.b, 0.5 * alpha);
    rim_grad.add_color_stop_rgba(0.5, base_color.r, base_color.g, base_color.b, 0.1 * alpha);
    rim_grad.add_color_stop_rgba(1.0, base_color.r, base_color.g, base_color.b, 0.3 * alpha);

    ctx.set_line_width(1.0);
    ctx.set_source(&rim_grad).unwrap();
    ctx.stroke().unwrap();
    ctx.restore().unwrap();

    // Text Content
    let summary = &event.title;
    let time_str = event
        .start
        .as_ref()
        .map(|s| format!("{}", s.local().format(&context.hm_format.event)))
        .unwrap_or_default();

    // Label
    ctx.set_source_rgba(base_color.r, base_color.g, base_color.b, 0.8 * alpha); // Dark text for light background
    ctx.select_font_face(&context.font, FontSlant::Normal, FontWeight::Bold);

    let is_tiny_event = h < 30.0;
    let padding_x = 10.0;

    if is_tiny_event {
        // Single line layout: [ Summary ... Time ]
        ctx.set_font_size(10.0);
        let cy = y + (h / 2.0);

        // Only draw time if lane is wide enough
        // let _time_extents = ctx.text_extents(&time_str).unwrap();
        if w > 100.0 {
            CairoShapesExt::rjust_text(ctx, &time_str, x + w - 8.0, cy, true);
        }

        // Draw summary with clipping (implicit)
        CairoShapesExt::vert_centered_text(ctx, &summary, x + padding_x, cy);
    } else {
        // Multi-line layout
        ctx.set_font_size(11.0);
        ctx.move_to(x + padding_x, y + 16.0);
        ctx.show_text(&summary).unwrap();

        ctx.set_font_size(9.0);
        ctx.select_font_face(&context.font, FontSlant::Normal, FontWeight::Normal);

        // Time below title
        ctx.move_to(x + padding_x, y + 28.0);
        ctx.show_text(&time_str).unwrap();

        // Location at bottom or 3rd line
        if let Some(loc) = &event.location {
            if h > 45.0 {
                let loc_width = ctx.text_extents(&loc).unwrap().width();
                let inner_width = w - 2.0 * padding_x;
                let text = if loc_width > inner_width {
                    let avg_char_width = loc_width / loc.len() as f64;
                    let take_chars = (inner_width / avg_char_width).floor() as usize;

                    &format!("{}…", &loc[..take_chars.saturating_sub(1)])
                } else {
                    loc
                };

                ctx.move_to(x + padding_x, y + 40.0);
                ctx.show_text(&text).unwrap();
            }
        }
    }

    Some(())
}

fn draw_allday_event(
    ctx: &Context,
    event: &CalDavEvent,
    context: &CalendarContext,
    x_offset: &mut f64,
) {
    let color = event.calendar_info.color.as_deref().unwrap_or("#e9a949");

    // Event label
    let title = &event.title;
    let extent = ctx.text_extents(&title).unwrap();

    // Subtract x_bearing here to align ink to padding
    let width = extent.width() + 10.0;
    let height = extent.height() + 10.0;
    let x_start = context.padding - extent.x_bearing() + *x_offset;
    let y_start = context.padding_top - 15.0 - height;

    *x_offset += width + 5.0;

    let event_color = Rgba::from_str(color).unwrap_or_default();
    ctx.set_source_rgba(event_color.r, event_color.g, event_color.b, 0.9);
    CairoShapesExt::rounded_rectangle(ctx, x_start, y_start, width, height, (5.0, 5.0, 5.0, 5.0));
    ctx.fill().unwrap();

    ctx.set_font_size(11.0);
    ctx.set_source_rgba(
        0.0,
        0.0,
        0.0,
        0.7 * event_color.perceived_brightness_gamma(),
    );
    CairoShapesExt::centered_text(ctx, &title, x_start + width / 2.0, y_start + height / 2.0);
}

struct TempLayout {
    index: usize,
    start_secs: f64,
    end_secs: f64,
    lane: u8,
}
fn compute_event_hitboxes(events: &[CalDavEvent], ctx: &CalendarContext) -> Vec<EventHitbox> {
    if events.is_empty() {
        return Vec::new();
    }

    let mut spans: Vec<TempLayout> = events
        .iter()
        .enumerate()
        .filter_map(|(idx, event)| {
            let start = event.start.as_ref()?.utc_time().with_timezone(&Local);
            let end = event.end.as_ref()?.utc_time().with_timezone(&Local);

            let duration = end.signed_duration_since(start);
            let start_dt = ctx.todate.and_time(start.time());
            let end_dt = start_dt + duration;

            if end_dt <= ctx.window_start || start_dt >= ctx.window_end {
                return None;
            }

            let visible_start = start_dt.max(ctx.window_start);
            let visible_end = end_dt.min(ctx.window_end);

            Some(TempLayout {
                index: idx,
                start_secs: (visible_start - ctx.window_start).num_seconds() as f64,
                end_secs: (visible_end - ctx.window_start).num_seconds() as f64,
                lane: 0,
            })
        })
        .collect();

    spans.sort_by(|a, b| a.start_secs.total_cmp(&b.start_secs));

    let mut hitboxes = Vec::with_capacity(spans.len());
    let mut cluster: Vec<TempLayout> = Vec::new();
    let mut cluster_end = 0.0;

    for item in spans {
        if cluster.is_empty() || item.start_secs < cluster_end {
            cluster_end = cluster_end.max(item.end_secs);
            cluster.push(item);
        } else {
            flush_cluster_to_hitboxes(&mut cluster, &mut hitboxes, ctx);
            cluster.clear();

            cluster_end = item.end_secs;
            cluster.push(item);
        }
    }
    flush_cluster_to_hitboxes(&mut cluster, &mut hitboxes, ctx);

    hitboxes
}

fn flush_cluster_to_hitboxes(
    cluster: &mut Vec<TempLayout>,
    results: &mut Vec<EventHitbox>,
    ctx: &CalendarContext,
) {
    if cluster.is_empty() {
        return;
    }

    let mut max_lane = 0;

    for i in 0..cluster.len() {
        let mut lane = 0;
        while cluster[..i].iter().any(|prev| {
            prev.lane == lane
                && cluster[i].start_secs < prev.end_secs
                && cluster[i].end_secs > prev.start_secs
        }) {
            lane += 1;
        }
        cluster[i].lane = lane;
        max_lane = max_lane.max(lane);
    }

    let lanes_totel = (max_lane + 1) as f64;
    let lane_width = (ctx.inner_width - ctx.line_offset) / lanes_totel;

    for i in 0..cluster.len() {
        let item = &cluster[i];

        let y_start = (item.start_secs / ctx.total_seconds) * ctx.inner_height + ctx.padding_top;
        let y_end = (item.end_secs / ctx.total_seconds) * ctx.inner_height + ctx.padding_top;
        let x = ctx.padding + ctx.line_offset + (item.lane as f64 * lane_width);
        let h = (y_end - y_start).max(18.0);

        let has_neighbor_above = results.iter().any(|prev_hb| {
            let is_same_lane = (prev_hb.x - x).abs() < 1.0;
            let touches_top = (prev_hb.y + prev_hb.h - y_start).abs() < 1.5;

            is_same_lane && touches_top
        });

        results.push(EventHitbox {
            index: item.index,
            x,
            y: y_start,
            w: lane_width - 3.0,
            h,
            has_neighbor_above,
        })
    }
}

#[derive(Debug)]
pub struct CalendarDataStore {
    pub timed: Rc<RefCell<Vec<CalDavEvent>>>,
    pub allday: Rc<RefCell<Vec<CalDavEvent>>>,
    pub selection: Option<CalendarRule>,
}
impl CalendarDataStore {
    pub fn new(specs: &Rc<WidgetSpec>) -> Self {
        let selection = if let WidgetSpec::Calendar { selection, .. } = specs.as_ref() {
            selection.clone()
        } else {
            None
        };

        Self {
            timed: Rc::new(RefCell::new(Vec::new())),
            allday: Rc::new(RefCell::new(Vec::new())),
            selection,
        }
    }
    pub fn load_from_cache(&self) -> Result<(), WatsonError> {
        let mut path = get_cache_dir()?;
        path.push("calendar_cache.bin");

        if !path.exists() {
            return Ok(());
        }

        let file = fs::File::open(path)
            .map_err(|e| watson_err!(WatsonErrorKind::FileOpen, e.to_string()))?;

        let reader = BufReader::new(file);
        let (cached_timed, cached_allday): (Vec<CalDavEvent>, Vec<CalDavEvent>) =
            bincode::deserialize_from(reader)
                .map_err(|e| watson_err!(WatsonErrorKind::Deserialize, e.to_string()))?;

        *self.timed.borrow_mut() = cached_timed;
        *self.allday.borrow_mut() = cached_allday;

        Ok(())
    }
    pub fn save_to_cache(&self) -> Result<(), WatsonError> {
        let mut path = get_cache_dir()?;
        path.push("calendar_cache.bin");

        let file = fs::File::create(path)
            .map_err(|e| watson_err!(WatsonErrorKind::FileOpen, e.to_string()))?;

        let writer = BufWriter::new(file);
        let data = (&*self.timed.borrow(), &*self.allday.borrow());
        bincode::serialize_into(writer, &data)
            .map_err(|e| watson_err!(WatsonErrorKind::Serialize, e.to_string()))?;

        Ok(())
    }
    pub async fn refresh(&self) {
        let mut credential_manager = match CredentialManager::new() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{:?}", e);
                return;
            }
        };
        if let Err(e) = credential_manager.unlock() {
            eprintln!("{:?}", e);
            return;
        }

        let today = Local::now().date_naive();
        let mut new_timed = Vec::new();
        let mut new_allday = Vec::new();
        let seen_ids: HashSet<String> = {
            let timed = self.timed.borrow();
            let allday = self.allday.borrow();

            let mut ids = HashSet::with_capacity(timed.len() + allday.len());

            ids.extend(timed.iter().map(|e| e.uid.clone()));
            ids.extend(allday.iter().map(|e| e.uid.clone()));
            ids
        };

        for account in credential_manager.credentials {
            let Some(mut provider) = account.provider() else {
                continue;
            };

            if let Err(e) = provider.init().await {
                // TODO: Log err
                eprintln!("{:?}", e);
                continue;
            }

            let calendars = match provider.get_calendars().await {
                Ok(v) => v,
                Err(e) => {
                    // TODO: Log err
                    eprintln!("{:?}", e);
                    continue;
                }
            };

            let mut events = match provider.get_events(calendars).await {
                Ok(v) => v,
                Err(e) => {
                    // TODO: Log err
                    eprintln!("{:?}", e);
                    continue;
                }
            };

            // Filter events
            if let Some(selection) = &self.selection {
                events.retain(|e| {
                    selection.is_allowed(&e.calendar_info.name) && e.occurs_on_day(&today)
                });
            } else {
                events.retain(|e| e.occurs_on_day(&today));
            }

            // Extend the Events
            {
                for item in events {
                    if !seen_ids.contains(&item.uid) {
                        match item.event_type {
                            CalEventType::Timed => new_timed.push(item),
                            CalEventType::AllDay => new_allday.push(item),
                        }
                    }
                }
            }
        }

        {
            self.timed.borrow_mut().extend(new_timed);
            self.allday.borrow_mut().extend(new_allday);
        }

        let _ = self.save_to_cache();
    }
}
