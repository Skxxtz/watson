use std::sync::atomic::Ordering;

use common::protocol::{AtomicSystemState, Request};

use crate::{DAEMON_TX, ui::widgets::BackendFunc};

pub trait WidgetBehavior {
    fn clone_box(&self) -> Box<dyn WidgetBehavior>;
    fn icon_name(&self, val: u8) -> &'static str;
    fn set_percentage(&self, state: &AtomicSystemState, value: u8);
    fn as_request(&self, state: &AtomicSystemState) -> Option<(u8, Request)>;
    fn get_percentage(&self, state: &AtomicSystemState) -> u8;
    fn func(&self) -> BackendFunc;
    fn execute(&self, state: &AtomicSystemState) -> Option<u8> {
        let (val, request) = self.as_request(state)?;
        DAEMON_TX.get().map(|d| d.send(request));
        Some(val)
    }
}

#[derive(Clone)]
pub struct ToggleButton {
    pub icons: [&'static str; 2],
    // A closure that defines how to get/set the value
    pub getter: fn(&AtomicSystemState) -> bool,
    pub setter: fn(&AtomicSystemState, bool),
    pub request_builder: fn(bool) -> Request,
    pub func: BackendFunc,
}

impl WidgetBehavior for ToggleButton {
    fn clone_box(&self) -> Box<dyn WidgetBehavior> {
        Box::new(self.clone())
    }
    fn get_percentage(&self, state: &AtomicSystemState) -> u8 {
        (self.getter)(state) as u8
    }
    fn set_percentage(&self, state: &AtomicSystemState, value: u8) {
        (self.setter)(state, value != 0);
    }
    fn icon_name(&self, val: u8) -> &'static str {
        let idx = val.clamp(0, 1);
        self.icons[idx as usize]
    }
    fn as_request(&self, state: &AtomicSystemState) -> Option<(u8, Request)> {
        let new_state = !(self.getter)(state);
        (self.setter)(state, new_state);
        Some((new_state as u8, (self.request_builder)(new_state)))
    }
    fn func(&self) -> BackendFunc {
        self.func
    }
}

#[derive(Clone)]
pub struct CycleButton {
    pub icons: &'static [&'static str], // List of icons for each state
    pub max_states: u8,
    pub field: fn(&AtomicSystemState) -> &std::sync::atomic::AtomicU8,
    pub request_builder: fn(u8) -> Request,
    pub func: BackendFunc,
}
impl WidgetBehavior for CycleButton {
    fn clone_box(&self) -> Box<dyn WidgetBehavior> {
        Box::new(self.clone())
    }
    fn as_request(&self, state: &AtomicSystemState) -> Option<(u8, Request)> {
        let atomic = (self.field)(state);
        let mut old = atomic.load(Ordering::Relaxed);
        let mut target;
        loop {
            target = (old + 1) % self.max_states;
            match atomic.compare_exchange_weak(old, target, Ordering::SeqCst, Ordering::Relaxed) {
                Ok(_) => break,
                Err(actual) => old = actual,
            }
        }
        Some((target, (self.request_builder)(target)))
    }

    fn icon_name(&self, val: u8) -> &'static str {
        self.icons.get(val as usize).unwrap_or(&"image-missing")
    }

    fn set_percentage(&self, state: &AtomicSystemState, value: u8) {
        (self.field)(state).store(value, Ordering::Relaxed);
    }

    fn get_percentage(&self, state: &AtomicSystemState) -> u8 {
        (self.field)(state).load(Ordering::Relaxed)
    }

    fn func(&self) -> BackendFunc {
        self.func
    }
}

#[derive(Clone)]
pub struct RangeBehavior {
    pub icons: &'static [&'static str], // List of icons for each state
    pub field: fn(&AtomicSystemState) -> &std::sync::atomic::AtomicU8,
    pub request_builder: fn(u8) -> Request,
    pub func: BackendFunc,
}
impl WidgetBehavior for RangeBehavior {
    fn clone_box(&self) -> Box<dyn WidgetBehavior> {
        Box::new(self.clone())
    }

    fn as_request(&self, state: &AtomicSystemState) -> Option<(u8, Request)> {
        let target = self.get_percentage(state);
        Some((target, (self.request_builder)(target)))
    }

    fn icon_name(&self, val: u8) -> &'static str {
        let max_idx = self.icons.len().saturating_sub(1);
        let index = (val as usize * max_idx) / 100;
        self.icons.get(index).unwrap_or(&"image-missing")
    }

    fn set_percentage(&self, state: &AtomicSystemState, value: u8) {
        (self.field)(state).store(value, Ordering::Relaxed);
    }

    fn get_percentage(&self, state: &AtomicSystemState) -> u8 {
        (self.field)(state).load(Ordering::Relaxed)
    }

    fn func(&self) -> BackendFunc {
        self.func
    }
}

impl Clone for Box<dyn WidgetBehavior> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
