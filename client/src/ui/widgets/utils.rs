use common::protocol::Request;
use gtk4::{
    cairo::Context,
    gdk::{FrameClock, RGBA},
    glib::{
        WeakRef,
        object::{ObjectExt, ObjectType},
    },
};
use serde::{Deserialize, Serialize};
use std::{cell::Cell, str::FromStr, sync::atomic::Ordering};
use strum::Display;

use crate::ui::widgets::interactives::{CycleButton, RangeBehavior, ToggleButton, WidgetBehavior};

pub struct CairoShapesExt;
impl CairoShapesExt {
    pub fn rounded_rectangle(
        ctx: &Context,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: (f64, f64, f64, f64),
    ) {
        let (rad_tl, rad_tr, rad_br, rad_bl) = radius;
        let tl = rad_tl.min(width / 2.0).min(height / 2.0);
        let tr = rad_tr.min(width / 2.0).min(height / 2.0);
        let br = rad_br.min(width / 2.0).min(height / 2.0);
        let bl = rad_bl.min(width / 2.0).min(height / 2.0);

        ctx.new_sub_path();

        // Start at top-left corner
        ctx.move_to(x + tl, y);

        // Top edge + top-right corner
        ctx.line_to(x + width - tr, y);
        ctx.arc(
            x + width - tr,
            y + tr,
            tr,
            -90_f64.to_radians(),
            0_f64.to_radians(),
        );

        // Right edge + bottom-right corner
        ctx.line_to(x + width, y + height - br);
        ctx.arc(
            x + width - br,
            y + height - br,
            br,
            0_f64.to_radians(),
            90_f64.to_radians(),
        );

        // Bottom edge + bottom-left corner
        ctx.line_to(x + bl, y + height);
        ctx.arc(
            x + bl,
            y + height - bl,
            bl,
            90_f64.to_radians(),
            180_f64.to_radians(),
        );

        // Left edge + top-left corner
        ctx.line_to(x, y + tl);
        ctx.arc(
            x + tl,
            y + tl,
            tl,
            180_f64.to_radians(),
            270_f64.to_radians(),
        );

        ctx.close_path();
    }

