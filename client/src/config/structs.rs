use std::fs::File;
use std::{io::BufReader, path::PathBuf};

use common::utils::errors::{WatsonError, WatsonErrorKind};
use common::watson_err;
use serde::{Deserialize, Serialize};

use crate::ui::widgets::BackendFuncType;
use crate::ui::widgets::{
    BackendFunc, HandStyle, SliderRange,
    calendar::types::{CalendarConfig, CalendarHMFormat, CalendarRule},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WidgetBase {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub ratio: Option<f32>,
    #[serde(default)]
    pub valign: Option<AlignmentWrapper>,
    #[serde(default)]
    pub halign: Option<AlignmentWrapper>,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum AlignmentWrapper {
    Start,
    End,
    Center,
    Fill,
}
impl From<AlignmentWrapper> for gtk4::Align {
    fn from(value: AlignmentWrapper) -> Self {
        match value {
            AlignmentWrapper::Start => Self::Start,
            AlignmentWrapper::End => Self::End,
            AlignmentWrapper::Center => Self::Center,
            AlignmentWrapper::Fill => Self::Fill,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
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

        dimensions: Option<(i32, i32)>,

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

        #[serde(default)]
        hm_format: CalendarHMFormat,
    },
    Clock {
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default)]
        time_zone: Option<String>,

        #[serde(default)]
        hand_style: HandStyle,

        #[serde(default = "default_accent")]
        accent_color: String,

        #[serde(default = "default_font")]
        font: String,
    },
    Notifications {
        #[serde(flatten)]
        base: WidgetBase,
    },
    Button {
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default)]
        func: BackendFunc,

        #[serde(default)]
        icon: Option<String>,
    },
    Row {
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default)]
        spacing: i32,
        children: Vec<WidgetSpec>,
    },
    Slider {
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default)]
        func: BackendFunc,

        #[serde(default)]
        range: SliderRange,

        #[serde(default)]
        orientation: WidgetOrientation,
    },
    Column {
        #[serde(flatten)]
        base: WidgetBase,

        #[serde(default)]
        spacing: i32,
        children: Vec<WidgetSpec>,
    },
    Spacer {
        #[serde(flatten)]
        base: WidgetBase,
    },
    Separator {
        #[serde(flatten)]
        base: WidgetBase,
    },
}

macro_rules! delegate_base {
    ($self:ident, [$($variant:ident),*], $item:ident => $body:expr) => {
        match $self {
            $(Self::$variant { $item, .. } => $body,)*
        }
    };
}
macro_rules! delegate_required_services {
    ($self:ident, { $($custom_arm:tt)* }, [$($no_service_variant:ident),* $(,)?]) => {
        match $self {
            $($custom_arm)*
            $(Self::$no_service_variant { .. } => 0,)*
        }
    };
}
impl WidgetSpec {
    pub fn base(&self) -> &WidgetBase {
        delegate_base!(self, [
            Battery,
            Button,
            Calendar,
            Clock,
            Column,
            Notifications,
            Row,
            Separator,
            Slider,
            Spacer
        ], base => base)
    }
    pub fn id(&self) -> Option<&String> {
        self.base().id.as_ref()
    }
    pub fn class(&self) -> Option<&String> {
        self.base().class.as_ref()
    }
    pub fn required_services(&self) -> u8 {
        delegate_required_services!(self,
            {
                Self::Battery { .. } => 1 << 0,
                Self::Slider { func, ..} => {
                    match BackendFuncType::from(func) {
                        BackendFuncType::Brightness => {
                            0
                        },
                        BackendFuncType::Volume => {
                            1 << 1
                        }
                        _ => 0
                    }
                }
                Self::Row { children, .. } => {
                    children.iter().map(|c| c.required_services()).reduce(|acc, b| acc | b).unwrap_or(0)
                }
                Self::Column { children, .. } => {
                    children.iter().map(|c| c.required_services()).reduce(|acc, b| acc | b).unwrap_or(0)
                }
            },
            // Empty services
            [
                Button,
                Calendar,
                Clock,
                Notifications,
                Separator,
                Spacer,
            ]
        )
    }
}
impl WidgetSpec {
    pub fn as_calendar<'w>(&'w self) -> CalendarConfig<'w> {
        match self {
            WidgetSpec::Calendar {
                accent_color,
                font,
                hours_past,
                hours_future,
                hm_format,
                ..
            } => CalendarConfig {
                accent_color,
                font,
                hm_format: Some(hm_format),
                hours_past: *hours_past,
                hours_future: *hours_future,
            },
            _ => CalendarConfig {
                accent_color: "#e9a949",
                font: "Sans",
                hm_format: None,
                hours_past: 2,
                hours_future: 6,
            },
        }
    }
    pub fn as_button(self) -> Option<(WidgetBase, BackendFunc, Option<String>)> {
        if let Self::Button { base, func, icon } = self {
            Some((base, func, icon))
        } else {
            None
        }
    }
    pub fn as_slider(self) -> Option<(WidgetBase, BackendFunc, SliderRange, WidgetOrientation)> {
        if let Self::Slider {
            base,
            func,
            range,
            orientation,
        } = self
        {
            Some((base, func, range, orientation))
        } else {
            None
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
        .map_err(|e| watson_err!(WatsonErrorKind::Deserialize, e.to_string()))
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq, Eq, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum WidgetOrientation {
    #[default]
    Vertical,
    Horizontal,
}
