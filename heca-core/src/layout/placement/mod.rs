//! **Putting a pane at an exact place** — and taking it out of wherever it was.
//!
//! What a drop means once the gesture has said where it landed, and what the `place_pane` action
//! asks for. The workspace owns it so no caller removes and re-inserts panes by
//! hand, each deciding clamping and focus for itself.

use super::column::Pane;
use super::scrolling::SpaceEffect;
use super::types::*;
use super::workspace::{FocusDomain, Workspace};

impl Workspace {
    /// **The gaps where a new column for this pane would change the strip** — counted with the pane
    /// where it is. All of them, except the two beside a pane that is alone in its column: a column
    /// made there puts it back where it is.
    pub fn new_column_gaps(&self, pane_id: PaneId) -> Vec<usize> {
        let columns = self.scrolling.columns.len();
        let alone_at = self
            .scrolling
            .pane_indices(pane_id)
            .and_then(|(col, _)| (self.scrolling.columns[col].panes.len() == 1).then_some(col));
        (0..=columns)
            .filter(|gap| alone_at.is_none_or(|own| *gap != own && *gap != own + 1))
            .collect()
    }

    /// **The rows where putting this pane would change the strip**, as `(column, row)` — above
    /// each pane and below the last of every column, counted with the pane where it is, except the
    /// two beside the pane itself in its own column (they put it back where it is).
    pub fn new_row_places(&self, pane_id: PaneId) -> Vec<(usize, usize)> {
        let own = self.scrolling.pane_indices(pane_id);
        let mut out = Vec::new();
        for (col, column) in self.scrolling.columns.iter().enumerate() {
            for row in 0..=column.panes.len() {
                let beside = matches!(own, Some((c, r)) if c == col && (row == r || row == r + 1));
                if !beside {
                    out.push((col, row));
                }
            }
        }
        out
    }
}

impl Workspace {
    /// Take pane `pane_id` out of this workspace, wherever it is: its column, or the floating
    /// layer. What it did to the columns comes with it — nothing, for a floating pane. `None` when
    /// it is not here.
    pub fn take_pane(&mut self, pane_id: PaneId) -> Option<(Pane, Option<SpaceEffect>)> {
        if let Some((col, row)) = self.scrolling.pane_indices(pane_id) {
            return self
                .scrolling
                .take_pane_from(col, row)
                .map(|(pane, effect)| (pane, Some(effect)));
        }
        let idx = self
            .floating_panes
            .iter()
            .position(|f| f.pane.id == pane_id)?;
        let float = self.floating_panes.remove(idx);
        if self.floating_panes.is_empty() {
            self.focus_domain = FocusDomain::Tiled;
        }
        Some((float.pane, None))
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
    /// two gaps beside its own lone column, or its own lone column) changes nothing, and says so
    /// with no effects.
    ///
    /// `None` when the pane is not in this workspace.
    pub fn move_pane(
        &mut self,
        pane_id: PaneId,
        col: usize,
        row: Option<usize>,
        new_column_id: ColumnId,
        working_height: f64,
    ) -> Option<Vec<SpaceEffect>> {
        let (col, row) = match self.scrolling.pane_indices(pane_id) {
            Some((own_col, own_row)) => {
                let alone = self.scrolling.columns[own_col].panes.len() == 1;
                let col_after = if alone && own_col < col { col - 1 } else { col };
                match row {
                    // Into the column it is alone in: nothing to move.
                    Some(_) if alone && col == own_col => return Some(Vec::new()),
                    Some(row) if own_col == col && own_row < row => (col_after, Some(row - 1)),
                    row => (col_after, row),
                }
            }
            None => (col, row),
        };
        let (pane, taken) = self.take_pane(pane_id)?;
        let placed = self.place_pane(pane, col, row, new_column_id, working_height)?;
        Some(taken.into_iter().chain([placed]).collect())
    }

    /// Put `pane` at row `row` of column `col` — or, with no row (or no such column), in a new
    /// column `new_column_id` at `col` — making room for it in a window `working_height` tall.
    /// Indices past the end clamp to it. The pane is in the tiling now, so the floating layer has
    /// no focus.
    pub fn place_pane(
        &mut self,
        pane: Pane,
        col: usize,
        row: Option<usize>,
        new_column_id: ColumnId,
        working_height: f64,
    ) -> Option<SpaceEffect> {
        let columns = self.scrolling.columns.len();
        self.deactivate_floating_panes();
        self.focus_domain = FocusDomain::Tiled;
        match row {
            Some(row) if col < columns => self
                .scrolling
                .insert_pane(col, Some(row), pane, working_height)
                .map(SpaceEffect::Pane),
            _ => {
                let column = self.scrolling.new_column(new_column_id, pane);
                Some(SpaceEffect::Column(
                    self.scrolling.insert_column(col.min(columns), column),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests;
