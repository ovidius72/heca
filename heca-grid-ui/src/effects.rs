//! Small reusable animation effects that widgets embed.

/// A press flash: snaps to `1.0` on [`trigger`](Flash::trigger) and fades back
/// to `0.0`. Embed it in any clickable widget (Button, Checkbox, Toggle…),
/// `trigger()` on press, `tick()` each frame, and paint it with
/// [`PaintCx::flash`](crate::component::PaintCx::flash).
#[derive(Debug, Clone, Copy)]
pub struct Flash {
    amount: f32,
    duration: f32,
}

impl Flash {
    /// A flash with the default fade duration (~180ms).
    pub fn new() -> Self {
        Self {
            amount: 0.0,
            duration: 0.18,
        }
    }

    /// A flash with a custom fade duration (seconds).
    pub fn with_duration(duration: f32) -> Self {
        Self {
            amount: 0.0,
            duration,
        }
    }

    /// Trigger the flash (e.g. on press).
    pub fn trigger(&mut self) {
        self.amount = 1.0;
    }

    /// Advance the fade by `dt` seconds. Returns `true` while still animating.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.amount <= 0.0 {
            return false;
        }
        self.amount = (self.amount - dt / self.duration).max(0.0);
        true
    }

    /// Current flash strength in `0.0..=1.0`.
    pub fn amount(&self) -> f32 {
        self.amount
    }
}

impl Default for Flash {
    fn default() -> Self {
        Self::new()
    }
}
