//! Swapping panes and columns, wherever they are: nothing is added or removed, two things trade
//! places.

use super::{Arranged, Record};
use crate::layout::scrolling::{ColumnEffect, PaneEffect, SpaceEffect};
use crate::layout::{PaneId, Session};

/// What a place now holding something else says: what was there went, and something arrived.
fn replaced_pane(col: usize, row: usize) -> [SpaceEffect; 2] {
    [
        SpaceEffect::Pane(PaneEffect::Removed { col, row }),
        SpaceEffect::Pane(PaneEffect::Inserted { col, row }),
    ]
}

/// What a column place now holding another column says.
fn replaced_column(idx: usize) -> [SpaceEffect; 2] {
    [
        SpaceEffect::Column(ColumnEffect::Removed { idx }),
        SpaceEffect::Column(ColumnEffect::Inserted { idx }),
    ]
}

impl Session {
    /// Swap two tiled panes: each takes the other's place, in one column, two columns or two
    /// workspaces. A column's active pane travels with a swap inside it. `None` — and nothing
    /// changes — when they are the same pane or either is not in a column.
    pub fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Option<Arranged> {
        if a == b {
            return None;
        }
        let (a_ws, a_col, a_row) = self.pane_location(a)?;
        let (b_ws, b_col, b_row) = self.pane_location(b)?;
        let (a_id, b_id) = (self.workspaces[a_ws].id, self.workspaces[b_ws].id);
        let mut record = Record::default();
        if (a_ws, a_col) == (b_ws, b_col) {
            let effect = self.workspaces[a_ws]
                .scrolling
                .swap_panes_in_column(a_col, a_row, b_row)?;
            record.note(a_id, [SpaceEffect::Pane(effect)]);
            return Some(record.finish(self));
        }
        // Two different columns: lifting `a` out of its column leaves `b`'s place where it is.
        let lifted = self.workspaces[a_ws].scrolling.columns[a_col]
            .panes
            .remove(a_row);
        let other = std::mem::replace(
            &mut self.workspaces[b_ws].scrolling.columns[b_col].panes[b_row],
            lifted,
        );
        self.workspaces[a_ws].scrolling.columns[a_col]
            .panes
            .insert(a_row, other);
        record.note(a_id, replaced_pane(a_col, a_row));
        record.note(b_id, replaced_pane(b_col, b_row));
        Some(record.finish(self))
    }

    /// Swap the columns at `(a_ws, a_col)` and `(b_ws, b_col)`. Within a workspace they trade
    /// places; across two they trade workspaces, so neither workspace empties. `None` when an index
    /// is out of range or both name the same column.
    pub fn swap_columns(
        &mut self,
        a_ws: usize,
        a_col: usize,
        b_ws: usize,
        b_col: usize,
    ) -> Option<Arranged> {
        let has = |ws: usize, col: usize| {
            self.workspaces
                .get(ws)
                .is_some_and(|w| col < w.scrolling.columns.len())
        };
        if !has(a_ws, a_col) || !has(b_ws, b_col) {
            return None;
        }
        let (a_id, b_id) = (self.workspaces[a_ws].id, self.workspaces[b_ws].id);
        let mut record = Record::default();
        if a_ws == b_ws {
            let effect = self.workspaces[a_ws].scrolling.swap_columns(a_col, b_col)?;
            record.note(a_id, [SpaceEffect::Column(effect)]);
            return Some(record.finish(self));
        }
        let lifted = self.workspaces[a_ws].scrolling.columns.remove(a_col);
        let other = std::mem::replace(&mut self.workspaces[b_ws].scrolling.columns[b_col], lifted);
        self.workspaces[a_ws].scrolling.columns.insert(a_col, other);
        record.note(a_id, replaced_column(a_col));
        record.note(b_id, replaced_column(b_col));
        Some(record.finish(self))
    }
}
