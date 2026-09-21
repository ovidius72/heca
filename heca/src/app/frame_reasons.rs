//! **Why the loop drew this frame** — the reasons as a list, not a boolean chain.
//!
//! Ten separate things can ask for the next frame, and they used to be ORed together into a single
//! `bool`. That reads fine and is impossible to debug: when the app spins at 100% with nothing
//! happening, exactly one of the ten is stuck true, and the answer is not written down anywhere —
//! so the only way to find it is to guess, which is how the question stayed open.
//!
//! The same defect, one line below, was already fixed for the *wake* side (F003/P082/T515: "the
//! loop picks its deadline with a chain, not a list"). This is that treatment applied to the
//! reasons themselves. Naming them costs nothing at runtime and makes `HECA_LOG_FRAMES=1` able to
//! say which one it is.

use std::time::{Duration, Instant};

/// Every reason the loop has to draw. One field per reason, so the answer to "which one" is a
/// value rather than something to work out by reading.
#[derive(Default, Clone, Copy)]
pub(crate) struct FrameReasons {
    /// Something asked for a repaint outright (`mark_full_redraw` and friends).
    pub(crate) marked: bool,
    /// A terminal produced output.
    pub(crate) backend_data: bool,
    /// A backend exited.
    pub(crate) backend_closed: bool,
    /// A chrome signal changed the tree's signature.
    pub(crate) chrome_runtime: bool,
    /// A visual-bell flash is fading.
    pub(crate) bell_flashing: bool,
    /// The session's own animations (pane open, column slide).
    pub(crate) session_animating: bool,
    /// A terminal is mid-animation.
    pub(crate) terminal_animating: bool,
    /// An inline GIF/APNG is advancing.
    pub(crate) image_animating: bool,
    /// A widget tree is mid-animation.
    pub(crate) chrome_animating: bool,
    /// A widget's booked wake came due.
    pub(crate) widget_due: bool,
}

impl FrameReasons {
    /// **The one list.** Every reason, paired with the name it reports under — so `any`, the tally
    /// and the printed line cannot drift apart, which is the whole failure this module exists to
    /// stop being possible.
    fn all(self) -> [(bool, &'static str); COUNT] {
        [
            (self.marked, "marked"),
            (self.backend_data, "backend_data"),
            (self.backend_closed, "backend_closed"),
            (self.chrome_runtime, "chrome_runtime"),
            (self.bell_flashing, "bell_flashing"),
            (self.session_animating, "session_animating"),
            (self.terminal_animating, "terminal_animating"),
            (self.image_animating, "image_animating"),
            (self.chrome_animating, "chrome_animating"),
            (self.widget_due, "widget_due"),
        ]
    }

    /// Whether anything at all wants the frame.
    pub(crate) fn any(self) -> bool {
        self.all().into_iter().any(|(on, _)| on)
    }

    /// The reasons that were true, by name.
    #[cfg(test)]
    fn named(self) -> Vec<&'static str> {
        self.all()
            .into_iter()
            .filter_map(|(on, name)| on.then_some(name))
            .collect()
    }
}

/// How many reasons there are. One place, so the tally cannot be sized wrong.
const COUNT: usize = 10;

/// How often the tally is printed.
const REPORT_EVERY: Duration = Duration::from_secs(1);

/// **A running tally of what has been asking for frames.**
///
/// Set `HECA_LOG_FRAMES=1` and leave the app idle: once a second it prints how many frames were
/// requested and which reasons asked for them. A spin names itself in the first line — the reason
/// that is stuck shows a count equal to the frame count, and everything else shows zero.
#[derive(Default)]
pub(crate) struct FrameLog {
    since: Option<Instant>,
    frames: u32,
    idle_wakes: u32,
    counts: [u32; COUNT],
    /// How often each wake source armed the timer, and the shortest delay each asked for.
    /// `next_wake`'s own rule is that every entry is a deadline something is *actually* waiting
    /// for — one that is always `Some` turns an idle window into a poller, and this is what says
    /// which one it is.
    wake_animating: u32,
    wake_widget: u32,
    wake_toast: u32,
    widget_soonest: Option<f32>,
    /// How many events arrived through the loop's own door, by kind. A loop in `ControlFlow::Wait`
    /// sleeps until an event — so when it wakes repeatedly with no timer armed, an event source is
    /// what is doing it, and this says which.
    events: Vec<(&'static str, u32)>,
}

#[cfg(debug_assertions)]
impl FrameLog {
    /// Record one event arriving through the loop's door, by kind.
    pub(crate) fn record_event(&mut self, kind: &'static str) {
        if std::env::var_os("HECA_LOG_FRAMES").is_none() {
            return;
        }
        match self.events.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => self.events.push((kind, 1)),
        }
    }

