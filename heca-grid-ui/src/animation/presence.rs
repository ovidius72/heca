//! [`Presence`] — whether a surface is up, and the animation carrying it there and away.

use super::{Animate, AnimationFrame};

/// **Where a surface is in its coming and going.**
///
/// A widget that can appear and disappear embeds one of these the way a clickable widget embeds a
/// [`Flash`](crate::effects::Flash): construct it, [`enter`](Presence::enter) or
/// [`leave`](Presence::leave) it, tick it, and paint its [`frame`](Presence::frame).
/// [`Overlay`](crate::widgets::Overlay) is the first and the reference.
///
/// **The animation is optional and that is the whole of it.** No animation ⇒ entering and leaving
/// are instant, [`is_leaving`](Presence::is_leaving) is never true, and nothing anywhere waits for
/// anything. A surface that declares nothing is not paying for a null object — there is no object.
///
/// # The two rules that live here, so no caller repeats them
///
/// - **Entering something already up is not an arrival.** A surface is rebuilt and re-entered
///   whenever what it shows changes underneath it; replaying the arrival there made the exposé zoom
///   open on every keystroke (Antonio, driving, 2026-08-11).
/// - **A surface on its way out is not resurrected.** Its exit was already decided; arriving
///   mid-exit made the map snap back to full opacity and start leaving again (2026-08-19). The
///   exposé hits this on the ordinary path: it dismisses itself and *then* focuses the pane you
///   chose, and focusing rebuilds it.
///
/// A host also hands this whole value over when it **rebuilds** a surface, so a gesture in flight
/// carries across to the new tree instead of starting again — see
/// [`Component::presence_mut`](crate::Component::presence_mut).
#[derive(Default)]
pub struct Presence {
    /// Whether the surface is up, as this has acted on it.
    open: bool,
    /// How it arrives and leaves. `None` ⇒ it cuts.
    animation: Option<Box<dyn Animate>>,
    /// **An exit is playing and has not been settled yet.**
    ///
    /// Set when [`leave`](Self::leave) starts a gesture that will take frames, cleared by the
    /// [`tick`](Self::tick) that sees it end — which is the moment the surface may finally be taken
    /// down. Without it, "has finished leaving" is indistinguishable from "was never up", and a
    /// widget that never used a presence at all looks like one whose exit just completed.
    exit_pending: bool,
}

impl Presence {
    /// A closed surface with no animation: it will appear and go between two frames.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare how this surface arrives and leaves, or `None` to cut. Replaces whatever it had.
    pub fn set_animation(&mut self, animation: Option<Box<dyn Animate>>) {
        self.animation = animation;
    }

    /// Whether an animation was declared at all.
    pub fn is_animated(&self) -> bool {
        self.animation.is_some()
    }

    /// **Begin arriving.** Returns whether the surface is (now, or already) up — `false` only when
    /// it refused, which is when it is still leaving.
    pub fn enter(&mut self) -> bool {
        if self.is_leaving() {
            return false;
        }
        if self.open {
            return true;
        }
        self.open = true;
        if let Some(animation) = &mut self.animation {
            animation.enter();
        }
        true
    }

    /// **Begin leaving.** Returns whether the surface may go **now** — `true` when it declared no
    /// animation, or its exit has already finished; `false` while a gesture is still playing.
    pub fn leave(&mut self) -> bool {
        if self.open {
            self.open = false;
            if let Some(animation) = &mut self.animation {
                animation.leave();
            }
        }
        // A gesture that will take frames has to be settled when it ends; one that cuts is already
        // settled by the caller this returned `true` to.
        self.exit_pending = self.is_leaving();
        !self.is_leaving()
    }

    /// **Follow an open flag** — for a surface driven by a signal rather than by a caller (a
    /// composing widget binding `open_signal`). States the truth; the rules above still apply.
    pub fn follow(&mut self, open: bool) {
        match open {
            true => {
                self.enter();
            }
            false => {
                self.leave();
            }
        }
    }

    /// **Adopt a state without playing it** — a surface *born* open, and the other half of carrying
    /// one across a rebuild. Order-independent by construction: it never touches the animation, so
    /// `.default_open(true).animation(..)` and `.animation(..).default_open(true)` are the same surface.
    pub fn assume_open(&mut self, open: bool) {
        self.open = open;
    }

    /// Is the surface up? (A leaving one is **not** — it is on screen, but dismissed.)
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// **Is an exit still playing?** The whole gesture, not the part that finishes first. Always
    /// `false` for a surface with no animation.
    pub fn is_leaving(&self) -> bool {
        self.animation.as_ref().is_some_and(|a| a.is_leaving())
    }

