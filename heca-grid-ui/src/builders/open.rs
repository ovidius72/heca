//! **What opens a surface** — `.open(signal)` or `.open("an.action")`, one builder on every surface
//! that comes and goes ([`Overlay`](crate::widgets::Overlay), [`Dialog`](crate::widgets::Dialog),
//! [`KeyHintGroup`](crate::widgets::KeyHintGroup), the command palette).
//!
//! ```ignore
//! let editing = signal(false);
//! Overlay::new().panel(body).open(editing)          // your state: it follows it
//! KeyHintGroup::new(cards).open("map.jump")         // an action that opens it: a key, RPC, a plugin
//! ```
//!
//! - **A signal** is the state, held by whoever wrote it: the surface is up exactly while it is
//!   `true`, and writes it `false` when it closes itself (its dismiss key, a choice, a click
//!   outside), so the holder always knows. For native code that already has the state.
//! - **A value that changes** — a function reading signals, or a `Memo` — for a surface that is up
//!   exactly while something else is true (a field's suggestion list: up while a word is open and
//!   something matches it). The surface follows it into its own state; closing itself holds until
//!   the value changes.
//! - **An action name** opens the surface when that action runs — bound to a key in config
//!   (`[[keys.surface]]`), called over RPC, or dispatched by a plugin as an `Intent`. The surface
//!   keeps its own state. For anything that cannot hold a Rust signal — a description, a plugin, a
//!   script — and for a key. A description says `"open": "mypanel.pick"`.
//!
//! Closing is the same for every surface and needs no name: the dismiss key (`dismiss` in
//! `[keys.widgets]`, `Escape` by default), a choice, a click outside, or `close_overlay` from RPC.

use crate::component::Component;
use crate::reactive::{Signal, SignalUpdate};

/// **What opens a surface**: a state to follow, or an action that opens it. See the module docs.
#[derive(Clone)]
pub enum Open {
    /// Follow this signal: up while it is `true`, written `false` when the surface closes itself.
    State(Signal<bool>),
    /// Up while this value reads `true`, followed into the surface's own state.
    Value(crate::builders::LiveValue<bool>),
    /// Open when this action runs.
    Action(String),
}

/// **What `.open(..)` takes** — a `Signal<bool>`, a value that changes (a function reading signals,
/// a `Memo<bool>`), or an action name (`&str` / `String`).
pub trait IntoOpen {
    fn into_open(self) -> Open;
}

impl IntoOpen for Signal<bool> {
    fn into_open(self) -> Open {
        Open::State(self)
    }
}

impl IntoOpen for &str {
    fn into_open(self) -> Open {
        Open::Action(self.to_string())
    }
}

impl IntoOpen for String {
    fn into_open(self) -> Open {
        Open::Action(self)
    }
}

impl IntoOpen for &String {
    fn into_open(self) -> Open {
        Open::Action(self.clone())
    }
}

impl<F: Fn() -> bool + 'static> IntoOpen for F {
    fn into_open(self) -> Open {
        Open::Value(crate::builders::LiveValue::live(self))
    }
}

impl IntoOpen for crate::reactive::Memo<bool> {
    fn into_open(self) -> Open {
        use crate::reactive::SignalGet;
        Open::Value(crate::builders::LiveValue::live(move || self.get()))
    }
}

impl IntoOpen for Open {
    fn into_open(self) -> Open {
        self
    }
}

impl Open {
    /// **The one rule every surface's `.open(..)` follows.** `own` is the surface's own state; the
    /// signal it should follow is returned to it — the caller's for a [`State`](Open::State), its
    /// own for an [`Action`](Open::Action) — and `surface` gets the action, when there is one, that
    /// sets it. Written once here so no surface can open by a name differently from another.
    pub(crate) fn apply<W: Component>(
        self,
        own: Signal<bool>,
        follow: impl FnOnce(W, Signal<bool>) -> W,
        surface: W,
    ) -> W {
        match self {
            Open::State(state) => follow(surface, state),
            Open::Value(value) => {
                value.show_in(own);
                follow(surface, own)
            }
            Open::Action(action) => {
                let mut surface = follow(surface, own);
                // Reachable while the surface is closed — that is when its opening action is needed.
                surface
                    .base_mut()
                    .actions
                    .push(crate::hint::DeclaredAction {
                        name: action,
                        run: Box::new(move || own.set(true)),
                        while_closed: true,
                    });
                surface
            }
        }
    }
}
