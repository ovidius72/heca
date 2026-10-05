//! Taking things out of the layout for good: a pane, a column with its panes, a workspace with
//! everything in it. Each says which panes went, so whatever ran in them can be ended.

use crate::layout::{LayoutMut, PaneId};

/// What a removal took out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removed {
    /// The panes that are gone.
    pub panes: Vec<PaneId>,
    /// The workspace that went with them, numbered as it was before.
    pub removed_workspace: Option<usize>,
}

impl LayoutMut<'_> {
    /// Remove a pane — tiled or floating, in whichever workspace it is. A workspace it leaves with
    /// nothing in it goes too, when another remains. `None` when the pane is nowhere.
    pub fn remove_pane_anywhere(&mut self, pane: PaneId) -> Option<Removed> {
        let ws_idx = self.session.workspace_holding(pane)?;
        let mut ws = self.workspace_mut(ws_idx)?;
        let taken = ws.take_pane(pane)?;
        let removed_workspace = (!ws.has_panes() && self.remove_workspace_if_empty(ws_idx)).then_some(ws_idx);
        Some(Removed { panes: vec![taken.id], removed_workspace })
    }

    /// Remove column `col` of workspace `ws_idx` with its panes; `col` past the last means the
    /// last. The workspace stays, empty or not. `None` when there is no such workspace or column.
    pub fn remove_column_with_panes(&mut self, ws_idx: usize, col: usize) -> Option<Removed> {
        let mut ws = self.workspace_mut(ws_idx)?;
        let last = ws.scrolling.columns.len().checked_sub(1)?;
        let column = ws.scroll_mut().remove_column(col.min(last))?;
        let panes = column.panes.iter().map(|p| p.id).collect();
        Some(Removed { panes, removed_workspace: None })
    }

    /// Remove workspace `ws_idx` with everything in it, the last one too — the session is then
    /// empty until a workspace is made. `None` when there is no such workspace.
    pub fn remove_workspace_with_panes(&mut self, ws_idx: usize) -> Option<Removed> {
        let ws = self.session.workspaces.get(ws_idx)?;
        let mut panes: Vec<PaneId> = ws
            .scrolling
            .columns
            .iter()
            .flat_map(|c| c.panes.iter().map(|p| p.id))
            .collect();
        panes.extend(ws.floating_panes.iter().map(|f| f.pane.id));
        self.remove_workspace(ws_idx).then_some(Removed {
            panes,
            removed_workspace: Some(ws_idx),
        })
    }
}
