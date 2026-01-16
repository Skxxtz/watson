use common::protocol::{AtomicSystemState, Request, Response, SocketData, UpdateField};
use common::tokio::{AsyncSizedMessage, SizedMessageObj};
use common::utils::errors::{WatsonError, WatsonErrorKind};
use common::watson_err;
use std::mem::discriminant;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{Notify, broadcast, mpsc};

pub struct ClientConnection {
    writer: OwnedWriteHalf,
    reader: OwnedReadHalf,
}

#[allow(dead_code)]
impl ClientConnection {
    pub async fn new() -> Result<Self, WatsonError> {
        let stream = UnixStream::connect(SocketData::SOCKET_ADDR)
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::StreamConnect, e.to_string()))?;
        let (reader, writer) = stream.into_split();

        Ok(Self { reader, writer })
    }
    pub async fn spawn_engine(
        self,
        response_tx: broadcast::Sender<Response>,
        state: Arc<AtomicSystemState>,
        notify: Arc<Notify>,
    ) -> Result<mpsc::UnboundedSender<Request>, WatsonError> {
        let (mut reader, mut writer) = (self.reader, self.writer);
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<Request>();

        // 1. Task for WRITING to the Daemon
        tokio::spawn(async move {
            while let Some(req) = request_rx.recv().await {
                let mut latest_msg = req;

                // Drain the channel
                while let Ok(next_msg) = request_rx.try_recv() {
                    if discriminant(&next_msg) == discriminant(&latest_msg) {
                        latest_msg = next_msg;
                    } else {
                        if let Ok(buf) = SizedMessageObj::from_struct(&latest_msg) {
                            let _ = writer.write_sized(buf).await;
                        }
                        latest_msg = next_msg;
                    }
                }

                if let Ok(buf) = SizedMessageObj::from_struct(&latest_msg) {
                    if let Err(e) = writer.write_sized(buf).await {
                        eprintln!("Failed to write to daemon: {:?}", e);
                        break;
                    }
                }
            }
        });
        // 2. Task for READING from the Daemon
        tokio::spawn(async move {
            let mut throttle = Throttle::new(60);

            loop {
                match reader.read_sized().await {
                    Ok(buf) => {
                        if let Ok(v) = bincode::deserialize::<Response>(&buf) {
                            match v {
                                Response::VolumeState { percentage } => {
                                    state.volume.store(percentage, Ordering::Relaxed);
                                    state.updated.fetch_or(
                                        1 << UpdateField::Volume as u8,
                                        Ordering::Relaxed,
                                    );

                                    if throttle.can_notify() {
                                        notify.notify_one();
                                    }
                                }
                                Response::SystemState(s) => {
                                    state.update_from_state(s);

                                    if throttle.can_notify() {
                                        notify.notify_one();
                                    }
                                }
                                _ => {
                                    let _result = response_tx.send(v);
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(request_tx) // Return this so the UI can send Pings, etc.
    }
}

struct Throttle {
    last_sent: Instant,
    interval: Duration,
}

impl Throttle {
    fn new(fps: u64) -> Self {
        Self {
            last_sent: Instant::now() - Duration::from_secs(1),
            interval: Duration::from_millis(1000 / fps),
        }
    }

    fn can_notify(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_sent) >= self.interval {
            self.last_sent = now;
            true
        } else {
            false
        }
    }
}
