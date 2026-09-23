//! [`Fade`] — a surface dissolving into or out of the picture.

use super::{Animate, AnimationFrame};

/// How long a dissolve takes when nobody says otherwise.
const DEFAULT_DURATION: f32 = 0.2;

/// **A dissolve**: the surface arrives by becoming visible and leaves by becoming invisible.
///
/// ```ignore
/// Overlay::new().panel(body).animation(Fade::new())              // both ways, 0.2s
/// Overlay::new().panel(body).animation(Fade::new().seconds(0.35))
/// Overlay::new().panel(body).animation(Fade::out())              // a cut in, a dissolve out
/// ```
///
/// [`out`](Fade::out) is for a surface that arrives by doing something else — a [`Zoom`](super::Zoom)
/// growing in, a slide — where dissolving at the same time hides the movement before it has played.
/// Riding one on the other is [`Sequence`](super::Sequence)'s job.
///
/// **Linear on purpose.** An eased dissolve spends its first frames barely changing, which on a
/// dismissal reads as the surface hesitating before it goes; a dismissal wants to leave immediately
/// and finish gently.
#[derive(Debug, Clone, Copy)]
pub struct Fade {
    /// Seconds left of the dissolve, or `None` at rest.
    left: Option<f32>,
    /// Which way the last gesture went — an arrival or an exit.
    leaving: bool,
    duration: f32,
    /// Whether arriving dissolves too, or is a cut. See [`Fade::out`].
    fades_in: bool,
}

impl Fade {
    /// A dissolve both ways, over [`DEFAULT_DURATION`].
    pub fn new() -> Self {
        Self {
            left: None,
            leaving: false,
            duration: DEFAULT_DURATION,
            fades_in: true,
        }
    }

    /// A dissolve on the way **out only**: arriving is a cut, so the surface is fully present the
    /// moment it appears and something else can carry the arrival.
    pub fn out() -> Self {
        Self {
            fades_in: false,
            ..Self::new()
        }
    }

    /// How long the dissolve takes. Zero means a cut, so "no animation" needs no special case at
    /// any call site.
    pub fn seconds(mut self, seconds: f32) -> Self {
        self.duration = seconds.max(0.0);
        self
    }

    /// Current opacity: `1.0` at rest, `0.0` once an exit has played out.
    pub fn amount(&self) -> f32 {
        let Some(left) = self.left else {
            // **A finished exit rests invisible.** The surface has gone; resting at full opacity
            // would flash it back for the frame between the animation ending and its host taking
            // it away. `enter` and `cancel` both clear `leaving`, so this is only ever the
            // completed-exit case.
            return match self.leaving {
                true => 0.0,
                false => 1.0,
            };
        };
        if self.duration <= 0.0 {
            return 1.0;
        }
        // 0 → 1 through the dissolve, whichever way it is going.
        let progress = 1.0 - (left / self.duration).clamp(0.0, 1.0);
        match self.leaving {
            true => 1.0 - progress,
            false => progress,
        }
    }

    /// Is a dissolve in progress, either way?
    pub fn is_running(&self) -> bool {
        self.left.is_some()
    }
}

impl Default for Fade {
    fn default() -> Self {
        Self::new()
    }
}

impl Animate for Fade {
    fn enter(&mut self) {
        self.leaving = false;
        self.left = (self.fades_in && self.duration > 0.0).then_some(self.duration);
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
        AnimationFrame::opacity(self.amount())
    }

    fn duration(&self) -> f32 {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fade_falls_from_one_to_zero_and_then_reports_done() {
        let mut f = Fade::new().seconds(0.1);
        assert_eq!(f.amount(), 1.0, "at rest it is fully present");

        f.leave();
        f.tick(0.05);
        assert!((f.amount() - 0.5).abs() < 0.01, "halfway: {}", f.amount());
        assert!(
            !f.tick(0.05),
            "the frame it finishes is the frame the surface can go"
        );
        assert_eq!(f.amount(), 0.0, "and a finished exit rests invisible");
        assert!(!f.is_leaving(), "…so nothing holds the surface any more");
    }

    /// Both ways by default — a plugin writing `Fade::new()` gets the whole gesture.
    #[test]
    fn arriving_dissolves_in_unless_it_was_asked_not_to() {
        let mut f = Fade::new().seconds(0.1);
        f.enter();
        assert_eq!(f.amount(), 0.0, "an arrival starts invisible");
        while f.tick(0.05) {}
        assert_eq!(f.amount(), 1.0, "…and ends fully present");

        let mut cut = Fade::out().seconds(0.1);
        cut.enter();
        assert!(!cut.is_running(), "an exit-only fade arrives as a cut");
        assert_eq!(cut.amount(), 1.0, "…fully present from the first frame");
        cut.leave();
        assert!(cut.is_leaving(), "and still dissolves on the way out");
    }

    /// Re-shown mid-dissolve it is fully there again, not stuck half-transparent.
    #[test]
    fn cancelling_a_fade_restores_the_surface() {
        let mut f = Fade::new().seconds(0.1);
        f.leave();
        f.tick(0.05);
        f.cancel();
        assert_eq!(f.amount(), 1.0);
        assert!(!f.is_running());
    }

    /// A zero duration is "no animation" without the caller needing a special case.
    #[test]
    fn a_zero_duration_fade_never_runs() {
        let mut f = Fade::new().seconds(0.0);
        f.leave();
        assert!(!f.is_running());
        assert!(!f.is_leaving(), "so nothing waits for it");
    }
}
