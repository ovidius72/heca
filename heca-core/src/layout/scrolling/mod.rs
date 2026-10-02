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
        let Some((src_col, pane_idx)) = self.pane_indices(pane_id) else {
            return false;
        };
        if self.columns[src_col].panes.len() <= 1 {
            return false;
        }
        let Some(pane) = self.remove_pane(src_col, pane_idx) else {
            return false;
        };
        let column = self.new_column(new_id, pane);
        self.add_column(Some(src_col + 1), column, true);
        true
    }

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

        // **Room first.** A pane that is about to exist needs somewhere to be, and heights other
        // panes were dragged to are preferences that give way to that.
        col.make_room_for_one_more(self.working_area.size.h, self.options.gaps);
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
    /// A new column holding `pane`, at the layout's
    /// [`default_column_width`](LayoutOptions::default_column_width).
    pub fn new_column(&self, id: ColumnId, pane: Pane) -> Column {
        Column::new(id, pane, self.options.default_column_width)
    }

    /// Where pane `pane_id` sits: its column index and its row within that column.
    pub fn pane_indices(&self, pane_id: PaneId) -> Option<(usize, usize)> {
        self.columns.iter().enumerate().find_map(|(ci, col)| {
            col.panes
                .iter()
                .position(|p| p.id == pane_id)
                .map(|pi| (ci, pi))
        })
    }

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
    ///
    /// This moves the column's **right** edge — its left one is where the column before it ends.
    /// To move that boundary instead, use
    /// [`move_active_column_left_boundary`](Self::move_active_column_left_boundary).
    pub fn resize_active_column(&mut self, delta: f64) {
        self.resize_column(self.active_column_idx, delta);
    }

    /// Move the boundary **to the left of** the active column by `delta`, positive being *right*
    /// like every other resize verb — so a positive delta shrinks the active column from the left
    /// and a negative one grows it leftwards.
    ///
    /// # A boundary moves space between its OWN two columns
    ///
    /// The same rule [`Column::resize_pane_height`] states for panes, and for the same reason: the
    /// boundary belongs to *both* the column before it and the one after, so both change by equal
    /// and opposite amounts and the active column's far edge stays exactly where it was.
    ///
    /// Resizing only the column on the left is **not** this: that widens the neighbour and shoves
    /// the active column sideways without changing it at all — which is what the first version did,
    /// and it reads as resizing the wrong column (Antonio, driving, 2026-09-04).
    ///
    /// The transfer is clamped **once, by whichever side runs out first**, so the two can never
    /// disagree: if the neighbour cannot grow any further, the boundary simply stops.
    ///
    /// **No-op for the first column**, which has nothing to its left to trade with.
    pub fn move_active_column_left_boundary(&mut self, delta: f64) {
        let Some(left) = self.active_column_idx.checked_sub(1) else {
            return;
        };
        let active = self.active_column_idx;
        // What each side can actually take. The boundary moves by the smaller of the two, so
        // neither column is asked for room it does not have.
        let grow = self.achievable_width_delta(left, delta);
        let shrink = self.achievable_width_delta(active, -delta);
        let moved = if delta >= 0.0 {
            grow.min(-shrink)
        } else {
            grow.max(-shrink)
        };
        if moved == 0.0 {
            return;
        }
        self.resize_column(left, moved);
        self.resize_column(active, -moved);
    }

    /// **How much of `delta` column `idx` can actually take**, as a proportion delta — the same
    /// clamp [`resize_column`](Self::resize_column) applies, asked in advance.
    ///
    /// It exists so a two-sided move can be clamped **once** rather than applied twice and left
    /// inconsistent when only one side hits its limit.
    fn achievable_width_delta(&self, idx: usize, delta: f64) -> f64 {
        let Some(col) = self.columns.get(idx) else {
            return 0.0;
        };
        let working_w = self.working_area.size.w;
        let gaps = self.options.gaps;
        let available_width = (working_w - gaps * 2.0).max(MIN_COLUMN_WIDTH);
        let min_prop = ((MIN_COLUMN_WIDTH + gaps) / (working_w - gaps)).clamp(0.01, 1.0);
        let base_width = if col.is_zoomed() {
            ColumnWidth::Fixed(available_width)
        } else {
            col.width
        };
        match base_width {
            ColumnWidth::Proportion(p) => (p + delta).clamp(min_prop, 1.0) - p,
            ColumnWidth::Fixed(w) => {
                let target = (w + delta * working_w).clamp(MIN_COLUMN_WIDTH, available_width);
                (target - w) / working_w
            }
        }
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
        self.laid_out_columns()
            .into_iter()
            .flat_map(|col| col.panes)
            .map(|p| (p.id, p.rect))
            .collect()
    }

    /// **Where each column is on screen**, with the panes inside it — one walk, so a caller asking
    /// about a column and a caller asking about a pane cannot disagree about where either sits.
    ///
    /// The column geometry was always computed here and then thrown away: only the pane rects came
    /// out, and anything wanting a column's box had to rebuild the same arithmetic from
    /// [`column_x`](Self::column_x), the render offset, the view offset and the gaps.
    pub fn columns_with_positions(&self) -> Vec<LaidOutColumn> {
        self.laid_out_columns()
    }

    /// The one walk both public views are built on.
    fn laid_out_columns(&self) -> Vec<LaidOutColumn> {
        let view_off = Point::new(-self.view_pos(), 0.0);
        let gaps = self.options.gaps;

        self.columns
            .iter()
            .enumerate()
            .map(|(col_idx, col)| {
                let col_pos = Point::new(self.column_x(col_idx) + col.render_offset(), 0.0);
                let mut pane_y = self.working_area.loc.y + gaps;
                let mut panes = Vec::with_capacity(col.panes.len());

                for (pane_idx, pane) in col.panes.iter().enumerate() {
                    let size = col.pane_sizes.get(pane_idx).copied().unwrap_or(Size::new(
                        col.computed_width,
                        self.working_area.size.h / col.panes.len().max(1) as f64,
                    ));

                    // **Flow, then transform.** The slot is where the column stacks this pane; the
                    // displacement is the pane's own, from a move animation or a drag in flight.
                    // They are returned apart because a container places its children by the first
                    // and the child carries the second — the same split CSS makes between layout
                    // and `transform`.
                    let pane_offset = pane.move_offset.current();
                    let rubber = pane.interactive_move_offset;
                    let slot = view_off + col_pos + Point::new(0.0, pane_y);
                    let displacement =
                        Point::new(pane_offset.x + rubber.x, pane_offset.y + rubber.y);
                    panes.push(LaidOutPane {
                        id: pane.id,
                        rect: Rectangle::new(slot + displacement, size),
                        slot: Rectangle::new(slot, size),
                        displacement,
                    });

                    pane_y += size.h + gaps;
                }

                LaidOutColumn {
                    id: col.id,
                    idx: col_idx,
                    rect: enclosing(&panes).unwrap_or_else(|| {
                        // An empty column has no panes to span, so it is its own width at the top
                        // of the working area — the box it would occupy the moment one arrives.
                        Rectangle::new(
                            view_off + col_pos + Point::new(0.0, self.working_area.loc.y + gaps),
                            Size::new(col.computed_width, 0.0),
                        )
                    }),
                    panes,
                }
            })
            .collect()
    }
}

