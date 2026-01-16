use std::{collections::HashMap, time::Instant};

use common::{
    utils::errors::{WatsonError, WatsonErrorKind},
    watson_err,
};
use zbus::{
    Proxy,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::hardware::HardwareController;

impl HardwareController {
    // ----- Wifi -----
    pub async fn set_wifi(&self, enabled: bool) -> Result<(), WatsonError> {
        let proxy = Proxy::new(
            &self.conn,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

        proxy
            .set_property("WirelessEnabled", enabled)
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::DBusPropertySet, e.to_string()))
    }
    pub async fn get_wifi(&self) -> Result<bool, WatsonError> {
        let proxy = Proxy::new(
            &self.conn,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

        proxy
            .get_property("WirelessEnabled")
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::DBusPropertyGet, e.to_string()))
    }
    pub async fn get_wifi_list(&self) -> Result<HashMap<String, u8>, WatsonError> {
        let proxy = Proxy::new(
            &self.conn,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

        let devices: Vec<zbus::zvariant::OwnedObjectPath> = proxy
            .call("GetDevices", &())
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::DBusProxyCall, e.to_string()))?;

        let mut all_aps: HashMap<String, u8> = HashMap::new();
        for device_path in devices {
            let device_proxy = Proxy::new(
                &self.conn,
                "org.freedesktop.NetworkManager",
                &device_path,
                "org.freedesktop.NetworkManager.Device.Wireless",
            )
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

            // Try to get Access Points (if fails, device is not wireless)
            if let Ok(ap_paths) = device_proxy
                .call::<&str, (), Vec<zbus::zvariant::OwnedObjectPath>>("GetAllAccessPoints", &())
                .await
            {
                for ap_path in ap_paths {
                    let ap_proxy = Proxy::new(
                        &self.conn,
                        "org.freedesktop.NetworkManager",
                        &ap_path,
                        "org.freedesktop.NetworkManager.AccessPoint",
                    )
                    .await
                    .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

                    // Extract SSID and Strength
                    // SSID is returned as Vec<u8> because it's not guaranteed to be UTF-8
                    let ssid_raw: Vec<u8> = ap_proxy.get_property("Ssid").await.unwrap_or_default();
                    let ssid = String::from_utf8_lossy(&ssid_raw).into_owned();
                    let strength: u8 = ap_proxy.get_property("Strength").await.unwrap_or(0);

                    if !ssid.is_empty() {
                        all_aps
                            .entry(ssid)
                            .and_modify(|s| {
                                if strength > *s {
                                    *s = strength
                                }
                            })
                            .or_insert(strength);
                    }
                }
            }
        }
        Ok(all_aps)
    }
    // ----- Bluetooth -----
    /// Requires bluetooth service running
    pub async fn set_bluetooth(&self, enabled: bool) -> Result<(), WatsonError> {
        if let Some(path) = self.get_bluetooth_path().await? {
            let adapter = Proxy::new(&self.conn, "org.bluez", path, "org.bluez.Adapter1")
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

            adapter
                .set_property("Powered", enabled)
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::DBusPropertySet, e.to_string()))?;
        }
        Ok(())
    }
    pub async fn get_bluetooth(&self) -> Result<bool, WatsonError> {
        if let Some(path) = self.get_bluetooth_path().await? {
            let adapter = Proxy::new(&self.conn, "org.bluez", path, "org.bluez.Adapter1")
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

            adapter
                .get_property("Powered")
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::DBusPropertyGet, e.to_string()))
        } else {
            Err(watson_err!(
                WatsonErrorKind::BluetoothServiceDisabled,
                "Bluetooth service is not enabled. Cannot find bluetooth adapter."
            ))
        }
    }
    pub async fn get_bluetooth_path(&self) -> Result<Option<OwnedObjectPath>, WatsonError> {
        let proxy = Proxy::new(
            &self.conn,
            "org.bluez",
            "/",
            "org.freedesktop.DBus.ObjectManager",
        )
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

        let objects: HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>> = proxy
            .call("GetManagedObjects", &())
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::DBusProxyCall, e.to_string()))?;

        let path = objects
            .into_iter()
            .find(|(_, ifaces)| ifaces.contains_key("org.bluez.Adapter1"))
            .map(|(p, _)| p);
        Ok(path)
    }
}
