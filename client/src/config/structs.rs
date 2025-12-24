use std::fs::File;
use std::{io::BufReader, path::PathBuf};

use common::errors::{WatsonError, WatsonErrorKind};
use common::watson_err;
use serde::{Deserialize, Serialize};

use crate::ui::widgets::HandStyle;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WidgetBase {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WidgetSpec {
    Battery {
        // Inherit Base Properties
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default = "default_battery_gradient")]
        colors: [String; 3],
        #[serde(default = "default_battery_threshold")]
        threshold: u8,
    },
    Calendar {
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default)]
        selection: Option<CalendarRule>,

        #[serde(default = "default_accent")]
        accent_color: String,

        #[serde(default = "default_font")]
        font: String,

        #[serde(default = "default_calendar_hours_past")]
        hours_past: u8,
        #[serde(default = "default_calendar_hours_fut")]
        hours_future: u8,
    },
    Clock {
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default)]
        time_zone: Option<String>,

        #[serde(default)]
        head_style: HandStyle,

        #[serde(default = "default_accent")]
        accent_color: String,

        #[serde(default = "default_font")]
        font: String,
    },
    Row {
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default)]
        spacing: i32,
        children: Vec<WidgetSpec>,
    },
    Column {
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default)]
        spacing: i32,
        children: Vec<WidgetSpec>,
    },
}
impl WidgetSpec {
    pub fn base(&self) -> Option<&WidgetBase> {
        match self {
            Self::Battery { base, .. } => Some(base),
            Self::Calendar { base, .. } => Some(base),
            Self::Clock { base, .. } => Some(base),
            Self::Column { base, .. } => Some(base),
            Self::Row { base, .. } => Some(base),
        }
    }
    pub fn id(&self) -> Option<&String> {
        self.base().and_then(|b| b.id.as_ref())
    }
    pub fn class(&self) -> Option<&String> {
        self.base().and_then(|b| b.class.as_ref())
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

pub fn load_config() -> Result<Vec<WidgetSpec>, WatsonError> {
    let home = std::env::var("HOME").unwrap();
    let loc = PathBuf::from(home).join(".config/watson/fallback.json");

    let file =
        File::open(loc).map_err(|e| watson_err!(WatsonErrorKind::FileOpen, e.to_string()))?;

    let reader = BufReader::new(file);

    serde_json::from_reader::<_, Vec<WidgetSpec>>(reader)
        .map_err(|e| watson_err!(WatsonErrorKind::Deserialization, e.to_string()))
}

fn default_font() -> String {
    "Arial".into()
}
fn default_accent() -> String {
    "#bf4759".into()
}
fn default_calendar_hours_past() -> u8 {
    2
}
fn default_calendar_hours_fut() -> u8 {
    8
}
fn default_battery_gradient() -> [String; 3] {
    [
        "#68A357".to_string(),
        "#F9C22E".to_string(),
        "#E84855".to_string(),
    ]
}
fn default_battery_threshold() -> u8 {
    40
}
