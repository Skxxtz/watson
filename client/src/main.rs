use crate::connection::ClientConnection;
use common::protocol::Request;

mod connection;

#[tokio::main]
async fn main() {
    connect().await
}

async fn connect() {
    match ClientConnection::new().await {
        Ok(mut c) => {
            if let Ok(response) = c.send(Request::PendingNotifications).await {
                println!("{:?}", response);
            }
        }
        Err(e) => {
            eprintln!("{:?}", e);
        }
    }
}
