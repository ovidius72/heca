use super::types::*;
use super::workspace::Workspace;

/// A session: **the content every window shares** — which workspaces exist, and the columns and
/// panes in them.
///
/// This is the top-level layout container — NIRI's `Layout<W>` (monitors + workspaces). What a
/// window sees of it — the workspace shown, the switch animation, the scroll, the size — is a
/// [`WindowView`](super::WindowView); anything that needs both goes through
/// [`Layout`](super::Layout) / [`LayoutMut`](super::LayoutMut).
///
/// **It has no overview state.** heca's overview is the exposé, a *layer* built out of widgets
/// (`heca/src/chrome/expose/`) that draws its own map and never asks the session to zoom. The
/// pre-layer overview that lived here — `OverviewState`, `toggle_overview`, `overview_zoom`,
/// `overview_workspace_geometries` — was unreachable code by the time it was removed: nothing
/// outside this file ever set it active (F003/P082/T422).
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub workspaces: Vec<Workspace>,
    /// Layout options.
    pub options: LayoutOptions,
    /// Next ID counter.
    next_id: u64,
}

impl Session {
    /// A session with one empty workspace.
    pub fn new(id: SessionId, options: LayoutOptions) -> Self {
        let mut session = Self {
            id,
            workspaces: Vec::new(),
            options,
            next_id: 1,
        };

        // Create initial workspace.
        session.add_workspace();

        session
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Where a tiled pane is: its workspace, column and row. `None` for a pane that is not in a
    /// column (floating, or not here at all).
    pub fn pane_location(&self, pane: PaneId) -> Option<(usize, usize, usize)> {
        self.workspaces.iter().enumerate().find_map(|(ws, workspace)| {
            workspace
                .scrolling
                .pane_indices(pane)
                .map(|(col, row)| (ws, col, row))
        })
    }

    /// Create a new workspace and append it.
    pub fn add_workspace(&mut self) -> WorkspaceId {
        let id = WorkspaceId(self.next_id());
        self.workspaces
            .push(Workspace::new(id, self.options.clone()));
        id
    }
}
