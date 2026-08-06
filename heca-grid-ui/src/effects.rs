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

/// A value that **glides toward a target** instead of jumping to it.
///
/// Embed it wherever a widget moves itself — a view following a cursor, a marker sliding between
/// tabs, a bar settling on a new reading — `set` the target, `tick` each frame, paint
/// [`value`](Eased::value). The same shape as [`Flash`] and [`Attention`]: a value, a `tick(dt)`
/// that reports whether it is still going, and nothing else to wire.
///
/// **Exponential, not a fixed-duration tween**, and that is the point of it existing rather than a
/// start/end animation: the motion is always toward wherever the target is *now*. Retarget it three
/// times mid-flight and it glides to the third place, instead of finishing two journeys nobody is
/// waiting for any more. Frame-rate independent — the step is `1 − e^(−dt/τ)`, so a slow frame
/// covers proportionally more ground rather than the animation running at the frame rate.
#[derive(Debug, Clone, Copy)]
pub struct Eased {
    current: f64,
    target: f64,
    /// Time constant in seconds: after `τ` it has covered ~63% of the remaining distance.
    tau: f32,
    /// Distance below which it lands exactly and stops. An exponential never truly arrives, so
    /// without this it would ask for frames forever on an ever-smaller remainder.
    epsilon: f64,
}

impl Eased {
    /// Settled at `0.0`, gliding with time constant `tau` seconds.
    pub fn new(tau: f32) -> Self {
        Self { current: 0.0, target: 0.0, tau: tau.max(1e-3), epsilon: 0.5 }
    }

    /// How close counts as arrived (default `0.5`, half a pixel). Set it in the units of whatever
    /// this eases — a value in `0..=1` wants something far smaller than a pixel count does.
    pub fn epsilon(mut self, epsilon: f64) -> Self {
        self.epsilon = epsilon.max(f64::EPSILON);
        self
    }

    /// Glide toward `target` from wherever it is now.
    pub fn set(&mut self, target: f64) {
        self.target = target;
    }

    /// Land on `value` **now**, cancelling any motion — for a move the user is driving directly,
    /// which has to track the input one to one rather than trail it.
    pub fn snap(&mut self, value: f64) {
        self.current = value;
        self.target = value;
    }

    /// The value to paint this frame.
    pub fn value(&self) -> f64 {
        self.current
    }

    /// Where it is heading.
    pub fn target(&self) -> f64 {
        self.target
    }

    /// Advance by `dt` seconds. Returns `true` while it is still moving.
    pub fn tick(&mut self, dt: f32) -> bool {
        if (self.target - self.current).abs() < self.epsilon {
            self.current = self.target;
            return false;
        }
        let k = 1.0 - (-(dt.max(0.0) as f64) / self.tau as f64).exp();
        self.current += (self.target - self.current) * k;
        true
    }
}

/// A **dissolve**: [`start`](Fade::start) sets it to `1.0` and it falls linearly to `0.0` over a
/// fixed duration, then reports itself finished.
///
/// The counterpart of [`Flash`], which snaps on and fades out as a *decoration*; this is meant to
/// carry a whole surface out — multiply it into an opacity and take the surface away when
/// [`tick`](Fade::tick) returns `false`.
///
/// Linear on purpose. An eased fade spends its first frames barely changing, which on a dismissal
/// reads as the surface hesitating before it goes; a dismissal wants to leave immediately and
/// finish gently.
#[derive(Debug, Clone, Copy)]
pub struct Fade {
    left: Option<f32>,
    duration: f32,
}

impl Fade {
    /// A dissolve of `duration` seconds, not running.
    pub fn new(duration: f32) -> Self {
        Self { left: None, duration: duration.max(0.0) }
    }

    /// Begin dissolving. A `duration` of zero finishes immediately, so "no fade" needs no special
    /// case at the call site.
    pub fn start(&mut self) {
        self.left = (self.duration > 0.0).then_some(self.duration);
    }

    /// Stop dissolving and be fully present again — what a surface re-shown mid-fade needs, rather
    /// than opening half-transparent and finishing a disappearance nobody still wants.
    pub fn cancel(&mut self) {
        self.left = None;
    }

    /// Is a dissolve in progress?
    pub fn is_running(&self) -> bool {
        self.left.is_some()
    }

    /// Current opacity: `1.0` unless dissolving, then falling to `0.0`.
    pub fn amount(&self) -> f32 {
        match self.left {
            Some(left) if self.duration > 0.0 => (left / self.duration).clamp(0.0, 1.0),
            Some(_) => 0.0,
            None => 1.0,
        }
    }

    /// Advance by `dt` seconds. Returns `true` while still dissolving; the frame it returns `false`
    /// after having been running is the frame the surface can go.
    pub fn tick(&mut self, dt: f32) -> bool {
        let Some(left) = self.left else { return false };
        let left = left - dt;
        match left > 0.0 {
            true => {
                self.left = Some(left);
                true
            }
            false => {
                self.left = None;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **It retargets rather than restarting.** A cursor moved three times mid-flight ends at the
    /// third place, having travelled once — the reason this is exponential and not a tween.
    #[test]
    fn an_eased_value_glides_to_wherever_the_target_moved_to() {
        let mut e = Eased::new(0.09);
        e.set(80.0);
        assert!(e.value() < 80.0, "not there on the frame it was asked");
        for _ in 0..3 {
            e.tick(1.0 / 60.0);
        }
        e.set(20.0); // changed its mind mid-flight
        let mut frames = 0;
        while e.tick(1.0 / 60.0) && frames < 240 {
            frames += 1;
        }
        assert!(frames < 240, "it finishes rather than easing forever");
        assert!((e.value() - 20.0).abs() < 0.5, "and it arrives at the LAST target: {}", e.value());
    }

    /// A move the user drives lands on the same frame — a view that trails a finger reads as broken.
    #[test]
    fn snapping_cancels_the_glide() {
        let mut e = Eased::new(0.09);
        e.set(100.0);
        e.snap(40.0);
        assert_eq!(e.value(), 40.0);
        assert!(!e.tick(1.0 / 60.0), "and it is not still going somewhere");
    }

    #[test]
    fn a_fade_falls_from_one_to_zero_and_then_reports_done() {
        let mut f = Fade::new(0.1);
        assert_eq!(f.amount(), 1.0, "not running means fully present");
        f.start();
        f.tick(0.05);
        assert!((f.amount() - 0.5).abs() < 0.01, "halfway: {}", f.amount());
        assert!(!f.tick(0.05), "the frame it finishes is the frame the surface can go");
        assert_eq!(f.amount(), 1.0, "and it is no longer dissolving");
    }

    /// Re-shown mid-fade it is fully there again, not stuck half-transparent.
    #[test]
    fn cancelling_a_fade_restores_the_surface() {
        let mut f = Fade::new(0.1);
        f.start();
        f.tick(0.05);
        f.cancel();
        assert_eq!(f.amount(), 1.0);
        assert!(!f.is_running());
    }

    /// A zero duration is "no fade" without the caller needing a special case.
    #[test]
    fn a_zero_duration_fade_never_runs() {
        let mut f = Fade::new(0.0);
        f.start();
        assert!(!f.is_running());
        assert_eq!(f.amount(), 1.0);
    }
}
