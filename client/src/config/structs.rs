use std::fs::File;
use std::{io::BufReader, path::PathBuf};

use common::errors::{WatsonError, WatsonErrorType};
use serde::{Deserialize, Serialize};

use crate::ui::widgets::HandStyle;

#[derive(Debug, Deserialize, Serialize)]
pub struct WidgetBase {
    pub priority: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WidgetSpec {
    Clock {
        #[serde(default)]
        time_zone: Option<String>,

        #[serde(default)]
        head_style: HandStyle,

        #[serde(default = "default_accent")]
        accent_color: String,

        #[serde(default = "default_font")]
        font: String,
    },
    Calendar {
        #[serde(default)]
        selection: Option<CalendarRule>,

        #[serde(default = "default_accent")]
        accent_color: String,

        #[serde(default = "default_font")]
        font: String,
    },
    Row {
        #[serde(default)]
        spacing: i32,
        children: Vec<WidgetSpec>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WidgetInstance {
    #[serde(flatten)]
    pub base: WidgetBase,
    #[serde(flatten)]
    pub spec: WidgetSpec,
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

pub fn load_config() -> Result<Vec<WidgetInstance>, WatsonError> {
    let home = std::env::var("HOME").unwrap();
    let loc = PathBuf::from(home).join(".config/watson/fallback.json");

    let file = File::open(loc).map_err(|e| WatsonError {
        r#type: WatsonErrorType::FileOpen,
        error: e.to_string(),
    })?;

    let reader = BufReader::new(file);

    serde_json::from_reader::<_, Vec<WidgetInstance>>(reader).map_err(|e| WatsonError {
        r#type: WatsonErrorType::Deserialization,
        error: e.to_string(),
    })
}


fn default_font() -> String {
    "Arial".into()
}
fn default_accent() -> String {
    "#bf4759".into()
}
