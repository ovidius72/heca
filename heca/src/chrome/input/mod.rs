//! **Input** — how a gesture on the chrome becomes something the app does: pointer dispatch into
//! the trees, the intent sink, drags, and the names that keys, picks and drops address things by.

pub(super) mod dispatch;
pub(crate) mod drag;
pub(super) mod emitter;
pub(super) mod identity;
pub(super) mod names;
