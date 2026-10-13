//! **The layout as the server may use it** — the shared content, by index and by id, and nothing
//! that depends on what a window is showing.
//!
//! A window shows one workspace, has one column active in it and is scrolled somewhere; the server
//! must not ask. It is told where its asker is ([`Asker`](super::Asker)) and addresses everything
//! else by position or id. This type holds the session and no window's view, so "the server does
//! not read or move the view" is a fact the compiler checks: there is no view here to reach.

use std::ops::{Deref, DerefMut};

use heca_core::layout::{ScrollingSpace, Session};

/// The session, moved by index and by id.
pub(crate) struct ServerLayout<'a>(&'a mut Session);

impl<'a> ServerLayout<'a> {
    pub(crate) fn new(session: &'a mut Session) -> Self {
        Self(session)
    }

    /// Workspace `idx`'s columns, to change. A change made here is shown to a window by the fact
    /// it reports.
    pub(crate) fn columns_mut(&mut self, idx: usize) -> Option<&mut ScrollingSpace> {
        self.0.workspaces.get_mut(idx).map(|ws| &mut ws.scrolling)
    }
}

impl Deref for ServerLayout<'_> {
    type Target = Session;

    fn deref(&self) -> &Session {
        self.0
    }
}

impl DerefMut for ServerLayout<'_> {
    fn deref_mut(&mut self) -> &mut Session {
        self.0
    }
}
