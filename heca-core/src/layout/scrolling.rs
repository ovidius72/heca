use super::animation::{Animation, AnimationConfig};
use super::column::{Column, Pane};
use super::types::*;
use super::view_offset::{compute_new_view_offset, ViewOffset};

// Re-export InsertPosition for convenience.
pub use super::types::InsertPosition;

/// Direction for creating a new column when moving a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Left,
    Right,
}

/// A scrollable horizontal space for columns of panes.
///
/// This is heca's equivalent of NIRI's `ScrollingSpace<W>`.
/// Columns are arranged left-to-right with gaps, and the view scrolls horizontally
/// to bring the active column into view.
#[derive(Debug, Clone)]
pub struct ScrollingSpace {
    /// Columns of panes.
    pub columns: Vec<Column>,
    /// Cached per-column data (computed widths).
    pub column_widths: Vec<f64>,
    /// Index of the currently active column.
    pub active_column_idx: usize,
    /// Horizontal scroll offset.
    pub view_offset: ViewOffset,
    /// Whether to activate the previous column on removal.
    pub activate_prev_on_removal: Option<f64>,
    /// Working area for layout computations.
    pub working_area: Rectangle,
    /// Current scale factor.
    pub scale: f64,
    /// Layout options.
    pub options: LayoutOptions,
}

