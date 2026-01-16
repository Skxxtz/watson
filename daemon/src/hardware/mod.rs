use std::sync::Arc;

use common::{protocol::SystemStateRaw, utils::errors::WatsonError};
use tokio::sync::{Semaphore, mpsc};
use zbus::Connection;

use crate::hardware::{audio::VolumeState, backlight::BrightnessState};

mod audio;
mod backlight;
mod network;
mod power;

pub use audio::{AudioCommand, audio_actor};

pub struct SystemStateBuilder;
impl SystemStateBuilder {
    pub(crate) async fn new(
        hardware: &mut HardwareController,
    ) -> Result<SystemStateRaw, WatsonError> {
        Ok(SystemStateRaw {
            wifi: hardware.get_wifi().await?,
            bluetooth: hardware.get_bluetooth().await?,
            powermode: hardware.get_powermode().await?.into(),
            brightness: hardware.get_brightness().await?,
            volume: hardware.get_volume().await?,
        })
    }
}

pub struct HardwareController {
    conn: Connection,
    brightness_state: Option<BrightnessState>,
    volume_state: Option<VolumeState>,
    throttle: Arc<Semaphore>,
}
impl HardwareController {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            brightness_state: None,
            volume_state: None,
            throttle: Arc::new(Semaphore::new(1)),
        }
    }
    pub fn set_audio_state(&mut self, tx: mpsc::Sender<AudioCommand>) {
        self.volume_state.replace(VolumeState::new(tx));
    }
}
