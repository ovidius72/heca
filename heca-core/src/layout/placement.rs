//! **Putting a pane at an exact place** — and taking it out of wherever it was.
//!
//! What a drop means once the gesture has said where it landed, and what the `place_pane` action
//! asks for (F003/P082/T509). The workspace owns it so no caller removes and re-inserts panes by
//! hand, each deciding clamping and focus for itself.

use super::column::Pane;
use super::types::*;
use super::workspace::{FocusDomain, Workspace};

impl Workspace {
    /// Take pane `pane_id` out of this workspace, wherever it is: its column, or the floating
    /// layer. `None` when it is not here.
    pub fn take_pane(&mut self, pane_id: PaneId) -> Option<Pane> {
        if let Some((col, row)) = self.scrolling.pane_indices(pane_id) {
            return self.scrolling.remove_pane(col, row);
        }
        let idx = self
            .floating_panes
            .iter()
            .position(|f| f.pane.id == pane_id)?;
        let float = self.floating_panes.remove(idx);
        if self.floating_panes.is_empty() {
            self.focus_domain = FocusDomain::Tiled;
        }
        Some(float.pane)
    }

    /// **Move a pane of this workspace to a place counted as the workspace is *now*, with the pane
    /// still where it is** — row `row` of column `col`, or with no row a new column at gap `col`
    /// (gap `i` is the one left of column `i`; gap `columns` is the end). Past the end clamps to it.
    ///
    /// A caller names what it sees: the gap it drew, the row it landed on. Taking the pane out
    /// first changes those numbers — a pane alone in its column takes the column with it, so every
    /// place to its right is one nearer, and a pane above the target row in the same column leaves it
    /// one row higher — and that is this layout's own business, so it is resolved here and nowhere a
    /// drop, a key or a plugin has to know. A place that would put the pane back where it is (the
    /// two gaps beside its own lone column, or its own lone column) changes nothing.
    ///
    /// `false` when the pane is not in this workspace.
    pub fn move_pane(
        &mut self,
        pane_id: PaneId,
        col: usize,
        row: Option<usize>,
        new_column_id: ColumnId,
    ) -> bool {
        let (col, row) = match self.scrolling.pane_indices(pane_id) {
            Some((own_col, own_row)) => {
                let alone = self.scrolling.columns[own_col].panes.len() == 1;
                let col_after = if alone && own_col < col { col - 1 } else { col };
                match row {
                    // Into the column it is alone in: nothing to move.
                    Some(_) if alone && col == own_col => return true,
                    Some(row) if own_col == col && own_row < row => (col_after, Some(row - 1)),
                    row => (col_after, row),
                }
            }
            None => (col, row),
        };
        let Some(pane) = self.take_pane(pane_id) else {
            return false;
        };
        self.place_pane(pane, col, row, new_column_id);
        true
    }

    /// **The gaps where a new column for this pane would change the strip** — counted with the pane
    /// where it is. All of them, except the two beside a pane that is alone in its column: a column
    /// made there puts it back where it is.
    pub fn new_column_gaps(&self, pane_id: PaneId) -> Vec<usize> {
        let columns = self.scrolling.columns.len();
        let alone_at = self.scrolling.pane_indices(pane_id).and_then(|(col, _)| {
            (self.scrolling.columns[col].panes.len() == 1).then_some(col)
        });
        (0..=columns)
            .filter(|gap| alone_at.is_none_or(|own| *gap != own && *gap != own + 1))
            .collect()
    }

    /// Put `pane` at row `row` of column `col` — or, with no row (or no such column), in a new
    /// column `new_column_id` at `col`. Indices past the end clamp to it. The pane is focused, in
    /// the tiling.
    pub fn place_pane(
        &mut self,
        pane: Pane,
        col: usize,
        row: Option<usize>,
        new_column_id: ColumnId,
    ) {
        let columns = self.scrolling.columns.len();
        match row {
            Some(row) if col < columns => {
                let row = row.min(self.scrolling.columns[col].panes.len());
                self.scrolling
                    .add_pane_to_column(col, Some(row), pane, true);
            }
            _ => {
                let column = self.scrolling.new_column(new_column_id, pane);
                self.scrolling
                    .add_column(Some(col.min(columns)), column, true);
            }
        }
        self.deactivate_floating_panes();
        self.focus_domain = FocusDomain::Tiled;
    }
}

#[cfg(test)]
mod tests;
