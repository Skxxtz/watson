use serde::{Deserialize, Serialize};

pub struct CalendarConfig<'w> {
    pub accent_color: &'w str,
    pub font: &'w str,
    pub hm_format: Option<&'w CalendarHMFormat>,
    pub hours_past: u8,
    pub hours_future: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CalendarHMFormat {
    pub event: String,
    pub timeline: String,
}
impl Default for CalendarHMFormat {
    fn default() -> Self {
        Self {
            event: "%H:%M".into(),
            timeline: "%H:%M".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CalendarRule {
    /// Show all except these specific calendars.
    Exclude(Vec<String>),
    /// Standard filter: show only these specific calendars.
    Include(Vec<String>),
    /// Exclusive: show ONLY these and strictly nothing else (same logic as Include, but clearer intent).
    Only(Vec<String>),
}
impl CalendarRule {
    pub fn is_allowed(&self, name: &str) -> bool {
        match self {
            Self::Only(r) | Self::Include(r) => r.iter().any(|s| s == name),
            Self::Exclude(r) => !r.iter().any(|s| s == name),
        }
    }
}

#[derive(Debug)]
pub struct EventHitbox {
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub has_neighbor_above: bool,
}
