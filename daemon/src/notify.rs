use std::collections::HashMap;
use std::sync::Arc;

use common::notification::Notification;
use tokio::sync::{RwLock, broadcast};
use zbus::interface;
use zbus::zvariant::OwnedValue;

pub struct DaemonHandle {
    daemon: Arc<RwLock<NotificationDaemon>>,
}
impl DaemonHandle {
    pub fn new(daemon: Arc<RwLock<NotificationDaemon>>) -> Self {
        Self { daemon }
    }
}

pub struct NotificationDaemon {
    id: u32,
    buffer: HashMap<u32, Notification>,
    sender: broadcast::Sender<u32>,
}
impl NotificationDaemon {
    pub fn new(sender: broadcast::Sender<u32>) -> Self {
        Self {
            id: 0,
            buffer: HashMap::new(),
            sender,
        }
    }

    pub fn get_by_id(&self, id: u32) -> Option<&Notification> {
        self.buffer.get(&id)
    }

    pub fn pending_notifications(&self) -> Vec<Notification> {
        self.buffer.values().cloned().collect()
    }
}

#[interface(name = "org.freedesktop.Notifications")]
impl DaemonHandle {
    async fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        // log!("Notification received");
        let mut daemon = self.daemon.write().await;
        daemon.id += 1;
        let id = daemon.id;

        let notification = Notification {
            id,
            app_name,
            replaces_id,
            app_icon,
            body,
            summary,
            actions,
            hints,
            expire_timeout,
        };
        daemon.buffer.insert(id, notification);

        // Notify that a new notification has been added
        let _result = daemon.sender.send(id);

        id
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "watson-daemon".into(),
            "me".into(),
            "1.0".into(),
            "1.2".into(),
        )
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "actions".into()]
    }
}
