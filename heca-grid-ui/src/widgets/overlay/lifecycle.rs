//! Shared opening, closing, focus placement and animation lifecycle.

use super::*;

impl Overlay {
    /// **Put it on screen.** If an [`animation`](Overlay::animation) was declared it plays;
    /// if not, it is simply up. Already up, or still on its way out, and nothing happens.
    ///
    /// Opening is also where the keyboard is placed, when the surface said where it should go —
    /// here rather than at construction, because a surface is built once and opened many times.
    pub fn show(&mut self) {
        // Refuses while the exit is still playing, exactly as `Presence::enter` does — asked here
        // so the open flag is not raised on a surface that is still going away.
        if self.base.presence.is_leaving() {
            return;
        }
        self.base.open.set(true);
        self.settle();
    }

    /// **The one place this surface reacts to becoming open or closed**, whoever asked.
    ///
    /// There are two ways in and they must not each carry their own version of what opening means:
    /// a caller says [`show`](Overlay::show), or the open flag is simply set — by
    /// [`SurfaceHandle`], by [`open_when`](Overlay::open_when), by a host binding its own state.
    /// Reacting in the verb alone is what left the second way half-done: the arrival played,
    /// because [`tick`](Component::tick) follows the flag every frame, but the keyboard was never
    /// put where the surface said, because that lived in `show` and nothing else called it. A
    /// surface raised by a signal came up with the keyboard nowhere.
    ///
    /// So the rule lives here and is asked at both moments: `show` runs it at once (the keyboard
    /// must not wait for the next frame), and `tick` runs it every frame for everyone else.
    pub(super) fn settle(&mut self) {
        let was_open = self.base.presence.is_open();
        self.base.presence.follow(self.base.open.get_untracked());
        if !was_open && self.base.presence.is_open() {
            // A completed exit hid the surface. Every opening path returns through this edge.
            self.base.visible.set(true);
            self.base.mark_needs_paint();
            self.place_default_focus();
        }
    }

    /// Put the keyboard where [`default_focus`](Overlay::default_focus) said, if that control is
    /// there. Goes **through the focus manager**, which is the point: writing the control's
    /// `focused` signal by hand leaves the manager's own position unset, so the next Tab is spent
    /// moving to the first control instead of the next one and the first press appears to do
    /// nothing.
    fn place_default_focus(&mut self) {
        let Some(name) = self.default_focus.clone() else {
            return;
        };
        if let Some(panel) = self.base.children.first_mut() {
            self.focus.focus_named(panel.as_mut(), &name);
        }
    }

    /// **Dismiss it.** With an animation this begins the exit and the surface stays on screen,
    /// inert, until the gesture has played out; with none, it is gone now.
    pub fn close(&mut self) {
        self.base.presence.leave();
        self.base.open.set(false);
    }

    /// Open it if it is closed, dismiss it if it is open.
    pub fn toggle(&mut self) {
        match self.base.presence.is_open() {
            true => self.close(),
            false => self.show(),
        }
    }

    /// The open-state signal — the host (or composing widget) binds this.
    /// **A handle to show and close this surface from anywhere.** See [`SurfaceHandle`].
    pub fn handle(&self) -> SurfaceHandle {
        SurfaceHandle::new(self.base.open)
    }

    pub fn open_signal(&self) -> Signal<bool> {
        self.base.open
    }

    pub(super) fn is_open(&self) -> bool {
        self.base.open.get_untracked()
    }

    /// **Is there still something to draw?** Open, or dismissed and still playing its exit.
    ///
    /// The distinction that makes an animated dismissal possible at all: input follows
    /// [`is_open`](Self::is_open) — a dissolving surface holds nothing, or the map would refuse
    /// every act on the pane it exists to let you choose — while *painting* follows this.
    pub(super) fn is_present(&self) -> bool {
        self.is_open() || self.base.presence.is_leaving()
    }
}
