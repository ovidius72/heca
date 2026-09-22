//! **Why the loop draws a frame** — the reasons as a list, not a boolean chain.
//!
//! Ten separate things can ask for the next frame, and they used to be ORed together into a single
//! `bool`. That reads fine and cannot be examined: when the app spins with nothing happening,
//! exactly one of the ten is stuck true, and a chain has no way to say which.
//!
//! The same defect, one line below, was already fixed for the wake side (F003/P082/T515: "the loop
//! picks its deadline with a chain, not a list"). This is that treatment applied to the reasons.

/// Every reason the loop has to draw. One field per reason, so which one asked is a value rather
/// than something to work out by reading.
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

/// How many reasons there are. One place, so nothing sized from it can be wrong.
const COUNT: usize = 10;

impl FrameReasons {
    /// **The one list.** Every reason, paired with the name it goes by — so `any` and anything that
    /// reports them cannot drift apart.
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
