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

    /// **Take a pane out of its column and give it one of its own, immediately to the right.**
    ///
    /// Distinct from [`add_pane_to_column`](Self::add_pane_to_column), which puts a pane *into* a
    /// column that exists. The nearest existing move only creates a column when the target is past
    /// the end of the strip, so asked for a position between two columns it stacks into the
    /// existing one instead — this always creates, which is the whole act.
    ///
    /// **A pane already alone in its column is left where it is** and this answers `false`: it
    /// would leave a column of one and land in a column of one, so nothing on screen would change.
    ///
    /// `new_id` is passed in rather than derived: an id taken from the pane could name a column
    /// that already exists, and two columns with one id are two rows the cursor, the hint letters
    /// and a right-click cannot tell apart.
    pub fn extract_pane_to_new_column(&mut self, pane_id: PaneId, new_id: ColumnId) -> bool {
        let Some((src_col, pane_idx)) = self.space.pane_indices(pane_id) else {
            return false;
        };
        if self.space.columns[src_col].panes.len() <= 1 {
            return false;
        }
        let Some(pane) = self.remove_pane(src_col, pane_idx) else {
            return false;
        };
        self.add_new_column(Some(src_col + 1), new_id, pane, true);
        true
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

        let prev_next_x = self.reader().column_x(col_idx + 1);
        let height = self.view.area.size.h;
        let Some(PaneEffect::Inserted { row, .. }) =
            self.space.insert_pane(col_idx, pane_idx, pane, height)
        else {
            return;
        };

        if activate {
            self.space.columns[col_idx].activate_pane(row);
            if self.view.active_column != col_idx {
                self.activate_column(col_idx);
            }
        }

        // Animate column position changes.
        let offset = self.reader().column_x(col_idx + 1) - prev_next_x;
        if self.view.active_column <= col_idx {
            for c in &self.space.columns[col_idx + 1..] {
                self.view.motion.slide_column(c.id, -offset, AnimationConfig::default());
            }
        } else {
            for c in &self.space.columns[..=col_idx] {
                self.view.motion.slide_column(c.id, offset, AnimationConfig::default());
            }
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

        Some(col)
    }

    /// Remove a pane from a column. Removing its last pane removes the column.
    pub fn remove_pane(&mut self, col_idx: usize, pane_idx: usize) -> Option<Pane> {
        let before = self.reader().positions();
        let (pane, effect) = self.space.take_pane_from(col_idx, pane_idx)?;
        if let SpaceEffect::Column(effect) = effect {
            self.view.react(&*self.space, effect, &before);
        }
        Some(pane)
    }

    /// Focus left (previous column).
    pub fn focus_left(&mut self) -> bool {
        if self.view.active_column == 0 {
            return false;
        }
        self.activate_column(self.view.active_column - 1);
        true
    }

    /// Focus right (next column).
    pub fn focus_right(&mut self) -> bool {
        if self.view.active_column + 1 >= self.space.columns.len() {
            return false;
        }
        self.activate_column(self.view.active_column + 1);
        true
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

    /// Move the active pane to the previous column (left).
    /// If at first column and source has >1 pane, creates a new column to the left.
    /// If at first column and source has 1 pane, does nothing.
    pub fn move_active_pane_left(&mut self, new_column_id: ColumnId) -> bool {
        if self.view.active_column == 0 {
            // First column — create new column to the left if source has > 1 pane
            let source_has_multiple = self
                .columns
                .get(self.view.active_column)
                .map(|c| c.panes.len() > 1)
                .unwrap_or(false);
            if !source_has_multiple {
                return false;
            }
            return self.move_active_pane_to_new_column(Direction::Left, new_column_id);
        }
        let target_col = self.view.active_column - 1;
        self.move_active_pane_to_column(target_col)
    }

    /// Move the active pane to the next column (right).
    /// If at last column and source has >1 pane, creates a new column to the right.
    /// If at last column and source has 1 pane, does nothing.
    pub fn move_active_pane_right(&mut self, new_column_id: ColumnId) -> bool {
        if self.view.active_column + 1 >= self.space.columns.len() {
            // Last column — create new column to the right if source has > 1 pane
            let source_has_multiple = self
                .columns
                .get(self.view.active_column)
                .map(|c| c.panes.len() > 1)
                .unwrap_or(false);
            if !source_has_multiple {
                return false;
            }
            return self.move_active_pane_to_new_column(Direction::Right, new_column_id);
        }
        let target_col = self.view.active_column + 1;
        self.move_active_pane_to_column(target_col)
    }

    fn move_active_pane_to_column(&mut self, target_col: usize) -> bool {
        let source_col = self.view.active_column;
        let Some(row) = self.space.columns.get(source_col).map(|c| c.active_pane_idx) else {
            return false;
        };
        let before = self.reader().positions();
        let height = self.view.area.size.h;
        let Some(effects) = self.space.move_pane_between(source_col, row, target_col, height) else {
            return false;
        };
        self.show_pane_landed(&effects, &before)
    }

    /// Create a new column to the left or right of the current column
    /// with the active pane moved into it.
    fn move_active_pane_to_new_column(&mut self, dir: Direction, new_column_id: ColumnId) -> bool {
        let source_col = self.view.active_column;
        let Some(row) = self.space.columns.get(source_col).map(|c| c.active_pane_idx) else {
            return false;
        };
        let at = match dir {
            Direction::Left => source_col,
            Direction::Right => source_col + 1,
        };
        let before = self.reader().positions();
        let Some(effects) = self.space.extract_pane(source_col, row, new_column_id, at) else {
            return false;
        };
        self.show_pane_landed(&effects, &before)
    }

    /// Show the active pane's move to where `effects` say it landed: that column and pane become
    /// active, the pane and every column slide from where they were, and the view follows.
    fn show_pane_landed(&mut self, effects: &[SpaceEffect], before: &Positions) -> bool {
        let landed = effects.iter().rev().find_map(|effect| match *effect {
            SpaceEffect::Pane(PaneEffect::Inserted { col, row }) => Some((col, row)),
            SpaceEffect::Column(ColumnEffect::Inserted { idx }) => Some((idx, 0)),
            _ => None,
        });
        let Some((col, row)) = landed else {
            return false;
        };
        let Some(column) = self.space.columns.get_mut(col) else {
            return false;
        };
        let Some(moved) = column.panes.get(row).map(|pane| pane.id) else {
            return false;
        };
        column.active_pane_idx = row;
        self.view.active_column = col;
        self.view.slide_panes_from(&*self.space, &[moved], before);
        self.view.slide_to_new_positions(&*self.space, before);
        self.align_view_to_active_column();
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
