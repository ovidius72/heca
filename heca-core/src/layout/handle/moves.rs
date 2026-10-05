//! Moving a pane or a column to another place: another column, another workspace.
//!
//! Each move says where the thing landed, and which workspace — if any — it emptied and removed.
//! None of them changes which workspace a window shows beyond what removing one forces: showing the
//! new place is the window's decision.

use crate::layout::{ColumnId, LayoutMut, PaneId};

/// Where a moved pane or column landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moved {
    /// Its workspace, as numbered **after** any removal.
    pub workspace: usize,
    /// Its column.
    pub column: usize,
    /// The workspace the move left empty and removed, numbered as it was before the move.
    pub removed_workspace: Option<usize>,
}

impl LayoutMut<'_> {
    /// Remove workspace `idx` when nothing is left in it and another remains. Answers whether it
    /// was removed.
    pub fn remove_workspace_if_empty(&mut self, idx: usize) -> bool {
        let empty = self
            .session
            .workspaces
            .get(idx)
            .is_none_or(|ws| ws.scrolling.columns.iter().all(|c| c.panes.is_empty()));
        empty && self.session.workspaces.len() > 1 && self.remove_workspace(idx)
    }

    /// Move a pane into column `dst_col` of its own workspace; `dst_col` equal to the number of
    /// columns makes a new one at the end. `None` when nothing moves.
    pub fn move_pane_to_column(&mut self, pane: PaneId, dst_col: usize) -> Option<Moved> {
        let (ws_idx, src_col, row) = self.session.pane_location(pane)?;
        let mut ws = self.workspace_mut(ws_idx)?;
        let columns_before = ws.scrolling.columns.len();
        if src_col == dst_col || dst_col > columns_before {
            return None;
        }
        let moved = ws.scroll_mut().remove_pane(src_col, row)?;
        // A source column that held only this pane is gone, which shifts the ones after it.
        let columns_now = ws.scrolling.columns.len();
        let dst = match columns_now < columns_before && src_col < dst_col {
            true => dst_col - 1,
            false => dst_col,
        }
        .min(columns_now);
        match dst < columns_now {
            true => ws.scroll_mut().add_pane_to_column(dst, None, moved, true),
            false => {
                let id = ColumnId(self.session.next_id());
                let mut ws = self.workspace_mut(ws_idx)?;
                ws.scroll_mut().add_new_column(Some(dst), id, moved, true);
            }
        }
        Some(Moved { workspace: ws_idx, column: dst, removed_workspace: None })
    }

    /// Take a pane out of its column into a new one just right of it. `None` when it is already
    /// alone in its column, because that would change nothing.
    pub fn move_pane_to_new_column(&mut self, pane: PaneId) -> Option<Moved> {
        let (ws_idx, col, _) = self.session.pane_location(pane)?;
        let id = ColumnId(self.session.next_id());
        let moved = self
            .workspace_mut(ws_idx)?
            .scroll_mut()
            .extract_pane_to_new_column(pane, id);
        moved.then_some(Moved { workspace: ws_idx, column: col + 1, removed_workspace: None })
    }

    /// Move a pane to workspace `dst_ws`. With `join`, and a column at `dst_col`, it stacks into
    /// that column; otherwise it becomes a column of its own at `dst_col` (the end, when that is
    /// past the last). `None` when it is already in that workspace or not in a column.
    pub fn move_pane_to_workspace(
        &mut self,
        pane: PaneId,
        dst_ws: usize,
        dst_col: usize,
        join: bool,
    ) -> Option<Moved> {
        let (src_ws, col, row) = self.session.pane_location(pane)?;
        if src_ws == dst_ws || dst_ws >= self.session.workspaces.len() {
            return None;
        }
        let moved = self.workspace_mut(src_ws)?.scroll_mut().remove_pane(col, row)?;
        let column = {
            let id = ColumnId(self.session.next_id());
            let mut ws = self.workspace_mut(dst_ws)?;
            let count = ws.scrolling.columns.len();
            match join && dst_col < count {
                true => {
                    let end = ws.scrolling.columns[dst_col].panes.len();
                    ws.scroll_mut().add_pane_to_column(dst_col, Some(end), moved, true);
                    dst_col
                }
                false => {
                    let at = dst_col.min(count);
                    ws.scroll_mut().add_new_column(Some(at), id, moved, true);
                    at
                }
            }
        };
        Some(self.after_leaving(src_ws, dst_ws, column))
    }

    /// Move the column at `col` of workspace `src_ws` to the end of workspace `dst_ws`, active
    /// there. `None` when either is out of range or they are the same workspace.
    pub fn move_column_to_workspace(
        &mut self,
        src_ws: usize,
        col: usize,
        dst_ws: usize,
    ) -> Option<Moved> {
        if src_ws == dst_ws || dst_ws >= self.session.workspaces.len() {
            return None;
        }
        let column = self.workspace_mut(src_ws)?.scroll_mut().remove_column(col)?;
        let mut ws = self.workspace_mut(dst_ws)?;
        let at = ws.scrolling.columns.len();
        ws.scroll_mut().add_column(None, column, true);
        Some(self.after_leaving(src_ws, dst_ws, at))
    }

    /// Move the column at `(src_ws, src_col)` to index `dst_idx` of workspace `dst_ws`, which may
    /// be its own: that reorders. `activate` makes it the active column of its new place. `None`
    /// when an index is out of range.
    pub fn move_column(
        &mut self,
        src_ws: usize,
        src_col: usize,
        dst_ws: usize,
        dst_idx: usize,
        activate: bool,
    ) -> Option<Moved> {
        let in_range = |layout: &Self, ws: usize| ws < layout.session.workspaces.len();
        if !in_range(self, src_ws) || !in_range(self, dst_ws) {
            return None;
        }
        if src_col >= self.session.workspaces[src_ws].scrolling.columns.len() {
            return None;
        }
        if src_ws == dst_ws {
            self.workspace_mut(src_ws)?.scroll_mut().reorder_column(src_col, dst_idx);
            return Some(Moved { workspace: src_ws, column: dst_idx, removed_workspace: None });
        }
        let column = self.workspace_mut(src_ws)?.scroll_mut().remove_column(src_col)?;
        let removed = self.remove_workspace_if_empty(src_ws).then_some(src_ws);
        let dst_ws = match removed {
            Some(gone) if gone < dst_ws => dst_ws - 1,
            _ => dst_ws,
        };
        let mut ws = self.workspace_mut(dst_ws)?;
        let at = dst_idx.min(ws.scrolling.columns.len());
        ws.scroll_mut().add_column(Some(at), column, activate);
        Some(Moved { workspace: dst_ws, column: at, removed_workspace: removed })
    }

    /// Swap the columns at `(a_ws, a_col)` and `(b_ws, b_col)`. Within a workspace they trade
    /// places; across two they trade workspaces, so neither workspace empties. `false` when an
    /// index is out of range or both name the same column.
    pub fn swap_columns_between(
        &mut self,
        a_ws: usize,
        a_col: usize,
        b_ws: usize,
        b_col: usize,
    ) -> bool {
        let has = |layout: &Self, ws: usize, col: usize| {
            layout
                .session
                .workspaces
                .get(ws)
                .is_some_and(|w| col < w.scrolling.columns.len())
        };
        if !has(self, a_ws, a_col) || !has(self, b_ws, b_col) {
            return false;
        }
        if a_ws == b_ws {
            return a_col != b_col
                && self
                    .workspace_mut(a_ws)
                    .is_some_and(|mut ws| ws.scroll_mut().swap_columns(a_col, b_col));
        }
        let taken_a = self.workspace_mut(a_ws).and_then(|mut w| w.scroll_mut().remove_column(a_col));
        let taken_b = self.workspace_mut(b_ws).and_then(|mut w| w.scroll_mut().remove_column(b_col));
        let (Some(a), Some(b)) = (taken_a, taken_b) else {
            return false;
        };
        if let Some(mut ws) = self.workspace_mut(a_ws) {
            ws.scroll_mut().add_column(Some(a_col), b, false);
        }
        if let Some(mut ws) = self.workspace_mut(b_ws) {
            ws.scroll_mut().add_column(Some(b_col), a, false);
        }
        true
    }

    /// Tidy up after something left workspace `src_ws` for `dst_ws`, landing at `column`: remove
    /// the workspace it emptied, and number the destination as it is now.
    fn after_leaving(&mut self, src_ws: usize, dst_ws: usize, column: usize) -> Moved {
        let removed = self.remove_workspace_if_empty(src_ws).then_some(src_ws);
        let workspace = match removed {
            Some(gone) if gone < dst_ws => dst_ws - 1,
            _ => dst_ws,
        };
        Moved { workspace, column, removed_workspace: removed }
    }
}
