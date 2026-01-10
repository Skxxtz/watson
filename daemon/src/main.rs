use common::protocol::{BatteryState, InternalMessage, Request, Response, SocketData};
use futures_util::StreamExt;
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLockWriteGuard;
use tokio::{
    net::{UnixListener, UnixStream},
    sync::{RwLock, broadcast},
};
use zbus::Connection;
use zbus::zvariant::OwnedValue;

use common::tokio::AsyncSizedMessage;
use zbus::conn::Builder;

mod notify;
use crate::notify::{DaemonHandle, NotificationDaemon};

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let (tx, rx) = broadcast::channel::<InternalMessage>(16);
    let daemon = Arc::new(RwLock::new(NotificationDaemon::new(tx.clone())));

    let _result = tokio::spawn(battery_state_listener(tx.clone()));
    let _result = tokio::spawn(dbus_listener(daemon.clone()));

    // Setup Server
    let _ = std::fs::remove_file(SocketData::SOCKET_ADDR);
    let listener = UnixListener::bind(SocketData::SOCKET_ADDR)?;

    loop {
        let (stream, _) = listener.accept().await?;
        let rx = rx.resubscribe();
        tokio::spawn(handle_client(stream, daemon.clone(), rx));
    }
}

async fn dbus_listener(daemon: Arc<RwLock<NotificationDaemon>>) -> zbus::Result<()> {
    // Connect to session bus
    let daemon_handle = DaemonHandle::new(daemon);
    let _conn = Builder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", daemon_handle)?
        .build()
        .await?;

    println!("Notification daemon running");
    std::future::pending::<()>().await;

    Ok(())
}

async fn battery_state_listener(sender: broadcast::Sender<InternalMessage>) -> zbus::Result<()> {
    let conn = Connection::system().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        "org.freedesktop.DBus.Properties",
    )
    .await?;

    let mut stream = proxy.receive_signal("PropertiesChanged").await?;

    // Cache to prevent redundant updates
    let mut last_state = BatteryState::Invalid;

    while let Some(signal) = stream.next().await {
        let (iface, changed, _): (String, HashMap<String, OwnedValue>, Vec<String>) =
            signal.body().deserialize()?;

        if iface != "org.freedesktop.UPower.Device" {
            continue;
        }

        let new_state_raw = changed
            .get("State")
            .and_then(|v| TryInto::<u32>::try_into(v).ok());

        let mut changed_significantly = false;
        if let Some(s) = new_state_raw {
            let state = match s {
                1 => BatteryState::Charging,
                2 => BatteryState::Discharging,
                3 => BatteryState::Full,
                _ => BatteryState::Invalid,
            };
            if state != last_state {
                last_state = state;
                changed_significantly = true;
            }
        }

        // Check for changes
        if let Ok(percentage) = BatteryState::capacity() {
            if changed_significantly && last_state != BatteryState::Invalid {
                let _ = sender.send(InternalMessage::BatteryState {
                    state: last_state,
                    percentage,
                });
            }
        }
    }
    Ok(())
}

async fn handle_client(
    mut stream: UnixStream,
    daemon: Arc<RwLock<NotificationDaemon>>,
    mut rx: broadcast::Receiver<InternalMessage>,
) {
    loop {
        tokio::select! {
            result = stream.read_sized() => {
                let buf = match result {
                    Ok(b) => b,
                    Err(_) => break, // Client disconnected
                };

                let req: Request = match serde_json::from_slice(&buf) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let resp = req.handle(daemon.write().await);

                let out = serde_json::to_vec(&resp).unwrap();
                if stream.write_sized(&out).await.is_err() {
                    break;
                }
            }

            msg = rx.recv() => {
                let message = match msg {
                    Ok(id) => id,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Client fell behind
                        continue
                    }
                    Err(_) => break // channel closed
                };

                let resp = match message {
                    InternalMessage::Notification(id) => {
                        let daemon = daemon.read().await;
                        Response::Notification(daemon.get_by_id(id).cloned())
                    }
                    InternalMessage::BatteryState { state, percentage } => Response::BatteryStateChange {
                        state,
                        percentage
                    }
                };

                let out = serde_json::to_vec(&resp).unwrap();
                if stream.write_sized(&out).await.is_err() {
                    break;
                }
            }
        }
    }
}

trait RequestHandler {
    fn handle(self, daemon: RwLockWriteGuard<NotificationDaemon>) -> Response;
}
impl RequestHandler for Request {
    fn handle(self, mut daemon: RwLockWriteGuard<NotificationDaemon>) -> Response {
        match self {
            Request::Ping => Response::Pong,
            Request::GetStatus => Response::Status {
                running: true,
                silent: daemon.settings.silent,
            },
            Request::Notification(id) => Response::Notification(daemon.get_by_id(id).cloned()),
            Request::PendingNotifications => {
                let notifs = daemon.pending_notifications();
                Response::Notifications(notifs)
            }
            Request::Silence(value) => {
                daemon.settings.silent = value;
                Response::Ok
            }
        }
    }
}
