//! Resizing columns and panes, and zooming a column.

use super::*;

impl ScrollingMut<'_> {
    fn finish_column_width_change(
        &mut self,
        column: usize,
        old_xs: &[(ColumnId, f64)],
        old_view_pos: f64,
    ) {
        // Preserve view position so layout stays visually fixed during width changes.
        let new_view_pos = self.reader().view_pos();
        let view_delta = old_view_pos - new_view_pos;
        self.view.offset.offset(view_delta);

        // Ensure the changed column stays visible after the width change.
        let target_offset = self.reader().compute_view_offset_for_column(column, None);
        let pixel = 1.0 / self.view.scale;
        let diff = target_offset - self.view.offset.target();
        if diff.abs() < pixel {
            self.view.offset.offset(diff);
        } else {
            self.view.offset = ViewOffset::Animation(Animation::new(
                self.view.offset.current(),
                target_offset,
                AnimationConfig::default(),
            ));
        }

        // Animate columns to their new positions.
        let new_xs: Vec<f64> = self.reader().column_xs().collect();
        for (i, col) in self.space.columns.iter_mut().enumerate() {
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
        self.toggle_column_zoom(self.view.active_column)
    }

    /// Toggle the column at `idx` between viewport-wide zoom and its previous width, whichever
    /// column is active. The view follows the zoomed column so it stays visible.
    pub fn toggle_column_zoom(&mut self, idx: usize) -> bool {
        if idx >= self.space.columns.len() {
            return false;
        }

        let old_xs = self.reader().capture_column_positions();
        let old_view_pos = self.reader().view_pos();

        let column = &mut self.space.columns[idx];
        if column.is_zoomed() {
            if let Some(previous_width) = column.zoom_restore_width.take() {
                column.width = previous_width;
            }
        } else {
            column.zoom_restore_width = Some(column.width);
        }

        self.finish_column_width_change(idx, &old_xs, old_view_pos);
        true
    }

    /// Resize the active column by a delta (positive = wider, negative = narrower).
    /// NIRI behavior: only the active column changes. Other columns keep their widths.
    /// If the total exceeds the viewport, the view scrolls horizontally.
    ///
    /// This moves the column's **right** edge — its left one is where the column before it ends.
    /// To move that boundary instead, use
    /// [`move_active_column_left_boundary`](Self::move_active_column_left_boundary).
    pub fn resize_active_column(&mut self, delta: f64) {
        self.resize_column(self.view.active_column, delta);
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
        let Some(left) = self.view.active_column.checked_sub(1) else {
            return;
        };
        let active = self.view.active_column;
        // What each side can actually take. The boundary moves by the smaller of the two, so
        // neither column is asked for room it does not have.
        let grow = self.reader().achievable_width_delta(left, delta);
        let shrink = self.reader().achievable_width_delta(active, -delta);
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

    /// Resize column `idx` by `delta` (a proportion delta for `Proportion` widths,
    /// or a fraction of the working width for `Fixed`). Mutates the column's
    /// **canonical** [`ColumnWidth`] — so the change persists through later
    /// the layout recomputes — and preserves the view position. Used
    /// by the keyboard resize (active column), the mouse divider drag (any column),
    /// and RPC.
    pub fn resize_column(&mut self, idx: usize, delta: f64) {
        if idx >= self.space.columns.len() {
            return;
        }

        // Anchor on the **resized** column's left edge (its on-screen offset from
        // the view) so its right edge — the divider being dragged — tracks the
        // cursor, regardless of which column is active. Anchoring on the *active*
        // column (the old active-column recentre) made a left column
        // grow leftward when the right column was focused (the "wrong side" bug).
        let old_rel = self.reader().column_x(idx) - self.reader().view_pos();

        if let Some((_, new_width)) = self.reader().clamped_width(idx, delta) {
            let col = &mut self.space.columns[idx];
            col.width = new_width;
            col.zoom_restore_width = None;
            col.is_full_width = false;
        }

        // Restore the resized column's on-screen left edge by shifting the view by
        // the amount it moved. No active-column recenter / per-move animation —
        // those fight a smooth per-pixel drag.
        let new_rel = self.reader().column_x(idx) - self.reader().view_pos();
        self.view.offset.offset(new_rel - old_rel);
        // If the resize pushed the active column's far edge off-screen, scroll to
        // keep it reachable (#3) — only when resizing the active column, so a
        // divider drag on another column doesn't yank the view.
        if idx == self.view.active_column {
            self.ensure_active_column_visible();
        }
    }

    /// Resize the active pane's height by `delta` logical px, trading with its neighbour —
    /// the keyboard form of [`resize_pane_height`](Self::resize_pane_height).
    pub fn resize_active_pane_height(&mut self, delta: f64) {
        let (height, gaps) = (self.view.area.size.h, self.space.options.gaps);
        if let Some(col) = self.space.columns.get_mut(self.view.active_column) {
            col.resize_active_pane_height(delta, height, gaps);
        }
    }

    /// Resize the height of pane `pane_idx` within column `col_idx` by `delta`
    /// logical px (mouse divider drag / RPC). No-op for single-pane columns or
    /// out-of-range indices. Mirrors the keyboard `resize_active_pane_height`.
    pub fn resize_pane_height(&mut self, col_idx: usize, pane_idx: usize, delta: f64) {
        let (working_h, gaps) = (self.view.area.size.h, self.space.options.gaps);
        if let Some(col) = self.space.columns.get_mut(col_idx) {
            col.resize_pane_height(pane_idx, delta, working_h, gaps);
        }
    }
}

impl ScrollingRef<'_> {
    /// Column `idx`'s width before and after a resize by `delta`, clamped to what fits: no
    /// narrower than [`MIN_COLUMN_WIDTH`], no wider than the visible area. `None` when there is no
    /// such column.
    ///
    /// **One clamp, asked two ways.** [`ScrollingMut::resize_column`] applies it;
    /// [`achievable_width_delta`](Self::achievable_width_delta) asks it in advance so a two-sided
    /// move can be clamped once.
    fn clamped_width(&self, idx: usize, delta: f64) -> Option<(ColumnWidth, ColumnWidth)> {
        let col = self.space.columns.get(idx)?;
        let working_w = self.view.area.size.w;
        let gaps = self.space.options.gaps;
        // A column may grow to fill the full visible width and shrink no smaller than
        // MIN_COLUMN_WIDTH (so it never becomes a thin line).
        let available_width = (working_w - gaps * 2.0).max(MIN_COLUMN_WIDTH);
        // The proportion that resolves to MIN_COLUMN_WIDTH (see `Column::resolve_width`:
        // width = (working_w - gaps) * p - gaps); `p = 1.0` fills the visible width.
        let min_prop = ((MIN_COLUMN_WIDTH + gaps) / (working_w - gaps)).clamp(0.01, 1.0);
        let base = if col.is_zoomed() {
            ColumnWidth::Fixed(available_width)
        } else {
            col.width
        };
        let new = match base {
            ColumnWidth::Proportion(p) => ColumnWidth::Proportion((p + delta).clamp(min_prop, 1.0)),
            ColumnWidth::Fixed(w) => {
                ColumnWidth::Fixed((w + delta * working_w).clamp(MIN_COLUMN_WIDTH, available_width))
            }
        };
        Some((base, new))
    }

    /// **How much of `delta` column `idx` can actually take**, as a proportion delta — the same
    /// clamp [`resize_column`](ScrollingMut::resize_column) applies, asked in advance.
    ///
    /// It exists so a two-sided move can be clamped **once** rather than applied twice and left
    /// inconsistent when only one side hits its limit.
    fn achievable_width_delta(&self, idx: usize, delta: f64) -> f64 {
        match self.clamped_width(idx, delta) {
            Some((ColumnWidth::Proportion(p), ColumnWidth::Proportion(n))) => n - p,
            Some((ColumnWidth::Fixed(w), ColumnWidth::Fixed(n))) => (n - w) / self.view.area.size.w,
            _ => 0.0,
        }
    }
}
