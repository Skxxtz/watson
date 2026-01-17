use async_trait::async_trait;
use common::protocol::{
    BatteryState, DaemonService, InternalMessage, IntoResponse, Request, Response, SocketData,
};
use common::utils::errors::{WatsonError, WatsonErrorKind};
use common::watson_err;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;
use tokio::{
    net::{UnixListener, UnixStream},
    sync::{RwLock, broadcast},
};
use zbus::Connection;
use zbus::zvariant::OwnedValue;

use common::tokio::{AsyncSizedMessage, SizedMessageObj};
use zbus::conn::Builder;

mod hardware;
mod notify;
mod service_reg;
use notify::{DaemonHandle, NotificationDaemon};

use crate::hardware::{AudioCommand, SystemStateBuilder, audio_actor};

static DAEMON_TX: OnceLock<Sender<InternalMessage>> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<(), WatsonError> {
    let (tx, rx) = broadcast::channel::<InternalMessage>(16);
    DAEMON_TX.set(tx).expect("Failed to set daemon_tx");

    let daemon_raw = NotificationDaemon::new().await?;
    let wake_signal = Arc::clone(&daemon_raw.wake_signal);
    let daemon = Arc::new(RwLock::new(daemon_raw));

    // Start Battery Service
    let _result = tokio::spawn(battery_state_listener(Arc::clone(&daemon)));

    // Start Audio Service
    let audio_tx = {
        let (audio_tx, audio_rx) = mpsc::channel::<AudioCommand>(16);
        std::thread::spawn({
            let audio_tx = audio_tx.clone();
            let register = Arc::clone(&daemon.read().await.register);
            move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                rt.block_on(async move {
                    audio_actor(audio_tx, audio_rx, Arc::clone(&wake_signal), register).await
                });
            }
        });
        audio_tx
    };
    daemon.write().await.hardware.set_audio_state(audio_tx);

    // Start Dbus Service
    let _result = tokio::spawn(dbus_listener(Arc::clone(&daemon)));

    // Setup Server
    let _ = std::fs::remove_file(SocketData::SOCKET_ADDR);
    let listener = UnixListener::bind(SocketData::SOCKET_ADDR)
        .map_err(|e| watson_err!(WatsonErrorKind::StreamBind, e.to_string()))?;

    let connection_count = Arc::new(AtomicUsize::new(0));
    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::StreamConnect, e.to_string()))?;
        connection_count.fetch_add(1, Ordering::SeqCst);

        let rx = rx.resubscribe();
        tokio::spawn({
            let daemon_clone = Arc::clone(&daemon);
            let count_clone = Arc::clone(&connection_count);
            async move {
                handle_client(stream, daemon_clone.clone(), rx).await;
                count_clone.fetch_sub(1, Ordering::SeqCst);
                if count_clone.load(Ordering::SeqCst) == 0 {
                    daemon_clone.write().await.register.clear();
                }
            }
        });
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

async fn battery_state_listener(daemon: Arc<RwLock<NotificationDaemon>>) -> zbus::Result<()> {
    let conn = Connection::system().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        "org.freedesktop.DBus.Properties",
    )
    .await?;

    let wake_signal = Arc::clone(&daemon.read().await.wake_signal);

    let mut stream = proxy.receive_signal("PropertiesChanged").await?;

    // Cache to prevent redundant updates
    let mut last_state = BatteryState::Invalid;
    loop {
        // Ghost check
        loop {
            let active = daemon
                .read()
                .await
                .register
                .is_active(DaemonService::BatteryStateListener);

            if active {
                break;
            }

            wake_signal.notified().await;
        }

        tokio::select! {
            next_signal = stream.next() => {
                let Some(signal) = next_signal else {
                    break;
                };
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
                        4 => BatteryState::Full,
                        5 => BatteryState::Charging,
                        _ => BatteryState::Invalid
                    };
                    if state != last_state {
                        last_state = state;
                        changed_significantly = true;
                    }
                }

                // Check for changes
                if let Ok(percentage) = BatteryState::capacity() {
                    if changed_significantly && last_state != BatteryState::Invalid {
                        let _ = DAEMON_TX.get().map(|d| d.send(InternalMessage::BatteryState {
                            state: last_state,
                            percentage,
                        }));
                    }
                }

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

                let req: Request = match bincode::deserialize(&buf) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let daemon_clone = Arc::clone(&daemon);

                let resp = {
                    let mut daemon_guard = daemon_clone.write().await;
                    req.handle(&mut *daemon_guard).await
                };

                if !matches!(resp, Response::Ok) {
                    if let Ok(out) = SizedMessageObj::from_struct(&resp) {
                        if stream.write_sized(out).await.is_err() {
                            break;
                        }
                    }
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
                    InternalMessage::BatteryState { state, percentage } => Response::BatteryState {
                        state,
                        percentage
                    },
                    InternalMessage::VolumeStateChange { percentage } => Response::VolumeState { percentage },
                };

                if let Ok(out) = SizedMessageObj::from_struct(&resp) {
                    if stream.write_sized(out).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

#[async_trait]
trait RequestHandler {
    async fn handle(self, daemon: &mut NotificationDaemon) -> Response;
}
#[async_trait]
impl RequestHandler for Request {
    async fn handle(self, daemon: &mut NotificationDaemon) -> Response {
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
            Request::RegisterServices(services) => {
                daemon.register.set_registered_services(services);
                println!("Registered required services. {}", daemon.register);
                // Wake services
                daemon.wake_signal.notify_waiters();

                match SystemStateBuilder::new(&mut daemon.hardware).await {
                    Ok(state) => Response::SystemState(state),
                    Err(e) => Response::Error(e.message),
                }
            }
            Request::SetWifi(enabled) => daemon.hardware.set_wifi(enabled).await.into_response(),
            Request::SetBluetooth(enabled) => {
                daemon.hardware.set_bluetooth(enabled).await.into_response()
            }
            Request::SetPowerMode(mode) => daemon
                .hardware
                .set_powermode(mode.into())
                .await
                .into_response(),
            Request::SetBacklight(perc) => {
                daemon.hardware.set_brightness(perc).await.into_response()
            }
            Request::SetVolume(perc) => daemon.hardware.set_volume(perc).await.into_response(),
            Request::SystemState => match SystemStateBuilder::new(&mut daemon.hardware).await {
                Ok(state) => Response::SystemState(state),
                Err(e) => Response::Error(e.message),
            },
            Request::Command(_) => Response::Todo,
        }
    }
}
