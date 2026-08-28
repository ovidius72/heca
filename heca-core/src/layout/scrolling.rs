use super::animation::{Animation, AnimationConfig};
use super::column::{Column, Pane};
use super::types::*;
use super::view_offset::{ViewOffset, compute_new_view_offset};

// Re-export PaneInsertTarget for convenience.
pub use super::types::PaneInsertTarget;

/// Minimum width (logical px) a column may be shrunk to by a manual resize, so a
/// column never becomes a thin line.
pub const MIN_COLUMN_WIDTH: f64 = 150.0;

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
        let widths = self
            .column_widths
            .iter()
            .copied()
            .chain(std::iter::once(0.0));
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
        let mode = self
            .columns
            .get(idx)
            .map(|c| c.sizing_mode())
            .unwrap_or(SizingMode::Normal);

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
        let mode = self
            .columns
            .get(idx)
            .map(|c| c.sizing_mode())
            .unwrap_or(SizingMode::Normal);

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
        if idx >= self.columns.len() {
            return;
        }
        if self.active_column_idx == idx {
            // Already the active column — but it may have been scrolled/resized
            // out of view, so still re-fit it (#3: re-focusing a stranded column
            // must reveal it instead of doing nothing).
            self.ensure_active_column_visible();
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
            if was_empty {
                0
            } else {
                self.active_column_idx + 1
            }
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

    fn capture_column_positions(&self) -> Vec<(ColumnId, f64)> {
        self.column_xs()
            .zip(self.columns.iter())
            .map(|(x, col)| (col.id, x))
            .collect()
    }

    fn finish_active_column_width_change(&mut self, old_xs: &[(ColumnId, f64)], old_view_pos: f64) {
        self.update_all_column_widths();

        // Preserve view position so layout stays visually fixed during width changes.
        let new_view_pos = self.view_pos();
        let view_delta = old_view_pos - new_view_pos;
        self.view_offset.offset(view_delta);

        // Ensure the active column stays visible after the width change.
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

    /// Toggle the active column between viewport-wide zoom and its previous width.
    pub fn toggle_active_column_zoom(&mut self) -> bool {
        if self.active_column_idx >= self.columns.len() {
            return false;
        }

        let old_xs = self.capture_column_positions();
        let old_view_pos = self.view_pos();
        let active_idx = self.active_column_idx;
        let active_was_zoomed = self.columns[active_idx].is_zoomed();

        let active_col = &mut self.columns[active_idx];
        if active_was_zoomed {
            if let Some(previous_width) = active_col.zoom_restore_width.take() {
                active_col.width = previous_width;
            }
        } else {
            active_col.zoom_restore_width = Some(active_col.width);
        }

        self.finish_active_column_width_change(&old_xs, old_view_pos);
        true
    }

    /// Scroll the view (statically) so the active column is on-screen. Mirrors the
    /// re-fit that [`update_working_area`](Self::update_working_area) does on a
    /// window resize — used after a manual column resize that pushes the active
    /// column's edge off the viewport, and when re-focusing an already-active
    /// column that was scrolled out of view (#3). A column wider than the viewport
    /// is left-aligned; use [`scroll_view`](Self::scroll_view) to pan across its
    /// overflow. No-op while a view animation/gesture is in flight.
    pub fn ensure_active_column_visible(&mut self) {
        if !self.columns.is_empty() && self.view_offset.is_static() {
            let offset = self.compute_view_offset_for_column(self.active_column_idx, None);
            self.view_offset = ViewOffset::Static(offset);
        }
    }

    /// Pan the view horizontally by `delta` logical px (positive = reveal content
    /// to the **right**), clamped to the content bounds so it never scrolls the
    /// whole layout off-screen. Lets the user reach column overflow / content
    /// scrolled past an edge (#3). Snaps statically for responsiveness; no-op when
    /// all columns already fit the viewport.
    pub fn scroll_view(&mut self, delta: f64) {
        if self.columns.is_empty() {
            return;
        }
        let vw = self.working_area.size.w;
        let last = self.columns.len() - 1;
        let first_left = self.column_x(0);
        let last_right = self.column_x(last) + self.column_widths.get(last).copied().unwrap_or(0.0);
        // Everything fits → nothing to scroll.
        if last_right - first_left <= vw {
            return;
        }
        // view_pos() is the left edge of the viewport in column space.
        let min_view = first_left; // column 0 left-aligned
        let max_view = last_right - vw; // last column right-aligned
        let new_view = (self.view_pos() + delta).clamp(min_view, max_view);
        let new_offset = new_view - self.column_x(self.active_column_idx);
        self.view_offset = ViewOffset::Static(new_offset);
    }

    /// Resize the active column by a delta (positive = wider, negative = narrower).
    /// NIRI behavior: only the active column changes. Other columns keep their widths.
    /// If the total exceeds the viewport, the view scrolls horizontally.
    pub fn resize_active_column(&mut self, delta: f64) {
        self.resize_column(self.active_column_idx, delta);
    }

    /// Resize column `idx` by `delta` (a proportion delta for `Proportion` widths,
    /// or a fraction of the working width for `Fixed`). Mutates the column's
    /// **canonical** [`ColumnWidth`] — so the change persists through later
    /// `update_all_column_widths` recomputes — and preserves the view position. Used
    /// by the keyboard resize (active column), the mouse divider drag (any column),
    /// and RPC.
    pub fn resize_column(&mut self, idx: usize, delta: f64) {
        if idx >= self.columns.len() {
            return;
        }

        let working_w = self.working_area.size.w;
        let gaps = self.options.gaps;
        // A column may grow to fill the full visible width and shrink no smaller
        // than MIN_COLUMN_WIDTH (so it never becomes a thin line).
        let available_width = (working_w - gaps * 2.0).max(MIN_COLUMN_WIDTH);
        // The proportion that resolves to MIN_COLUMN_WIDTH (see `Column::resolve_width`:
        // width = (working_w - gaps) * p - gaps), and the proportion that fills the
        // visible width (p = 1.0 ⇒ width = working_w - 2·gaps = available_width).
        let min_prop = ((MIN_COLUMN_WIDTH + gaps) / (working_w - gaps)).clamp(0.01, 1.0);

        // Anchor on the **resized** column's left edge (its on-screen offset from
        // the view) so its right edge — the divider being dragged — tracks the
        // cursor, regardless of which column is active. Anchoring on the *active*
        // column (the old `finish_active_column_width_change`) made a left column
        // grow leftward when the right column was focused (the "wrong side" bug).
        let old_rel = self.column_x(idx) - self.view_pos();

        if let Some(col) = self.columns.get_mut(idx) {
            let base_width = if col.is_zoomed() {
                ColumnWidth::Fixed(available_width)
            } else {
                col.width
            };
            // A column grows at most to the full visible width (`p = 1.0` ⇒
            // `available_width`) and never beyond — no off-screen, weird super-wide
            // columns. The view scrolls to keep the resized column fully visible
            // (see the `ensure_active_column_visible` call below), so its right
            // divider stays reachable while dragging instead of stalling at the edge.
            let new_width = match base_width {
                ColumnWidth::Proportion(p) => {
                    ColumnWidth::Proportion((p + delta).clamp(min_prop, 1.0))
                }
                ColumnWidth::Fixed(w) => ColumnWidth::Fixed(
                    (w + delta * working_w).clamp(MIN_COLUMN_WIDTH, available_width),
                ),
            };
            col.width = new_width;
            col.zoom_restore_width = None;
            col.is_full_width = false;
        }

        self.update_all_column_widths();
        // Restore the resized column's on-screen left edge by shifting the view by
        // the amount it moved. No active-column recenter / per-move animation —
        // those fight a smooth per-pixel drag.
        let new_rel = self.column_x(idx) - self.view_pos();
        self.view_offset.offset(new_rel - old_rel);
        // If the resize pushed the active column's far edge off-screen, scroll to
        // keep it reachable (#3) — only when resizing the active column, so a
        // divider drag on another column doesn't yank the view.
        if idx == self.active_column_idx {
            self.ensure_active_column_visible();
        }
    }

    /// Resize the height of pane `pane_idx` within column `col_idx` by `delta`
    /// logical px (mouse divider drag / RPC). No-op for single-pane columns or
    /// out-of-range indices. Mirrors the keyboard `resize_active_pane_height`.
    pub fn resize_pane_height(&mut self, col_idx: usize, pane_idx: usize, delta: f64) {
        let (working_h, gaps) = (self.working_area.size.h, self.options.gaps);
        if let Some(col) = self.columns.get_mut(col_idx) {
            col.resize_pane_height(pane_idx, delta, working_h, gaps);
        }
    }

    /// Move the active pane to the previous column (left).
    /// If at first column and source has >1 pane, creates a new column to the left.
    /// If at first column and source has 1 pane, does nothing.
    pub fn move_active_pane_left(&mut self, new_column_id: ColumnId) -> bool {
        if self.active_column_idx == 0 {
            // First column — create new column to the left if source has > 1 pane
            let source_has_multiple = self
                .columns
                .get(self.active_column_idx)
                .map(|c| c.panes.len() > 1)
                .unwrap_or(false);
            if !source_has_multiple {
                return false;
            }
            return self.move_active_pane_to_new_column(Direction::Left, new_column_id);
        }
        let target_col = self.active_column_idx - 1;
        self.move_active_pane_to_column(target_col)
    }

    /// Move the active pane to the next column (right).
    /// If at last column and source has >1 pane, creates a new column to the right.
    /// If at last column and source has 1 pane, does nothing.
    pub fn move_active_pane_right(&mut self, new_column_id: ColumnId) -> bool {
        if self.active_column_idx + 1 >= self.columns.len() {
            // Last column — create new column to the right if source has > 1 pane
            let source_has_multiple = self
                .columns
                .get(self.active_column_idx)
                .map(|c| c.panes.len() > 1)
                .unwrap_or(false);
            if !source_has_multiple {
                return false;
            }
            return self.move_active_pane_to_new_column(Direction::Right, new_column_id);
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
        let old_xs: Vec<(ColumnId, f64)> = self
            .column_xs()
            .zip(self.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        let old_source_col_x = self.column_x(source_col);
        let old_pane_y =
            self.pane_y_in_column(source_col, self.columns[source_col].active_pane_idx);

        let pane_idx = self.columns[source_col].active_pane_idx;
        let pane = self.columns[source_col].remove_pane(pane_idx);
        let Some(pane) = pane else {
            return false;
        };

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
        let moved_pane = self.columns[self.active_column_idx]
            .panes
            .last_mut()
            .unwrap();
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
    fn move_active_pane_to_new_column(&mut self, dir: Direction, new_column_id: ColumnId) -> bool {
        let source_col = self.active_column_idx;
        let pane_idx = self.columns[source_col].active_pane_idx;

        // Save old column positions and pane position before any changes.
        let old_xs: Vec<(ColumnId, f64)> = self
            .column_xs()
            .zip(self.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        let old_source_col_x = self.column_x(source_col);
        let old_pane_y = self.pane_y_in_column(source_col, pane_idx);

        let pane = self.columns[source_col].remove_pane(pane_idx);
        let Some(pane) = pane else {
            return false;
        };

        let new_col = Column::new(new_column_id, pane, ColumnWidth::Proportion(0.5));

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
        let moved_pane = self.columns[self.active_column_idx]
            .panes
            .first_mut()
            .unwrap();
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
        self.reorder_column(self.active_column_idx, new_idx);
    }

    /// Move the column at `from` to index `to` within this workspace, animating the
    /// shift and leaving the moved column **active**. `to` is clamped to the column
    /// range; no-op if `from` is out of range or `from == to`. Returns whether it
    /// moved. The general primitive behind keyboard left/right *and* DnD reorder (F4.5).
    pub fn reorder_column(&mut self, from: usize, to: usize) -> bool {
        if from >= self.columns.len() {
            return false;
        }
        let to = to.min(self.columns.len() - 1);
        if from == to {
            return false;
        }

        // Save old column positions by ID (for the shift animation).
        let old_xs: Vec<(ColumnId, f64)> = self
            .column_xs()
            .zip(self.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        let old_view_pos = self.view_pos();

        // Remove from old position and insert at new position.
        let column = self.columns.remove(from);
        let width = self.column_widths.remove(from);
        self.columns.insert(to, column);
        self.column_widths.insert(to, width);

        // The moved column stays active. Update the index BEFORE computing positions.
        self.active_column_idx = to;

        // Preserve view position so the layout stays visually fixed.
        let new_view_pos = self.view_pos();
        let delta = old_view_pos - new_view_pos;
        self.view_offset.offset(delta);

        self.animate_columns_from(&old_xs);
        true
    }

    /// Swap the columns at `a` and `b` within this workspace (positions only — each
    /// column keeps its panes), animating the shift. No-op if either index is out of
    /// range or `a == b`. Returns whether it swapped. (DnD column swap — F4.5.)
    pub fn swap_columns(&mut self, a: usize, b: usize) -> bool {
        let len = self.columns.len();
        if a >= len || b >= len || a == b {
            return false;
        }
        let old_xs: Vec<(ColumnId, f64)> = self
            .column_xs()
            .zip(self.columns.iter())
            .map(|(x, c)| (c.id, x))
            .collect();
        self.columns.swap(a, b);
        self.column_widths.swap(a, b);
        self.animate_columns_from(&old_xs);
        true
    }

    /// Animate every column from its previous x (keyed by [`ColumnId`]) to its new
    /// laid-out x — shared by [`reorder_column`](Self::reorder_column) and
    /// [`swap_columns`](Self::swap_columns).
    fn animate_columns_from(&mut self, old_xs: &[(ColumnId, f64)]) {
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
                if let super::animation::Animated::Animating { ref animation, .. } =
                    pane.move_offset
                    && animation.is_done()
                {
                    pane.move_offset =
                        super::animation::Animated::Static(super::types::Point::default());
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
    pub fn insert_position(&self, pos: Point) -> PaneInsertTarget {
        let gaps = self.options.gaps;
        // pos is already in space coordinates (caller adds view_pos).
        let x = pos.x + gaps / 2.0;
        let y = pos.y + gaps / 2.0;

        // Before first column → NewColumn(0)
        if x < 0.0 {
            return PaneInsertTarget::NewColumn(0);
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
            return PaneInsertTarget::NewColumn(self.columns.len());
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
            PaneInsertTarget::NewColumn(closest_col_gap_idx.min(self.columns.len()))
        } else {
            PaneInsertTarget::InColumn {
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
                let pane_pos = view_off
                    + col_pos
                    + Point::new(pane_offset.x + rubber.x, pane_y + pane_offset.y + rubber.y);
                let rect = Rectangle::new(pane_pos, size);
                result.push((pane.id, rect));

                pane_y += size.h + gaps;
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_scrolling_space() -> ScrollingSpace {
        ScrollingSpace::new(
            Rectangle::from_size(Size::new(1000.0, 800.0)),
            1.0,
            LayoutOptions::default(),
        )
    }

    fn test_column(id: u64, width: ColumnWidth) -> Column {
        Column::new(
            ColumnId(id),
            Pane::new(PaneId(id), format!("Pane {id}")),
            width,
        )
    }

    /// A space with columns whose ids are `1..=n`, in order.
    fn space_with_columns(n: u64) -> ScrollingSpace {
        let mut space = test_scrolling_space();
        // activate=true appends in order (activate=false inserts at active+1).
        for id in 1..=n {
            space.add_column(None, test_column(id, ColumnWidth::Proportion(0.5)), true);
        }
        space
    }

    fn column_ids(space: &ScrollingSpace) -> Vec<u64> {
        space.columns.iter().map(|c| c.id.0).collect()
    }

    #[test]
    fn reorder_column_moves_and_activates() {
        let mut space = space_with_columns(4); // [1,2,3,4]
        assert!(space.reorder_column(0, 2));
        assert_eq!(column_ids(&space), vec![2, 3, 1, 4]);
        assert_eq!(
            space.active_column_idx, 2,
            "the moved column becomes active"
        );
    }

    #[test]
    fn reorder_column_clamps_and_no_ops() {
        let mut space = space_with_columns(3); // [1,2,3]
        assert!(space.reorder_column(0, 99), "dst clamps to the last index");
        assert_eq!(column_ids(&space), vec![2, 3, 1]);
        assert!(!space.reorder_column(1, 1), "same index is a no-op");
        assert!(
            !space.reorder_column(9, 0),
            "out-of-range source is a no-op"
        );
    }

    #[test]
    fn swap_columns_exchanges_positions() {
        let mut space = space_with_columns(4); // [1,2,3,4]
        assert!(space.swap_columns(0, 3));
        assert_eq!(column_ids(&space), vec![4, 2, 3, 1]);
        assert!(!space.swap_columns(1, 1), "self-swap is a no-op");
        assert!(!space.swap_columns(0, 9), "out-of-range is a no-op");
    }

    #[test]
    fn toggle_active_column_zoom_restores_previous_width() {
        let mut space = test_scrolling_space();
        space.add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);

        assert_eq!(space.columns[0].width, ColumnWidth::Proportion(0.5));
        assert!(!space.columns[0].is_zoomed());

        assert!(space.toggle_active_column_zoom());
        assert!(space.columns[0].is_zoomed());
        assert_eq!(
            space.columns[0].zoom_restore_width,
            Some(ColumnWidth::Proportion(0.5))
        );
        assert_eq!(space.column_widths[0], 984.0);

        assert!(space.toggle_active_column_zoom());
        assert_eq!(space.columns[0].width, ColumnWidth::Proportion(0.5));
        assert!(!space.columns[0].is_zoomed());
    }

    #[test]
    fn resize_column_persists_through_recompute_and_add() {
        let mut space = space_with_columns(2); // [1,2] each Proportion(0.5)
        space.resize_column(0, 0.2);
        assert_eq!(space.columns[0].width, ColumnWidth::Proportion(0.7));
        let w0 = space.column_widths[0];
        // A later layout mutation recomputes the width cache from the canonical
        // `col.width` — the resize must NOT be recomputed away (the niri landmine).
        space.update_all_column_widths();
        assert_eq!(space.columns[0].width, ColumnWidth::Proportion(0.7));
        assert_eq!(
            space.column_widths[0], w0,
            "recompute preserves the manual resize"
        );
        // Adding a column must not reflow column 0 (independent proportions).
        space.add_column(None, test_column(3, ColumnWidth::Proportion(0.5)), true);
        assert_eq!(
            space.columns[0].width,
            ColumnWidth::Proportion(0.7),
            "resize survives add"
        );
    }

    #[test]
    fn resize_column_clamps_and_ignores_out_of_range() {
        let mut space = space_with_columns(2);
        space.resize_column(0, 10.0); // huge delta clamps to the full-width cap (1.0 = viewport)
        assert_eq!(space.columns[0].width, ColumnWidth::Proportion(1.0));
        // A huge negative delta clamps to the min-width proportion (small, non-zero).
        space.resize_column(1, -10.0);
        match space.columns[1].width {
            ColumnWidth::Proportion(p) => {
                assert!(
                    p > 0.0 && p < 0.5,
                    "clamped to a small but non-zero min, got {p}"
                );
            }
            other => panic!("expected a proportion, got {other:?}"),
        }
        space.resize_column(99, 0.1); // out of range → no-op, no panic
        assert_eq!(space.columns.len(), 2);
    }

    #[test]
    fn resize_pane_height_sets_preferred_and_no_ops_single_pane() {
        let mut space = test_scrolling_space();
        space.add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
        // Single-pane column → no-op (the lone pane fills the column).
        space.resize_pane_height(0, 0, 30.0);
        assert_eq!(space.columns[0].panes[0].preferred_height, None);
        // Stack a second pane, then the resize takes effect.
        space.add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), true);
        space.resize_pane_height(0, 0, 30.0);
        assert!(space.columns[0].panes[0].preferred_height.is_some());
        // Out-of-range column / pane index → no panic.
        space.resize_pane_height(9, 0, 30.0);
        space.resize_pane_height(0, 9, 30.0);
    }

    /// **A boundary moves space between its own two panes, and nothing else** (F004/P084/T413).
    ///
    /// It used to pin one pane and let `compute_pane_sizes` redistribute the remainder over every
    /// pane still auto-sized. With two panes the only auto pane *was* the neighbour, so it looked
    /// right; with three, dragging the TOP boundary took space from the BOTTOM pane too, which
    /// collapsed to the floor and read as having disappeared.
    #[test]
    fn a_boundary_drag_leaves_the_pane_beyond_it_untouched() {
        let mut space = test_scrolling_space();
        space.add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
        space.add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), false);
        space.add_pane_to_column(0, None, Pane::new(PaneId(3), "p3".to_string()), false);

        let before: Vec<f64> = space.columns[0].pane_sizes.iter().map(|s| s.h).collect();
        assert_eq!(before.len(), 3, "three stacked panes");
        let third = before[2];

        // Drag the boundary between pane 0 and pane 1 downwards.
        space.resize_pane_height(0, 0, 60.0);
        let after: Vec<f64> = space.columns[0].pane_sizes.iter().map(|s| s.h).collect();

        assert!((after[0] - (before[0] + 60.0)).abs() < 0.5, "the pane above grew by the drag");
        assert!((after[1] - (before[1] - 60.0)).abs() < 0.5, "…and its neighbour gave exactly that");
        assert!(
            (after[2] - third).abs() < 0.5,
            "the third pane is not on this boundary and must not move: {third} -> {}",
            after[2],
        );
        // The column stays exactly full, so nothing is pushed past its bottom edge.
        let sum_before: f64 = before.iter().sum();
        let sum_after: f64 = after.iter().sum();
        assert!((sum_after - sum_before).abs() < 0.5, "the column is still exactly full");
    }

    /// The far side stops at its floor rather than the drag reaching past it for more space — which
    /// is what let one boundary eat a pane two positions away.
    #[test]
    fn a_boundary_drag_stops_when_its_neighbour_hits_the_floor() {
        let mut space = test_scrolling_space();
        space.add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
        space.add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), false);
        space.add_pane_to_column(0, None, Pane::new(PaneId(3), "p3".to_string()), false);
        let third = space.columns[0].pane_sizes[2].h;

        // Far more than the neighbour can give, repeatedly.
        for _ in 0..20 {
            space.resize_pane_height(0, 0, 500.0);
        }
        let after: Vec<f64> = space.columns[0].pane_sizes.iter().map(|s| s.h).collect();

        assert!(
            after[1] >= crate::layout::column::MIN_PANE_HEIGHT - 0.5,
            "the neighbour never goes below the floor: {}",
            after[1],
        );
        assert!(
            (after[2] - third).abs() < 0.5,
            "and the pane beyond the boundary is still untouched: {third} -> {}",
            after[2],
        );
    }

    /// **The divider goes the way the key says, whichever pane is active** (F004/P084/T414).
    ///
    /// `j` is directional; "grow the active pane" is not. They disagree for the last pane, which
    /// has no boundary beneath it and so grows *upwards* — which is why `prefix+r` felt inverted on
    /// the top and middle panes and correct on the bottom one. Naming a boundary instead of a size
    /// makes one statement of it: a positive amount moves that boundary **down**, always.
    #[test]
    fn the_keyboard_moves_a_divider_the_same_way_from_every_pane() {
        // Each seat in a three-pane column, and the boundary each one owns.
        for (active, boundary) in [(0usize, 0usize), (1, 1), (2, 1)] {
            let mut space = test_scrolling_space();
            space.add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
            space.add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), false);
            space.add_pane_to_column(0, None, Pane::new(PaneId(3), "p3".to_string()), false);

            let before: Vec<f64> = space.columns[0].pane_sizes.iter().map(|s| s.h).collect();
            let (h, gaps) = (
                space.working_area.size.h,
                space.options.gaps,
            );
            let col = &mut space.columns[0];
            col.active_pane_idx = active;
            col.move_active_pane_boundary(40.0, h, gaps);
            let after: Vec<f64> = col.pane_sizes.iter().map(|s| s.h).collect();

            // A boundary moving DOWN grows the pane above it and shrinks the pane below it — the
            // same two panes, by the same amount, from whichever seat the key was pressed.
            assert!(
                (after[boundary] - (before[boundary] + 40.0)).abs() < 0.5,
                "active {active}: the pane above the boundary grew, {} -> {}",
                before[boundary],
                after[boundary],
            );
            assert!(
                (after[boundary + 1] - (before[boundary + 1] - 40.0)).abs() < 0.5,
                "active {active}: the pane below it gave exactly that, {} -> {}",
                before[boundary + 1],
                after[boundary + 1],
            );
            // The third pane is not on this boundary (T413's rule still holds).
            let untouched = if boundary == 0 { 2 } else { 0 };
            assert!(
                (after[untouched] - before[untouched]).abs() < 0.5,
                "active {active}: pane {untouched} is not on this boundary and must not move",
            );
        }
    }

    /// The size verbs keep meaning size. `pane_height_increase` says "increase", so it grows the
    /// active pane whichever edge has to move — the opposite reading to the directional one above,
    /// and both are correct for the words they are spelled with.
    #[test]
    fn the_size_verb_still_grows_the_active_pane_from_every_seat() {
        for active in [0usize, 1, 2] {
            let mut space = test_scrolling_space();
            space.add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
            space.add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), false);
            space.add_pane_to_column(0, None, Pane::new(PaneId(3), "p3".to_string()), false);

            let before: Vec<f64> = space.columns[0].pane_sizes.iter().map(|s| s.h).collect();
            let (h, gaps) = (space.working_area.size.h, space.options.gaps);
            let col = &mut space.columns[0];
            col.active_pane_idx = active;
            col.resize_active_pane_height(40.0, h, gaps);

            assert!(
                (col.pane_sizes[active].h - (before[active] + 40.0)).abs() < 0.5,
                "active {active}: the active pane grew, {} -> {}",
                before[active],
                col.pane_sizes[active].h,
            );
        }
    }

    #[test]
    fn columns_can_be_zoomed_independently() {
        let mut space = test_scrolling_space();
        space.add_column(None, test_column(1, ColumnWidth::Proportion(0.4)), true);
        space.add_column(None, test_column(2, ColumnWidth::Proportion(0.6)), false);

        assert!(space.toggle_active_column_zoom());
        assert!(space.columns[0].is_zoomed());
        assert_eq!(
            space.columns[0].zoom_restore_width,
            Some(ColumnWidth::Proportion(0.4))
        );

        space.activate_column(1);
        assert!(space.toggle_active_column_zoom());

        assert!(space.columns[0].is_zoomed());
        assert_eq!(
            space.columns[0].zoom_restore_width,
            Some(ColumnWidth::Proportion(0.4))
        );
        assert!(space.columns[1].is_zoomed());
        assert_eq!(
            space.columns[1].zoom_restore_width,
            Some(ColumnWidth::Proportion(0.6))
        );

        assert!(space.toggle_active_column_zoom());
        assert!(space.columns[0].is_zoomed());
        assert!(!space.columns[1].is_zoomed());
        assert_eq!(space.columns[1].width, ColumnWidth::Proportion(0.6));
    }

    #[test]
    fn scroll_view_pans_and_clamps_to_content_bounds() {
        // Three ~half-viewport columns overflow the 1000px viewport, so the view
        // can pan — but only within the content (never scrolls the layout away).
        let mut space = space_with_columns(3);
        space.update_all_column_widths();
        let vw = space.working_area.size.w;

        // Pan hard left, then again → second is a no-op (already at the left bound).
        space.scroll_view(-vw * 10.0);
        let left_bound = space.view_pos();
        space.scroll_view(-vw * 10.0);
        assert!(
            (space.view_pos() - left_bound).abs() < 1.0,
            "clamped at the left content bound"
        );

        // Pan hard right, then again → clamped at the right bound.
        space.scroll_view(vw * 10.0);
        let right_bound = space.view_pos();
        space.scroll_view(vw * 10.0);
        assert!(
            (space.view_pos() - right_bound).abs() < 1.0,
            "clamped at the right content bound"
        );
        assert!(
            right_bound > left_bound,
            "the right bound is further right than the left bound"
        );
    }

    #[test]
    fn scroll_view_is_noop_when_all_columns_fit() {
        let mut space = test_scrolling_space();
        space.add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
        space.update_all_column_widths();
        let before = space.view_pos();
        space.scroll_view(500.0);
        assert!(
            (space.view_pos() - before).abs() < f64::EPSILON,
            "a single column that fits the viewport does not scroll"
        );
    }

    #[test]
    fn refocusing_active_column_refits_a_scrolled_view() {
        // After panning the view away, re-activating the already-active column must
        // scroll it back into view (#3: focusing a stranded column reveals it).
        let mut space = space_with_columns(3);
        space.update_all_column_widths();
        let active = space.active_column_idx;
        let fitted = space.view_pos();
        // Pan far away so the active column is off-screen.
        space.scroll_view(-space.working_area.size.w * 10.0);
        assert!(
            (space.view_pos() - fitted).abs() > 1.0,
            "precondition: the view has moved away from the active column"
        );
        // Re-activating the same column re-fits it.
        space.activate_column(active);
        assert!(
            space.view_offset.is_static(),
            "ensure-visible snaps the view statically"
        );
    }
}