    pub fn circle(ctx: &Context, x: f64, y: f64, radius: f64) {
        ctx.new_path();
        ctx.arc(x, y, radius, 0.0, 2.0 * std::f64::consts::PI);
        ctx.close_path();
        ctx.fill().unwrap();
    }
    pub fn circle_path(ctx: &Context, x: f64, y: f64, radius: f64, frac: f64) {
        let end_angle = -std::f64::consts::PI / 2.0;
        let start_angle = end_angle - frac * 2.0 * std::f64::consts::PI;

        ctx.new_path();
        ctx.arc(x, y, radius, start_angle, end_angle);
    }
    pub fn centered_text(ctx: &Context, text: &str, cx: f64, cy: f64) {
        let ext = ctx.text_extents(text).unwrap();
        let font_ext = ctx.font_extents().unwrap();

        // Horizontal: center ink box
        let x = cx - (ext.width() / 2.0 + ext.x_bearing());

        // Vertical: center using baseline + ascent/descent
        let y = cy + (font_ext.ascent() - font_ext.descent()) / 2.0;

        ctx.move_to(x, y);
        ctx.show_text(text).unwrap();
    }
    pub fn vert_centered_text(ctx: &Context, text: &str, cx: f64, cy: f64) {
        let font_ext = ctx.font_extents().unwrap();
        let y = cy + (font_ext.ascent() - font_ext.descent()) / 2.0;
        ctx.move_to(cx, y);
        ctx.show_text(text).unwrap();
    }
    pub fn rjust_text(ctx: &Context, text: &str, cx: f64, cy: f64, center_vert: bool) {
        let ext = ctx.text_extents(text).unwrap();

        let y = if center_vert {
            let font_ext = ctx.font_extents().unwrap();
            cy + (font_ext.ascent() - font_ext.descent()) / 2.0
        } else {
            cy
        };

        let x = cx - (ext.width() + ext.x_bearing());
        ctx.move_to(x, y);
        ctx.show_text(text).unwrap();
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Rgba {
    pub r: f64, // 0.0 to 1.0
    pub g: f64,
    pub b: f64,
    pub a: f64,
}
impl Rgba {
    pub fn lerp(&self, other: &Rgba, t: f64) -> Self {
        let clamp = |x: f64| x.max(0.0).min(1.0);
        Rgba {
            r: clamp(self.r + (other.r - self.r) * t),
            g: clamp(self.g + (other.g - self.g) * t),
            b: clamp(self.b + (other.b - self.b) * t),
            a: clamp(self.a + (other.a - self.a) * t),
        }
    }

    pub fn perceived_brightness_gamma(&self) -> f64 {
        let r = self.r.powf(2.2);
        let g = self.g.powf(2.2);
        let b = self.b.powf(2.2);
        (0.299 * r + 0.587 * g + 0.114 * b).powf(1.0 / 2.2)
    }

    pub fn invert(&self) -> Self {
        Self {
            r: 1.0 - self.r,
            g: 1.0 - self.g,
            b: 1.0 - self.b,
            a: self.a,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Hsl {
    pub h: f64, // 0.0 to 360.0
    pub s: f64, // 0.0 to 1.0
    pub l: f64,
}

// Implement FromStr to allow: let color: Rgba = "#ff0000".parse().unwrap();
impl FromStr for Rgba {
    type Err = ();

    fn from_str(hex: &str) -> Result<Self, Self::Err> {
        let hex = hex.trim_start_matches('#');
        let len = hex.len();
        if len != 6 && len != 8 {
            return Err(());
        }

        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ())? as f64 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ())? as f64 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ())? as f64 / 255.0;
        let a = if len == 8 {
            u8::from_str_radix(&hex[6..8], 16).map_err(|_| ())? as f64 / 255.0
        } else {
            1.0
        };

        Ok(Self { r, g, b, a })
    }
}

// Trait for easy conversion between types
impl From<Rgba> for Hsl {
    fn from(rgba: Rgba) -> Self {
        let max = rgba.r.max(rgba.g).max(rgba.b);
        let min = rgba.r.min(rgba.g).min(rgba.b);
        let delta = max - min;
        let l = (max + min) / 2.0;

        let s = if delta == 0.0 {
            0.0
        } else {
            delta / (1.0 - (2.0 * l - 1.0).abs())
        };

        let mut h = if delta == 0.0 {
            0.0
        } else if max == rgba.r {
            60.0 * (((rgba.g - rgba.b) / delta) % 6.0)
        } else if max == rgba.g {
            60.0 * (((rgba.b - rgba.r) / delta) + 2.0)
        } else {
            60.0 * (((rgba.r - rgba.g) / delta) + 4.0)
        };

        if h < 0.0 {
            h += 360.0;
        }
        Self { h, s, l }
    }
}

impl From<Hsl> for Rgba {
    fn from(hsl: Hsl) -> Self {
        let q = if hsl.l < 0.5 {
            hsl.l * (1.0 + hsl.s)
        } else {
            hsl.l + hsl.s - hsl.l * hsl.s
        };
        let p = 2.0 * hsl.l - q;
        let h_norm = hsl.h / 360.0;

        let res = (
            hue_to_rgb(p, q, h_norm + 1.0 / 3.0),
            hue_to_rgb(p, q, h_norm),
            hue_to_rgb(p, q, h_norm - 1.0 / 3.0),
        );
        Self {
            r: res.0,
            g: res.1,
            b: res.2,
            a: 1.0,
        }
    }
}

impl From<RGBA> for Rgba {
    fn from(v: RGBA) -> Self {
        Self {
            r: v.red() as f64,
            g: v.green() as f64,
            b: v.blue() as f64,
            a: v.alpha() as f64,
        }
    }
}

fn hue_to_rgb(p: f64, q: f64, t: f64) -> f64 {
    let mut t = t;
    if t < 0.0 {
        t += 1.0
    } else if t > 1.0 {
        t -= 1.0
    };
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

// Animation Stuff

#[allow(dead_code)]
#[derive(Clone, Copy, Default)]
pub enum EaseFunction {
    #[default]
    None,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseOutCubic,
}
impl EaseFunction {
    pub fn apply(&self, time: f64) -> f64 {
        let t = time.clamp(0.0, 1.0);
        match self {
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
            Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Self::None => t,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Default)]
pub enum AnimationDirection {
    #[default]
    Uninitialized,
    Forward {
        duration: f64,
        function: EaseFunction,
    },
    Backward {
        duration: f64,
        function: EaseFunction,
    },
}
impl AnimationDirection {
    fn end(&self) -> f64 {
        match self {
            Self::Uninitialized | Self::Forward { .. } => 1.0,
            Self::Backward { .. } => 0.0,
        }
    }
}

#[derive(Default)]
pub struct AnimationState {
    pub progress: Cell<f64>,
    pub running: Cell<bool>,
    last_time: Cell<Option<i64>>,
    direction: Cell<AnimationDirection>,
    pub n_runs: Cell<u64>,
}

impl AnimationState {
    pub fn new() -> Self {
        Self {
            last_time: Cell::new(None),
            running: Cell::new(false),
            progress: Cell::new(0.0),
            direction: Cell::new(AnimationDirection::Uninitialized),
            n_runs: Cell::new(0),
        }
    }
    pub fn start(&self, direction: AnimationDirection) {
        self.n_runs.set(self.n_runs.get() + 1);
        self.running.set(true);
        self.last_time.set(None);
        self.direction.set(direction);
        self.progress.set(1.0 - direction.end());
    }

    pub fn update(&self, frame_clock: &FrameClock) {
        if !self.running.get() {
            return;
        }

        let now = frame_clock.frame_time(); // microseconds

        let elapsed = if let Some(start_ns) = self.last_time.get() {
            (now - start_ns) as f64 / 1_000_000.0 // seconds
        } else {
            self.last_time.set(Some(now));
            return;
        };

        let eased_progress = match self.direction.get() {
            AnimationDirection::Forward { duration, function } => {
                let linear = (elapsed / duration).min(1.0);
                function.apply(linear)
            }
            AnimationDirection::Backward { duration, function } => {
                let linear = (1.0 - elapsed / duration).max(0.0);
                function.apply(linear)
            }
            AnimationDirection::Uninitialized => {
                self.reset();
                return;
            }
        };

        self.progress.set(eased_progress);

        // Stop animation if done
        if eased_progress >= 1.0 || eased_progress <= 0.0 {
            self.progress.set(self.direction.get().end());
            self.running.set(false);
        }
    }
    fn reset(&self) {
        self.progress.set(self.direction.get().end());
        self.running.set(false);
        self.last_time.set(None);
        self.direction.set(AnimationDirection::Uninitialized);
    }
}
pub enum WidgetOption<T: ObjectType> {
    Borrowed(WeakRef<T>),
    Owned(T),
}
impl<T: ObjectType> Default for WidgetOption<T> {
    fn default() -> Self {
        Self::Borrowed(WeakRef::new())
    }
}
impl<T: ObjectType> WidgetOption<T> {
    /// Take the owned value and replace it with a weak reference
    pub fn take(&mut self) -> Option<T> {
        match std::mem::replace(self, WidgetOption::Borrowed(WeakRef::default())) {
            WidgetOption::Owned(obj) => {
                *self = WidgetOption::Borrowed(obj.downgrade());
                Some(obj)
            }
            WidgetOption::Borrowed(weak) => weak.upgrade(),
        }
    }
    pub fn downgrade(&self) -> WeakRef<T> {
        match self {
            Self::Owned(obj) => obj.downgrade(),
            Self::Borrowed(b) => b.clone(),
        }
    }
}

// ----- Backend Functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, Hash, Display)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum BackendFunc {
    #[default]
    None,

    // Buttons
    Wifi,
    Bluetooth,
    Dnd,
    Powermode,

    // Sliders
    Volume,
    Brightness,
}
impl BackendFunc {
    pub fn build(self) -> Box<dyn WidgetBehavior> {
        match self {
            Self::Wifi => Box::new(ToggleButton {
                icon: "network-wireless-signal-excellent-symbolic",
                getter: |s| s.wifi.load(Ordering::Relaxed),
                setter: |s, v| s.wifi.store(v, Ordering::Relaxed),
                request_builder: |v| Request::SetWifi(v),
                func: self,
            }),
            Self::Bluetooth => Box::new(ToggleButton {
                icon: "bluetooth-symbolic",
                getter: |s| s.bluetooth.load(Ordering::Relaxed),
                setter: |s, v| s.bluetooth.store(v, Ordering::Relaxed),
                request_builder: |v| Request::SetBluetooth(v),
                func: self,
            }),
            Self::Dnd => Box::new(ToggleButton {
                icon: "weather-clear-night-symbolic",
                getter: |s| s.dnd.load(Ordering::Relaxed),
                setter: |s, v| s.dnd.store(v, Ordering::Relaxed),
                request_builder: |_v| Request::Ping,
                func: self,
            }),
            Self::Powermode => Box::new(CycleButton {
                icons: &[
                    "power-profile-power-saver-symbolic",
                    "power-profile-balanced-symbolic",
                    "power-profile-performance-symbolic",
                ],
                max_states: 3,
                field: |s| &s.powermode,
                request_builder: |v| Request::SetPowerMode(v),
                func: self,
            }),
            Self::Brightness => Box::new(RangeBehavior {
                icons: &[
                    "display-brightness-high-symbolic",
                    "display-brightness-medium-symbolic",
                    "display-brightness-low-symbolic",
                    "display-brightness-off-symbolic",
                ],
                field: |s| &s.brightness,
                request_builder: |v| Request::SetBacklight(v),
                func: self,
            }),
            Self::Volume => Box::new(RangeBehavior {
                icons: &[
                    "audio-volume-high-symbolic",
                    "audio-volume-medium-symbolic",
                    "audio-volume-low-symbolic",
                    "audio-volume-muted-symbolic",
                ],
                field: |s| &s.volume,
                request_builder: |v| Request::SetBacklight(v),
                func: self,
            }),

            Self::None => Box::new(ToggleButton {
                icon: "",
                getter: |_| false,
                setter: |_, _| {},
                request_builder: |_| Request::Ping,
                func: self,
            }),
        }
    }
}
