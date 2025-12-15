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
}
