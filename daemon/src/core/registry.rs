use common::protocol::DaemonService;
use std::{
    fmt::Display,
    sync::atomic::{AtomicU8, Ordering},
};
use strum::IntoEnumIterator;

pub struct ServiceRegistry {
    /// Format:
    /// ```text
    /// 00000000
    /// 0. BatteryStateListener
    /// ```
    registered_services: AtomicU8,
}
#[allow(dead_code)]
impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            registered_services: AtomicU8::new(0),
        }
    }

    pub fn register(&self, service: DaemonService) {
        self.registered_services
            .fetch_or(1 << service as u8, Ordering::Relaxed);
    }

    pub fn unregister(&self, service: DaemonService) {
        self.registered_services
            .fetch_and(!(1 << service as u8), Ordering::Relaxed);
    }

    pub fn is_active(&self, service: DaemonService) -> bool {
        let mask = 1 << service as u8;
        (self.registered_services.load(Ordering::Relaxed) & mask) != 0
    }

    pub fn has_any_listeners(&self) -> bool {
        self.registered_services.load(Ordering::Relaxed) != 0
    }

    pub fn set_registered_services(&self, services: u8) {
        self.registered_services
            .fetch_or(services, Ordering::Relaxed);
    }

    pub fn clear(&self) {
        self.registered_services.store(0, Ordering::Relaxed);
    }
}
impl Display for ServiceRegistry {
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
