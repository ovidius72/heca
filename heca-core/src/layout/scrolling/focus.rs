//! Moving focus to the column beside the active one, and which of its panes it lands on.

use super::{ScrollingMut, ScrollingRef};
use crate::layout::types::ColumnFocus;

impl ScrollingMut<'_> {
    /// Focus left (previous column).
    pub fn focus_left(&mut self) -> bool {
        let Some(idx) = self.view.active_column.checked_sub(1) else {
            return false;
        };
        self.focus_column_beside(idx);
        true
    }

    /// Focus right (next column).
    pub fn focus_right(&mut self) -> bool {
        let idx = self.view.active_column + 1;
        if idx >= self.space.columns.len() {
            return false;
        }
        self.focus_column_beside(idx);
        true
    }

    /// Focus column `idx` beside the active one, landing on the pane the layout's
    /// [`ColumnFocus`] rule names: the one last used there, or the one level with the pane left.
    fn focus_column_beside(&mut self, idx: usize) {
        if self.space.options.column_focus == ColumnFocus::Row
            && let Some(row) = self.reader().row_level_with_active(idx)
            && let Some(column) = self.space.columns.get_mut(idx)
        {
            column.active_pane_idx = row;
        }
        self.activate_column(idx);
    }
}

impl ScrollingRef<'_> {
    /// The pane of column `idx` whose vertical span overlaps the active pane's the most (the upper
    /// one on a tie); the nearest one when none overlaps. `None` when there is no active pane.
    ///
    /// Read from where the panes are laid out in this window, so it agrees with what is drawn.
    fn row_level_with_active(&self, idx: usize) -> Option<usize> {
        let laid = self.laid_out_columns();
        let here = laid.get(self.view.active_column)?;
        let active_row = self.space.columns.get(self.view.active_column)?.active_pane_idx;
        let active = here.panes.get(active_row)?;
        let (top, bottom) = (active.slot.loc.y, active.slot.loc.y + active.slot.size.h);
        let mut best: Option<(usize, f64)> = None;
        for (row, pane) in laid.get(idx)?.panes.iter().enumerate() {
            let (y, end) = (pane.slot.loc.y, pane.slot.loc.y + pane.slot.size.h);
            // Overlap, or minus the distance between the spans when they do not touch.
            let overlap = bottom.min(end) - top.max(y);
            if best.is_none_or(|(_, o)| overlap > o) {
                best = Some((row, overlap));
            }
        }
        best.map(|(row, _)| row)
    }
}
