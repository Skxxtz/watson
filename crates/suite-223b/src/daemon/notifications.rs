use crate::notification::HintValue;

impl From<zbus::zvariant::OwnedValue> for HintValue {
    fn from(value: zbus::zvariant::OwnedValue) -> Self {
        // Try to downcast to common types used in Notification Hints
        if let Ok(s) = String::try_from(value.clone()) {
            return HintValue::String(s);
        }
        if let Ok(i) = i32::try_from(value.clone()) {
            return HintValue::Int(i);
        }
        if let Ok(u) = u32::try_from(value.clone()) {
            return HintValue::Uint(u);
        }
        if let Ok(b) = bool::try_from(value.clone()) {
            return HintValue::Bool(b);
        }

        HintValue::None
    }
}
