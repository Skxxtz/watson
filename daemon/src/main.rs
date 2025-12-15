use std::sync::Arc;

use common::protocol::{Request, Response, SocketData};
use serde_json;
use tokio::sync::RwLockWriteGuard;
use tokio::{
    net::{UnixListener, UnixStream},
    sync::{RwLock, broadcast},
};

use common::tokio::AsyncSizedMessage;
use zbus::conn::Builder;

mod notify;
use crate::notify::{DaemonHandle, NotificationDaemon};

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let (sender, receiver) = broadcast::channel::<u32>(2);
    let daemon = Arc::new(RwLock::new(NotificationDaemon::new(sender)));

    let _result = tokio::spawn(dbus_listener(daemon.clone()));

    // Setup Server
    let _ = std::fs::remove_file(SocketData::SOCKET_ADDR);
    let listener = UnixListener::bind(SocketData::SOCKET_ADDR)?;

    loop {
        let (stream, _) = listener.accept().await?;
        let rx = receiver.resubscribe();
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
    futures_util::future::pending::<()>().await;

    Ok(())
}
async fn handle_client(
    mut stream: UnixStream,
    daemon: Arc<RwLock<NotificationDaemon>>,
    mut rx: broadcast::Receiver<u32>,
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
                let id = match msg {
                    Ok(id) => id,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Client fell behind
                        continue
                    }
                    Err(_) => break // channel closed
                };

                let daemon = daemon.read().await;
                let resp = Response::Notification(daemon.get_by_id(id).cloned());
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
