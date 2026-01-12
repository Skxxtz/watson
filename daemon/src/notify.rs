use std::collections::HashMap;
use std::sync::Arc;

use common::errors::{WatsonError, WatsonErrorKind};
use common::notification::Notification;
use common::protocol::InternalMessage;
use common::watson_err;
use tokio::sync::{Notify, RwLock, broadcast};
use zbus::zvariant::OwnedValue;
use zbus::{Connection, interface};

use crate::hardware::HardwareController;
use crate::service_reg::ServiceRegister;

pub struct DaemonHandle {
    daemon: Arc<RwLock<NotificationDaemon>>,
}
impl DaemonHandle {
    pub fn new(daemon: Arc<RwLock<NotificationDaemon>>) -> Self {
        Self { daemon }
    }
}

pub struct DaemonSettings {
    pub silent: bool,
}

pub struct NotificationDaemon {
    id: u32,
    buffer: HashMap<u32, Notification>,
    sender: broadcast::Sender<InternalMessage>,
    pub wake_signal: Arc<Notify>,
    pub hardware: HardwareController,
    pub settings: DaemonSettings,
    pub register: ServiceRegister,
}
impl NotificationDaemon {
    pub async fn new(sender: broadcast::Sender<InternalMessage>) -> Result<Self, WatsonError> {
        let conn = Connection::system()
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::DBusConnect, e.to_string()))?;
        Ok(Self {
            id: 0,
            buffer: HashMap::new(),
            sender,
            wake_signal: Arc::new(Notify::new()),
            hardware: HardwareController::new(conn),
            settings: DaemonSettings { silent: false },
            register: ServiceRegister::new(),
        })
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

        let urgency = hints
            .get("urgency")
            .and_then(|v| v.downcast_ref::<u8>().ok())
            .unwrap_or(1);

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
            urgency: urgency.into(),
        };
        daemon.buffer.insert(id, notification);

        // Notify that a new notification has been added
        let _result = daemon.sender.send(InternalMessage::Notification(id));

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
