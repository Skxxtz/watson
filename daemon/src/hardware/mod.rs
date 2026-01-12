use std::{cell::Cell, sync::Arc};

use common::{
    errors::{WatsonError, WatsonErrorKind},
    protocol::SystemState,
    watson_err,
};
use tokio::sync::Semaphore;
use zbus::Connection;

use crate::hardware::{audio::VolumeState, backlight::BrightnessState};

mod audio;
mod backlight;
mod network;
mod power;

pub struct SystemStateBuilder;
impl SystemStateBuilder {
    pub(crate) async fn new() -> Result<SystemState, WatsonError> {
        let conn = Connection::system()
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::DBusConnect, e.to_string()))?;

        let mut tmp_hardware = HardwareController::new(conn);
        Ok(SystemState {
            wifi: Cell::new(tmp_hardware.get_wifi().await?),
            bluetooth: Cell::new(tmp_hardware.get_bluetooth().await?),
            powermode: Cell::new(tmp_hardware.get_powermode().await?),
            brightness: Cell::new(tmp_hardware.get_brightness().await?),
            volume: Cell::new(tmp_hardware.get_volume().await?),
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
}
