//! [`Sequence`] — one animation riding another, the lag a **share** of what it rides.

use super::{Animate, AnimationFrame};

/// **Two animations as one gesture**: a `lead` plays, and a `follow` joins it part of the way
/// through.
///
/// ```ignore
/// // The exposé: it shrinks away, and the dissolve rides the shrink.
/// Sequence::new(Zoom::new().from(1.3), Fade::out()).lag(0.15)
/// ```
///
/// # Why the lag is a share and never a second duration
///
/// A surface that leaves by doing two things at once has to keep them in step. Written as two
/// durations they drift the moment either is tuned — and the drift is invisible in a test and
/// obvious on screen. `lag` is therefore a **fraction of the lead's own duration**: change the
/// lead and the sequencing still holds, with nothing to keep in step by hand.
///
/// **Small.** At `0.66` the exposé held fully opaque for two thirds of its shrink and then dropped
/// — movement with no dissolve, then dissolve with no movement, and the step between them read as
/// a flash on the way out (Antonio, driving, 2026-08-19). The follower has to **ride** the lead,
/// not follow it: one gesture, not two. A short lead-in is all that is wanted, so the surface is
/// still solid as it starts to move.
///
/// # It is an animation itself
///
/// So it composes: a sequence can lead another sequence, and everything that asks "is it still
/// leaving" gets one answer covering every part. That is a **type guarantee** rather than something
/// each host has to remember — getting it wrong retired the exposé when only its dissolve had
/// finished, leaving the cards on screen after the map had gone (F003/P082/T327).
pub struct Sequence {
    lead: Box<dyn Animate>,
    follow: Box<dyn Animate>,
    /// Share of the lead's duration that plays before the follower starts.
    lag: f32,
    /// The follower's start, still waiting to happen.
    pending: Option<Pending>,
}

/// A follower's start that has not come round yet.
struct Pending {
    /// Seconds still to wait.
    left: f32,
    /// Which gesture it will begin when the wait runs out.
    leaving: bool,
}

impl Sequence {
    /// `follow` rides `lead`, starting with it until [`lag`](Sequence::lag) says otherwise.
    pub fn new(lead: impl Animate + 'static, follow: impl Animate + 'static) -> Self {
        Self {
            lead: Box::new(lead),
            follow: Box::new(follow),
            lag: 0.0,
            pending: None,
        }
    }

    /// How much of the lead plays before the follower joins, as a **share of the lead's own
    /// duration** (`0.0..=1.0`). A lead that runs on no clock ([`Animate::duration`] of zero)
    /// gets no lag, because a share of nothing is nothing.
    pub fn lag(mut self, share: f32) -> Self {
        self.lag = share.clamp(0.0, 1.0);
        self
    }

    /// Seconds to wait before the follower starts.
    fn wait(&self) -> f32 {
        self.lag * self.lead.duration()
    }

    /// Begin `leaving` (or arriving) on both, holding the follower back by [`wait`](Self::wait).
    fn begin(&mut self, leaving: bool) {
        match leaving {
            true => self.lead.leave(),
            false => self.lead.enter(),
        }
        let wait = self.wait();
        if wait <= 0.0 {
            self.pending = None;
            match leaving {
                true => self.follow.leave(),
                false => self.follow.enter(),
            }
            return;
        }
        self.pending = Some(Pending {
            left: wait,
            leaving,
        });
    }
}

impl Animate for Sequence {
    fn enter(&mut self) {
        self.begin(false);
    }

    fn leave(&mut self) {
        self.begin(true);
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.lead.cancel();
        self.follow.cancel();
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut running = self.lead.tick(dt);
        // Burn the wait first, and spend only what is left of this frame on the follower — so a lag
        // never costs a frame of the animation it delays.
        let dt = match self.pending.take() {
            None => dt,
            Some(mut pending) => {
                let spent = dt.min(pending.left);
                pending.left -= spent;
                if pending.left > 0.0 {
                    self.pending = Some(pending);
                    return true; // still waiting: the follower has not begun, and this is a frame.
                }
                match pending.leaving {
                    true => self.follow.leave(),
                    false => self.follow.enter(),
                }
                dt - spent
            }
        };
        running |= self.follow.tick(dt);
        running
    }

