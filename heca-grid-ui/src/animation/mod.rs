//! **How a surface arrives, and how it leaves.**
//!
//! An [`Animate`] is a thing that, given time, says how a surface should be *drawn* while it is
//! coming or going. It owns its own clock and reports when it has finished; it draws nothing
//! itself. What it produces is an [`AnimationFrame`] — an opacity, a scale and an offset — which
//! the surface applies in **one** place ([`AnimationFrame::apply`]).
//!
//! **A caller names one; nobody composes one at a call site.**
//!
//! ```ignore
//! Overlay::new().panel(body).animation(Animation::Fade)
//! Overlay::new().panel(body).animation(Animation::ZoomFade)         // the exposé's gesture
//! Overlay::new().panel(body).animation(Animation::Zoom.from(0.8))   // tuned
//! Overlay::new().panel(body).animation(Animation::of(MyWhirl::new()))
//! ```
//!
//! [`Animation`] is that vocabulary; [`Animate`] is the trait behind it, which is what a **new**
//! animation implements — including one written outside this crate, passed through
//! [`Animation::of`] exactly where a built-in goes.
//!
//! # A trait, not an enum — and what that buys
//!
//! A third party must be able to write an animation **without touching this crate**. That is the
//! whole reason this is a trait: an enum cannot be extended without editing the host, which is
//! precisely what a plugin author cannot do (`AGENTS.md` ⭐⭐ RULE ZERO).
//!
//! It only holds if the vocabulary is right. [`AnimationFrame`] is *everything* a surface's
//! presentation needs — a fade sets `opacity`, a zoom sets `scale`, a slide sets `offset` — so
//! **adding an animation adds no plumbing anywhere**: a new file in this folder, and nothing else
//! edited. No enum arm, no `match` in a painter, no registry entry. If a future animation needs
//! something outside the frame (a rotation, a clip), add it to the frame **once**, here, where
//! every surface picks it up at the same time. If you find yourself editing a second file to land
//! a new animation, the model is wrong — fix the model, not the new file.
//!
//! # One file, one animation
//!
//! ```text
//! animation/
//! ├── mod.rs        the trait, the frame, the presence, shared easing, re-exports
//! ├── fade.rs       Fade
//! ├── zoom.rs       Zoom
//! ├── sequence.rs   the composition — one animation riding another, the lag a SHARE
//! ├── zoom_fade.rs   ZoomFade — the composed gesture, named once so no call site composes it
//! ├── vocabulary.rs  Animation — the built-ins by NAME, which is what a caller passes
//! └── presence.rs    Presence — the arriving/leaving state a surface widget embeds
//! ```
//!
//! `mod.rs` holds only what is genuinely *common*. An animation's own timing and state belong to
//! that animation, in its own file.
//!
//! # Beside `effects`, not inside it
//!
//! [`effects`](crate::effects) keeps the *per-widget* decorations — a press [`Flash`](crate::effects::Flash),
//! an [`Attention`](crate::effects::Attention) pulse, an [`Eased`](crate::effects::Eased) value.
//! Those are things a widget does to itself while it is present. These are things done to a whole
//! **surface** as it comes and goes, and they are driven by whoever mounts it.

mod fade;
mod presence;
mod sequence;
mod vocabulary;
mod zoom;
mod zoom_fade;

pub use fade::Fade;
pub use presence::Presence;
pub use sequence::Sequence;
pub use vocabulary::Animation;
pub use zoom::Zoom;
pub use zoom_fade::ZoomFade;

