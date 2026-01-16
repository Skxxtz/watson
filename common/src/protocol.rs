use std::{
    cell::Cell,
    io::{Read, Write},
    ops::Not,
    os::unix::net::UnixStream,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumIter};
use zbus::zvariant::OwnedValue;

use crate::{
    notification::Notification,
    utils::errors::{WatsonError, WatsonErrorKind},
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
                Err(e) => return Err(watson_err!(WatsonErrorKind::Deserialize, e.to_string())),
            }
        };
        Ok(capacity)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SystemState {
    pub wifi: Cell<bool>,
    pub bluetooth: Cell<bool>,
    pub powermode: Cell<u8>,
    pub brightness: Cell<u8>,
    pub volume: Cell<u8>,
}
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct SystemStateRaw {
    pub wifi: bool,
    pub bluetooth: bool,
    pub powermode: u8,
    pub brightness: u8,
    pub volume: u8,
}
#[derive(Debug, Default)]
pub struct AtomicSystemState {
    pub initialized: AtomicBool,
    pub updated: AtomicU8,
    pub wifi: AtomicBool,
    pub bluetooth: AtomicBool,
    pub powermode: AtomicU8,
    pub brightness: AtomicU8,
    pub volume: AtomicU8,
}

#[repr(u8)]
#[derive(Default)]
pub enum UpdateField {
    #[default]
    None = 0,
    Init = 1,
    Wifi = 2,
    Bluetooth = 3,
    Powermode = 4,
    Brightness = 5,
    Volume = 6,
}
impl From<u8> for UpdateField {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Init,
            2 => Self::Wifi,
            3 => Self::Bluetooth,
            4 => Self::Powermode,
            5 => Self::Brightness,
            6 => Self::Volume,
            _ => Self::None,
        }
    }
}

impl AtomicSystemState {
    pub fn update_from_state(&self, state: SystemStateRaw) {
        self.initialized.store(true, Ordering::Relaxed);
        self.updated
            .fetch_or(1 << UpdateField::Init as u8, Ordering::Relaxed);
        self.wifi.store(state.wifi, Ordering::Relaxed);
        self.bluetooth.store(state.bluetooth, Ordering::Relaxed);
        self.powermode.store(state.powermode, Ordering::Relaxed);
        self.brightness.store(state.brightness, Ordering::Relaxed);
        self.volume.store(state.volume, Ordering::Relaxed);
    }
}

impl From<&SystemStateRaw> for SystemState {
    fn from(v: &SystemStateRaw) -> Self {
        Self {
            wifi: Cell::new(v.wifi),
            bluetooth: Cell::new(v.bluetooth),
            powermode: Cell::new(v.powermode),
            brightness: Cell::new(v.brightness),
            volume: Cell::new(v.volume),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum InternalMessage {
    BatteryState {
        state: BatteryState,
        percentage: u32,
    },
    Notification(u32),
    VolumeStateChange {
        percentage: u8,
    },
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
    SystemState(SystemStateRaw),
    BatteryState {
        state: BatteryState,
        percentage: u32,
    },
    VolumeState {
        percentage: u8,
    },
}
impl Response {
    pub fn is_state_change(&self) -> bool {
        match self {
            Self::SystemState(_) | Self::VolumeState { .. } | Self::BatteryState { .. } => true,
            _ => false,
        }
    }
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
    SetPowerMode(u8),
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
impl From<u8> for PowerMode {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::PowerSave,
            1 => Self::Balanced,
            2 => Self::Performace,
            _ => Self::Balanced,
        }
    }
}
impl From<PowerMode> for u8 {
    fn from(v: PowerMode) -> Self {
        match v {
            PowerMode::PowerSave => 0,
            PowerMode::Balanced => 1,
            PowerMode::Performace => 2,
        }
    }
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
