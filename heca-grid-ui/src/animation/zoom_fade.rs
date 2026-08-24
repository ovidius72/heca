//! [`ZoomFade`] — a surface that shrinks away, with the dissolve riding the shrink.

use super::{Animate, AnimationFrame, Fade, Sequence, Zoom};

/// How long the gesture takes, in seconds.
///
/// niri's own overview animation is in this range: long enough to read as one picture pulling
/// back, short enough that opening a map never feels like waiting. The dissolve **matches** the
/// movement rather than being shorter — one that finishes first leaves the last of the movement
/// playing on an already-invisible surface, and one that finishes after leaves a still picture
/// fading on nothing. Either reads as a step.
const DURATION: f32 = 0.2;

/// How much of the shrink plays before the dissolve joins it, as a **share** of it.
///
/// ⚠️ **Small.** At `0.66` the surface held fully opaque for two thirds of its shrink and then
/// dropped — movement with no dissolve, then dissolve with no movement, and the step between them
/// read as a flash on the way out (Antonio, driving, 2026-08-19). The dissolve has to **ride** the
/// movement, not follow it: one gesture, not two.
const LAG: f32 = 0.15;

/// **Arrives by pulling back, leaves by shrinking away while it dissolves.**
///
/// The exposé's gesture, and the reason it is a *named* animation rather than something each
/// surface composes: "the dissolve rides the movement, lagging by a share of it" is a rule about
/// how a surface leaves, and a rule written at a call site is a rule the next author gets wrong.
///
/// ```ignore
/// Overlay::new().panel(body).animation(Animation::ZoomFade)             // as it ships
/// Overlay::new().panel(body).animation(Animation::ZoomFade.from(1.3))   // …from further away
/// ```
///
/// It is [`Sequence`] over [`Zoom`] and [`Fade::out`] — nothing this file invents. Reach for the
/// parts directly only to build a gesture that is not this one.
pub struct ZoomFade(Sequence);

impl ZoomFade {
    /// The gesture as it ships.
    pub fn new() -> Self {
        Self::from(Zoom::new().from(1.3).seconds(DURATION))
    }

    /// The same gesture, starting from a different distance — `from` below `1.0` grows in from
    /// smaller, above it pulls back from larger.
    pub fn from_scale(from: f32) -> Self {
        Self::from(Zoom::new().from(from).seconds(DURATION))
    }

    fn from(zoom: Zoom) -> Self {
        Self(Sequence::new(zoom, Fade::out().seconds(DURATION)).lag(LAG))
    }
}

impl Default for ZoomFade {
    fn default() -> Self {
        Self::new()
    }
}

impl Animate for ZoomFade {
    fn enter(&mut self) {
        self.0.enter();
    }
    fn leave(&mut self) {
        self.0.leave();
    }
    fn cancel(&mut self) {
        self.0.cancel();
    }
    fn tick(&mut self, dt: f32) -> bool {
        self.0.tick(dt)
    }
    fn is_leaving(&self) -> bool {
        self.0.is_leaving()
    }
    fn frame(&self) -> AnimationFrame {
        self.0.frame()
    }
    fn duration(&self) -> f32 {
        self.0.duration()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One gesture: it is still moving while it dissolves, and the surface is held until **both**
    /// have played out.
    #[test]
    fn the_dissolve_rides_the_shrink() {
        let mut g = ZoomFade::new();
        g.leave();
        assert!(g.is_leaving());
        assert_eq!(g.frame().opacity, 1.0, "solid as it starts to move");

        g.tick(DURATION * LAG + 0.02);
        let mid = g.frame();
        assert!(mid.opacity < 1.0, "dissolving: {}", mid.opacity);
        assert!(mid.scale > 1.0, "…while still moving: {}", mid.scale);

        let mut frames = 0;
        while g.tick(1.0 / 60.0) && frames < 600 {
            frames += 1;
        }
        assert!(!g.is_leaving(), "and the surface is let go only at the end");
    }
}
