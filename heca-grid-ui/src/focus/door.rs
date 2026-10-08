//! The door: the only code that writes [`Base::focused`].
//!
//! `focused` is the browser's `document.activeElement` — the widget the keyboard is aimed at, and
//! at most **one** per tree. Everything that moves focus goes through here: Tab, a press, a surface
//! opening, a host asking for a named control. A widget cannot blur *another* widget (it holds no
//! tree), so a request made here is **claimed**, and [`settle`](super::settle) — run by the router
//! before any key or press is delivered, and once a frame by the host — makes it true: the previous
//! holder lets go, and the claimant is the only one left.

use crate::component::Base;
use crate::reactive::{Signal, SignalGet, SignalUpdate};
use std::cell::Cell;

thread_local! {
    /// Hands out the identities widgets are remembered by (see [`Base::focus_id`]).
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}

/// What a widget has asked of the next [`settle`](super::settle).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(super) enum Claim {
    /// Nothing.
    #[default]
    None,
    /// Focus **me**, whoever has it: the user did it (a press, Tab) or code did ([`Base::focus`]).
    Take,
    /// A host signal turned true: take focus **unless something inside me already has it**, so a
    /// dock told it holds the keyboard does not steal it from the terminal the user just clicked in.
    Follow,
}

/// A host signal a widget's focus follows, and what happens when it lets go.
#[derive(Clone, Copy)]
pub(super) struct Follow {
    signal: Signal<bool>,
    /// A modal gives focus back to whoever had it when it took it (a dialog returns the keyboard to
    /// the control that opened it). A region does not: leaving a dock puts the keyboard on the panes.
    pub(super) modal: bool,
}

/// What [`Base`] remembers about its own focus.
#[derive(Default)]
pub(crate) struct FocusDoor {
    /// This widget's identity for focus purposes, `0` until it is first needed.
    pub(super) id: Cell<u64>,
    pub(super) claim: Cell<Claim>,
    pub(super) follows: Cell<Option<Follow>>,
    /// The value of the followed signal last acted on, so only a *change* moves focus.
    followed: Cell<bool>,
    /// The followed signal turned false: whatever held the keyboard inside me must let go.
    pub(super) released: Cell<bool>,
    /// Whether I held the keyboard myself at that moment (rather than something inside me).
    pub(super) held_when_released: Cell<bool>,
    /// Who held the keyboard when this modal took it, to give it back on close. `0` for none.
    pub(super) opener: Cell<u64>,
    /// The widget inside me that held the keyboard when I last let go, so taking it again returns
    /// there (back to the terminal in the dock) rather than to the dock itself. `0` for none.
    pub(super) remembered: Cell<u64>,
}

impl Base {
    /// **The door in.** Give this widget the keyboard; `visible` says the keyboard (not the mouse)
    /// did it, which is what makes the ring draw.
    ///
    /// With this and [`blur`](Self::blur) — and the signal a surface follows — as the only writers
    /// of [`focused`](Self::focused), who holds the keyboard is answered in one place. The previous
    /// holder lets go at the next settle, which the router runs before it delivers anything.
    pub fn focus(&self, visible: bool) {
        self.focused.set(true);
        self.focus_visible.set(visible);
        self.focus_door.claim.set(Claim::Take);
    }

    /// **The door out.** The widget no longer holds the keyboard.
    pub fn blur(&self) {
        self.focused.set(false);
        self.focus_visible.set(false);
    }

    /// **Does this widget hold the keyboard?** The one way to ask: a widget that follows a host
    /// signal ([`follow_focus`](Self::follow_focus)) is brought up to date first, so the answer is
    /// never a frame behind the signal.
    pub fn is_focused(&self) -> bool {
        self.sync_focus_follow();
        self.focused.get_untracked()
    }

    /// **Hold the keyboard exactly while `source` is true** — for a region whose focus a host
    /// decides (a dock the host says holds the keyboard).
    ///
    /// The widget keeps its *own* `focused` flag and follows the signal into it instead of **being**
    /// that signal. Sharing the signal made `focused` mean two things (the keyboard is here / this
    /// thing is open), and a click that blurred the widget would have closed it.
    ///
    /// When the signal turns true the region takes the keyboard — unless something inside it already
    /// has it — and returns to whatever inside it held it last time. When it turns false whatever
    /// inside it held the keyboard lets go, and the keyboard is nobody's (the host decides where it
    /// goes next).
    pub fn follow_focus(&mut self, source: Signal<bool>) {
        self.follow(source, false);
    }

    /// **Hold the keyboard exactly while `source` is true, and give it back** — for a surface that
    /// opens over the page: a dialog, a menu, a palette. Like [`follow_focus`](Self::follow_focus),
    /// and when it closes the keyboard returns to the widget that held it when the surface opened —
    /// the browser's "restore focus to the opener".
    pub fn follow_focus_modal(&mut self, source: Signal<bool>) {
        self.follow(source, true);
    }

    fn follow(&mut self, signal: Signal<bool>, modal: bool) {
        let door = &self.focus_door;
        door.follows.set(Some(Follow { signal, modal }));
        door.followed.set(false);
    }

    /// Bring a followed widget up to date with its signal. A no-op for every other widget.
    pub(crate) fn sync_focus_follow(&self) {
        let door = &self.focus_door;
        let Some(follow) = door.follows.get() else {
            return;
        };
        let now = follow.signal.get_untracked();
        if now == door.followed.replace(now) {
            return;
        }
        if now {
            self.focused.set(true);
            self.focus_visible.set(false);
            door.claim.set(Claim::Follow);
        } else {
            door.held_when_released.set(self.focused.get_untracked());
            self.blur();
            door.claim.set(Claim::None);
            door.released.set(true);
        }
    }

    /// This widget's identity for focus purposes — what lets a surface remember *which* widget to
    /// give the keyboard back to, across a frame that rebuilt nothing it can point at.
    pub(super) fn focus_id(&self) -> u64 {
        let door = &self.focus_door;
        if door.id.get() == 0 {
            door.id.set(NEXT_ID.with(|n| {
                let id = n.get();
                n.set(id + 1);
                id
            }));
        }
        door.id.get()
    }
}
