use crate::config::WidgetSpec;
use std::{cell::RefCell, rc::Rc, str::FromStr};

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use gtk4::{
    DrawingArea, cairo::Context, glib::object::ObjectExt, prelude::{DrawingAreaExtManual, WidgetExt}
};

use crate::ui::widgets::utils::{CairoShapesExt, Rgba};
use common::calendar::{
    icloud::{CalDavEvent, CalEventType, PropfindInterface}, utils::structs::DateTimeSpec,
};



struct CalendarContext {
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
        let events_timed = Rc::new(RefCell::new(Vec::new()));
        let events_allday = Rc::new(RefCell::new(Vec::new()));
        let calendar_area = DrawingArea::builder()
            .vexpand(false)
            .hexpand(false)
            .valign(gtk4::Align::Start)
            .css_classes(["widget", "calendar"])
            .build();
        calendar_area.set_size_request(400, 400);

        // Draw function
        calendar_area.set_draw_func({
            let events_timed = Rc::clone(&events_timed);
            let events_allday = Rc::clone(&events_allday);
            let specs = Rc::clone(&specs);
            move |area, ctx, width, height| {
                Calendar::draw(area, ctx, width, height, &events_allday.borrow(), &events_timed.borrow(), &specs);
            }
        });

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

        // Get the calendar events async
        gtk4::glib::MainContext::default().spawn_local({
            let events_timed = Rc::clone(&events_timed);
            let events_allday = Rc::clone(&events_allday);
            let cal_weak = calendar_area.downgrade();
            async move {
                let mut interface = PropfindInterface::new();

                match interface.get_principal().await {
                    Ok(_) => match interface.get_calendars().await {
                        Ok(calendar_info) => match interface.get_events(calendar_info).await {
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


                                if let Some(cal) = cal_weak.upgrade() {
                                    cal.queue_draw();
                                }
                            }
                            Err(e) => eprintln!("Failed to fetch events: {:?}", e),
                        },
                        Err(e) => eprintln!("Failed to fetch calendars: {:?}", e),
                    },
                    Err(e) => eprintln!("Failed to get principal: {:?}", e),
                }
            }
        });

        calendar_area
    }
    pub fn draw(
        area: &DrawingArea,
        ctx: &Context,
        width: i32,
        height: i32,
        events_allday: &[CalDavEvent],
        events_timed: &[CalDavEvent],
        spec: &WidgetSpec,
    ) {
        let WidgetSpec::Calendar { selection:_, accent_color, font } = spec else {
            return;
        };
        let accent_color = accent_color.as_deref().unwrap_or("#bf4759");
        let font = font.as_deref().unwrap_or("Sans");

        // Create timeline
        let hours_to_show = 8;
        let today = Local::now();
        let todate = today.date_naive();
        let now = today.time();

        let color = area.color();
        let (color_r, color_g, color_b) = (color.red() as f64, color.green() as f64, color.blue() as f64);

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
        } else if now_hour < (hours_to_show / 2) {
            // Window pinned to the beginning of the day
            window_start = todate.and_time(midnight_start);
            window_end = todate.and_time(NaiveTime::from_hms_opt(hours_to_show, 0, 0).unwrap());
        } else {
            // Normal sliding window centered on now_hour
            let start_hour = now_hour.saturating_sub(hours_to_show / 2);
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
                padding, 
                padding_top,
                inner_width: width as f64 - 2.0 * padding,
                inner_height: height as f64 -  padding - padding_top,
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
            font,
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Normal,
        );
        let today_string = today.format("%b %-d").to_string();
        let extents1 = ctx.text_extents(&today_string).unwrap();

        ctx.move_to(context.padding - extents1.x_bearing(), context.padding - extents1.y_bearing());
        ctx.show_text(&today_string).unwrap();

        ctx.set_font_size(15.0);
        let Rgba{r, g, b, a} = Rgba::from_str(accent_color).unwrap_or_default();
        ctx.set_source_rgba(r, g, b, a);
        let weekday_string = today.format("%A").to_string();
        let extents2 = ctx.text_extents(&weekday_string).unwrap();

        let y_pos = context.padding - extents1.y_bearing();
        ctx.move_to(context.padding - extents2.x_bearing(), y_pos - extents2.y_bearing() + 10.0);
        ctx.show_text(&weekday_string).unwrap();

        // Draw hour lines
        for offset in 0..=hours_to_show {
            ctx.set_source_rgb(color_r, color_g, color_b);
            ctx.set_line_cap(gtk4::cairo::LineCap::Round);
            ctx.set_line_width(0.5);

            let hour = window_start.hour() + offset as u32;
            let y = (offset as f64 / hours_to_show as f64) * context.inner_height;
            ctx.move_to(context.padding + context.line_offset, y + context.padding_top);
            ctx.line_to(context.inner_width + context.padding, y + context.padding_top);
            ctx.stroke().unwrap();

            // Draw hour text
            ctx.set_source_rgb(color_r, color_g, color_b);
            ctx.select_font_face(
                font,
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
            font,
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Bold,

        );

        for event in events_allday {
            draw_allday_event(ctx, event, &context);
        }
        for event in events_timed {
            draw_event(ctx, event, &context);
        }

        // Draw current time line
        let now = Local::now().naive_local();
        let current_y = (now - window_start).num_seconds() as f64 / context.total_seconds * context.inner_height;
        let Rgba { r, g, b, a } = Rgba::from_str(accent_color).unwrap_or_default();
        ctx.set_source_rgba(r, g, b, a); // Red line
        ctx.set_line_width(2.0);
        ctx.move_to(context.padding, current_y + context.padding_top);
        ctx.line_to(context.inner_width + context.padding, current_y + context.padding_top);
        ctx.stroke().unwrap();

        let rad = 3.0;
        CairoShapesExt::circle(ctx, context.padding - rad, current_y + context.padding_top, rad);
    }
}

