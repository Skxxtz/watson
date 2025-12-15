use common::protocol::{Request, Response, SocketData};
use common::tokio::AsyncSizedMessage;
use tokio::net::UnixStream;

pub struct ClientConnection {
    stream: UnixStream,
}
impl ClientConnection {
    pub async fn new() -> std::io::Result<Self> {
        let stream = UnixStream::connect(SocketData::SOCKET_ADDR).await?;

        Ok(Self { stream })
    }
    pub async fn send(&mut self, req: Request) -> tokio::io::Result<Response> {
        // Serialize the request
        let out = serde_json::to_vec(&req).unwrap();
        self.stream.write_sized(&out).await?;

        // Read the response
        let buf = self.stream.read_sized().await?;
        let resp: Response = serde_json::from_slice(&buf).unwrap();
        Ok(resp)
    }
}
