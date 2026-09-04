//! [`Slide`] — a surface travelling in from an edge, and going back out the way it came.

use super::{Animate, AnimationFrame, smoothstep};

/// How long a slide takes when nobody says otherwise. Shorter than a zoom: a travel reads as
/// movement immediately, where a scale needs a moment to be legible as one.
const DEFAULT_DURATION: f32 = 0.18;
/// How far it travels when nobody says otherwise — about a notification card's width, which is the
/// surface this gesture was added for. A surface of a very different size sets its own.
const DEFAULT_DISTANCE: f64 = 320.0;

/// Which edge a [`Slide`] comes in from — and goes back out towards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum SlideFrom {
    /// In from the right, out to the right. The default: notifications live in a right-hand corner.
    #[default]
    Right,
    Left,
    Top,
    Bottom,
}

impl SlideFrom {
    /// The unit direction of the journey — what [`DEFAULT_DISTANCE`] is multiplied by.
    ///
    /// Positive `x` is rightwards and positive `y` is downwards, the same as everywhere else in the
    /// library, so a surface coming from the right starts at a **positive** offset and travels back
    /// to zero.
    fn unit(self) -> (f64, f64) {
        match self {
            Self::Right => (1.0, 0.0),
            Self::Left => (-1.0, 0.0),
            Self::Top => (0.0, -1.0),
            Self::Bottom => (0.0, 1.0),
        }
    }
}

/// **A slide**: the surface travels in from [`from`](Slide::from) to where it belongs, and back out
/// to that edge on the way out.
///
/// ```ignore
/// Toast::new("Saved").animation(Animation::Slide)                          // in from the right
/// Toast::new("Saved").animation(Animation::of(Slide::new().from(SlideFrom::Bottom)))
/// Toast::new("Saved").animation(Animation::of(Slide::new().distance(48.0).seconds(0.12)))
/// ```
///
/// What a fade cannot express: a surface that arrives by *travelling* says "this came from over
/// there", where a dissolve says "this was always here, you just could not see it". A notification
/// entering a corner is the case this exists for — it reads as something delivered rather than
/// something that materialised.
///
/// It owns no drawing. The offset becomes a picture through
/// [`AnimationFrame`], applied by whoever mounts the surface — so nothing is measured again, no
/// widget is told, and **no bounds move**: a card mid-slide is still exactly where it can be
/// clicked.
#[derive(Debug, Clone, Copy)]
pub struct Slide {
    /// Seconds left, or `None` at rest.
    left: Option<f32>,
    leaving: bool,
    duration: f32,
    /// Which edge the journey runs to and from.
    from: SlideFrom,
    /// How far the journey is, in logical pixels.
    distance: f64,
}

impl Slide {
    /// A slide over [`DEFAULT_DURATION`], in from the right by [`DEFAULT_DISTANCE`].
    pub fn new() -> Self {
        Self {
            left: None,
            leaving: false,
            duration: DEFAULT_DURATION,
            from: SlideFrom::default(),
            distance: DEFAULT_DISTANCE,
        }
    }

    /// Which edge it comes in from — and leaves towards.
    pub fn from(mut self, from: SlideFrom) -> Self {
        self.from = from;
        self
    }

    /// How far it travels, in logical pixels. Zero means a cut.
    pub fn distance(mut self, distance: f64) -> Self {
        self.distance = distance.max(0.0);
        self
    }

    /// How long the slide takes. Zero means a cut.
    pub fn seconds(mut self, seconds: f32) -> Self {
        self.duration = seconds.max(0.0);
        self
    }

