use crate::{
    config::WidgetSpec,
    ui::widgets::utils::{AnimationDirection, AnimationState, EaseFunction},
};
use std::{cell::RefCell, rc::Rc, str::FromStr};

use chrono::{Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use gtk4::{
    DrawingArea, GestureClick,
    cairo::Context,
    glib::object::ObjectExt,
    prelude::{DrawingAreaExtManual, GestureSingleExt, WidgetExt, WidgetExtManual},
};

use crate::ui::widgets::utils::{CairoShapesExt, Rgba};
use common::{
    auth::{CredentialData, CredentialManager},
    calendar::{
        google::GoogleCalendarClient,
        icloud::PropfindInterface,
        utils::{CalDavEvent, CalEventType},
    },
};

struct CalendarContext {
    font: String,
    padding: f64,
    padding_top: f64,

    inner_width: f64,
    inner_height: f64,
    line_offset: f64,

    todate: NaiveDate,
    window_start: NaiveDateTime,
    window_end: NaiveDateTime,
    total_seconds: f64,
}

pub struct Calendar;
impl Calendar {
    pub fn new(specs: &WidgetSpec) -> DrawingArea {
        let specs = Rc::new(specs.clone());
        let state = Rc::new(AnimationState::new());

        let mut height = 400;

        if let WidgetSpec::Calendar {
            hours_past,
            hours_future,
            ..
        } = specs.as_ref()
        {
            let span = (*hours_past + *hours_future).clamp(1, 24);
            let tmp = 200 + span as i32 * 50;
            height = tmp - tmp % 100;
        };

        let events_timed = Rc::new(RefCell::new(Vec::new()));
        let events_allday = Rc::new(RefCell::new(Vec::new()));
        let calendar_area = DrawingArea::builder()
            .vexpand(false)
            .hexpand(false)
            .valign(gtk4::Align::Start)
            .css_classes(["widget", "calendar"])
            .width_request(400)
            .height_request(height)
            .build();

        if let Some(id) = specs.id() {
            calendar_area.set_widget_name(id);
        }
        if let Some(class) = specs.class() {
            calendar_area.add_css_class(class);
        }

        // Draw function
        calendar_area.set_draw_func({
            let events_timed = Rc::clone(&events_timed);
            let events_allday = Rc::clone(&events_allday);
            let specs = Rc::clone(&specs);
            let state = Rc::clone(&state);
            move |area, ctx, width, height| {
                Calendar::draw(
                    area,
                    ctx,
                    width,
                    height,
                    &events_allday.borrow(),
                    &events_timed.borrow(),
                    &specs,
                    Rc::clone(&state),
                );
            }
        });

        Self::connect_clicked(&calendar_area);

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

        // Get the calendar events async
        gtk4::glib::MainContext::default().spawn_local({
            let events_timed = Rc::clone(&events_timed);
            let events_allday = Rc::clone(&events_allday);
            let state = Rc::clone(&state);
            async move {
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

                for account in credential_manager.credentials {
                    let evs = match account.data {
                        CredentialData::OAuth { .. } => {
                            let mut client = GoogleCalendarClient::new(account);
                            let calendars = match client.get_calendars().await {
                                Ok(v) => v,
                                Err(e) => {
                                    println!("{:?}", e);
                                    return;
                                }
                            };
                            client.get_events(calendars).await
                        }
                        CredentialData::Password { .. } => {
                            let mut interface = PropfindInterface::new(account);
                            match interface.get_principal().await {
                                Ok(_) => match interface.get_calendars().await {
                                    Ok(calendar_info) => interface.get_events(calendar_info).await,
                                    Err(e) => {
                                        eprintln!("Failed to fetch calendars: {:?}", e);
                                        continue;
                                    }
                                },
                                Err(e) => {
                                    eprintln!("Failed to get principal: {:?}", e);
                                    continue;
                                }
                            }
                        }
                        CredentialData::Empty => continue,
                    };

                    // Filter events
                    match evs {
                        Ok(mut evs) => {
                            let today = Local::now().to_utc();

                            if let WidgetSpec::Calendar { selection, .. } = specs.as_ref() {
                                evs.retain(|e| {
                                    selection
                                        .as_ref()
                                        .map_or(true, |s| s.is_allowed(&e.calendar_info.name))
                                        && e.occurs_on_day(&today)
                                });
                            } else {
                                evs.retain(|e| e.occurs_on_day(&today));
                            }

                            let mut timed = Vec::new();
                            let mut allday = Vec::new();
                            for item in evs {
                                match item.event_type {
                                    CalEventType::Timed => timed.push(item),
                                    CalEventType::AllDay => allday.push(item),
                                }
                            }
                            events_timed.borrow_mut().extend(timed);
                            events_allday.borrow_mut().extend(allday);

                            // Internally ques draw
                            state.start(AnimationDirection::Forward {
                                duration: 0.7,
                                function: EaseFunction::EaseOutCubic,
                            });
                        }
                        Err(e) => eprintln!("Failed to fetch events: {:?}", e),
                    }
                }
            }
        });

        calendar_area
    }
    fn connect_clicked(area: &DrawingArea) {
        // Create a GestureClick controller
        let click = GestureClick::new();
        click.set_button(0);

        // Connect to the clicked signal
        click.connect_pressed(move |_gesture, n_press, x, y| {
            println!("Clicked {} times at ({}, {})", n_press, x, y);
        });

        area.add_controller(click);
    }
    pub fn draw(
        area: &DrawingArea,
        ctx: &Context,
        width: i32,
        height: i32,
        events_allday: &[CalDavEvent],
        events_timed: &[CalDavEvent],
        spec: &WidgetSpec,
        state: Rc<AnimationState>,
    ) {
        let WidgetSpec::Calendar {
            accent_color,
            font,
            hours_past,
            hours_future,
            ..
        } = spec
        else {
            return;
        };

        // Create timeline
        let hours_to_show = (*hours_past + *hours_future).clamp(1, 24) as u32;
        let hours_past = *hours_past as u32;
        let today = Local::now();
        let todate = today.date_naive();
        let now = today.time();

        let color = area.color();
        let (color_r, color_g, color_b) = (
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
        );

        // Tentative start/end centered on now
        let now_hour = now.hour();
        let window_start: NaiveDateTime;
        let window_end: NaiveDateTime;

        // Constants for boundary safety
        let midnight_start = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let day_end = NaiveTime::from_hms_opt(23, 59, 59).unwrap();

        if now_hour + hours_to_show > 24 {
            // Window pinned to the end of the day
            let start_hour = 24 - hours_to_show;
            window_start = todate.and_time(NaiveTime::from_hms_opt(start_hour, 0, 0).unwrap());
            window_end = todate.and_time(day_end);
        } else if now_hour < hours_past {
            // Window pinned to the beginning of the day
            window_start = todate.and_time(midnight_start);
            window_end = todate.and_time(NaiveTime::from_hms_opt(hours_to_show, 0, 0).unwrap());
        } else {
            // Normal sliding window centered on now_hour
            let start_hour = now_hour.saturating_sub(hours_past);
            let end_hour = (start_hour + hours_to_show).min(23);

            window_start = todate.and_time(NaiveTime::from_hms_opt(start_hour, 0, 0).unwrap());
            window_end = todate.and_time(NaiveTime::from_hms_opt(end_hour, 0, 0).unwrap());
        }

        // Initialize the area and frame
        let context = {
            let padding = (width as f64 * 0.05).min(20.0);
            let padding_top = if events_allday.len() > 0 {
                120.0
            } else {
                100.0
            };
            CalendarContext {
                font: font.to_string(),
                padding,
                padding_top,
                inner_width: width as f64 - 2.0 * padding,
                inner_height: height as f64 - padding - padding_top,
                line_offset: 40.0,
                todate,
                window_start,
                window_end,
                total_seconds: (window_end - window_start).num_seconds() as f64,
            }
        };

        // Set date string
        ctx.set_source_rgb(color_r, color_g, color_b);
        ctx.set_font_size(50.0);
        ctx.select_font_face(
            &context.font,
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Normal,
        );
        let today_string = today.format("%b %-d").to_string();
        let extents1 = ctx.text_extents(&today_string).unwrap();

        ctx.move_to(
            context.padding - extents1.x_bearing(),
            context.padding - extents1.y_bearing(),
        );
        ctx.show_text(&today_string).unwrap();

        ctx.set_font_size(15.0);
        let Rgba { r, g, b, a } = Rgba::from_str(accent_color).unwrap_or_default();
        ctx.set_source_rgba(r, g, b, a);
        let weekday_string = today.format("%A").to_string();
        let extents2 = ctx.text_extents(&weekday_string).unwrap();

        let y_pos = context.padding - extents1.y_bearing();
        ctx.move_to(
            context.padding - extents2.x_bearing(),
            y_pos - extents2.y_bearing() + 10.0,
        );
        ctx.show_text(&weekday_string).unwrap();

        // Draw hour lines
        for offset in 0..=hours_to_show {
            ctx.set_source_rgb(color_r, color_g, color_b);
            ctx.set_line_cap(gtk4::cairo::LineCap::Round);
            ctx.set_line_width(0.5);

            let hour = window_start.hour() + offset as u32;
            let y = (offset as f64 / hours_to_show as f64) * context.inner_height;
            ctx.move_to(
                context.padding + context.line_offset,
                y + context.padding_top,
            );
            ctx.line_to(
                context.inner_width + context.padding,
                y + context.padding_top,
            );
            ctx.stroke().unwrap();

            // Draw hour text
            ctx.set_source_rgb(color_r, color_g, color_b);
            ctx.select_font_face(
                &context.font,
                gtk4::cairo::FontSlant::Normal,
                gtk4::cairo::FontWeight::Bold,
            );
            ctx.set_font_size(12.0);
            let time_label = format!("{:02}:00", hour);
            ctx.move_to(context.padding, y + context.padding_top + 4.0);
            ctx.show_text(&time_label).unwrap();
        }

        // Draw events
        ctx.set_operator(gtk4::cairo::Operator::Over);
        ctx.select_font_face(
            &context.font,
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Bold,
        );

        let mut allday_x_offset = 0.0;
        for event in events_allday {
            draw_allday_event(ctx, event, &context, &mut allday_x_offset);
        }

        let layouts = compute_event_layouts(events_timed, &context);
        for layout in layouts {
            draw_event(ctx, layout, &context, state.progress.get());
        }

        // Draw current time line
        let now = Local::now().naive_local();
        let current_y = (now - window_start).num_seconds() as f64 / context.total_seconds
            * context.inner_height;
        let Rgba { r, g, b, a } = Rgba::from_str(accent_color).unwrap_or_default();
        ctx.set_source_rgba(r, g, b, a); // Red line
        ctx.set_line_width(2.0);
        ctx.move_to(context.padding, current_y + context.padding_top);
        ctx.line_to(
            context.inner_width + context.padding,
            current_y + context.padding_top,
        );
        ctx.stroke().unwrap();

        let rad = 3.0;
        CairoShapesExt::circle(
            ctx,
            context.padding - rad,
            current_y + context.padding_top,
            rad,
        );
    }
}

fn draw_event(
    ctx: &Context,
    layout: EventLayout,
    context: &CalendarContext,
    progress: f64,
) -> Option<()> {
    let EventLayout {
        event,
        start_secs,
        end_secs,
        lane,
        lanes_total,
    } = layout;
    let progress = if layout.event.seen.get() {
        1.0
    } else {
        if progress >= 1.0 {
            layout.event.seen.set(true);
        }
        progress
    };

    let start_y =
        (start_secs / context.total_seconds) * (context.inner_height) + context.padding_top;
    let end_y = (end_secs / context.total_seconds) * context.inner_height + context.padding_top;
    let top = start_y + 1.0;

    let color = event.calendar_info.color.as_deref().unwrap_or("#e9a949");
    let event_color = Rgba::from_str(color).unwrap_or_default();
    ctx.set_source_rgba(event_color.r, event_color.g, event_color.b, 0.9 * progress);

    // Handle event that ends past the window
    let (rect_height, rad) = if layout.end_secs > context.total_seconds {
        let overflow = layout.end_secs - context.total_seconds;
        let pad_time = (context.padding / context.inner_height) * context.total_seconds;
        let frac = (overflow / pad_time).min(1.0);

        let rect_height = (end_y - start_y).max(1.0) - 2.0 + frac * context.padding;

        let rad = if frac < 1.0 {
            (5.0, 5.0, 5.0, 5.0)
        } else {
            (5.0, 5.0, 0.0, 0.0)
        };
        (rect_height, rad)
    } else {
        ((end_y - start_y).max(1.0) - 2.0, (5.0, 5.0, 5.0, 5.0))
    };

    let lane_width = (context.inner_width - context.line_offset) / lanes_total as f64;
    let x = context.padding + context.line_offset + lane as f64 * lane_width;

    CairoShapesExt::rounded_rectangle(ctx, x, top, lane_width - 2.0, rect_height, rad);
    ctx.fill().unwrap();

    // Event label
    ctx.set_font_size(11.0);
    ctx.set_source_rgba(
        0.0,
        0.0,
        0.0,
        0.7 * event_color.perceived_brightness_gamma() * progress,
    );
    let summary = event.summary.as_deref().unwrap_or("Untitled Event");
    let start_time = context.window_start + Duration::seconds(start_secs as i64);
    let time = start_time.time().format("%H:%M").to_string();

    let extents = ctx.text_extents(&summary).unwrap();
    let text_height = extents.height();

    if (rect_height - text_height).abs() <= 10.0 {
        let bottom = start_y + rect_height - 1.0;
        let usable_height = bottom - top;
        let cy = top + usable_height / 2.0;
        let cx = x + 10.0;
        CairoShapesExt::vert_centered_text(ctx, summary, cx, cy);
        let cx = x + lane_width - 10.0;
        CairoShapesExt::vert_centered_text(ctx, &time, cx, cy);
    } else {
        // Title
        ctx.move_to(x + 10.0, start_y + 15.0);
        ctx.show_text(summary).unwrap();

        // Time
        ctx.move_to(x + lane_width - 45.0, start_y + 15.0);
        ctx.show_text(&time).unwrap();

        // Location
        if let Some(loc) = &event.location {
            ctx.save().unwrap();
            ctx.set_font_size(10.0);
            ctx.select_font_face(
                &context.font,
                gtk4::cairo::FontSlant::Normal,
                gtk4::cairo::FontWeight::Normal,
            );
            ctx.move_to(x + 10.0, start_y + 26.0);
            ctx.show_text(loc).unwrap();

            ctx.restore().unwrap();
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
    let title = event.summary.as_deref().unwrap_or("Untitled Event");
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

struct EventLayout<'a> {
    event: &'a CalDavEvent,
    start_secs: f64,
    end_secs: f64,
    lane: usize,
    lanes_total: usize,
}

fn compute_event_layouts<'a>(
    events: &'a [CalDavEvent],
    ctx: &CalendarContext,
) -> Vec<EventLayout<'a>> {
    let mut spans: Vec<_> = events
        .iter()
        .filter_map(|event| {
            let start = event.start.as_ref()?.utc_time().with_timezone(&Local);
            let end = event.end.as_ref()?.utc_time().with_timezone(&Local);

            let duration = end.signed_duration_since(start);
            let start = ctx.todate.and_time(start.time());
            let end = start + duration;

            if end <= ctx.window_start || start >= ctx.window_end {
                return None;
            }

            let visible_start = start.max(ctx.window_start);
            let visible_end = end.min(ctx.window_end);

            Some((
                event,
                (visible_start - ctx.window_start).num_seconds() as f64,
                (visible_end - ctx.window_start).num_seconds() as f64,
            ))
        })
        .collect();

    // Sort by start time
    spans.sort_by(|a, b| a.1.total_cmp(&b.1));

    // Sweep line to assign lanes
    let mut active: Vec<EventLayout> = Vec::new();
    let mut layouts = Vec::new();

    for (event, start_secs, end_secs) in spans {
        // Partition active into currently overlapping and finished
        let (mut still_active, mut finished): (Vec<_>, Vec<_>) = active
            .into_iter()
            .partition(|layout| layout.end_secs > start_secs);

        // Flush finished events
        layouts.append(&mut finished);

        // Find the smallest free lane
        let mut lane = 0;
        while still_active.iter().any(|l| l.lane == lane) {
            lane += 1;
        }

        // Add the new event
        still_active.push(EventLayout {
            event,
            start_secs,
            end_secs,
            lane,
            lanes_total: 0,
        });

        // Update lanes_total for all currently active events
        let lanes_total = still_active.len();
        for layout in still_active.iter_mut() {
            layout.lanes_total = lanes_total;
        }

        // Update active
        active = still_active;
    }

    // Flush any remaining active events
    layouts.extend(active);

    layouts
}
