//! The view of a scrolling space: where it is looking, and the moves that change what it shows.

use super::*;

impl ScrollingRef<'_> {
    /// Current view position (column_x + view_offset).
    pub fn view_pos(&self) -> f64 {
        if self.space.columns.is_empty() {
            return 0.0;
        }
        self.column_x(self.view.active_column) + self.view.offset.current()
    }

    /// Target view position.
    pub fn target_view_pos(&self) -> f64 {
        if self.space.columns.is_empty() {
            return 0.0;
        }
        self.column_x(self.view.active_column) + self.view.offset.target()
    }

    /// Compute the view offset to fit a column into view.
    pub fn compute_view_offset_for_column(&self, idx: usize, prev_idx: Option<usize>) -> f64 {
        if self.space.is_centering_focused_column() {
            return self.compute_view_offset_centered(idx);
        }

        match self.space.options.center_focused_column {
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
                    (idx + 1).min(self.space.columns.len() - 1)
                } else {
                    idx.saturating_sub(1)
                };
                let source_x = self.column_x(source_idx);
                let source_w = self.column_width(source_idx);
                let target_x = self.column_x(idx);
                let target_w = self.column_width(idx);

                let total_width = if source_x < target_x {
                    target_x - source_x + target_w
                } else {
                    source_x - target_x + source_w
                } + self.space.options.gaps * 2.0;

                if total_width <= self.view.area.size.w {
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
        let col_w = self.column_width(idx);
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
            (self.view.area, 0.0)
        } else {
            (self.view.area, self.space.options.gaps)
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
        let col_w = self.column_width(idx);
        let mode = self
            .columns
            .get(idx)
            .map(|c| c.sizing_mode())
            .unwrap_or(SizingMode::Normal);

        if mode.is_fullscreen() {
            return self.compute_view_offset_fit(idx);
        }

        let area = self.view.area;

        // Columns wider than view are left-aligned.
        if area.size.w <= col_w {
            return self.compute_view_offset_fit(idx);
        }

        -(area.size.w - col_w) / 2.0 - area.loc.x
    }

    /// Check if any animations are ongoing.
    pub fn are_animations_ongoing(&self) -> bool {
        self.view.offset.is_animation_ongoing() || self.view.motion.is_animating()
    }
}

