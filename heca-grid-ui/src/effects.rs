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
    /// How long to stay fully present before the dissolve begins. See [`Fade::delay`].
    delay: f32,
    /// What is left of that wait, while one is running.
    waiting: f32,
}

impl Fade {
    /// A dissolve of `duration` seconds, not running.
    pub fn new(duration: f32) -> Self {
        Self { left: None, duration: duration.max(0.0), delay: 0.0, waiting: 0.0 }
    }

    /// Stay fully present for `seconds` after [`start`](Fade::start), then dissolve.
    ///
    /// For a surface that leaves by doing something else first — a [`Zoom`] shrinking away, a panel
    /// sliding off — where fading at the same time hides the movement before it has played. The
    /// exposé is the case: it shrinks and then goes, rather than dimming out of a shrink you never
    /// see finish.
    pub fn delay(mut self, seconds: f32) -> Self {
        self.delay = seconds.max(0.0);
        self
    }

    /// Begin dissolving. A `duration` of zero finishes immediately, so "no fade" needs no special
    /// case at the call site.
    pub fn start(&mut self) {
        self.left = (self.duration > 0.0).then_some(self.duration);
        self.waiting = if self.left.is_some() { self.delay } else { 0.0 };
    }

    /// Stop dissolving and be fully present again — what a surface re-shown mid-fade needs, rather
    /// than opening half-transparent and finishing a disappearance nobody still wants.
    pub fn cancel(&mut self) {
        self.left = None;
        self.waiting = 0.0;
    }

    /// Is a dissolve in progress?
    pub fn is_running(&self) -> bool {
        self.left.is_some()
    }

    /// Current opacity: `1.0` unless dissolving, then falling to `0.0`.
    pub fn amount(&self) -> f32 {
        if self.waiting > 0.0 {
            return 1.0; // still fully present: the wait has not run out.
        }
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
        // Burn the wait first, and spend only what is left of this frame on the dissolve — so a
        // delay never costs a frame of the fade itself.
        let dt = if self.waiting > 0.0 {
            let spent = dt.min(self.waiting);
            self.waiting -= spent;
            if self.waiting > 0.0 {
                return true;
            }
            dt - spent
        } else {
            dt
        };
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
    /// **A finished leave rests small.** The scale is where the surface *is*, and one that has
    /// zoomed away has not come back — resting at life size snapped the cards to full for the last
    /// frames of an exit whose fade was still running, a flash right before they vanished.
    #[test]
    fn a_finished_leave_rests_at_from_not_back_at_life_size() {
        let mut z = super::Zoom::new(0.2, 0.8);
        z.enter();
        while z.tick(0.05) {}
        assert_eq!(z.amount(), 1.0, "arrived: life size");

        z.leave();
        while z.tick(0.05) {}
        assert_eq!(z.amount(), 0.8, "left: still small, not snapped back");

        z.cancel();
        assert_eq!(z.amount(), 1.0, "cancelled is a return to life size");
    }

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

/// **A zoom: a surface arriving by growing to life size, and leaving by shrinking away.**
///
/// The counterpart of [`Fade`], and the same shape — a value over time that a widget or a host
/// embeds, ticks, and reads. It owns no drawing: pair it with
/// [`PaintCx::with_scale`](crate::component::PaintCx::with_scale), which is what turns the number
/// into a picture.
///
/// **Why beside `Fade` rather than inside the surface that wanted it first.** An overview opens by
/// zooming out from life size (niri's overview does exactly this), a dropdown can grow from its
/// trigger, a toast can pop in — these are one behaviour with one implementation, and the moment it
/// lives inside one of them the next one hand-rolls a countdown of its own. That has already
/// happened once here: the easing now in [`Eased`] was inlined in a scroll region and hand-rolled a
/// third time as a countdown in the layer registry before it was moved here.
///
/// ```ignore
/// // A surface that grows in from half size over 0.14s.
/// let mut zoom = Zoom::new(0.14, 0.5);
/// zoom.enter();
/// // …each frame:
/// zoom.tick(dt);
/// cx.with_scale(zoom.amount(), origin, |cx| self.paint_body(cx));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Zoom {
    /// Seconds remaining, and which way it is going.
    left: Option<f32>,
    leaving: bool,
    duration: f32,
    /// The scale at the far end of the journey — niri's `0.5`, "half life size".
    from: f32,
}

impl Zoom {
    /// A zoom of `duration` seconds between `from` and life size, at rest and fully present.
    ///
    /// `from` is the *away* end: below 1.0 the surface grows in from smaller (an overview), above
    /// it, from larger (a dialog dropping toward you). A duration of zero means "cut", so a caller
    /// that does not want an animation needs no special case.
    pub fn new(duration: f32, from: f32) -> Self {
        Self {
            left: None,
            leaving: false,
            duration: duration.max(0.0),
            from: from.max(0.0),
        }
    }

    /// Begin arriving: from `from` toward life size.
    pub fn enter(&mut self) {
        self.leaving = false;
        self.left = (self.duration > 0.0).then_some(self.duration);
    }