impl ScrollingSpace {
    pub fn new(working_area: Rectangle, scale: f64, options: LayoutOptions) -> Self {
        Self {
            columns: Vec::new(),
            column_widths: Vec::new(),
            active_column_idx: 0,
            view_offset: ViewOffset::Static(0.0),
            activate_prev_on_removal: None,
            working_area,
            scale,
            options,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn active_column(&self) -> Option<&Column> {
        self.columns.get(self.active_column_idx)
    }

    pub fn active_column_mut(&mut self) -> Option<&mut Column> {
        self.columns.get_mut(self.active_column_idx)
    }

    pub fn active_pane(&self) -> Option<&Pane> {
        self.active_column().and_then(|c| c.active_pane())
    }

    /// X position of each column (cumulative, starting at 0).
    fn column_xs(&self) -> impl Iterator<Item = f64> + '_ {
        let gaps = self.options.gaps;
        let mut x = 0.0;
        let widths = self.column_widths.iter().copied().chain(std::iter::once(0.0));
        widths.map(move |width| {
            let rv = x;
            x += width + gaps;
            rv
        })
    }

    /// X position of a specific column.
    pub fn column_x(&self, idx: usize) -> f64 {
        self.column_xs().nth(idx).unwrap_or(0.0)
    }

    /// Compute the Y offset of a pane within a column.
    pub fn pane_y_in_column(&self, col_idx: usize, pane_idx: usize) -> f64 {
        let col = &self.columns[col_idx];
        let gaps = self.options.gaps;
        let mut y = gaps;
        for i in 0..pane_idx.min(col.pane_sizes.len()) {
            y += col.pane_sizes[i].h + gaps;
        }
        y
    }

    /// Current view position (column_x + view_offset).
    pub fn view_pos(&self) -> f64 {
        if self.columns.is_empty() {
            return 0.0;
        }
        self.column_x(self.active_column_idx) + self.view_offset.current()
    }

    /// Target view position.
    pub fn target_view_pos(&self) -> f64 {
        if self.columns.is_empty() {
            return 0.0;
        }
        self.column_x(self.active_column_idx) + self.view_offset.target()
    }

    /// Compute the view offset to fit a column into view.
    pub fn compute_view_offset_for_column(&self, idx: usize, prev_idx: Option<usize>) -> f64 {
        if self.is_centering_focused_column() {
            return self.compute_view_offset_centered(idx);
        }

        match self.options.center_focused_column {
            CenterFocusedColumn::Always => self.compute_view_offset_centered(idx),
            CenterFocusedColumn::OnOverflow => {
                let Some(prev_idx) = prev_idx else {
                    return self.compute_view_offset_fit(idx);
                };
                if prev_idx == idx {
                    return self.compute_view_offset_fit(idx);
                }

                // Check if both columns fit together.
                let source_idx = if prev_idx > idx {
                    (idx + 1).min(self.columns.len() - 1)
                } else {
                    idx.saturating_sub(1)
                };
                let source_x = self.column_x(source_idx);
                let source_w = self.column_widths.get(source_idx).copied().unwrap_or(0.0);
                let target_x = self.column_x(idx);
                let target_w = self.column_widths.get(idx).copied().unwrap_or(0.0);

                let total_width = if source_x < target_x {
                    target_x - source_x + target_w
                } else {
                    source_x - target_x + source_w
                } + self.options.gaps * 2.0;

                if total_width <= self.working_area.size.w {
                    self.compute_view_offset_fit(idx)
                } else {
                    self.compute_view_offset_centered(idx)
                }
            }
            CenterFocusedColumn::Never => self.compute_view_offset_fit(idx),
        }
    }

    fn compute_view_offset_fit(&self, idx: usize) -> f64 {
        let col_x = self.column_x(idx);
        let col_w = self.column_widths.get(idx).copied().unwrap_or(0.0);
        let mode = self.columns.get(idx).map(|c| c.sizing_mode()).unwrap_or(SizingMode::Normal);

        if mode.is_fullscreen() {
            return 0.0;
        }

        let (area, padding) = if mode.is_maximized() {
            // Maximzed uses full viewport, no padding
            (self.working_area, 0.0)
        } else {
            (self.working_area, self.options.gaps)
        };

        compute_new_view_offset(
            self.target_view_pos() + area.loc.x,
            area.size.w,
            col_x,
            col_w,
            padding,
        ) - area.loc.x
    }

    fn compute_view_offset_centered(&self, idx: usize) -> f64 {
        let col_w = self.column_widths.get(idx).copied().unwrap_or(0.0);
        let mode = self.columns.get(idx).map(|c| c.sizing_mode()).unwrap_or(SizingMode::Normal);

        if mode.is_fullscreen() {
            return self.compute_view_offset_fit(idx);
        }

        let area = self.working_area;

        // Columns wider than view are left-aligned.
        if area.size.w <= col_w {
            return self.compute_view_offset_fit(idx);
        }

        -(area.size.w - col_w) / 2.0 - area.loc.x
    }

    fn is_centering_focused_column(&self) -> bool {
        self.options.center_focused_column == CenterFocusedColumn::Always
            || (self.options.always_center_single_column && self.columns.len() <= 1)
    }

    /// Activate a column and animate the view to bring it into view.
    /// NIRI behavior: focus changes scroll the viewport.
    pub fn activate_column(&mut self, idx: usize) {
        if idx >= self.columns.len() || self.active_column_idx == idx {
            return;
        }

        let prev_idx = self.active_column_idx;
        let new_offset = self.compute_view_offset_for_column(idx, Some(prev_idx));

        // Offset the view to account for column position change.
        let new_col_x = self.column_x(idx);
        let old_col_x = self.column_x(prev_idx);
        self.view_offset.offset(old_col_x - new_col_x);

        // Animate to new view offset.
        let pixel = 1.0 / self.scale;
        let to_diff = new_offset - self.view_offset.target();
        if to_diff.abs() < pixel {
            self.view_offset.offset(to_diff);
        } else {
            self.view_offset = ViewOffset::Animation(Animation::new(
                self.view_offset.current(),
                new_offset,
                AnimationConfig::default(),
            ));
        }

        self.active_column_idx = idx;
        self.activate_prev_on_removal = None;
    }

    /// Add a column at a specific index (None = after active column).
    pub fn add_column(&mut self, idx: Option<usize>, mut column: Column, activate: bool) {
        let was_empty = self.columns.is_empty();
        let idx = idx.unwrap_or_else(|| {
            if was_empty { 0 } else { self.active_column_idx + 1 }
        });

        // Compute column width.
        column.computed_width = column.resolve_width(self.working_area.size.w, self.options.gaps);
        column.compute_pane_sizes(self.working_area.size.h, self.options.gaps);

        self.column_widths.insert(idx, column.computed_width);
        self.columns.insert(idx, column);

        if !was_empty && idx <= self.active_column_idx {
            self.active_column_idx += 1;
        }

        // Animate movement of other columns.
        let offset = self.column_x(idx + 1) - self.column_x(idx);
        if self.active_column_idx <= idx {
            for col in &mut self.columns[idx + 1..] {
                col.animate_move_from(-offset, AnimationConfig::default());
            }
        } else {
            for col in &mut self.columns[..idx] {
                col.animate_move_from(offset, AnimationConfig::default());
            }
        }

        if activate {
            if was_empty {
                self.view_offset = ViewOffset::Static(0.0);
                let fit_offset = self.compute_view_offset_for_column(idx, None);
                self.view_offset = ViewOffset::Static(fit_offset);
            }

            let prev_offset = if !was_empty && idx == self.active_column_idx + 1 {
                Some(self.view_offset.stationary())
            } else {
                None
            };

            self.activate_column(idx);
            self.activate_prev_on_removal = prev_offset;
        }

        self.update_all_column_widths();
    }

    /// Add a pane to a column.
    pub fn add_pane_to_column(
        &mut self,
        col_idx: usize,
        pane_idx: Option<usize>,
        pane: Pane,
        activate: bool,
    ) {
        if col_idx >= self.columns.len() {
            return;
        }

        let prev_next_x = self.column_x(col_idx + 1);
        let col = &mut self.columns[col_idx];
        let idx = pane_idx.unwrap_or(col.panes.len());

        col.add_pane_at(idx, pane);

        if activate {
            col.activate_pane(idx);
            if self.active_column_idx != col_idx {
                self.activate_column(col_idx);
            }
        }

        // Recompute width since adding a pane may change it.
        self.update_all_column_widths();

        // Animate column position changes.
        let offset = self.column_x(col_idx + 1) - prev_next_x;
        if self.active_column_idx <= col_idx {
            for c in &mut self.columns[col_idx + 1..] {
                c.animate_move_from(-offset, AnimationConfig::default());
            }
        } else {
            for c in &mut self.columns[..=col_idx] {
                c.animate_move_from(offset, AnimationConfig::default());
            }
        }
    }

    /// Remove a column by index.
    pub fn remove_column(&mut self, idx: usize) -> Option<Column> {
        if idx >= self.columns.len() {
            return None;
        }

        // Animate movement of remaining columns.
        let offset = self.column_x(idx + 1) - self.column_x(idx);
        if self.active_column_idx <= idx {
            for col in &mut self.columns[idx + 1..] {
                col.animate_move_from(offset, AnimationConfig::default());
            }
        } else {
            for col in &mut self.columns[..idx] {
                col.animate_move_from(-offset, AnimationConfig::default());
            }
        }

        let col = self.columns.remove(idx);
        self.column_widths.remove(idx);

        if self.columns.is_empty() {
            self.active_column_idx = 0;
            return Some(col);
        }

        if idx < self.active_column_idx {
            self.active_column_idx -= 1;
            self.activate_prev_on_removal = None;
        } else if idx == self.active_column_idx {
            // Activate previous or next.
            let new_idx = if self.activate_prev_on_removal.is_some() && idx > 0 {
                idx - 1
            } else {
                idx.min(self.columns.len() - 1)
            };
            self.active_column_idx = new_idx;
            self.activate_prev_on_removal = None;
            let offset = self.compute_view_offset_for_column(new_idx, None);
            self.view_offset = ViewOffset::Static(offset);
        }

        Some(col)
    }

    /// Remove a pane from a column.
    pub fn remove_pane(&mut self, col_idx: usize, pane_idx: usize) -> Option<Pane> {
        if col_idx >= self.columns.len() {
            return None;
        }

        // If removing the last pane in a column, remove the whole column.
        if self.columns[col_idx].panes.len() == 1 {
            return self.remove_column(col_idx).map(|mut c| c.panes.remove(0));
        }

        let prev_width = self.columns[col_idx].computed_width;
        let pane = {
            let col = &mut self.columns[col_idx];
            col.remove_pane(pane_idx)?
        };

        // Recompute sizes.
        self.columns[col_idx].compute_pane_sizes(self.working_area.size.h, self.options.gaps);
        self.update_all_column_widths();

        // Animate column width changes.
        let offset = prev_width - self.columns[col_idx].computed_width;
        if offset != 0.0 {
            if self.active_column_idx <= col_idx {
                for c in &mut self.columns[col_idx + 1..] {
                    c.animate_move_from(offset, AnimationConfig::default());
                }
            } else {
                for c in &mut self.columns[..=col_idx] {
                    c.animate_move_from(-offset, AnimationConfig::default());
                }
            }
        }

        Some(pane)
    }

    /// Focus left (previous column).
    pub fn focus_left(&mut self) -> bool {
        if self.active_column_idx == 0 {
            return false;
        }
        self.activate_column(self.active_column_idx - 1);
        true
    }

    /// Focus right (next column).
    pub fn focus_right(&mut self) -> bool {
        if self.active_column_idx + 1 >= self.columns.len() {
            return false;
        }
        self.activate_column(self.active_column_idx + 1);
        true
    }

    /// Move the active column left.
    pub fn move_column_left(&mut self) -> bool {
        if self.active_column_idx == 0 {
            return false;
        }
        self.move_column_to(self.active_column_idx - 1);
        true
    }

    /// Move the active column right.
    pub fn move_column_right(&mut self) -> bool {
        let new_idx = self.active_column_idx + 1;
        if new_idx >= self.columns.len() {
            return false;
        }
        self.move_column_to(new_idx);
        true
    }

    /// Resize the active column by a delta (positive = wider, negative = narrower).
    /// NIRI behavior: only the active column changes. Other columns keep their widths.
    /// If the total exceeds the viewport, the view scrolls horizontally.
    pub fn resize_active_column(&mut self, delta: f64) {
        if let Some(col) = self.columns.get_mut(self.active_column_idx) {
            let new_width = match col.width {
                ColumnWidth::Proportion(p) => {
                    ColumnWidth::Proportion((p + delta).clamp(0.05, 0.95))
                }
                ColumnWidth::Fixed(w) => {
                    ColumnWidth::Fixed((w + delta * self.working_area.size.w).clamp(50.0, self.working_area.size.w))
                }
            };
            col.width = new_width;
            col.is_full_width = false;

            // Save old column positions before update.
            let old_xs: Vec<(ColumnId, f64)> = self.column_xs()
                .zip(self.columns.iter())
                .map(|(x, c)| (c.id, x))
                .collect();

            self.update_all_column_widths();

            // Preserve view position so layout stays visually fixed during resize.
            let old_view_pos = self.view_pos();
            let new_view_pos = self.view_pos();
            let view_delta = old_view_pos - new_view_pos;
            self.view_offset.offset(view_delta);

            // Ensure the active column stays visible after resize.
            let target_offset = self.compute_view_offset_for_column(self.active_column_idx, None);
            let pixel = 1.0 / self.scale;
            let diff = target_offset - self.view_offset.target();
            if diff.abs() < pixel {
                self.view_offset.offset(diff);
            } else {
                self.view_offset = ViewOffset::Animation(Animation::new(
                    self.view_offset.current(),
                    target_offset,
                    AnimationConfig::default(),
                ));
            }

            // Animate columns to their new positions.
            let new_xs: Vec<f64> = self.column_xs().collect();
            for (i, col) in self.columns.iter_mut().enumerate() {
                let old_x = old_xs
                    .iter()
                    .find(|(id, _)| *id == col.id)
                    .map(|(_, x)| *x)
                    .unwrap_or(new_xs[i]);
                let diff = old_x - new_xs[i];
                if diff.abs() > 0.5 {
                    col.animate_move_from(diff, AnimationConfig::default());
                }
            }
        }
    }

    /// Move the active pane to the previous column (left).
    /// If at first column and source has >1 pane, creates a new column to the left.
    /// If at first column and source has 1 pane, does nothing.
    pub fn move_active_pane_left(&mut self) -> bool {
        if self.active_column_idx == 0 {
            // First column — create new column to the left if source has > 1 pane
            let source_has_multiple = self.columns.get(self.active_column_idx)
                .map(|c| c.panes.len() > 1)
                .unwrap_or(false);
            if !source_has_multiple {
                return false;
            }
            return self.move_active_pane_to_new_column(Direction::Left);
        }
        let target_col = self.active_column_idx - 1;
        self.move_active_pane_to_column(target_col)
    }

    /// Move the active pane to the next column (right).
    /// If at last column and source has >1 pane, creates a new column to the right.
    /// If at last column and source has 1 pane, does nothing.
    pub fn move_active_pane_right(&mut self) -> bool {
        if self.active_column_idx + 1 >= self.columns.len() {
            // Last column — create new column to the right if source has > 1 pane
            let source_has_multiple = self.columns.get(self.active_column_idx)
                .map(|c| c.panes.len() > 1)
                .unwrap_or(false);
            if !source_has_multiple {
                return false;
            }
            return self.move_active_pane_to_new_column(Direction::Right);
        }
        let target_col = self.active_column_idx + 1;
        self.move_active_pane_to_column(target_col)
    }

    fn move_active_pane_to_column(&mut self, target_col: usize) -> bool {
        let source_col = self.active_column_idx;
        if source_col == target_col {
            return false;
        }

        // Save old column positions and pane position before any changes.
        let old_xs: Vec<(ColumnId, f64)> = self.column_xs()
            .zip(self.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        let old_source_col_x = self.column_x(source_col);
        let old_pane_y = self.pane_y_in_column(source_col, self.columns[source_col].active_pane_idx);

        let pane_idx = self.columns[source_col].active_pane_idx;
        let pane = self.columns[source_col].remove_pane(pane_idx);
        let Some(pane) = pane else { return false; };

        // If source column became empty, remove it.
        if self.columns[source_col].is_empty() {
            self.remove_column(source_col);
            // Adjust target index if we removed a column before it.
            let target_col = if target_col > source_col {
                target_col - 1
            } else {
                target_col
            };
            let target = &mut self.columns[target_col];
            target.add_pane_at(target.panes.len(), pane);
            target.active_pane_idx = target.panes.len() - 1;
            self.active_column_idx = target_col;
        } else {
            let target = &mut self.columns[target_col];
            target.add_pane_at(target.panes.len(), pane);
            target.active_pane_idx = target.panes.len() - 1;
            self.active_column_idx = target_col;
        }

        self.update_all_column_widths();

        // Animate the moved pane from its old visual position to its new one.
        let new_col_x = self.column_x(self.active_column_idx);
        let new_pane_y = self.pane_y_in_column(
            self.active_column_idx,
            self.columns[self.active_column_idx].panes.len() - 1,
        );
        let offset_x = old_source_col_x - new_col_x;
        let offset_y = old_pane_y - new_pane_y;
        let moved_pane = self.columns[self.active_column_idx].panes.last_mut().unwrap();
        moved_pane.animate_move_from(Point::new(offset_x, offset_y), AnimationConfig::default());

        // Animate all columns from their old positions.
        let new_xs: Vec<f64> = self.column_xs().collect();
        for (i, col) in self.columns.iter_mut().enumerate() {
            let old_x = old_xs
                .iter()
                .find(|(id, _)| *id == col.id)
                .map(|(_, x)| *x)
                .unwrap_or(new_xs[i]);
            let diff = old_x - new_xs[i];
            if diff.abs() > 0.5 {
                col.animate_move_from(diff, AnimationConfig::default());
            }
        }

        // Animate view to bring the active column into view.
        self.align_view_to_active_column();
        true
    }

    /// Create a new column to the left or right of the current column
    /// with the active pane moved into it.
    fn move_active_pane_to_new_column(&mut self, dir: Direction) -> bool {
        let source_col = self.active_column_idx;
        let pane_idx = self.columns[source_col].active_pane_idx;

        // Save old column positions and pane position before any changes.
        let old_xs: Vec<(ColumnId, f64)> = self.column_xs()
            .zip(self.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        let old_source_col_x = self.column_x(source_col);
        let old_pane_y = self.pane_y_in_column(source_col, pane_idx);

        let pane = self.columns[source_col].remove_pane(pane_idx);
        let Some(pane) = pane else { return false; };

        let new_col = Column::new(
            ColumnId(pane.id.0),
            pane,
            ColumnWidth::Proportion(0.5),
        );

        let insert_idx = match dir {
            Direction::Left => source_col,
            Direction::Right => source_col + 1,
        };

        // If source column became empty, remove it and adjust insert index.
        let removed_source = self.columns[source_col].is_empty();
        if removed_source {
            self.remove_column(source_col);
            let insert_idx = if dir == Direction::Right && insert_idx > source_col {
                insert_idx - 1
            } else {
                insert_idx
            };
            self.columns.insert(insert_idx, new_col);
            self.column_widths.insert(insert_idx, 0.0);
            self.active_column_idx = insert_idx;
        } else {
            self.columns.insert(insert_idx, new_col);
            self.column_widths.insert(insert_idx, 0.0);
            self.active_column_idx = insert_idx;
        }

        self.update_all_column_widths();

        // Animate the moved pane from its old visual position to its new one.
        let new_col_x = self.column_x(self.active_column_idx);
        let new_pane_y = self.pane_y_in_column(self.active_column_idx, 0);
        let offset_x = old_source_col_x - new_col_x;
        let offset_y = old_pane_y - new_pane_y;
        let moved_pane = self.columns[self.active_column_idx].panes.first_mut().unwrap();
        moved_pane.animate_move_from(Point::new(offset_x, offset_y), AnimationConfig::default());

        // Animate all columns from their old positions.
        let new_xs: Vec<f64> = self.column_xs().collect();
        for (i, col) in self.columns.iter_mut().enumerate() {
            let old_x = old_xs
                .iter()
                .find(|(id, _)| *id == col.id)
                .map(|(_, x)| *x)
                .unwrap_or(new_xs[i]);
            let diff = old_x - new_xs[i];
            if diff.abs() > 0.5 {
                col.animate_move_from(diff, AnimationConfig::default());
            }
        }

        // Animate view to bring the new active column into view.
        self.align_view_to_active_column();
        true
    }

    /// Align the view so the active column is fully visible.
    /// If the column is off-screen, animate the view to bring it into view.
    pub fn align_view_to_active_column(&mut self) {
        let idx = self.active_column_idx;
        let target_offset = self.compute_view_offset_for_column(idx, None);
        let current_offset = self.view_offset.current();
        let pixel = 1.0 / self.scale;
        let diff = target_offset - current_offset;
        if diff.abs() < pixel {
            self.view_offset = ViewOffset::Static(target_offset);
        } else {
            self.view_offset = ViewOffset::Animation(Animation::new(
                current_offset,
                target_offset,
                AnimationConfig::default(),
            ));
        }
    }

    fn move_column_to(&mut self, new_idx: usize) {
        if self.active_column_idx == new_idx {
            return;
        }

        let old_idx = self.active_column_idx;

        // Save old column positions by ID.
        let old_xs: Vec<(ColumnId, f64)> = self.column_xs()
            .zip(self.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();

        let old_view_pos = self.view_pos();

        // Remove from old position and insert at new position.
        let column = self.columns.remove(old_idx);
        let width = self.column_widths.remove(old_idx);
        self.columns.insert(new_idx, column);
        self.column_widths.insert(new_idx, width);

        // Update active index BEFORE computing new positions.
        self.active_column_idx = new_idx;

        // Preserve view position so the layout stays visually fixed.
        let new_view_pos = self.view_pos();
        let delta = old_view_pos - new_view_pos;
        self.view_offset.offset(delta);

        // Animate all columns from their old positions to new.
        let new_xs: Vec<f64> = self.column_xs().collect();
        for (i, col) in self.columns.iter_mut().enumerate() {
            let old_x = old_xs
                .iter()
                .find(|(id, _)| *id == col.id)
                .map(|(_, x)| *x)
                .unwrap_or(new_xs[i]);
            let diff = old_x - new_xs[i];
            if diff.abs() > 0.5 {
                col.animate_move_from(diff, AnimationConfig::default());
            }
        }
    }

    /// Update all column widths from their configurations.
    pub fn update_all_column_widths(&mut self) {
        for (i, col) in self.columns.iter_mut().enumerate() {
            col.computed_width = col.resolve_width(self.working_area.size.w, self.options.gaps);
            col.compute_pane_sizes(self.working_area.size.h, self.options.gaps);
            if let Some(width) = self.column_widths.get_mut(i) {
                *width = col.computed_width;
            }
        }
    }

    /// Update the working area (e.g., on resize).
    pub fn update_working_area(&mut self, working_area: Rectangle) {
        self.working_area = working_area;
        self.update_all_column_widths();
        if !self.columns.is_empty() && self.view_offset.is_static() {
            let offset = self.compute_view_offset_for_column(self.active_column_idx, None);
            self.view_offset = ViewOffset::Static(offset);
        }
    }

    /// Advance animations for all columns, panes, and the view offset.
    pub fn advance_animations(&mut self) {
        // Advance view offset animation.
        if let ViewOffset::Animation(anim) = &self.view_offset
            && anim.is_done()
        {
            self.view_offset = ViewOffset::Static(anim.target());
        }

        // Advance column animations.
        for col in &mut self.columns {
            if let super::animation::Animated::Animating { ref animation, .. } = col.move_offset
                && animation.is_done()
            {
                col.move_offset = super::animation::Animated::Static(0.0);
            }
            // Advance pane Y-move animations.
            for pane in &mut col.panes {
                if let super::animation::Animated::Animating { ref animation, .. } = pane.move_offset
                    && animation.is_done()
                {
                    pane.move_offset = super::animation::Animated::Static(super::types::Point::default());
                }
            }
        }
    }

    /// Check if any animations are ongoing.
    pub fn are_animations_ongoing(&self) -> bool {
        self.view_offset.is_animation_ongoing()
            || self.columns.iter().any(|c| {
                matches!(c.move_offset, super::animation::Animated::Animating { .. })
                    || c.panes.iter().any(|p| {
                        matches!(p.move_offset, super::animation::Animated::Animating { .. })
                    })
            })
    }

    /// Compute the insert position for a point in space coordinates.
    /// Used during interactive move to determine where to drop a pane.
    ///
    /// Algorithm (from NIRI's `scrolling.insert_position()`):
    /// 1. Transform to space coords and aim for center of gaps.
    /// 2. Find closest column gap vs closest tile gap.
    /// 3. Return whichever is closer.
    pub fn insert_position(&self, pos: Point) -> InsertPosition {
        let gaps = self.options.gaps;
        // pos is already in space coordinates (caller adds view_pos).
        let x = pos.x + gaps / 2.0;
        let y = pos.y + gaps / 2.0;

        // Before first column → NewColumn(0)
        if x < 0.0 {
            return InsertPosition::NewColumn(0);
        }

        // Find the column containing x.
        let mut col_idx = 0usize;
        let mut found_col = false;
        for (i, col_x) in self.column_xs().enumerate() {
            let col_w = self.column_widths.get(i).copied().unwrap_or(0.0);
            if x >= col_x && x < col_x + col_w {
                col_idx = i;
                found_col = true;
                break;
            }
        }

        // Past last column → NewColumn at end.
        if !found_col {
            return InsertPosition::NewColumn(self.columns.len());
        }

        // Find closest column gap.
        let mut closest_col_gap_idx = 0usize;
        let mut closest_col_gap_dist = f64::MAX;
        for (i, col_x) in self.column_xs().enumerate() {
            let dist = (col_x - x).abs();
            if dist < closest_col_gap_dist {
                closest_col_gap_dist = dist;
                closest_col_gap_idx = i;
            }
            // Also check right edge of column (gap center after this column).
            let col_w = self.column_widths.get(i).copied().unwrap_or(0.0);
            let right_x = col_x + col_w + gaps;
            let right_dist = (right_x - x).abs();
            if right_dist < closest_col_gap_dist {
                closest_col_gap_dist = right_dist;
                closest_col_gap_idx = i + 1;
            }
        }

        // Find closest tile gap within the containing column.
        let col = &self.columns[col_idx];
        let _col_x = self.column_x(col_idx);
        let mut tile_y = gaps;
        let mut closest_tile_idx = 0usize;
        let mut closest_tile_gap_dist = f64::MAX;

        for (i, size) in col.pane_sizes.iter().enumerate() {
            let dist = (tile_y - y).abs();
            if dist < closest_tile_gap_dist {
                closest_tile_gap_dist = dist;
                closest_tile_idx = i;
            }
            tile_y += size.h + gaps;
        }
        // Check bottom edge.
        let bottom_dist = (tile_y - y).abs();
        if bottom_dist < closest_tile_gap_dist {
            closest_tile_gap_dist = bottom_dist;
            closest_tile_idx = col.pane_sizes.len();
        }

        // Compare distances: column gap vs tile gap.
        if closest_col_gap_dist <= closest_tile_gap_dist {
            InsertPosition::NewColumn(closest_col_gap_idx.min(self.columns.len()))
        } else {
            InsertPosition::InColumn {
                col_idx,
                pane_idx: closest_tile_idx.min(col.panes.len()),
            }
        }
    }

    /// Get all panes with their render positions.
    pub fn panes_with_positions(&self) -> Vec<(PaneId, Rectangle)> {
        let mut result = Vec::new();
        let view_pos = self.view_pos();
        let view_off = Point::new(-view_pos, 0.0);
        let gaps = self.options.gaps;

        for (col_idx, col) in self.columns.iter().enumerate() {
            let col_x = self.column_x(col_idx);
            let col_render_off = col.render_offset();
            let col_pos = Point::new(col_x + col_render_off, 0.0);

            let mut pane_y = self.working_area.loc.y + gaps;

            for (pane_idx, pane) in col.panes.iter().enumerate() {
                let size = col.pane_sizes.get(pane_idx).copied().unwrap_or(Size::new(
                    col.computed_width,
                    self.working_area.size.h / col.panes.len().max(1) as f64,
                ));

                let pane_offset = pane.move_offset.current();
                let rubber = pane.interactive_move_offset;
                let pane_pos = view_off + col_pos + Point::new(
                    pane_offset.x + rubber.x,
                    pane_y + pane_offset.y + rubber.y,
                );
                let rect = Rectangle::new(pane_pos, size);
                result.push((pane.id, rect));

                pane_y += size.h + gaps;
            }
        }

        result
    }

}
