use chrono::{Local, NaiveDateTime};
use common::calendar::utils::CalDavEvent;

use crate::ui::widgets::calendar::{context::CalendarContext, types::EventHitbox};

struct TempLayout {
    index: usize,
    start_secs: f64,
    end_secs: f64,
    lane: u8,
}

#[derive(Default)]
pub struct CalendarCache {
    pub last_width: f64,
    pub last_height: f64,
    pub last_window_start: NaiveDateTime,
    pub hitboxes: Vec<EventHitbox>,
}
impl CalendarCache {
    pub fn calculate_hitboxes(
        events: &[CalDavEvent],
        context: &CalendarContext,
    ) -> Vec<EventHitbox> {
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
                let start_dt = context.todate.and_time(start.time());
                let end_dt = start_dt + duration;

                if end_dt <= context.window_start || start_dt >= context.window_end {
                    return None;
                }

                let visible_start = start_dt.max(context.window_start);
                let visible_end = end_dt.min(context.window_end);

                Some(TempLayout {
                    index: idx,
                    start_secs: (visible_start - context.window_start).num_seconds() as f64,
                    end_secs: (visible_end - context.window_start).num_seconds() as f64,
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
                Self::flush_cluster_to_hitboxes(&mut cluster, &mut hitboxes, context);
                cluster.clear();

                cluster_end = item.end_secs;
                cluster.push(item);
            }
        }
        Self::flush_cluster_to_hitboxes(&mut cluster, &mut hitboxes, context);

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

            let y_start =
                (item.start_secs / ctx.total_seconds) * ctx.inner_height + ctx.padding_top;
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
}
