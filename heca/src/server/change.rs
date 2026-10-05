//! **What the server tells a window changed** — plain data a window drains and reacts to.
//!
//! The server never reaches into a window. It reports what happened to the state it owns, and the
//! window does what only a window can: redraw, rebuild what it drew, re-flow. A [`Change`] says
//! *what* changed, never what to do about it — the same rule events follow everywhere else.

/// Something that changed in the state every window shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    /// The notifications on show, or when they expire, are different.
    NotificationsChanged,
    /// The arrangement of panes or columns is different: a column was resized, a pane was moved.
    LayoutChanged,
}