/// The smoothed value the whole library accelerates and settles with — write nothing else, so two
/// animations playing together cannot read as different materials.
///
/// `progress` runs `0.0..=1.0` and comes back eased the same way.
pub fn smoothstep(progress: f32) -> f32 {
    let t = progress.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

use crate::component::PaintCx;
use heca_core::layout::Point;

/// **How a surface arrives, animates and disappears.**
///
/// Implement it and any surface can use it — nothing else in this crate has to learn the new
/// name. The shape mirrors the effects a widget already embeds: a value over time, a
/// `tick(dt)` that says whether it is still going, and nothing to wire.
///
/// Four calls drive it. [`enter`](Animate::enter) and [`leave`](Animate::leave) begin an
/// arrival and an exit; [`cancel`](Animate::cancel) abandons whatever is playing and settles
/// fully present; [`tick`](Animate::tick) advances it. What the surface *draws* comes from
/// [`frame`](Animate::frame), and whether it may be taken away comes from
/// [`is_leaving`](Animate::is_leaving).
///
/// # Writing one
///
/// ```ignore
/// /// A surface that swings in from the left.
/// struct SlideIn { from: f64, left: Option<f32>, leaving: bool, duration: f32 }
///
/// impl Animate for SlideIn {
///     fn enter(&mut self)  { self.leaving = false; self.left = Some(self.duration); }
///     fn leave(&mut self)  { self.leaving = true;  self.left = Some(self.duration); }
///     fn cancel(&mut self) { self.leaving = false; self.left = None; }
///     fn tick(&mut self, dt: f32) -> bool { /* count down, return whether still going */ }
///     fn is_leaving(&self) -> bool { self.leaving && self.left.is_some() }
///     fn frame(&self) -> AnimationFrame { AnimationFrame::offset(self.x(), 0.0) }
///     fn duration(&self) -> f32 { self.duration }
/// }
/// ```
///
/// That file is the whole change. Nothing in `Overlay`, in the painter or in the host knows the
/// type exists.
pub trait Animate {
    /// Begin **arriving**. Called again while already arriving, it restarts the arrival.
    fn enter(&mut self);

    /// Begin **leaving**. From here [`is_leaving`](Animate::is_leaving) answers `true` until the
    /// exit has played out, which is what keeps the surface mounted long enough to be seen going.
    fn leave(&mut self);

    /// Abandon whatever is playing and be **fully present** — what a surface re-shown mid-arrival
    /// needs, rather than finishing a journey nobody is waiting for.
    fn cancel(&mut self);

    /// Advance by `dt` seconds. Returns `true` while still moving, so the host keeps asking for
    /// frames — an animation nobody ticks is a frozen surface.
    fn tick(&mut self, dt: f32) -> bool;

    /// **Is an exit still playing?**
    ///
    /// A surface's exit is one gesture that may be made of several effects, and it is not over
    /// until every one of them has finished. Everything that asks "is it still there" reads this,
    /// so a composition answers for all of its parts and no host has to know which effects a
    /// surface chose. Tying a surface's lifetime to one half of its exit is what left the exposé's
    /// cards on screen after the map itself had gone (F003/P082/T327).
    fn is_leaving(&self) -> bool;

    /// What to draw *this frame* — see [`AnimationFrame`]. At rest, [`AnimationFrame::IDENTITY`].
    fn frame(&self) -> AnimationFrame;

    /// How long one arrival or exit takes, in seconds. `0.0` (the default) means "a cut", and is
    /// also the honest answer for an animation that does not run on a clock.
    ///
    /// A **composition** reads it so a lag can be expressed as a *share* of the animation it rides
    /// ([`Sequence::lag`]) rather than as a second duration that drifts when either is tuned.
    fn duration(&self) -> f32 {
        0.0
    }
}

/// **What an animation has to say about a surface this frame** — and the whole vocabulary a
/// surface's presentation needs.
///
/// Three channels, because between them they cover appearing and disappearing: how visible it is,
/// how big it is, and where it is. A fade sets `opacity`, a zoom sets `scale`, a slide sets
/// `offset`, and a composition multiplies them together. Nothing outside this struct is needed to
/// play a new animation — which is exactly what makes a new one a single new file.
///
/// Every channel is a **paint** transform, never a layout number: the tree is laid out once, at
/// life size, and this is something done *to* the picture. A surface mid-animation is still
/// exactly where its bounds say it is, so hit-testing, focus and drag are untouched (`AGENTS.md`
/// § 0b: sizes are shares, an animation's scale is not a size).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationFrame {
    /// `0.0..=1.0`, multiplied into everything the surface draws.
    pub opacity: f32,
    /// `1.0` is life size. Scales the picture about the surface's own centre — position **and**
    /// size, and with them the font size, the radius, the border width and the glow.
    pub scale: f32,
    /// Logical pixels, `(dx, dy)`, added to where the surface draws.
    pub offset: (f64, f64),
}

impl AnimationFrame {
    /// A surface at rest: fully present, life size, where it belongs.
    pub const IDENTITY: Self = Self {
        opacity: 1.0,
        scale: 1.0,
        offset: (0.0, 0.0),
    };

    /// A frame that only dims.
    pub fn opacity(opacity: f32) -> Self {
        Self {
            opacity,
            ..Self::IDENTITY
        }
    }

    /// A frame that only resizes.
    pub fn scale(scale: f32) -> Self {
        Self {
            scale,
            ..Self::IDENTITY
        }
    }

    /// A frame that only moves.
    pub fn offset(dx: f64, dy: f64) -> Self {
        Self {
            offset: (dx, dy),
            ..Self::IDENTITY
        }
    }

