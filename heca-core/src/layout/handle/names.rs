//! Naming a pane, a column or a workspace. An empty name clears the custom one, and the thing goes
//! back to its default label (a pane to its program, a workspace to `Workspace N`).

use crate::layout::{LayoutMut, PaneId};

/// A name that is there: `None` for an empty one.
fn custom(name: &str) -> Option<String> {
    (!name.is_empty()).then(|| name.to_string())
}

impl LayoutMut<'_> {
    /// Name a pane, wherever it is. `false` when it is nowhere.
    pub fn rename_pane(&mut self, pane: PaneId, name: &str) -> bool {
        let Some(idx) = self.session.workspace_holding(pane) else {
            return false;
        };
        let Some(pane) = self.session.workspaces[idx].find_pane_mut(pane) else {
            return false;
        };
        pane.custom_name = custom(name);
        true
    }

    /// Name workspace `ws`. `false` when there is none.
    pub fn rename_workspace(&mut self, ws: usize, name: &str) -> bool {
        let Some(workspace) = self.session.workspaces.get_mut(ws) else {
            return false;
        };
        workspace.name = custom(name);
        true
    }

    /// Name column `col` of workspace `ws`. `false` when there is none.
    pub fn rename_column(&mut self, ws: usize, col: usize, name: &str) -> bool {
        let Some(column) = self
            .session
            .workspaces
            .get_mut(ws)
            .and_then(|w| w.scrolling.columns.get_mut(col))
        else {
            return false;
        };
        column.name = custom(name);
        true
    }
}
