use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zbus::zvariant::OwnedValue;

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub body: String,
    pub summary: String,
    pub actions: Vec<String>,
    pub hints: HashMap<String, OwnedValue>,
    pub replaces_id: u32,
    pub expire_timeout: i32,
    pub urgency: Urgency,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}
impl Urgency {
    pub fn is_low(&self) -> bool {
        matches!(self, Self::Low)
    }
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::Critical)
    }
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Low => "prio-low",
            Self::Normal => "prio-normal",
            Self::Critical => "prio-critical",
        }
    }
}
impl Default for Urgency {
    fn default() -> Self {
        Self::Normal
    }
}
impl From<u8> for Urgency {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Low,
            1 => Self::Normal,
            2 => Self::Critical,
            _ => Self::Normal,
        }
    }
}