    /// Record which sources asked for the next wake, before the nearest of them wins.
    pub(crate) fn record_wake(
        &mut self,
        animating: bool,
        widget_in: Option<f32>,
        toast: Option<Instant>,
    ) {
        if std::env::var_os("HECA_LOG_FRAMES").is_none() {
            return;
        }
        self.wake_animating += u32::from(animating);
        self.wake_widget += u32::from(widget_in.is_some());
        self.wake_toast += u32::from(toast.is_some());
        if let Some(secs) = widget_in {
            self.widget_soonest = Some(self.widget_soonest.map_or(secs, |m: f32| m.min(secs)));
        }
    }

    /// Record one pass of the loop. Costs a branch when the variable is unset.
    pub(crate) fn record(&mut self, reasons: FrameReasons, now: Instant) {
        if std::env::var_os("HECA_LOG_FRAMES").is_none() {
            return;
        }
        let start = *self.since.get_or_insert(now);
        if reasons.any() {
            self.frames += 1;
        } else {
            self.idle_wakes += 1;
        }
        for (i, (on, _)) in reasons.all().into_iter().enumerate() {
            if on {
                self.counts[i] += 1;
            }
        }
        if now.duration_since(start) < REPORT_EVERY {
            return;
        }
        let mut asked: Vec<String> = FrameReasons::default()
            .all()
            .into_iter()
            .zip(self.counts)
            .filter(|(_, c)| *c > 0)
            .map(|((_, n), c)| format!("{n}={c}"))
            .collect();
        if asked.is_empty() {
            asked.push("nothing".into());
        }
        let mut armed: Vec<String> = Vec::new();
        if self.wake_animating > 0 {
            armed.push(format!("animating={}", self.wake_animating));
        }
        if self.wake_widget > 0 {
            armed.push(match self.widget_soonest {
                Some(s) => format!("widget={} (soonest {:.3}s)", self.wake_widget, s),
                None => format!("widget={}", self.wake_widget),
            });
        }
        if self.wake_toast > 0 {
            armed.push(format!("toast={}", self.wake_toast));
        }
        if armed.is_empty() {
            armed.push("nothing".into());
        }
        let events = match self.events.is_empty() {
            true => "none".to_string(),
            false => self
                .events
                .iter()
                .map(|(k, n)| format!("{k}={n}"))
                .collect::<Vec<_>>()
                .join(" "),
        };
        eprintln!(
            "[frames] {} drawn, {} wakes drew nothing, in {:.1}s — asked: {} | timer armed by: {} \
             | events in: {}",
            self.frames,
            self.idle_wakes,
            now.duration_since(start).as_secs_f32(),
            asked.join(" "),
            armed.join(" "),
            events,
        );
        *self = Self {
            since: Some(now),
            ..Self::default()
        };
    }
}

#[cfg(not(debug_assertions))]
impl FrameLog {
    pub(crate) fn record(&mut self, _: FrameReasons, _: Instant) {}
    pub(crate) fn record_wake(&mut self, _: bool, _: Option<f32>, _: Option<Instant>) {}
    pub(crate) fn record_event(&mut self, _: &'static str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing asking means no frame — the whole point of the list is that this stays answerable.
    #[test]
    fn nothing_asking_draws_nothing() {
        assert!(!FrameReasons::default().any());
    }

    /// Any single reason is enough, and it says which one it was.
    #[test]
    fn one_reason_is_enough_and_names_itself() {
        let r = FrameReasons {
            chrome_animating: true,
            ..Default::default()
        };
        assert!(r.any());
        assert_eq!(r.named(), vec!["chrome_animating"]);
    }

    /// **A widget's booked wake, on its own, draws.**
    ///
    /// The regression this pins: arriving at the deadline every other reason is false — the widget
    /// no longer reports a pending wake, because it is due *now* — so a loop that does not count
    /// the wake itself wakes up and goes straight back to sleep, and the tooltip under a resting
    /// pointer is never drawn. Scheduling the wake is only half of it.
    #[test]
    fn a_due_wake_is_itself_a_reason_to_draw() {
        let r = FrameReasons {
            widget_due: true,
            ..Default::default()
        };
        assert!(r.any(), "the loop wakes for it and then does nothing");
        assert_eq!(r.named(), vec!["widget_due"]);
    }

    /// Several at once are all reported, in the order they are declared.
    #[test]
    fn every_reason_that_asked_is_named() {
        let r = FrameReasons {
            marked: true,
            widget_due: true,
            ..Default::default()
        };
        assert_eq!(r.named(), vec!["marked", "widget_due"]);
    }
}
