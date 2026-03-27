use crate::protocol::PowerMode;
use zbus::zvariant::OwnedValue;

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
