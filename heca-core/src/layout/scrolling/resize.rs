//! Resizing columns and panes, and zooming a column.

use super::*;

impl ScrollingMut<'_> {
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

        let before = self.reader().positions();
        let Some(effect) = self.space.zoom_column(idx) else {
            return false;
        };
        self.view.react(&*self.space, effect, &before);
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
        let active = self.view.active_column;
        let Some(moved) = self.space.left_boundary_transfer(active, delta, self.view.area.size.w)
        else {
            return;
        };
        let left = active - 1;
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

        let before = self.reader().positions();
        if let Some((_, new_width)) = self.reader().clamped_width(idx, delta)
            && let Some(effect) = self.space.set_column_width(idx, new_width)
        {
            self.view.react(&*self.space, effect, &before);
        } else if idx == self.view.active_column {
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
        self.space.clamped_width(idx, delta, self.view.area.size.w)
    }
}
