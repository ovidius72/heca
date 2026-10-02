//! The view of a scrolling space: where it is looking, and the moves that change what it shows.

use super::super::animation::Animated;
use super::*;

impl ScrollingSpace {
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
            if let Animated::Animating { ref animation, .. } = col.move_offset
                && animation.is_done()
            {
                col.move_offset = Animated::Static(0.0);
            }
            // Advance pane Y-move animations.
            for pane in &mut col.panes {
                if let Animated::Animating { ref animation, .. } =
                    pane.move_offset
                    && animation.is_done()
                {
                    pane.move_offset =
                        Animated::Static(Point::default());
                }
            }
        }
    }

    /// Check if any animations are ongoing.
    pub fn are_animations_ongoing(&self) -> bool {
        self.view_offset.is_animation_ongoing()
            || self.columns.iter().any(|c| {
                matches!(c.move_offset, Animated::Animating { .. })
                    || c.panes.iter().any(|p| {
                        matches!(p.move_offset, Animated::Animating { .. })
                    })
            })
    }
}
