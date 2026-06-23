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

/// A "needs attention" pulse: [`trigger`](Attention::trigger) runs a fixed number
/// of sawtooth flashes (snap to `1.0`, fade to `0.0`, repeat), then stops. Embed
/// it in a widget driven by a host-owned attention signal, `tick()` each frame,
/// and paint the [`amount`](Attention::amount) as a colored glow. The matching
/// **sound** is the host's job (grid-ui is audio-free) — the app plays its own
/// beep when it sets the attention signal that triggers this.
#[derive(Debug, Clone, Copy)]
pub struct Attention {
    amount: f32,
    /// Per-pulse fade duration (seconds).
    duration: f32,
    /// Pulses still to play (including the current one).
    pulses_left: u32,
}

impl Attention {
    /// An attention effect with the default per-pulse duration (~220ms).
    pub fn new() -> Self {
        Self {
            amount: 0.0,
            duration: 0.22,
            pulses_left: 0,
        }
    }

    /// Start `pulses` flashes (clamped to at least 1).
    pub fn trigger(&mut self, pulses: u32) {
        self.pulses_left = pulses.max(1);
        self.amount = 1.0;
    }

    /// Advance by `dt` seconds. Returns `true` while still pulsing. Each time a
    /// pulse fades to zero it consumes one count and restarts until none remain.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.is_active() {
            return false;
        }
        self.amount = (self.amount - dt / self.duration).max(0.0);
        if self.amount <= 0.0 {
            self.pulses_left = self.pulses_left.saturating_sub(1);
            if self.pulses_left > 0 {
                self.amount = 1.0;
            }
        }
        self.is_active()
    }

    /// Current pulse strength in `0.0..=1.0`.
    pub fn amount(&self) -> f32 {
        self.amount
    }

    /// Whether a pulse sequence is in progress.
    pub fn is_active(&self) -> bool {
        self.pulses_left > 0 || self.amount > 0.0
    }
}

impl Default for Attention {
    fn default() -> Self {
        Self::new()
    }
}
