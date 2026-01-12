use common::{
    errors::{WatsonError, WatsonErrorKind},
    protocol::PowerMode,
    watson_err,
};
use zbus::Proxy;

use crate::hardware::HardwareController;

impl HardwareController {
    // ----- Power Mode -----
    /// Requires `power-profiles-daemon` installed and running
    pub async fn set_powermode(&self, mode: PowerMode) -> Result<(), WatsonError> {
        let proxy = Proxy::new(
            &self.conn,
            "net.hadess.PowerProfiles",
            "/net/hadess/PowerProfiles",
            "net.hadess.PowerProfiles",
        )
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

        // Note: power-profiles-daemon expects the string representation
        proxy
            .set_property("ActiveProfile", mode.to_string())
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::DBusPropertySet, e.to_string()))
    }
    pub async fn get_powermode(&self) -> Result<PowerMode, WatsonError> {
        let proxy = Proxy::new(
            &self.conn,
            "net.hadess.PowerProfiles",
            "/net/hadess/PowerProfiles",
            "net.hadess.PowerProfiles",
        )
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::ProxyCreate, e.to_string()))?;

        proxy
            .get_property("ActiveProfile")
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::DBusPropertySet, e.to_string()))
    }
}
