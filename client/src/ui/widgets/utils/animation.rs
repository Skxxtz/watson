use std::cell::Cell;

use gtk4::gdk::FrameClock;

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
