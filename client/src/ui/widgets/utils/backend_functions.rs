use std::sync::atomic::Ordering;

use common::protocol::Request;
use serde::{Deserialize, Serialize};

use crate::ui::widgets::utils::interactives::*;
macro_rules! define_backend_functions {
    (
        $(
            $( #[$attr:meta] )* $variant:ident $( {
                $( $(#[$f_meta:meta])* $field:ident : $type:ty ),* $(,)?
            } )?
        ),* $(,)?
    ) => {
        #[derive(Debug, Clone, Deserialize, Serialize)]
        #[serde(rename_all = "lowercase")]
        pub enum BackendFunc {
            $(
                $( #[$attr] )* $variant $( { $( $(#[$f_meta])* $field : $type ),* } )?,
            )*
        }

        // BackendFuncType Enum
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display)]
        #[strum(serialize_all = "lowercase")]
        pub enum BackendFuncType {
            $( $variant, )*
        }

        impl From<&BackendFunc> for BackendFuncType {
            fn from(value: &BackendFunc) -> Self {
                match value {
                    $( BackendFunc::$variant { .. } => Self::$variant, )*
                }
            }
        }
        impl From<BackendFunc> for BackendFuncType {
            fn from(value: BackendFunc) -> Self {
                match value {
                    $( BackendFunc::$variant { .. } => Self::$variant, )*
                }
            }
        }
    };
}

// Call it exactly as you had it
define_backend_functions!(
    None,
    Wifi,
    Bluetooth,
    Dnd,
    Powermode,
    Volume,
    Brightness,
    Custom {
        id: String,
        states: Vec<FunctionConfig>,
    }
);
impl Default for BackendFunc {
    fn default() -> Self {
        Self::None
    }
}

// ----- Backend Functions
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default, Hash)]
pub struct FunctionConfig {
    icon: String,
    command: String,
}
impl BackendFunc {
    pub fn build(self) -> Box<dyn WidgetBehavior> {
        let func = BackendFuncType::from(&self);
        match self {
            Self::Wifi => Box::new(ToggleButton {
                icons: [
                    "network-wireless-disabled-symbolic",
                    "network-wireless-signal-excellent-symbolic",
                ],
                getter: |s| s.wifi.load(Ordering::Relaxed),
                setter: |s, v| s.wifi.store(v, Ordering::Relaxed),
                request_builder: |v| Request::SetWifi(v),
                func,
            }),
            Self::Bluetooth => Box::new(ToggleButton {
                icons: ["bluetooth-disabled-symbolic", "bluetooth-symbolic"],
                getter: |s| s.bluetooth.load(Ordering::Relaxed),
                setter: |s, v| s.bluetooth.store(v, Ordering::Relaxed),
                request_builder: |v| Request::SetBluetooth(v),
                func,
            }),
            Self::Dnd => Box::new(ToggleButton {
                icons: [
                    "weather-clear-night-symbolic",
                    "weather-clear-night-symbolic",
                ],
                getter: |s| s.dnd.load(Ordering::Relaxed),
                setter: |s, v| s.dnd.store(v, Ordering::Relaxed),
                request_builder: |_v| Request::Ping,
                func,
            }),
            Self::Powermode => Box::new(CycleButton {
                icons: &[
                    "watson-leaf-symbolic",
                    "watson-scale-symbolic",
                    "watson-bunny-symbolic",
                ],
                max_states: 3,
                field: |s| &s.powermode,
                request_builder: |v| Request::SetPowerMode(v),
                func,
            }),
            Self::Brightness => Box::new(RangeBehavior {
                icons: &[
                    "display-brightness-off-symbolic",
                    "display-brightness-low-symbolic",
                    "display-brightness-medium-symbolic",
                    "display-brightness-high-symbolic",
                ],
                field: |s| &s.brightness,
                request_builder: |v| Request::SetBacklight(v),
                func,
            }),
            Self::Volume => Box::new(RangeBehavior {
                icons: &[
                    "audio-volume-muted-symbolic",
                    "audio-volume-low-symbolic",
                    "audio-volume-medium-symbolic",
                    "audio-volume-high-symbolic",
                ],
                field: |s| &s.volume,
                request_builder: |v| Request::SetVolume(v),
                func,
            }),
            Self::Custom { id, states, .. } => {
                let l_id: &'static str = Box::leak(id.into_boxed_str());
                let l_states: Vec<(&'static str, &'static str)> = states
                    .into_iter()
                    .map(|s| {
                        let i = Box::leak(s.icon.into_boxed_str()) as &'static str;
                        let c = Box::leak(s.command.into_boxed_str()) as &'static str;
                        (i, c)
                    })
                    .collect();

                let l_slice: &'static [(&'static str, &'static str)] =
                    Box::leak(l_states.into_boxed_slice());

                Box::new(DynamicCycleButton {
                    id: l_id,
                    states: l_slice,
                    max_states: l_slice.len() as u8,
                    func,
                })
            }

            Self::None => Box::new(ToggleButton {
                icons: ["", ""],
                getter: |_| false,
                setter: |_, _| {},
                request_builder: |_| Request::Ping,
                func,
            }),
        }
    }
}
