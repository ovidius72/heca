//! Adding, removing, moving and reordering columns and the panes in them.

use super::*;

impl ScrollingMut<'_> {
    /// Add a column holding `pane`, at the layout's default width — [`new_column`] then
    /// [`add_column`](Self::add_column), as one act. A caller that built the column by hand
    /// would borrow the space twice and decide the width for itself.
    ///
    /// [`new_column`]: ScrollingSpace::new_column
    pub fn add_new_column(&mut self, idx: Option<usize>, id: ColumnId, pane: Pane, activate: bool) {
        let column = self.space.new_column(id, pane);
        self.add_column(idx, column, activate);
    }

    /// Add a column at a specific index (None = after active column).
    pub fn add_column(&mut self, idx: Option<usize>, column: Column, activate: bool) {
        let was_empty = self.space.columns.is_empty();
        let idx = idx.unwrap_or_else(|| {
            if was_empty {
                0
            } else {
                self.view.active_column + 1
            }
        });

        let before = self.reader().positions();
        let effect = self.space.insert_column(idx, column);
        self.view.react(&*self.space, effect, &before);

        if activate {
            if was_empty {
                self.view.offset = ViewOffset::Static(0.0);
                let fit_offset = self.reader().compute_view_offset_for_column(idx, None);
                self.view.offset = ViewOffset::Static(fit_offset);
            }

            let prev_offset = if !was_empty && idx == self.view.active_column + 1 {
                Some(self.view.offset.stationary())
            } else {
                None
            };

            self.activate_column(idx);
            self.view.activate_prev_on_removal = prev_offset;
        }
    }

    /// Add a pane to a column.
    pub fn add_pane_to_column(
        &mut self,
        col_idx: usize,
        pane_idx: Option<usize>,
        pane: Pane,
        activate: bool,
    ) {
        if col_idx >= self.space.columns.len() {
            return;
        }

        let before = self.reader().positions();
        let height = self.view.area.size.h;
        let Some(effect) = self.space.insert_pane(col_idx, pane_idx, pane, height) else {
            return;
        };
        let PaneEffect::Inserted { row, .. } = effect else {
            return;
        };
        if activate {
            self.focus_pane_in(col_idx, row);
        }
        self.show(&[SpaceEffect::Pane(effect)], &before);
        if activate {
            self.activate_column(col_idx);
        }
    }

    /// Remove a column by index.
    pub fn remove_column(&mut self, idx: usize) -> Option<Column> {
        if idx >= self.space.columns.len() {
            return None;
        }

        let before = self.reader().positions();
        let (col, effect) = self.space.take_column(idx)?;
        self.view.react(&*self.space, effect, &before);
        self.prune_column_focus();
        Some(col)
    }

    /// Remove a pane from a column. Removing its last pane removes the column.
    pub fn remove_pane(&mut self, col_idx: usize, pane_idx: usize) -> Option<Pane> {
        let before = self.reader().positions();
        let (pane, effect) = self.space.take_pane_from(col_idx, pane_idx)?;
        self.show(&[effect], &before);
        // If this window had the removed pane focused, land its focus on the one that took its
        // place (or the new last). A window focused on another pane, or on none, is left as it is.
        let landing = self.space.columns.get(col_idx).and_then(|col| {
            let gone = self
                .view
                .focused_pane(col.id)
                .is_some_and(|id| !col.panes.iter().any(|p| p.id == id));
            gone.then(|| pane_idx.min(col.panes.len().saturating_sub(1)))
        });
        if let Some(row) = landing {
            self.focus_pane_in(col_idx, row);
        }
        self.prune_column_focus();
        Some(pane)
    }

    /// Drop this window's focus memory for columns the content no longer has.
    fn prune_column_focus(&mut self) {
        let space = &*self.space;
        self.view
            .retain_columns(|id| space.columns.iter().any(|c| c.id == id));
    }

    /// Move the active column left.
    pub fn move_column_left(&mut self) -> bool {
        if self.view.active_column == 0 {
            return false;
        }
        self.move_column_to(self.view.active_column - 1);
        true
    }

    /// Move the active column right.
    pub fn move_column_right(&mut self) -> bool {
        let new_idx = self.view.active_column + 1;
        if new_idx >= self.space.columns.len() {
            return false;
        }
        self.move_column_to(new_idx);
        true
    }

    /// Move the active pane to the previous column (left). At the first column, when the pane is
    /// not alone in its column, it goes into a new column to the left; alone, nothing happens.
    pub fn move_active_pane_left(&mut self, new_column_id: ColumnId) -> bool {
        self.move_active_pane_along(Direction::Left, new_column_id)
    }

    /// Move the active pane to the next column (right). At the last column, when the pane is not
    /// alone in its column, it goes into a new column to the right; alone, nothing happens.
    pub fn move_active_pane_right(&mut self, new_column_id: ColumnId) -> bool {
        self.move_active_pane_along(Direction::Right, new_column_id)
    }

    fn move_active_pane_along(&mut self, dir: Direction, new_column_id: ColumnId) -> bool {
        let col = self.view.active_column;
        let Some(row) = self.reader().active_pane_row(col) else {
            return false;
        };
        let before = self.reader().positions();
        let height = self.view.area.size.h;
        let Some(effects) = self.space.move_pane_along(col, row, dir, new_column_id, height) else {
            return false;
        };
        self.show_pane_landed(&effects, &before)
    }

    /// Show the active pane's move to where `effects` say it landed, and follow it there: that
    /// column and pane become the active ones.
    fn show_pane_landed(&mut self, effects: &[SpaceEffect], before: &Positions) -> bool {
        let Some((col, row)) = SpaceEffect::landing(effects) else {
            return false;
        };
        if self.space.columns.get(col).is_none() {
            return false;
        }
        self.focus_pane_in(col, row);
        self.show(effects, before);
        self.view.follow(&*self.space, col);
        true
    }

    fn move_column_to(&mut self, new_idx: usize) {
        self.reorder_column(self.view.active_column, new_idx);
    }

    /// Move the column at `from` to index `to` within this workspace, animating the
    /// shift and leaving the moved column **active**. `to` is clamped to the column
    /// range; no-op if `from` is out of range or `from == to`. Returns whether it
    /// moved. The one move behind keyboard left/right and a dragged reorder.
    pub fn reorder_column(&mut self, from: usize, to: usize) -> bool {
        if from >= self.space.columns.len() {
            return false;
        }
        let to = to.min(self.space.columns.len() - 1);
        if from == to {
            return false;
        }

        let before = self.reader().positions();
        let Some(effect) = self.space.move_column(from, to) else {
            return false;
        };
        self.view.react(&*self.space, effect, &before);
        true
    }

    /// Swap the columns at `a` and `b` within this workspace (positions only — each
    /// column keeps its panes), animating the shift. No-op if either index is out of
    /// range or `a == b`. Returns whether it swapped.
    pub fn swap_columns(&mut self, a: usize, b: usize) -> bool {
        let before = self.reader().positions();
        let Some(effect) = self.space.swap_columns(a, b) else {
            return false;
        };
        self.view.react(&*self.space, effect, &before);
        true
    }
}
