//! [`Zoom`] — a surface arriving by growing to life size, and leaving by shrinking away.

use super::{smoothstep, Animate, AnimationFrame};

/// How long a zoom takes when nobody says otherwise. niri's own overview animation is in this
/// range: long enough to read as one picture pulling back, short enough that it never feels like
/// waiting.
const DEFAULT_DURATION: f32 = 0.2;
/// The far end of the journey when nobody says otherwise — niri's overview, at half life size.
const DEFAULT_FROM: f32 = 0.5;

/// **A zoom**: the surface grows in from [`from`](Zoom::from) to life size, and shrinks back to it
/// on the way out.
///
/// ```ignore
/// Overlay::new().panel(body).animation(Zoom::new().from(0.8))               // grows in from small
/// Overlay::new().panel(body).animation(Zoom::new().from(1.3).seconds(0.14)) // drops in from large
/// ```
///
/// What a fade cannot express: a surface that appears by *zooming out from life size* says "this is
/// the same thing, further away", where a dissolve says "a different picture". An overview opens
/// exactly this way, a dropdown can grow from its trigger, a toast can pop in.
///
/// It owns no drawing. The number becomes a picture through
/// [`PaintCx::with_scale`](crate::component::PaintCx::with_scale), applied by whoever mounts the
/// surface — so nothing is measured again, no widget is told, and no bounds change.
#[derive(Debug, Clone, Copy)]
pub struct Zoom {
    /// Seconds left, or `None` at rest.
    left: Option<f32>,
    leaving: bool,
    duration: f32,
    /// The scale at the far end of the journey — niri's `0.5`, "half life size".
    from: f32,
}

impl Zoom {
    /// A zoom over [`DEFAULT_DURATION`] between [`DEFAULT_FROM`] and life size.
    pub fn new() -> Self {
        Self {
            left: None,
            leaving: false,
            duration: DEFAULT_DURATION,
            from: DEFAULT_FROM,
        }
    }

    /// The *away* end of the journey: below `1.0` the surface grows in from smaller (an overview),
    /// above it, from larger (a dialog dropping toward you).
    pub fn from(mut self, from: f32) -> Self {
        self.from = from.max(0.0);
        self
    }

    /// How long the zoom takes. Zero means a cut.
    pub fn seconds(mut self, seconds: f32) -> Self {
        self.duration = seconds.max(0.0);
        self
    }

    /// The scale to draw at right now: `1.0` at rest, otherwise between `from` and `1.0`.
    ///
    /// Eased with the library's own [`smoothstep`], so a zoom does not read as a linear slide
    /// while everything beside it accelerates.
    pub fn amount(&self) -> f32 {
        let Some(left) = self.left else {
            // **A finished exit rests at `from`, not back at life size.** The scale is where the
            // surface *is*, and one that has zoomed away is small — it has not come back. Resting
            // at `1.0` snapped the exposé's cards to full size for the last frames of an exit whose
            // dissolve was still running, which reads as a flash immediately before they vanish
            // (Antonio, driving, 2026-08-19). `enter` and `cancel` both clear `leaving`, so this is
            // only ever the completed-exit case.
            return match self.leaving {
                true => self.from,
                false => 1.0,
            };
        };
        if self.duration <= 0.0 {
            return 1.0;
        }
        // 0 → 1 through the animation, whichever way it is going.
        let eased = smoothstep(1.0 - (left / self.duration).clamp(0.0, 1.0));
        // Arriving walks `from → 1`; leaving walks `1 → from`.
        let t = match self.leaving {
            true => 1.0 - eased,
            false => eased,
        };
        self.from + (1.0 - self.from) * t
    }

    /// Is a zoom in progress, either way?
    pub fn is_running(&self) -> bool {
        self.left.is_some()
    }
}

impl Default for Zoom {
    fn default() -> Self {
        Self::new()
    }
}

impl Animate for Zoom {
    fn enter(&mut self) {
        self.leaving = false;
        self.left = (self.duration > 0.0).then_some(self.duration);
    }

    fn leave(&mut self) {
        self.leaving = true;
        self.left = (self.duration > 0.0).then_some(self.duration);
    }

    fn cancel(&mut self) {
        self.left = None;
        self.leaving = false;
    }

    fn tick(&mut self, dt: f32) -> bool {
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

    fn is_leaving(&self) -> bool {
        self.leaving && self.is_running()
    }

    fn frame(&self) -> AnimationFrame {
        AnimationFrame::scale(self.amount())
    }

    fn duration(&self) -> f32 {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **It arrives at life size and leaves back to where it came from**, and a zero duration is a
    /// cut — so "no animation" needs no special case at any call site.
    #[test]
    fn a_zoom_walks_between_its_far_end_and_life_size() {
        let mut z = Zoom::new().from(0.5).seconds(1.0);
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

        let mut cut = Zoom::new().seconds(0.0);
        cut.enter();
        assert!(!cut.is_running(), "a duration of zero is a cut");
        assert_eq!(cut.amount(), 1.0);
    }

    /// **A finished leave rests small.** The scale is where the surface *is*, and one that has
    /// zoomed away has not come back — resting at life size snapped the exposé's cards to full for
    /// the last frames of an exit whose dissolve was still running, a flash right before they
    /// vanished (F003/P082/T327).
    #[test]
    fn a_finished_leave_rests_at_from_not_back_at_life_size() {
        let mut z = Zoom::new().from(0.8).seconds(0.2);
        z.enter();
        while z.tick(0.05) {}
        assert_eq!(z.amount(), 1.0, "arrived: life size");

        z.leave();
        while z.tick(0.05) {}
        assert_eq!(z.amount(), 0.8, "left: still small, not snapped back");

        z.cancel();
        assert_eq!(z.amount(), 1.0, "cancelled is a return to life size");
    }
}
