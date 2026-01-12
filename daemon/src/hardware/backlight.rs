use std::{fs, path::PathBuf};

use common::{
    errors::{WatsonError, WatsonErrorKind},
    watson_err,
};
use zbus::Proxy;

use crate::hardware::HardwareController;

pub struct BrightnessState {
    path: PathBuf,
    name: String,
    max: u32,
    proxy: Proxy<'static>,
}

impl HardwareController {
    // ----- Brightness (Native Sysfs) -----
    async fn set_brightness_state(&mut self) -> Result<(), WatsonError> {
        let device_path = fs::read_dir("/sys/class/backlight/")
            .map_err(|e| watson_err!(WatsonErrorKind::DirRead, e.to_string()))?
            .next()
            .ok_or_else(|| {
                watson_err!(WatsonErrorKind::IO, "No backlight device found".to_string())
            })?
            .map_err(|e| watson_err!(WatsonErrorKind::IO, e.to_string()))?;

        let name = device_path
            .file_name()
            .into_string()
            .map_err(|_| watson_err!(WatsonErrorKind::IO, "Invalid device name".to_string()))?;

        let max: u32 = fs::read_to_string(device_path.path().join("max_brightness"))
            .map_err(|e| watson_err!(WatsonErrorKind::FileRead, e.to_string()))?
            .trim()
            .parse()
            .unwrap_or(100);

        let proxy = Proxy::new(
            &self.conn,
            "org.freedesktop.login1",
            "/org/freedesktop/login1/session/auto",
            "org.freedesktop.login1.Session",
        )
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;
        self.brightness_state = Some(BrightnessState {
            name,
            path: device_path.path(),
            max,
            proxy,
        });
        Ok(())
    }
    pub async fn set_brightness(&mut self, percent: u8) -> Result<(), WatsonError> {
        let _permit = match &self.throttle.try_acquire() {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };

        if self.brightness_state.is_none() {
            self.set_brightness_state().await?;
        }

        if let Some(state) = &self.brightness_state {
            let absolute = (percent as u32 * state.max) / 100;

            // Most logind versions expect (subsystem, device_name, brightness_value)
            // You can usually pass "backlight" and the device name (e.g., "intel_backlight")
            state
                .proxy
                .call::<_, _, ()>(
                    "SetBrightness",
                    &("backlight", state.name.as_str(), absolute),
                )
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::DBusProxyCall, e.to_string()))?;
        }

        Ok(())
    }
    pub async fn get_brightness(&mut self) -> Result<u8, WatsonError> {
        if self.brightness_state.is_none() {
            self.set_brightness_state().await?;
        }

        if let Some(state) = &self.brightness_state {
            if state.max == 0 {
                return Ok(0);
            }

            let current_raw = fs::read_to_string(state.path.join("brightness"))
                .map_err(|e| watson_err!(WatsonErrorKind::FileRead, e.to_string()))?;
            let current: u32 = current_raw.trim().parse().map_err(|_| {
                watson_err!(
                    WatsonErrorKind::Deserialization,
                    "Failed to parse current brightness as u32."
                )
            })?;

            let percent = ((current as f32 / state.max as f32) * 100.0) as u8;
            return Ok(percent);
        }

        Ok(0)
    }
}
