use common::errors::{WatsonError, WatsonErrorKind};
use common::protocol::{Request, SocketData};
use common::tokio::{AsyncSizedMessage, SizedMessageObj};
use common::watson_err;
use std::mem::discriminant;
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{broadcast, mpsc};

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
        response_tx: broadcast::Sender<Vec<u8>>,
    ) -> mpsc::UnboundedSender<Request> {
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
            loop {
                match reader.read_sized().await {
                    Ok(buf) => {
                        let _ = response_tx.send(buf);
                    }
                    Err(e) => {
                        eprintln!("Daemon connection lost: {:?}", e);
                        break;
                    }
                }
            }
        });

        request_tx // Return this so the UI can send Pings, etc.
    }
}