    /// How far from home it should be drawn right now: `0.0` at rest, otherwise between
    /// [`distance`](Slide::distance) and `0.0`.
    ///
    /// Eased with the library's own [`smoothstep`], so a slide does not read as a linear drift
    /// while everything beside it accelerates.
    pub fn travelled(&self) -> f64 {
        let Some(left) = self.left else {
            // **A finished exit rests away, not back home.** The offset is where the surface *is*,
            // and one that has slid out has not come back — the same rule `Zoom` learned when a
            // finished exit snapping to life size read as a flash right before it vanished.
            return match self.leaving {
                true => self.distance,
                false => 0.0,
            };
        };
        if self.duration <= 0.0 {
            return 0.0;
        }
        // 0 → 1 through the animation, whichever way it is going.
        let eased = smoothstep(1.0 - (left / self.duration).clamp(0.0, 1.0)) as f64;
        // Arriving walks `distance → 0`; leaving walks `0 → distance`.
        match self.leaving {
            true => self.distance * eased,
            false => self.distance * (1.0 - eased),
        }
    }

    /// Is a slide in progress, either way?
    pub fn is_running(&self) -> bool {
        self.left.is_some()
    }
}

impl Default for Slide {
    fn default() -> Self {
        Self::new()
    }
}

impl Animate for Slide {
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
        let (ux, uy) = self.from.unit();
        let d = self.travelled();
        AnimationFrame::offset(ux * d, uy * d)
    }

    fn duration(&self) -> f32 {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **It arrives where it belongs and leaves back towards its edge**, and a zero duration is a
    /// cut — so "no animation" needs no special case at any call site.
    #[test]
    fn a_slide_walks_between_its_edge_and_home() {
        let mut s = Slide::new().distance(100.0).seconds(1.0);
        assert_eq!(s.travelled(), 0.0, "at rest it is home");
        assert!(!s.is_running());

        s.enter();
        assert!((s.travelled() - 100.0).abs() < 0.01, "arriving starts at the far edge");
        assert!(s.tick(0.5));
        let mid = s.travelled();
        assert!(mid > 0.0 && mid < 100.0, "…and is somewhere between, got {mid}");
        assert!(!s.tick(0.6), "the frame it stops running");
        assert_eq!(s.travelled(), 0.0, "…it is home");

        s.leave();
        assert!(s.travelled() < 0.01, "leaving starts at home");
        assert!(s.tick(0.9));
        assert!(s.travelled() > 40.0, "…and travels out, got {}", s.travelled());

        // Re-shown mid-flight: present again, not finishing a departure nobody wants.
        s.cancel();
        assert_eq!(s.travelled(), 0.0);
        assert!(!s.is_running());

        let mut cut = Slide::new().seconds(0.0);
        cut.enter();
        assert!(!cut.is_running(), "a duration of zero is a cut");
        assert_eq!(cut.travelled(), 0.0);
    }

    /// **The edge decides the sign, on the axis it belongs to** — and nothing else in the library
    /// has to know which edge was chosen, because all four say it in the one `offset` channel.
    #[test]
    fn each_edge_travels_on_its_own_axis() {
        let cases = [
            (SlideFrom::Right, (50.0, 0.0)),
            (SlideFrom::Left, (-50.0, 0.0)),
            (SlideFrom::Top, (0.0, -50.0)),
            (SlideFrom::Bottom, (0.0, 50.0)),
        ];
        for (edge, want) in cases {
            let mut s = Slide::new().from(edge).distance(50.0).seconds(1.0);
            s.enter();
            let got = s.frame().offset;
            assert!(
                (got.0 - want.0).abs() < 0.01 && (got.1 - want.1).abs() < 0.01,
                "{edge:?} starts at {want:?}, got {got:?}",
            );
        }
    }

    /// **A finished leave rests away.** The offset is where the surface *is*; resting back at home
    /// would put the card under the pointer again for the last frames of its own exit.
    #[test]
    fn a_finished_leave_rests_away_not_back_home() {
        let mut s = Slide::new().distance(80.0).seconds(0.2);
        s.enter();
        while s.tick(0.05) {}
        assert_eq!(s.travelled(), 0.0, "arrived: home");

        s.leave();
        while s.tick(0.05) {}
        assert_eq!(s.travelled(), 80.0, "left: still away, not snapped back");

        s.cancel();
        assert_eq!(s.travelled(), 0.0, "cancelled is a return home");
    }
}
