use std::{rc::Rc, str::FromStr};

use chrono::{Local, NaiveTime, Timelike};
use common::calendar::utils::CalDavEvent;
use gtk4::cairo::{Context, FontSlant, FontWeight};

use crate::ui::widgets::{
    calendar::{CalendarContext, EventHitbox, data_store::CalendarDataStore},
    utils::{CairoShapesExt, Rgba},
};

pub struct CalendarRenderer<'c> {
    ctx: &'c Context,
    context: &'c CalendarContext,
    progress: f64,
}
impl<'c> CalendarRenderer<'c> {
    pub fn new(ctx: &'c Context, context: &'c CalendarContext, progress: f64) -> Self {
        // Set default font
        ctx.select_font_face(&context.font, FontSlant::Normal, FontWeight::Normal);

        Self {
            ctx,
            context,
            progress,
        }
    }
    pub fn draw_all(&self, data_store: Rc<CalendarDataStore>) {
        // Draw header bar with date and weekday
        self.draw_header();

        // Hour lines and timeline
        self.draw_timeline();

        // Drawing events
        let mut allday_x = 0.0;
        for event in data_store.allday.borrow().iter() {
            self.draw_allday_event(event, &mut allday_x);
        }

        for hitbox in self.context.cache.hitboxes.iter() {
            if let Some(event) = data_store.timed.borrow().get(hitbox.index) {
                self.draw_event(hitbox, event);
            }
        }

        // Current time indicator
        self.draw_time_indicator();
    }
    fn draw_header(&self) {
        // Header: Date
        self.ctx.set_source_rgb(
            self.context.text.r,
            self.context.text.g,
            self.context.text.b,
        );
        self.ctx.set_font_size(50.0);
        let today_string = self.context.todate.format("%b %-d").to_string();
        let ext1 = self.ctx.text_extents(&today_string).unwrap();
        self.ctx
            .move_to(self.context.padding, self.context.padding + ext1.height());
        self.ctx.show_text(&today_string).unwrap();

        // Header: Weekday String
        self.ctx.set_source_rgba(
            self.context.accent.r,
            self.context.accent.g,
            self.context.accent.b,
            self.context.accent.a,
        );
        self.ctx.set_font_size(15.0);
        let weekday_string = self.context.todate.format("%A").to_string();
        self.ctx.move_to(
            self.context.padding,
            self.context.padding + ext1.height() + 20.0,
        );
        self.ctx.show_text(&weekday_string).unwrap();
    }

    fn draw_timeline(&self) {
        // Draw hour lines
        self.ctx.set_line_width(0.5);
        self.ctx.set_line_cap(gtk4::cairo::LineCap::Round);
        self.ctx.set_source_rgba(
            self.context.text.r,
            self.context.text.g,
            self.context.text.b,
            0.2,
        );
        for offset in 0..self.context.hours_to_show {
            let y = (offset as f64 / self.context.hours_to_show as f64) * self.context.inner_height
                + self.context.padding_top;
            self.ctx
                .move_to(self.context.padding + self.context.line_offset, y);
            self.ctx
                .line_to(self.context.inner_width + self.context.padding, y);
        }
        self.ctx.stroke().unwrap();

        // Draw hour labels
        self.ctx.set_font_size(12.0);
        self.ctx.set_source_rgb(
            self.context.text.r,
            self.context.text.g,
            self.context.text.b,
        );
        for offset in 0..=self.context.hours_to_show {
            let y = (offset as f64 / self.context.hours_to_show as f64) * self.context.inner_height
                + self.context.padding_top;
            let hour = (self.context.window_start.hour() + offset as u32) % 24;

            let fmt_str = self
                .context
                .hm_format
                .as_ref()
                .map(|f| f.timeline.as_str())
                .unwrap_or("%H:%M");

            let label = NaiveTime::from_hms_opt(hour, 0, 0)
                .unwrap()
                .format(fmt_str)
                .to_string();
            CairoShapesExt::vert_centered_text(self.ctx, &label, self.context.padding, y);
        }
    }
    fn draw_time_indicator(&self) {
        let now_full = Local::now().naive_local();
        if now_full >= self.context.window_start && now_full <= self.context.window_end {
            let current_y = (now_full - self.context.window_start).num_seconds() as f64
                / self.context.total_seconds
                * self.context.inner_height
                + self.context.padding_top;
            let x_start = self.context.padding + self.context.line_offset - 6.0;

            self.ctx.set_source_rgba(
                self.context.accent.r,
                self.context.accent.g,
                self.context.accent.b,
                self.context.accent.a,
            );
            self.ctx.set_line_width(2.0);
            self.ctx.move_to(x_start, current_y);
            self.ctx
                .line_to(self.context.inner_width + self.context.padding, current_y);
            self.ctx.stroke().unwrap();

            CairoShapesExt::circle(self.ctx, x_start, current_y, 3.0);
            self.ctx.fill().unwrap();
        }
    }
    fn draw_event(&self, hitbox: &EventHitbox, event: &CalDavEvent) -> Option<()> {
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
            if self.progress >= 1.0 {
                event.seen.set(true);
            }
            self.progress
        };

        // Coloring
        let color_str = event.calendar_info.color.as_deref().unwrap_or("#e9a949");
        let base_color = Rgba::from_str(color_str).unwrap_or_default();

