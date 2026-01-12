use std::collections::HashMap;

use common::{
    errors::{WatsonError, WatsonErrorKind},
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
