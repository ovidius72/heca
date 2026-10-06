//! **What a change to the columns did** — plain data the content reports and a window reacts to.
//!
//! The content changes by index and says what happened ([`ColumnEffect`]); where a window looks,
//! how far it is scrolled and what it animates are the window's own business, worked out from the
//! effect and from where things were before it ([`Positions`]).

use super::*;

/// What a change did to a workspace's columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnEffect {
    /// A column now sits at `idx`; the ones from there on moved one place along.
    Inserted { idx: usize },
    /// The column that was at `idx` is gone; the ones after it moved one place back.
    Removed { idx: usize },
    /// The column at `from` is now at `to`.
    Moved { from: usize, to: usize },
    /// The columns at `a` and `b` traded places.
    Swapped { a: usize, b: usize },
    /// The column at `idx` has a different width.
    Resized { idx: usize },
    /// The column at `idx` was zoomed to the viewport, or restored from it.
    Zoomed { idx: usize },
}

/// Where the columns and the view were, taken **before** a change, so a window can tell how far
/// everything has to move to show it.
#[derive(Clone, Debug, PartialEq)]
pub struct Positions {
    /// Each column's x by identity.
    pub columns: Vec<(ColumnId, f64)>,
    /// Where a column one past the last would start.
    pub end: f64,
    /// How far the view was scrolled.
    pub view_pos: f64,
}

impl ScrollingRef<'_> {
    /// Where things are now, to hold against where they are after a change.
    pub fn positions(&self) -> Positions {
        Positions {
            columns: self.capture_column_positions(),
            end: self.column_x(self.space.columns.len()),
            view_pos: self.view_pos(),
        }
    }
}

impl ScrollingSpace {
    /// Put `column` at `idx` (the end when past it).
    pub fn insert_column(&mut self, idx: usize, column: Column) -> ColumnEffect {
        let idx = idx.min(self.columns.len());
        self.columns.insert(idx, column);
        ColumnEffect::Inserted { idx }
    }

    /// Take the column at `idx` out, whole.
    pub fn take_column(&mut self, idx: usize) -> Option<(Column, ColumnEffect)> {
        (idx < self.columns.len()).then(|| (self.columns.remove(idx), ColumnEffect::Removed { idx }))
    }

    /// Move the column at `from` to `to` (clamped to the last place). `None` when there is
    /// nothing to move.
    pub fn move_column(&mut self, from: usize, to: usize) -> Option<ColumnEffect> {
        if from >= self.columns.len() {
            return None;
        }
        let to = to.min(self.columns.len() - 1);
        if from == to {
            return None;
        }
        let column = self.columns.remove(from);
        self.columns.insert(to, column);
        Some(ColumnEffect::Moved { from, to })
    }

    /// Trade the places of the columns at `a` and `b`. `None` when either is not there or they
    /// are the same column.
    pub fn swap_columns(&mut self, a: usize, b: usize) -> Option<ColumnEffect> {
        let len = self.columns.len();
        if a >= len || b >= len || a == b {
            return None;
        }
        self.columns.swap(a, b);
        Some(ColumnEffect::Swapped { a, b })
    }

    /// Give the column at `idx` this width. A resize clears a zoom and a full-width mark: the
    /// width asked for is the width it has.
    pub fn set_column_width(&mut self, idx: usize, width: ColumnWidth) -> Option<ColumnEffect> {
        let column = self.columns.get_mut(idx)?;
        column.width = width;
        column.zoom_restore_width = None;
        column.is_full_width = false;
        Some(ColumnEffect::Resized { idx })
    }

    /// Toggle the column at `idx` between viewport-wide zoom and its previous width.
    pub fn zoom_column(&mut self, idx: usize) -> Option<ColumnEffect> {
        let column = self.columns.get_mut(idx)?;
        match column.is_zoomed() {
            true => {
                if let Some(previous) = column.zoom_restore_width.take() {
                    column.width = previous;
                }
            }
            false => column.zoom_restore_width = Some(column.width),
        }
        Some(ColumnEffect::Zoomed { idx })
    }
}

