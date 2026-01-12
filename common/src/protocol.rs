use std::{
    cell::Cell,
    io::{Read, Write},
    ops::Not,
    os::unix::net::UnixStream,
};

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumIter};
use zbus::zvariant::OwnedValue;

use crate::{
    errors::{WatsonError, WatsonErrorKind},
    notification::Notification,
    watson_err,
};

pub struct SocketData;
impl SocketData {
    pub const SOCKET_ADDR: &'static str = "/tmp/watson.sock";
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, EnumIter, AsRefStr)]
pub enum DaemonService {
    BatteryStateListener = 0,
    AudioService = 1,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    Invalid,
}
impl BatteryState {
    pub fn capacity() -> Result<u32, WatsonError> {
        let capacity_path = "/sys/class/power_supply/BAT0/capacity";
        let capacity = {
            let capacity_opt = std::fs::read_to_string(capacity_path)
                .expect("Failed to read capacity")
                .trim()
                .parse::<u32>();

            match capacity_opt {
                Ok(c) => c,
                Err(e) => return Err(watson_err!(WatsonErrorKind::Deserialization, e.to_string())),
            }
        };
        Ok(capacity)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct SystemState {
    pub wifi: Cell<bool>,
    pub bluetooth: Cell<bool>,
    pub powermode: Cell<PowerMode>,
    pub brightness: Cell<u8>,
    pub volume: Cell<u8>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum InternalMessage {
    BatteryState {
        state: BatteryState,
        percentage: u32,
    },
    Notification(u32),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Response {
    Ok,
    Error(String),
    Pong,
    Status {
        running: bool,
        silent: bool,
    },
    Notification(Option<Notification>),
    Notifications(Vec<Notification>),
    BatteryStateChange {
        state: BatteryState,
        percentage: u32,
    },
    SystemState(SystemState),
}
pub trait IntoResponse {
    fn into_response(self) -> Response;
}
impl<E> IntoResponse for Result<E, WatsonError> {
    fn into_response(self) -> Response {
        match self {
            Ok(_) => Response::Ok,
            Err(e) => Response::Error(e.message),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Request {
    Ping,
    GetStatus,
    Silence(bool),
    Notification(u32),
    PendingNotifications,

    // Hardware
    RegisterServices(u8),
    SystemState,
    SetWifi(bool),
    SetBluetooth(bool),
    SetPowerMode(PowerMode),
    SetBacklight(u8),
    SetVolume(u8),
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum PowerMode {
    #[serde(rename = "power-saver")]
    PowerSave,
    #[serde(rename = "balanced")]
    #[default]
    Balanced,
    #[serde(rename = "performance")]
    Performace,
}

impl TryFrom<OwnedValue> for PowerMode {
    type Error = zbus::Error;

    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        let s: String = value.try_into()?;

        match s.as_str() {
            "power-saver" => Ok(Self::PowerSave),
            "balanced" => Ok(Self::Balanced),
            "performance" => Ok(Self::Performace),
            _ => Err(Self::Error::InvalidGUID),
        }
    }
}
impl ToString for PowerMode {
    fn to_string(&self) -> String {
        match self {
            Self::PowerSave => "power-saver",
            Self::Balanced => "balanced",
            Self::Performace => "performance",
        }
        .into()
    }
}
impl Not for PowerMode {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            Self::PowerSave => Self::Balanced,
            Self::Balanced => Self::PowerSave,
            Self::Performace => Self::PowerSave,
        }
    }
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
    fn write_sized<T: AsRef<[u8]>>(&mut self, buf: T) -> Result<(), WatsonError>;

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
    fn read_sized(&mut self) -> Result<Vec<u8>, WatsonError>;
}
impl SizedMessage for UnixStream {
    fn write_sized<T: AsRef<[u8]>>(&mut self, buf: T) -> Result<(), WatsonError> {
        let buf = buf.as_ref();

        // Safely convert buf_len from usize to u32
        let buf_len: u32 = buf
            .len()
            .try_into()
            .map_err(|_| watson_err!(WatsonErrorKind::InvalidData, "message too large"))?;

        // Write message size to stream
        self.write_all(&buf_len.to_be_bytes())
            .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;

        // Write message to stream
        self.write_all(buf)
            .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;

        Ok(())
    }
    fn read_sized(&mut self) -> Result<Vec<u8>, WatsonError> {
        let mut buf_len = [0u8; 4];

        // Read message length
        self.read_exact(&mut buf_len)
            .map_err(|e| watson_err!(WatsonErrorKind::StreamRead, e.to_string()))?;
        let msg_len = u32::from_be_bytes(buf_len) as usize;

        let mut buf = vec![0u8; msg_len];
        self.read_exact(&mut buf)
            .map_err(|e| watson_err!(WatsonErrorKind::StreamRead, e.to_string()))?;

        Ok(buf)
    }
}
