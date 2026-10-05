//! What a server handler is given: the shared content to change, and who asked.

use heca_core::layout::{Layout, PaneId, WorkspaceMut};

use super::{Change, ServerLayout};

/// Plain data about the window that asked — what a server action needs to know that only the
/// window knows. The server never reads it from a window: with several windows (F012) each request
/// carries its own, and two windows asking at once are two different askers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Asker {
    /// The pane this window has focused, if any. Focus is per window.
    pub(crate) focused_pane: Option<PaneId>,
    /// The workspace this window shows.
    pub(crate) workspace: usize,
    /// The column that is active in it.
    pub(crate) column: usize,
}

impl Asker {
    /// The asker for a window that sees the session as `layout` shows it, with `focused_pane`
    /// focused — the one place a window's place is turned into data.
    pub(crate) fn seen_through(layout: Layout<'_>, focused_pane: Option<PaneId>) -> Self {
        Self {
            focused_pane,
            workspace: layout.active_workspace_idx(),
            column: layout
                .active_workspace()
                .map_or(0, |ws| ws.scroll().active_column_idx()),
        }
    }
}

/// What a server handler runs against.
pub(crate) struct ServerCx<'a> {
    /// The session, by index and by id. **While standalone heca is one process** the asking
    /// window's view still travels inside it, so a move shifts that window's scroll exactly as
    /// before; 2c removes it.
    pub(crate) layout: ServerLayout<'a>,
    /// Who asked.
    pub(crate) asker: Asker,
}

impl ServerCx<'_> {
    /// Change the layout of the workspace the asker is in, and say the layout changed. Says nothing
    /// when there is no workspace — there was nothing to change.
    pub(crate) fn change_asker_workspace(
        &mut self,
        change: impl FnOnce(WorkspaceMut<'_>),
    ) -> Vec<Change> {
        match self.layout.workspace_mut(self.asker.workspace) {
            Some(ws) => {
                change(ws);
                vec![Change::LayoutChanged]
            }
            None => Vec::new(),
        }
    }
}