    /// **This frame on top of `under`** — how a composition combines its parts.
    ///
    /// Multiplicative on opacity and scale and additive on offset, which is the same arithmetic
    /// [`PaintCx::with_opacity`] and [`PaintCx::with_scale`] already do when they nest: a
    /// half-faded surface inside a half-faded one draws at a quarter.
    pub fn over(self, under: Self) -> Self {
        Self {
            opacity: self.opacity * under.opacity,
            scale: self.scale * under.scale,
            offset: (
                self.offset.0 + under.offset.0,
                self.offset.1 + under.offset.1,
            ),
        }
    }

    /// Nothing to do: a surface at rest.
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// **Paint `f`'s subtree under this frame — the one place a frame becomes a picture.**
    ///
    /// `origin` is the fixed point the scale works about: the surface's own centre, so it grows
    /// from and shrinks toward the middle of itself rather than a corner.
    ///
    /// Every channel goes through a `PaintCx` transform that already exists, so a new animation
    /// needs no new drawing code anywhere — the reason the vocabulary is fixed at three channels
    /// and widened here rather than per surface.
    pub fn apply<'a>(self, cx: &mut PaintCx<'a>, origin: Point, f: impl FnOnce(&mut PaintCx<'a>)) {
        cx.with_opacity(self.opacity, |cx| {
            cx.with_scale(self.scale, origin, |cx| {
                cx.with_translate(self.offset.0, self.offset.1, f)
            })
        });
    }
}

impl Default for AnimationFrame {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A new animation is a new file and nothing else.** This one is written entirely here, in a
    /// test module, using only the public trait — the proof that a third party can add one without
    /// touching the library: no enum arm, no painter `match`, no registry entry.
    struct Slide {
        distance: f64,
        left: Option<f32>,
        leaving: bool,
        duration: f32,
    }

    impl Slide {
        fn new(distance: f64) -> Self {
            Self { distance, left: None, leaving: false, duration: 0.2 }
        }
    }

    impl Animate for Slide {
        fn enter(&mut self) {
            self.leaving = false;
            self.left = Some(self.duration);
        }
        fn leave(&mut self) {
            self.leaving = true;
            self.left = Some(self.duration);
        }
        fn cancel(&mut self) {
            self.leaving = false;
            self.left = None;
        }
        fn tick(&mut self, dt: f32) -> bool {
            let Some(left) = self.left else { return false };
            match left - dt > 0.0 {
                true => {
                    self.left = Some(left - dt);
                    true
                }
                false => {
                    self.left = None;
                    false
                }
            }
        }
        fn is_leaving(&self) -> bool {
            self.leaving && self.left.is_some()
        }
        fn frame(&self) -> AnimationFrame {
            let Some(left) = self.left else {
                return match self.leaving {
                    true => AnimationFrame::offset(self.distance, 0.0),
                    false => AnimationFrame::IDENTITY,
                };
            };
            let progress = smoothstep(1.0 - left / self.duration);
            let away = match self.leaving {
                true => progress,
                false => 1.0 - progress,
            };
            AnimationFrame::offset(self.distance * away as f64, 0.0)
        }
        fn duration(&self) -> f32 {
            self.duration
        }
    }

    #[test]
    fn an_animation_written_outside_this_crate_needs_nothing_added_to_it() {
        let mut slide = Slide::new(120.0);
        assert_eq!(slide.frame(), AnimationFrame::IDENTITY, "at rest it is where it belongs");

        slide.enter();
        assert_eq!(slide.frame().offset.0, 120.0, "an arrival starts away");
        while slide.tick(0.05) {}
        assert_eq!(slide.frame(), AnimationFrame::IDENTITY, "and lands home");

        slide.leave();
        assert!(slide.is_leaving(), "an exit holds the surface while it plays");
        while slide.tick(0.05) {}
        assert!(!slide.is_leaving(), "…and releases it when done");
        assert_eq!(slide.frame().offset.0, 120.0, "a finished exit rests AWAY, not back home");
    }

    /// Composition is multiplication, exactly as nesting two `PaintCx` transforms is.
    #[test]
    fn frames_compose_the_way_the_painter_nests() {
        let dim = AnimationFrame::opacity(0.5);
        let small = AnimationFrame { opacity: 0.5, scale: 0.5, offset: (10.0, -4.0) };
        let both = dim.over(small);
        assert_eq!(both.opacity, 0.25, "two halves make a quarter");
        assert_eq!(both.scale, 0.5);
        assert_eq!(both.offset, (10.0, -4.0));
        assert!(AnimationFrame::IDENTITY.over(small) == small, "identity changes nothing");
    }
}