/// **One column as it is laid out on screen**, and the panes stacked inside it.
///
/// `rect` spans the **slots** it holds — a column is the box its contents occupy in the layout, so
/// a pane being dragged away does not stretch the column it is leaving.
#[derive(Clone, Debug, PartialEq)]
pub struct LaidOutColumn {
    /// The column's own identity — stable across a split, unlike its index.
    pub id: ColumnId,
    /// Its position in the strip, for the actions that still address a column positionally.
    pub idx: usize,
    /// The box it occupies, in the same space [`ScrollingSpace::panes_with_positions`] reports.
    pub rect: Rectangle,
    /// Its panes, top to bottom.
    pub panes: Vec<LaidOutPane>,
}

/// **One pane as it is laid out**, with its flow position and its own displacement kept apart.
///
/// `slot` is where its column stacks it; `displacement` is what a move animation or a drag in
/// flight has shifted it by; `rect` is the two together, which is where it is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaidOutPane {
    pub id: PaneId,
    /// Where it is drawn — `slot` plus `displacement`.
    pub rect: Rectangle,
    /// Where its column stacks it, before any displacement of its own.
    pub slot: Rectangle,
    /// Its own offset, from a move animation or a drag in flight.
    pub displacement: Point,
}

/// The smallest box containing every pane's **slot** — `None` when there are none.
fn enclosing(panes: &[LaidOutPane]) -> Option<Rectangle> {
    let mut it = panes.iter().map(|p| p.slot);
    let first = it.next()?;
    Some(it.fold(first, |acc, r| {
        let x = acc.loc.x.min(r.loc.x);
        let y = acc.loc.y.min(r.loc.y);
        let right = (acc.loc.x + acc.size.w).max(r.loc.x + r.size.w);
        let bottom = (acc.loc.y + acc.size.h).max(r.loc.y + r.size.h);
        Rectangle::new(Point::new(x, y), Size::new(right - x, bottom - y))
    }))
}

#[cfg(test)]
mod tests;