    /// **The whole gesture, not the half that finishes first.** Still leaving while either part is
    /// playing — or while the follower's exit has not even begun.
    fn is_leaving(&self) -> bool {
        self.lead.is_leaving()
            || self.follow.is_leaving()
            || self.pending.as_ref().is_some_and(|p| p.leaving)
    }

    fn frame(&self) -> AnimationFrame {
        self.follow.frame().over(self.lead.frame())
    }

    /// The lead, plus whatever of the follower runs past the end of it.
    fn duration(&self) -> f32 {
        self.lead
            .duration()
            .max(self.wait() + self.follow.duration())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Fade, Zoom};
    use super::*;

    /// The exposé's exit, as one object: it shrinks, the dissolve joins part of the way through,
    /// and the surface is held until **both** have played out.
    #[test]
    fn the_follower_rides_the_lead_and_the_whole_gesture_holds_the_surface() {
        let mut exit =
            Sequence::new(Zoom::new().from(0.8).seconds(0.2), Fade::out().seconds(0.2)).lag(0.5); // half of the shrink plays first

        exit.leave();
        assert!(
            exit.is_leaving(),
            "the gesture holds the surface from the first frame"
        );
        assert_eq!(exit.frame().scale, 1.0, "an exit leaves from life size");

        // While the lead-in runs, the surface is **moving and still solid** — the dissolve has not
        // started. Fading at the same time hides the movement before it has played.
        assert!(exit.tick(0.05));
        assert!(
            exit.frame().scale < 1.0,
            "already shrinking, got {}",
            exit.frame().scale
        );
        assert_eq!(
            exit.frame().opacity,
            1.0,
            "…and fully present while the wait runs"
        );

        assert!(exit.tick(0.05), "the wait is over; the dissolve begins");
        assert!(exit.tick(0.05));
        let mid = exit.frame();
        assert!(
            mid.opacity < 1.0 && mid.opacity > 0.0,
            "now dissolving, got {}",
            mid.opacity
        );

        // The shrink finishes first — and the surface is still held, because the dissolve is not
        // done. Tying the lifetime to one half is the bug this composition exists to prevent.
        assert!(exit.tick(0.06));
        assert!(exit.is_leaving(), "the shrink is over, the gesture is not");

        while exit.tick(0.05) {}
        assert!(
            !exit.is_leaving(),
            "…and it releases the surface only at the end"
        );
        assert_eq!(exit.frame().scale, 0.8, "resting where it left off");
        assert_eq!(exit.frame().opacity, 0.0);
    }

    /// **The lag is a share, so tuning the lead keeps the sequencing.** Doubling the shrink doubles
    /// the lead-in with nothing else edited — the drift a second duration would introduce.
    #[test]
    fn the_lag_follows_the_lead_it_is_a_share_of() {
        let short = Sequence::new(Zoom::new().seconds(0.2), Fade::out().seconds(0.2)).lag(0.5);
        let long = Sequence::new(Zoom::new().seconds(0.4), Fade::out().seconds(0.2)).lag(0.5);
        assert_eq!(short.wait(), 0.1);
        assert_eq!(long.wait(), 0.2, "the same share of a longer lead");
    }

    /// A sequence is an animation, so it nests — and a cut anywhere in it costs nothing.
    #[test]
    fn sequences_compose_and_a_cut_never_holds_anything() {
        let mut nested = Sequence::new(
            Sequence::new(Zoom::new().seconds(0.1), Fade::out().seconds(0.1)).lag(0.5),
            Fade::out().seconds(0.0),
        )
        .lag(0.5);
        nested.leave();
        assert!(nested.is_leaving());
        while nested.tick(0.05) {}
        assert!(!nested.is_leaving(), "every part played out");

        let mut cut = Sequence::new(Zoom::new().seconds(0.0), Fade::out().seconds(0.0));
        cut.leave();
        assert!(!cut.is_leaving(), "nothing to wait for");
    }
}
