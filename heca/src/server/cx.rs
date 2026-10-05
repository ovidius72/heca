//! What a server handler is given: the shared content to change, and who asked.

use heca_core::layout::{LayoutMut, PaneId};

/// Plain data about the window that asked — what a server action needs to know that only the
/// window knows. The server never reads it from a window: with several windows (F012) each request
/// carries its own, and two windows asking at once are two different askers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Asker {
    /// The pane this window has focused, if any. Focus is per window.
    pub(crate) focused_pane: Option<PaneId>,
}

/// What a server handler runs against.
pub(crate) struct ServerCx<'a> {
    /// The session, and — **while standalone heca is one process** — the asking window's view of
    /// it, which travels with the call so a move shifts that window's scroll exactly as before.
    /// This is temporary: 2c makes the server return view effects and never touch a view.
    pub(crate) layout: LayoutMut<'a>,
    /// Who asked.
    pub(crate) asker: Asker,
}