        // Background
        let rad = if h > self.context.inner_height {
            (6.0, 6.0, 0.0, 0.0)
        } else {
            (6.0, 6.0, 6.0, 6.0)
        };
        let neightbor_offset = if has_neighbor_above { 2.5 } else { 0.0 };

        CairoShapesExt::rounded_rectangle(
            self.ctx,
            x,
            y + neightbor_offset,
            w,
            h - neightbor_offset,
            rad,
        );

        // Set background
        self.ctx
            .set_source_rgba(base_color.r, base_color.g, base_color.b, 0.45 * alpha);
        self.ctx.fill_preserve().unwrap();

        // Border
        self.ctx.save().unwrap();
        let rim_grad = gtk4::cairo::LinearGradient::new(x, y, x, y + h);
        rim_grad.add_color_stop_rgba(0.0, base_color.r, base_color.g, base_color.b, 0.5 * alpha);
        rim_grad.add_color_stop_rgba(0.5, base_color.r, base_color.g, base_color.b, 0.1 * alpha);
        rim_grad.add_color_stop_rgba(1.0, base_color.r, base_color.g, base_color.b, 0.3 * alpha);

        self.ctx.set_line_width(1.0);
        self.ctx.set_source(&rim_grad).unwrap();
        self.ctx.stroke().unwrap();
        self.ctx.restore().unwrap();

        // Text Content
        let summary = &event.title;
        let fmt_str = self
            .context
            .hm_format
            .as_ref()
            .map(|f| f.event.as_str())
            .unwrap_or("%H:%M");
        let time_str = event
            .start
            .as_ref()
            .map(|s| format!("{}", s.local().format(fmt_str)))
            .unwrap_or_default();

        // Label
        self.ctx
            .set_source_rgba(base_color.r, base_color.g, base_color.b, 0.8 * alpha);
        self.ctx
            .select_font_face(&self.context.font, FontSlant::Normal, FontWeight::Bold);

        let is_tiny_event = h < 30.0;
        let padding_x = 10.0;

        if is_tiny_event {
            self.ctx.set_font_size(10.0);
            let cy = y + (h / 2.0);

            // Only draw time if lane is wide enough
            // let _time_extents = ctx.text_extents(&time_str).unwrap();
            if w > 100.0 {
                CairoShapesExt::rjust_text(self.ctx, &time_str, x + w - 8.0, cy, true);
            }

            // Draw summary with clipping (implicit)
            CairoShapesExt::vert_centered_text(self.ctx, &summary, x + padding_x, cy);
        } else {
            // Multi-line layout
            self.ctx.set_font_size(11.0);
            self.ctx.move_to(x + padding_x, y + 16.0);
            self.ctx.show_text(&summary).unwrap();

            self.ctx.set_font_size(9.0);
            self.ctx
                .select_font_face(&self.context.font, FontSlant::Normal, FontWeight::Normal);

            // Time below title
            self.ctx.move_to(x + padding_x, y + 28.0);
            self.ctx.show_text(&time_str).unwrap();

            // Location at bottom or 3rd line
            if let Some(loc) = &event.location {
                if h > 45.0 {
                    let loc_width = self.ctx.text_extents(&loc).unwrap().width();
                    let inner_width = w - 2.0 * padding_x;
                    let text = if loc_width > inner_width {
                        let avg_char_width = loc_width / loc.len() as f64;
                        let take_chars = (inner_width / avg_char_width).floor() as usize;

                        &format!("{}…", &loc[..take_chars.saturating_sub(1)])
                    } else {
                        loc
                    };

                    self.ctx.move_to(x + padding_x, y + 40.0);
                    self.ctx.show_text(&text).unwrap();
                }
            }
        }

        Some(())
    }
    fn draw_allday_event(&self, event: &CalDavEvent, x_offset: &mut f64) {
        let color = event.calendar_info.color.as_deref().unwrap_or("#e9a949");

        // Event label
        let title = &event.title;
        let extent = self.ctx.text_extents(&title).unwrap();

        // Color
        let color_str = event.calendar_info.color.as_deref().unwrap_or("#e9a949");
        let base_color = Rgba::from_str(color_str).unwrap_or_default();

        // Subtract x_bearing here to align ink to padding
        let width = extent.width() + 10.0;
        let height = extent.height() + 10.0;
        let x_start = self.context.padding - extent.x_bearing() + *x_offset;
        let y_start = self.context.padding_top - 15.0 - height;

        *x_offset += width + 5.0;

        let event_color = Rgba::from_str(color).unwrap_or_default();
        self.ctx
            .set_source_rgba(base_color.r, base_color.g, base_color.b, 0.45);
        CairoShapesExt::rounded_rectangle(
            self.ctx,
            x_start,
            y_start,
            width,
            height,
            (5.0, 5.0, 5.0, 5.0),
        );
        self.ctx.fill().unwrap();

        self.ctx.set_font_size(11.0);
        self.ctx.set_source_rgba(
            0.0,
            0.0,
            0.0,
            0.7 * event_color.perceived_brightness_gamma(),
        );
        self.ctx
            .set_source_rgba(base_color.r, base_color.g, base_color.b, 0.8);
        CairoShapesExt::centered_text(
            self.ctx,
            &title,
            x_start + width / 2.0,
            y_start + height / 2.0,
        );
    }
}