    /// Advance by `dt` seconds. `true` while the surface is still moving.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.animation.as_mut().is_some_and(|a| a.tick(dt))
    }

    /// **Did an exit finish on this tick?** — the one moment a surface that animates out may be
    /// taken down.
    ///
    /// Answers `true` exactly once per exit, and never for a surface that was never up: a widget
    /// with no presence in play has no exit pending, so it is never mistaken for one that has just
    /// finished going away. Call it after [`tick`](Self::tick).
    pub fn take_finished_exit(&mut self) -> bool {
        let finished = self.exit_pending && !self.is_leaving();
        if finished {
            self.exit_pending = false;
        }
        finished
    }

    /// What to draw this frame — [`AnimationFrame::IDENTITY`] unless an animation is playing.
    pub fn frame(&self) -> AnimationFrame {
        self.animation
            .as_ref()
            .map_or(AnimationFrame::IDENTITY, |a| a.frame())
    }
}

impl std::fmt::Debug for Presence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presence")
            .field("open", &self.open)
            .field("animated", &self.is_animated())
            .field("leaving", &self.is_leaving())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Animation, Fade};
    use super::*;

    /// **No animation, no ceremony**: it is up, then it is gone, and nothing ever waits for it.
    #[test]
    fn a_surface_with_no_animation_comes_and_goes_at_once() {
        let mut p = Presence::new();
        assert!(!p.is_animated());
        assert!(p.enter());
        assert!(p.is_open());
        assert!(p.leave(), "it may go now");
        assert!(!p.is_leaving());
        assert!(!p.tick(1.0 / 60.0));
        assert_eq!(p.frame(), AnimationFrame::IDENTITY);
    }

    /// With one declared, leaving is a gesture the surface is held for.
    #[test]
    fn an_exit_holds_the_surface_until_it_has_played_out() {
        let mut p = Presence::new();
        p.set_animation(Animation::Fade.build());
        p.enter();
        while p.tick(0.05) {}

        assert!(!p.leave(), "not yet — it is playing");
        assert!(p.is_leaving());
        while p.tick(0.05) {}
        assert!(!p.is_leaving(), "…and it lets go at the end");
    }

    /// A caller states the truth as often as it likes; only a **change** plays anything.
    #[test]
    fn entering_an_open_surface_replays_nothing() {
        let mut p = Presence::new();
        p.set_animation(Some(Box::new(Fade::new().seconds(0.2))));
        p.enter();
        p.tick(0.05);
        p.tick(0.05);
        let mid = p.frame().opacity;
        p.enter(); // said again — the exposé's rebuild path
        assert_eq!(
            p.frame().opacity,
            mid,
            "the arrival carries on from where it was"
        );
    }

    /// **A surface on its way out is not resurrected** — the rule a host used to have to know.
    #[test]
    fn entering_a_leaving_surface_does_not_cancel_its_exit() {
        let mut p = Presence::new();
        p.set_animation(Some(Box::new(Fade::new().seconds(0.2))));
        p.enter();
        while p.tick(0.05) {}

        p.leave();
        p.tick(0.05);
        let mid = p.frame().opacity;
        assert!(mid < 1.0, "on its way out: {mid}");

        assert!(!p.enter(), "it refuses");
        assert_eq!(
            p.frame().opacity,
            mid,
            "the exit carries on from where it was"
        );
        assert!(p.is_leaving());
    }

    /// **An exit that animates reports, exactly once, the frame it ends** — the moment a surface
    /// may be taken down.
    ///
    /// `close` leaves a surface `visible` while its exit plays, so something has to end it.
    /// Nothing did: a surface with an exit animation stayed visible forever after its first close,
    /// and the exposé kept being offered letters by `prefix+/` long after it was shut (Antonio,
    /// driving, 2026-09-15).
    #[test]
    fn an_animated_exit_reports_finishing_once() {
        let mut p = Presence::new();
        p.set_animation(Some(Box::new(Fade::new().seconds(0.2))));
        p.enter();
        assert!(!p.take_finished_exit(), "nothing has left yet");

        p.leave();
        assert!(p.is_leaving(), "the gesture holds it while it plays");
        assert!(!p.take_finished_exit(), "…and it has not finished");

        p.tick(1.0);
        assert!(p.take_finished_exit(), "the frame the exit ends");
        assert!(
            !p.take_finished_exit(),
            "and only that frame — a surface is taken down once",
        );
    }

    /// **A presence nobody ever used never reports an exit.** "Not open and not leaving" is also
    /// true of a widget that was never up, and reading it that way hid the entire tree on the first
    /// tick — which is why the question is asked of the presence instead of inferred.
    #[test]
    fn a_presence_that_was_never_up_never_finishes_an_exit() {
        let mut p = Presence::new();
        p.tick(1.0);
        assert!(!p.take_finished_exit());

        let mut animated = Presence::new();
        animated.set_animation(Some(Box::new(Fade::new().seconds(0.2))));
        animated.tick(1.0);
        assert!(!animated.take_finished_exit());
    }

    /// A surface that **cuts** is settled by the caller `leave` returned `true` to, so it must not
    /// also be reported here — that would be two answers to one question.
    #[test]
    fn an_exit_that_cuts_is_not_reported_as_finishing_later() {
        let mut p = Presence::new();
        p.enter();
        assert!(p.leave(), "with nothing to play, it may go now");
        p.tick(1.0);
        assert!(!p.take_finished_exit());
    }
}
