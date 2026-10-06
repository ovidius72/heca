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
}

/// Where the columns and the view were, taken **before** a change, so a window can tell how far
/// everything has to move to show it.
#[derive(Clone, Debug, PartialEq)]
pub struct Positions {
    /// Each column's x by identity.
    pub columns: Vec<(ColumnId, f64)>,
    /// How far the view was scrolled.
    pub view_pos: f64,
}

impl ScrollingRef<'_> {
    /// Where things are now, to hold against where they are after a change.
    pub fn positions(&self) -> Positions {
        Positions {
            columns: self.capture_column_positions(),
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
        Some(ColumnEffect::Resized { idx })
    }
}