    /// Begin leaving: from life size back toward `from`.
    pub fn leave(&mut self) {
        self.leaving = true;
        self.left = (self.duration > 0.0).then_some(self.duration);
    }

    /// Stop where it is and be fully present — what a surface re-shown mid-zoom needs, rather than
    /// finishing a departure nobody still wants.
    pub fn cancel(&mut self) {
        self.left = None;
        self.leaving = false;
    }

    /// Is a zoom in progress?
    /// Is this zoom **on its way out** — the shrink half of an exit, still playing?
    ///
    /// A surface's exit is one gesture made of several effects, and the surface is not gone until
    /// every one of them has finished. A host that retires it when only the fade ends takes the
    /// tree away mid-shrink; a host that asks this too can sequence them.
    pub fn is_leaving(&self) -> bool {
        self.leaving && self.is_running()
    }

    pub fn is_running(&self) -> bool {
        self.left.is_some()
    }

    /// The scale to draw at right now: `1.0` at rest, otherwise between `from` and `1.0`.
    ///
    /// Eased with the same smoothstep the rest of the library uses, so a zoom does not read as a
    /// linear slide while everything beside it accelerates.
    pub fn amount(&self) -> f32 {
        let Some(left) = self.left else {
            // **A finished LEAVE rests where it left off, not back at life size.** The scale is
            // where the surface *is*, and a surface that has zoomed away is small — it has not
            // returned. Resting at `1.0` snapped the cards back to full size for the last frames
            // of an exit whose fade was still running, which reads as a flash immediately before
            // they vanish (Antonio, driving, 2026-08-19). `enter` and `cancel` both clear
            // `leaving`, so this is only ever the completed-exit case.
            return match self.leaving {
                true => self.from,
                false => 1.0,
            };
        };
        if self.duration <= 0.0 {
            return 1.0;
        }
        // `progress` runs 0 → 1 through the animation, whichever way it is going.
        let progress = 1.0 - (left / self.duration).clamp(0.0, 1.0);
        let eased = progress * progress * (3.0 - 2.0 * progress);
        // Arriving walks `from → 1`; leaving walks `1 → from`.
        let t = match self.leaving {
            true => 1.0 - eased,
            false => eased,
        };
        self.from + (1.0 - self.from) * t
    }

    /// Advance by `dt` seconds. Returns `true` while still zooming; the frame it returns `false`
    /// after having been running is the frame a leaving surface can go.
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
mod zoom_tests {
    use super::*;

    /// **A delayed fade stays fully present until its wait runs out.** For a surface that leaves by
    /// doing something else first — a [`Zoom`] shrinking away — where fading at the same time dims
    /// the movement before it has played.
    #[test]
    fn a_delayed_fade_waits_before_it_starts_dissolving() {
        let mut f = Fade::new(0.2).delay(0.3);
        f.start();
        assert_eq!(f.amount(), 1.0, "the wait has not begun to run");

        assert!(f.tick(0.2), "still waiting");
        assert_eq!(f.amount(), 1.0, "…and still fully present");

        // This frame finishes the wait and spends its remainder on the fade, so a delay never
        // costs a frame of the dissolve itself.
        assert!(f.tick(0.2));
        let mid = f.amount();
        assert!(mid > 0.0 && mid < 1.0, "now dissolving, got {mid}");

        assert!(!f.tick(0.3), "and it finishes");
        assert_eq!(f.amount(), 1.0, "…at rest again");

        // No delay is the old behaviour exactly.
        let mut plain = Fade::new(0.2);
        plain.start();
        assert!(plain.tick(0.1));
        assert!((plain.amount() - 0.5).abs() < 0.01);
    }

    /// **It arrives at life size and leaves back to where it came from**, and a zero duration is a
    /// cut — so "no animation" needs no special case at any call site.
    #[test]
    fn a_zoom_walks_between_its_far_end_and_life_size() {
        let mut z = Zoom::new(1.0, 0.5);
        assert_eq!(z.amount(), 1.0, "at rest it is life size");
        assert!(!z.is_running());

        z.enter();
        assert!((z.amount() - 0.5).abs() < 0.01, "arriving starts at the far end");
        assert!(z.tick(0.5));
        let mid = z.amount();
        assert!(mid > 0.5 && mid < 1.0, "…and is somewhere between, got {mid}");
        assert!(!z.tick(0.6), "the frame it stops running");
        assert_eq!(z.amount(), 1.0, "…it is life size again");

        z.leave();
        assert!((z.amount() - 1.0).abs() < 0.01, "leaving starts at life size");
        assert!(z.tick(0.9));
        assert!(z.amount() < 0.6, "…and shrinks toward the far end, got {}", z.amount());

        // Re-shown mid-flight: present again, not finishing a departure nobody wants.
        z.cancel();
        assert_eq!(z.amount(), 1.0);
        assert!(!z.is_running());

        // A duration of zero is a cut.
        let mut cut = Zoom::new(0.0, 0.5);
        cut.enter();
        assert!(!cut.is_running());
        assert_eq!(cut.amount(), 1.0);
    }
}
