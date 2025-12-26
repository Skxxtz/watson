use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
};

use serde::{Deserialize, Serialize};

use crate::notification::Notification;

pub struct SocketData;
impl SocketData {
    pub const SOCKET_ADDR: &'static str = "/tmp/watson.sock";
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    Invalid,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum InternalMessage {
    BatteryState(BatteryState),
    Notification(u32),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Response {
    Ok,
    Error(String),
    Pong,
    Status { running: bool, silent: bool },
    Notification(Option<Notification>),
    Notifications(Vec<Notification>),
    BatteryStateChange(BatteryState),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Request {
    Ping,
    GetStatus,
    Silence(bool),
    Notification(u32),
    PendingNotifications,
}

pub trait SizedMessage {
    /// Writes a message to the channel with a length prefix.
    ///
    /// The message length is encoded as a 4-byte big-endian `u32` before
    /// the message itself. This allows the receiver to know exactly how
    /// many bytes to read.
    ///
    /// # Errors
    ///
    /// Returns an `std::io::Error` if:
    /// - Writing to the underlying channel fails, or
    /// - The message is too large to fit in a `u32` (greater than 4 GiB).
    fn write_sized<T: AsRef<[u8]>>(&mut self, buf: T) -> Result<(), std::io::Error>;

    /// Reads a length-prefixed message from the channel.
    ///
    /// Expects the first 4 bytes to be a big-endian `u32` representing
    /// the length of the message, followed by exactly that many bytes.
    ///
    /// # Errors
    ///
    /// Returns an `std::io::Error` if:
    /// - Reading from the underlying channel fails, or
    /// - The indicated message length is unreasonably large or invalid.
    fn read_sized(&mut self) -> Result<Vec<u8>, std::io::Error>;
}
impl SizedMessage for UnixStream {
    fn write_sized<T: AsRef<[u8]>>(&mut self, buf: T) -> Result<(), std::io::Error> {
        let buf = buf.as_ref();

        // Safely convert buf_len from usize to u32
        let buf_len: u32 = buf.len().try_into().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "message too large")
        })?;

        // Write message size to stream
        self.write_all(&buf_len.to_be_bytes())?;

        // Write message to stream
        self.write_all(buf)?;

        Ok(())
    }
    fn read_sized(&mut self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf_len = [0u8; 4];

        // Read message length
        self.read_exact(&mut buf_len)?;
        let msg_len = u32::from_be_bytes(buf_len) as usize;

        let mut buf = vec![0u8; msg_len];
        self.read_exact(&mut buf)?;

        Ok(buf)
    }
}
