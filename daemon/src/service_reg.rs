use common::protocol::DaemonService;
use std::fmt::Display;
use strum::IntoEnumIterator;

pub struct ServiceRegister {
    /// Format:
    /// ```text
    /// 00000000
    /// 0. BatteryStateListener
    /// ```
    registered_services: u8,
}
#[allow(dead_code)]
impl ServiceRegister {
    pub fn new() -> Self {
        Self {
            registered_services: 0,
        }
    }
    pub fn register(&mut self, service: DaemonService) {
        self.registered_services |= 1 << service as u8;
    }
    pub fn unregister(&mut self, service: DaemonService) {
        self.registered_services &= !(1 << service as u8);
    }
    pub fn is_active(&self, service: DaemonService) -> bool {
        let service_mask = 1 << service as u8;
        (self.registered_services & service_mask) != 0
    }
    pub fn has_any_listeners(&self) -> bool {
        self.registered_services != 0
    }
    pub fn registered_services(&mut self, services: u8) {
        self.registered_services |= services;
    }
    pub fn clear(&mut self) {
        self.registered_services = 0;
    }
}
impl Display for ServiceRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        let mut found_any = false;

        for service in DaemonService::iter() {
            if self.is_active(service) {
                if !found_any {
                    write!(f, "Active Services: ")?;
                    found_any = true;
                }

                if !first {
                    write!(f, ", ")?;
                }

                write!(f, "{}", service.as_ref())?;
                first = false;
            }
        }

        if !found_any {
            write!(f, "No active services")?;
        }

        Ok(())
    }
}
