use tokio::net::UnixListener;
use common::protocol::{Request, Response};
use serde_json;

use common::tokio::AsyncSizedMessage;

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let socket_path = "/tmp/myapp.sock";
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)?;
    println!("Daemon listening on {}", socket_path);

    loop {
        let (mut stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            loop {
                let buf = match stream.read_sized().await {
                    Ok(b) => b,
                    Err(e) => break // Client disconnected
                };

                let req: Request = match serde_json::from_slice(&buf) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let resp = match req {
                    Request::Ping => Response::Pong,
                    Request::GetStatus => Response::Status { running: true },
                };

                let out = serde_json::to_vec(&resp).unwrap();
                if stream.write_sized(&out).await.is_err() {
                    break;
                }
            }
        });
    }
}