impl ScrollingMut<'_> {
    /// Activate a column and animate the view to bring it into view.
    /// NIRI behavior: focus changes scroll the viewport.
    pub fn activate_column(&mut self, idx: usize) {
        if idx >= self.space.columns.len() {
            return;
        }
        if self.view.active_column == idx {
            // Already the active column — but it may have been scrolled/resized
            // out of view, so still re-fit it (#3: re-focusing a stranded column
            // must reveal it instead of doing nothing).
            self.ensure_active_column_visible();
            return;
        }

        let prev_idx = self.view.active_column;
        let new_offset = self.reader().compute_view_offset_for_column(idx, Some(prev_idx));

        // Offset the view to account for column position change.
        let new_col_x = self.reader().column_x(idx);
        let old_col_x = self.reader().column_x(prev_idx);
        self.view.offset.offset(old_col_x - new_col_x);

        // Animate to new view offset.
        let pixel = 1.0 / self.view.scale;
        let to_diff = new_offset - self.view.offset.target();
        if to_diff.abs() < pixel {
            self.view.offset.offset(to_diff);
        } else {
            self.view.offset = ViewOffset::Animation(Animation::new(
                self.view.offset.current(),
                new_offset,
                AnimationConfig::default(),
            ));
        }

        self.view.active_column = idx;
        self.view.activate_prev_on_removal = None;
    }

    /// Scroll the view (statically) so the active column is on-screen. Mirrors the
    /// re-fit that [`update_working_area`](Self::update_working_area) does on a
    /// window resize — used after a manual column resize that pushes the active
    /// column's edge off the viewport, and when re-focusing an already-active
    /// column that was scrolled out of view (#3). A column wider than the viewport
    /// is left-aligned; use [`scroll_view`](Self::scroll_view) to pan across its
    /// overflow. No-op while a view animation/gesture is in flight.
    pub fn ensure_active_column_visible(&mut self) {
        if !self.space.columns.is_empty() && self.view.offset.is_static() {
            let offset = self.reader().compute_view_offset_for_column(self.view.active_column, None);
            self.view.offset = ViewOffset::Static(offset);
        }
    }

    /// Pan the view horizontally by `delta` logical px (positive = reveal content
    /// to the **right**), clamped to the content bounds so it never scrolls the
    /// whole layout off-screen. Lets the user reach column overflow / content
    /// scrolled past an edge (#3). Snaps statically for responsiveness; no-op when
    /// all columns already fit the viewport.
    pub fn scroll_view(&mut self, delta: f64) {
        if self.space.columns.is_empty() {
            return;
        }
        let vw = self.view.area.size.w;
        let last = self.space.columns.len() - 1;
        let first_left = self.reader().column_x(0);
        let last_right = self.reader().column_x(last) + self.reader().column_width(last);
        // Everything fits → nothing to scroll.
        if last_right - first_left <= vw {
            return;
        }
        // view_pos() is the left edge of the viewport in column space.
        let min_view = first_left; // column 0 left-aligned
        let max_view = last_right - vw; // last column right-aligned
        let new_view = (self.reader().view_pos() + delta).clamp(min_view, max_view);
        let new_offset = new_view - self.reader().column_x(self.view.active_column);
        self.view.offset = ViewOffset::Static(new_offset);
    }

    /// Pan the view for a drag held at the window's edge. Unlike [`scroll_view`](Self::scroll_view)
    /// it may reveal every column before the active one, and scrolls until the last column's edge
    /// meets the right edge of the area (plus a gap of padding) — a drag has to be able to reach
    /// any column to drop on it.
    pub fn edge_scroll(&mut self, delta: f64) {
        let columns = &self.space.columns;
        let gaps = self.space.options.gaps;
        let active = self.view.active_column;
        let total_w: f64 = (0..self.space.columns.len()).map(|i| self.reader().column_width(i)).sum();
        let content_w = total_w + gaps * columns.len().max(1) as f64;
        let before_w: f64 = (0..active).map(|i| self.reader().column_width(i)).sum();
        // Left extent: reveal all columns before the active one.
        let min_view = -(before_w + active as f64 * gaps + gaps);
        // Right extent: the last content edge aligns with the right edge of the area.
        let max_view = (content_w - self.view.area.size.w + gaps).max(min_view);
        let current = self.view.offset.current();
        let new_off = (current + delta).clamp(min_view, max_view);
        self.view.offset.offset(new_off - current);
    }

    /// Align the view so the active column is fully visible.
    /// If the column is off-screen, animate the view to bring it into view.
    pub fn align_view_to_active_column(&mut self) {
        let idx = self.view.active_column;
        let target_offset = self.reader().compute_view_offset_for_column(idx, None);
        let current_offset = self.view.offset.current();
        let pixel = 1.0 / self.view.scale;
        let diff = target_offset - current_offset;
        if diff.abs() < pixel {
            self.view.offset = ViewOffset::Static(target_offset);
        } else {
            self.view.offset = ViewOffset::Animation(Animation::new(
                current_offset,
                target_offset,
                AnimationConfig::default(),
            ));
        }
    }

    /// Update the working area (e.g., on resize).
    pub fn update_working_area(&mut self, working_area: Rectangle) {
        self.view.area = working_area;
        if !self.space.columns.is_empty() && self.view.offset.is_static() {
            let offset = self.reader().compute_view_offset_for_column(self.view.active_column, None);
            self.view.offset = ViewOffset::Static(offset);
        }
    }

    /// Advance animations for all columns, panes, and the view offset.
    pub fn advance_animations(&mut self) {
        // Advance view offset animation.
        if let ViewOffset::Animation(anim) = &self.view.offset
            && anim.is_done()
        {
            self.view.offset = ViewOffset::Static(anim.target());
        }

        // Forget the slides that finished, and those of columns and panes that are gone.
        let space = &*self.space;
        self.view.motion.advance(
            |id| space.columns.iter().any(|c| c.id == id),
            |id| space.columns.iter().any(|c| c.panes.iter().any(|p| p.id == id)),
        );
    }
}
