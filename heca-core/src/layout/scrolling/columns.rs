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

        self.space.columns.insert(idx, column);

        if !was_empty && idx <= self.view.active_column {
            self.view.active_column += 1;
        }

        // Animate movement of other columns.
        let offset = self.reader().column_x(idx + 1) - self.reader().column_x(idx);
        if self.view.active_column <= idx {
            for col in &self.space.columns[idx + 1..] {
                self.view.motion.slide_column(col.id, -offset, AnimationConfig::default());
            }
        } else {
            for col in &self.space.columns[..idx] {
                self.view.motion.slide_column(col.id, offset, AnimationConfig::default());
            }
        }

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
    /// and a right-click cannot tell apart (F003/P082/T458).
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
        let col = &mut self.space.columns[col_idx];
        let idx = pane_idx.unwrap_or(col.panes.len());

        // **Room first.** A pane that is about to exist needs somewhere to be, and heights other
        // panes were dragged to are preferences that give way to that.
        col.make_room_for_one_more(self.view.area.size.h, self.space.options.gaps);
        col.add_pane_at(idx, pane);

        if activate {
            col.activate_pane(idx);
            if self.view.active_column != col_idx {
                self.activate_column(col_idx);
            }
        }

        // Recompute width since adding a pane may change it.

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

        // Animate movement of remaining columns.
        let offset = self.reader().column_x(idx + 1) - self.reader().column_x(idx);
        if self.view.active_column <= idx {
            for col in &self.space.columns[idx + 1..] {
                self.view.motion.slide_column(col.id, offset, AnimationConfig::default());
            }
        } else {
            for col in &self.space.columns[..idx] {
                self.view.motion.slide_column(col.id, -offset, AnimationConfig::default());
            }
        }

        let col = self.space.columns.remove(idx);

        if self.space.columns.is_empty() {
            self.view.active_column = 0;
            return Some(col);
        }

        if idx < self.view.active_column {
            self.view.active_column -= 1;
            self.view.activate_prev_on_removal = None;
        } else if idx == self.view.active_column {
            // Activate previous or next.
            let new_idx = if self.view.activate_prev_on_removal.is_some() && idx > 0 {
                idx - 1
            } else {
                idx.min(self.space.columns.len() - 1)
            };
            self.view.active_column = new_idx;
            self.view.activate_prev_on_removal = None;
            let offset = self.reader().compute_view_offset_for_column(new_idx, None);
            self.view.offset = ViewOffset::Static(offset);
        }

        Some(col)
    }

    /// Remove a pane from a column.
    pub fn remove_pane(&mut self, col_idx: usize, pane_idx: usize) -> Option<Pane> {
        if col_idx >= self.space.columns.len() {
            return None;
        }

        // If removing the last pane in a column, remove the whole column.
        if self.space.columns[col_idx].panes.len() == 1 {
            return self.remove_column(col_idx).map(|mut c| c.panes.remove(0));
        }

        let pane = self.space.columns[col_idx].remove_pane(pane_idx)?;

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
        if source_col == target_col {
            return false;
        }

        // Save old column positions and pane position before any changes.
        let old_xs: Vec<(ColumnId, f64)> = self
            .reader()
            .column_xs()
            .zip(self.space.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        let old_source_col_x = self.reader().column_x(source_col);
        let old_pane_y =
            self.reader().pane_y_in_column(source_col, self.space.columns[source_col].active_pane_idx);

        let pane_idx = self.space.columns[source_col].active_pane_idx;
        let pane = self.space.columns[source_col].remove_pane(pane_idx);
        let Some(pane) = pane else {
            return false;
        };

        // If source column became empty, remove it.
        if self.space.columns[source_col].is_empty() {
            self.remove_column(source_col);
            // Adjust target index if we removed a column before it.
            let target_col = if target_col > source_col {
                target_col - 1
            } else {
                target_col
            };
            let target = &mut self.space.columns[target_col];
            target.add_pane_at(target.panes.len(), pane);
            target.active_pane_idx = target.panes.len() - 1;
            self.view.active_column = target_col;
        } else {
            let target = &mut self.space.columns[target_col];
            target.add_pane_at(target.panes.len(), pane);
            target.active_pane_idx = target.panes.len() - 1;
            self.view.active_column = target_col;
        }


        // Animate the moved pane from its old visual position to its new one.
        let new_col_x = self.reader().column_x(self.view.active_column);
        let new_pane_y = self.reader().pane_y_in_column(
            self.view.active_column,
            self.space.columns[self.view.active_column].panes.len() - 1,
        );
        let offset_x = old_source_col_x - new_col_x;
        let offset_y = old_pane_y - new_pane_y;
        let moved_pane = self.space.columns[self.view.active_column]
            .panes
            .last()
            .unwrap();
        let moved_id = moved_pane.id;
        self.view.motion.slide_pane(moved_id, Point::new(offset_x, offset_y), AnimationConfig::default());

        // Animate all columns from their old positions.
        let new_xs: Vec<f64> = self.reader().column_xs().collect();
        for (i, col) in self.space.columns.iter().enumerate() {
            let old_x = old_xs
                .iter()
                .find(|(id, _)| *id == col.id)
                .map(|(_, x)| *x)
                .unwrap_or(new_xs[i]);
            let diff = old_x - new_xs[i];
            if diff.abs() > 0.5 {
                self.view.motion.slide_column(col.id, diff, AnimationConfig::default());
            }
        }

        // Animate view to bring the active column into view.
        self.align_view_to_active_column();
        true
    }

    /// Create a new column to the left or right of the current column
    /// with the active pane moved into it.
    fn move_active_pane_to_new_column(&mut self, dir: Direction, new_column_id: ColumnId) -> bool {
        let source_col = self.view.active_column;
        let pane_idx = self.space.columns[source_col].active_pane_idx;

        // Save old column positions and pane position before any changes.
        let old_xs: Vec<(ColumnId, f64)> = self
            .reader()
            .column_xs()
            .zip(self.space.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        let old_source_col_x = self.reader().column_x(source_col);
        let old_pane_y = self.reader().pane_y_in_column(source_col, pane_idx);

        let pane = self.space.columns[source_col].remove_pane(pane_idx);
        let Some(pane) = pane else {
            return false;
        };

        let new_col = Column::new(new_column_id, pane, ColumnWidth::Proportion(0.5));

        let insert_idx = match dir {
            Direction::Left => source_col,
            Direction::Right => source_col + 1,
        };

        // If source column became empty, remove it and adjust insert index.
        let removed_source = self.space.columns[source_col].is_empty();
        if removed_source {
            self.remove_column(source_col);
            let insert_idx = if dir == Direction::Right && insert_idx > source_col {
                insert_idx - 1
            } else {
                insert_idx
            };
            self.space.columns.insert(insert_idx, new_col);
            self.view.active_column = insert_idx;
        } else {
            self.space.columns.insert(insert_idx, new_col);
            self.view.active_column = insert_idx;
        }


        // Animate the moved pane from its old visual position to its new one.
        let new_col_x = self.reader().column_x(self.view.active_column);
        let new_pane_y = self.reader().pane_y_in_column(self.view.active_column, 0);
        let offset_x = old_source_col_x - new_col_x;
        let offset_y = old_pane_y - new_pane_y;
        let moved_pane = self.space.columns[self.view.active_column]
            .panes
            .first()
            .unwrap();
        let moved_id = moved_pane.id;
        self.view.motion.slide_pane(moved_id, Point::new(offset_x, offset_y), AnimationConfig::default());

        // Animate all columns from their old positions.
        let new_xs: Vec<f64> = self.reader().column_xs().collect();
        for (i, col) in self.space.columns.iter().enumerate() {
            let old_x = old_xs
                .iter()
                .find(|(id, _)| *id == col.id)
                .map(|(_, x)| *x)
                .unwrap_or(new_xs[i]);
            let diff = old_x - new_xs[i];
            if diff.abs() > 0.5 {
                self.view.motion.slide_column(col.id, diff, AnimationConfig::default());
            }
        }

        // Animate view to bring the new active column into view.
        self.align_view_to_active_column();
        true
    }

    fn move_column_to(&mut self, new_idx: usize) {
        self.reorder_column(self.view.active_column, new_idx);
    }

    /// Move the column at `from` to index `to` within this workspace, animating the
    /// shift and leaving the moved column **active**. `to` is clamped to the column
    /// range; no-op if `from` is out of range or `from == to`. Returns whether it
    /// moved. The general primitive behind keyboard left/right *and* DnD reorder (F4.5).
    pub fn reorder_column(&mut self, from: usize, to: usize) -> bool {
        if from >= self.space.columns.len() {
            return false;
        }
        let to = to.min(self.space.columns.len() - 1);
        if from == to {
            return false;
        }

        // Save old column positions by ID (for the shift animation).
        let old_xs: Vec<(ColumnId, f64)> = self
            .reader()
            .column_xs()
            .zip(self.space.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        let old_view_pos = self.reader().view_pos();

        // Remove from old position and insert at new position.
        let column = self.space.columns.remove(from);
        self.space.columns.insert(to, column);

        // The moved column stays active. Update the index BEFORE computing positions.
        self.view.active_column = to;

        // Preserve view position so the layout stays visually fixed.
        let new_view_pos = self.reader().view_pos();
        let delta = old_view_pos - new_view_pos;
        self.view.offset.offset(delta);

        self.animate_columns_from(&old_xs);
        true
    }

    /// Swap the columns at `a` and `b` within this workspace (positions only — each
    /// column keeps its panes), animating the shift. No-op if either index is out of
    /// range or `a == b`. Returns whether it swapped. (DnD column swap — F4.5.)
    pub fn swap_columns(&mut self, a: usize, b: usize) -> bool {
        let len = self.space.columns.len();
        if a >= len || b >= len || a == b {
            return false;
        }
        let old_xs: Vec<(ColumnId, f64)> = self
            .reader()
            .column_xs()
            .zip(self.space.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        self.space.columns.swap(a, b);
        self.animate_columns_from(&old_xs);
        true
    }

    /// Animate every column from its previous x (keyed by [`ColumnId`]) to its new laid-out x —
    /// shared by [`reorder_column`](Self::reorder_column) and [`swap_columns`](Self::swap_columns).
    fn animate_columns_from(&mut self, old_xs: &[(ColumnId, f64)]) {
        let new_xs: Vec<f64> = self.reader().column_xs().collect();
        for (i, col) in self.space.columns.iter().enumerate() {
            let old_x = old_xs
                .iter()
                .find(|(id, _)| *id == col.id)
                .map(|(_, x)| *x)
                .unwrap_or(new_xs[i]);
            let diff = old_x - new_xs[i];
            if diff.abs() > 0.5 {
                self.view.motion.slide_column(col.id, diff, AnimationConfig::default());
            }
        }
    }
}