fn draw_event(ctx: &Context, event: &CalDavEvent, context: &CalendarContext) -> Option<()> {
    let event_start = event.start.as_ref()?;
    let event_end = event.end.as_ref()?;
    let start_time = match event_start {
        DateTimeSpec::DateTime { .. } => event_start.utc_time().with_timezone(&Local),
        _ => return None,
    };
    let end_time = match event_end {
        DateTimeSpec::DateTime { .. } => event_end.utc_time().with_timezone(&Local),
        _ => return None,
    };

    // Required for events spanning over days
    let duration = end_time.signed_duration_since(start_time);
    let start = context.todate.and_time(start_time.time());
    let end = start + duration;

    // If not in window, skip
    if end <= context.window_start || start >= context.window_end {
        return None
    }


    let visible_start = start.max(context.window_start);
    let visible_end = end.min(context.window_end);

    let start_secs = (visible_start - context.window_start).num_seconds() as f64;
    let end_secs = (visible_end - context.window_start).num_seconds() as f64;

    let start_y = (start_secs / context.total_seconds) * context.inner_height + context.padding_top;
    let end_y = (end_secs / context.total_seconds) * context.inner_height + context.padding_top;
    let rect_height = (end_y - start_y).max(1.0);

    // Colors:
    // Blue: #4D99E6
    // Orange: #e8a849
    let color = event.calendar_info.color.as_deref().unwrap_or("#e9a949");
    let event_color = Rgba::from_str(color).unwrap_or_default();
    ctx.set_source_rgba(event_color.r, event_color.g, event_color.b, 0.9);
    CairoShapesExt::rounded_rectangle(
        ctx,
        context.padding + context.line_offset + 1.0,
        start_y + 1.0,
        context.inner_width - context.line_offset - 2.0,
        rect_height - 2.0,
        5.0,
    );
    ctx.fill().unwrap();

    // Event label
    ctx.set_font_size(11.0);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.5);
    ctx.move_to(context.padding + context.line_offset + 10.0, start_y + 15.0);
    let summary = event.summary.as_deref().unwrap_or("Untitled Event");
    ctx.show_text(summary).unwrap();

    ctx.move_to(context.inner_width + context.padding - 45.0, start_y + 15.0);
    let summary = start_time.time().format("%H:%M").to_string();
    ctx.show_text(&summary).unwrap();

    Some(())
}

fn draw_allday_event(ctx: &Context, event: &CalDavEvent, context: &CalendarContext) {
    println!("{:?}", event);
    let color = event.calendar_info.color.as_deref().unwrap_or("#e9a949");

    // Event label
    let title = event.summary.as_deref().unwrap_or("Untitled Event");
    let extent = ctx.text_extents(&title).unwrap();

    // Subtract x_bearing here to align ink to padding
    let width = extent.width() + 10.0;
    let height = extent.height() + 10.0;
    let x_start = context.padding - extent.x_bearing();
    let y_start = context.padding_top - 15.0 - height;



    let event_color = Rgba::from_str(color).unwrap_or_default();
    ctx.set_source_rgba(event_color.r, event_color.g, event_color.b, 0.9);
    CairoShapesExt::rounded_rectangle(
        ctx,
        x_start,
        y_start,
        width,
        height,
        5.0,
    );
    ctx.fill().unwrap();

    ctx.set_font_size(11.0);
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    CairoShapesExt::centered_text(ctx, &title, x_start + width / 2.0, y_start + height / 2.0);

}