impl ScrollView {
    /// **Show a change to the columns in this window**: keep the active column and the scroll
    /// where they were on screen, and slide what moved. `space` is the content after the change;
    /// `before` is [`ScrollingRef::positions`] taken before it. Every window reacts to the same
    /// effect for itself, so what one window scrolls and animates is never another's business.
    pub fn react(&mut self, space: &ScrollingSpace, effect: ColumnEffect, before: &Positions) {
        match effect {
            ColumnEffect::Inserted { idx } => self.react_to_insert(space, idx),
            ColumnEffect::Removed { idx } => self.react_to_removal(space, idx, before),
            ColumnEffect::Moved { to, .. } => {
                // The moved column stays active, and the layout stays visually fixed.
                self.active_column = to;
                self.keep_view_in_place(space, before);
                self.slide_to_new_positions(space, before);
                // The column you moved is the active one: keep it on screen, as focus does.
                self.align_to_active(space);
            }
            ColumnEffect::Swapped { .. } => self.slide_to_new_positions(space, before),
            ColumnEffect::Resized { idx } => {
                // Anchor on the resized column's left edge so its right edge — the divider being
                // dragged — tracks the cursor, whichever column is active.
                let old_rel = before.columns.get(idx).map_or(0.0, |(_, x)| *x) - before.view_pos;
                let new_rel = {
                    let now = space.through(self);
                    now.column_x(idx) - now.view_pos()
                };
                self.offset.offset(new_rel - old_rel);
                // Keep the active column reachable — only when it is the one resized, so a drag
                // on another column does not yank the view.
                if idx == self.active_column && !space.columns.is_empty() && self.offset.is_static()
                {
                    let fit = space.through(self).compute_view_offset_for_column(self.active_column, None);
                    self.offset = ViewOffset::Static(fit);
                }
            }
            ColumnEffect::Zoomed { .. } => {
                // The zoomed column is the active one: keep it on screen.
                self.align_to_active(space);
                self.slide_to_new_positions(space, before);
            }
        }
    }

    fn react_to_insert(&mut self, space: &ScrollingSpace, idx: usize) {
        let was_empty = space.columns.len() == 1;
        if !was_empty && idx <= self.active_column {
            self.active_column += 1;
        }
        let offset = {
            let now = space.through(self);
            now.column_x(idx + 1) - now.column_x(idx)
        };
        let (others, signed) = match self.active_column <= idx {
            true => (&space.columns[idx + 1..], -offset),
            false => (&space.columns[..idx], offset),
        };
        for column in others {
            self.motion.slide_column(column.id, signed, AnimationConfig::default());
        }
    }

    fn react_to_removal(&mut self, space: &ScrollingSpace, idx: usize, before: &Positions) {
        let x_of = |at: usize| before.columns.get(at).map_or(before.end, |(_, x)| *x);
        let offset = x_of(idx + 1) - x_of(idx);
        let (others, signed) = match self.active_column <= idx {
            true => (&space.columns[idx.min(space.columns.len())..], offset),
            false => (&space.columns[..idx.min(space.columns.len())], -offset),
        };
        for column in others {
            self.motion.slide_column(column.id, signed, AnimationConfig::default());
        }
        if space.columns.is_empty() {
            self.active_column = 0;
        } else if idx < self.active_column {
            self.active_column -= 1;
            self.activate_prev_on_removal = None;
        } else if idx == self.active_column {
            // Activate the previous column or the next.
            let new_idx = match self.activate_prev_on_removal.is_some() && idx > 0 {
                true => idx - 1,
                false => idx.min(space.columns.len() - 1),
            };
            self.active_column = new_idx;
            self.activate_prev_on_removal = None;
            let fit = space.through(self).compute_view_offset_for_column(new_idx, None);
            self.offset = ViewOffset::Static(fit);
        }
    }

    /// Bring the active column fully into view, easing there unless it is already (within a
    /// pixel).
    pub(crate) fn align_to_active(&mut self, space: &ScrollingSpace) {
        let target = space.through(self).compute_view_offset_for_column(self.active_column, None);
        let current = self.offset.current();
        match (target - current).abs() < 1.0 / self.scale {
            true => self.offset = ViewOffset::Static(target),
            false => {
                self.offset = ViewOffset::Animation(Animation::new(
                    current,
                    target,
                    AnimationConfig::default(),
                ));
            }
        }
    }

    /// Shift the scroll by how far the strip moved under it, so the layout stays visually fixed.
    fn keep_view_in_place(&mut self, space: &ScrollingSpace, before: &Positions) {
        let now = space.through(self).view_pos();
        self.offset.offset(before.view_pos - now);
    }

    /// Slide every column from where it was to where it is now.
    fn slide_to_new_positions(&mut self, space: &ScrollingSpace, before: &Positions) {
        let now: Vec<f64> = space.through(self).column_xs().collect();
        for (i, column) in space.columns.iter().enumerate() {
            let was = before
                .columns
                .iter()
                .find(|(id, _)| *id == column.id)
                .map_or(now[i], |(_, x)| *x);
            let diff = was - now[i];
            if diff.abs() > 0.5 {
                self.motion.slide_column(column.id, diff, AnimationConfig::default());
            }
        }
    }
}
