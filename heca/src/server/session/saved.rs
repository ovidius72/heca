//! A session to and from the data the saved-sessions file holds.

use super::*;
use crate::server::persist::SavedSession;

impl SessionRuntime {
    /// **This session as the file keeps it** — what it is, what it holds and what ran in it now.
    /// A stopped session's terminals are the ones recorded when it stopped.
    pub(crate) fn saved(&self) -> SavedSession {
        SavedSession {
            id: self.identity.id,
            key: self.key,
            name: self.identity.name.clone(),
            folder: self.identity.folder.clone(),
            label: self.identity.label.clone(),
            status: self.identity.status,
            content: self.session.content(),
            focus: self.last_focus,
            view: self.last_view.clone(),
            terminals: self.recorded_terminals(),
        }
    }

    /// **Make this the session that was saved**: its key, name, folder and label, the terminals it
    /// had, and the last place it was looked at. The session was built from the saved content and is
    /// stopped; [`restart`](Self::restart) brings it back if it was running.
    pub(crate) fn adopt(&mut self, saved: SavedSession) {
        self.key = saved.key;
        self.identity = SessionInfo {
            id: saved.id,
            name: saved.name,
            folder: saved.folder,
            label: saved.label,
            status: SessionStatus::Terminated,
            windows: 0,
            agents: 0,
        };
        self.saved_terminals = saved.terminals;
        self.last_focus = saved.focus;
        self.last_view = saved.view;
    }
}

#[cfg(test)]
impl SessionRuntime {
    /// Shape the session directly — its layout and its terminals — as a test's starting point.
    pub(crate) fn arrange(&mut self, shape: impl FnOnce(&mut heca_core::layout::Session, &mut ServerState)) {
        shape(&mut self.session, &mut self.state);
    }
}
