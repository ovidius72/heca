//! **What a change to the panes did** — the pane operations on the content, which change it by
//! index and say what happened, with no window's view in sight.
//!
//! A move is a removal and an insertion; a window that finds the inserted pane in the
//! before-picture ([`Positions::pane`]) knows it travelled and slides it from there.

use super::{ColumnEffect, Positions, ScrollingMut, ScrollingRef, ScrollingSpace};
use crate::layout::animation::AnimationConfig;
use crate::layout::column::Pane;
use crate::layout::types::{ColumnId, PaneId, Point};
use crate::layout::window_view::ScrollView;

/// What a change did to the panes of one column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneEffect {
    /// A pane now sits at row `row` of column `col`; the ones from there on moved one row down.
    Inserted { col: usize, row: usize },
    /// The pane that was at row `row` of column `col` is gone; the ones after it moved up a row.
    Removed { col: usize, row: usize },
    /// The panes at rows `a` and `b` of column `col` traded places.
    Swapped { col: usize, a: usize, b: usize },
}

/// What a change did to a workspace's columns and panes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceEffect {
    /// A whole column changed: one taken with its last pane counts here.
    Column(ColumnEffect),
    /// A pane changed inside a column that is still there.
    Pane(PaneEffect),
}

impl ScrollingRef<'_> {
    /// Where pane `id` is stacked: its column's x and its y within the column. `None` when it is
    /// not in a column here.
    pub fn pane_slot(&self, id: PaneId) -> Option<Point> {
        let (col, row) = self.space.pane_indices(id)?;
        Some(Point::new(self.column_x(col), self.pane_y_in_column(col, row)))
    }

    /// Every pane's slot, by identity.
    pub(super) fn capture_pane_positions(&self) -> Vec<(PaneId, Point)> {
        self.space
            .columns
            .iter()
            .enumerate()
            .flat_map(|(col, column)| {
                let x = self.column_x(col);
                column
                    .panes
                    .iter()
                    .enumerate()
                    .map(move |(row, pane)| (pane.id, x, col, row))
            })
            .map(|(id, x, col, row)| (id, Point::new(x, self.pane_y_in_column(col, row))))
            .collect()
    }
}

impl ScrollingSpace {
    /// Put `pane` at row `row` of column `col` (the bottom when `None` or past it), first making
    /// room for it in a window `working_height` tall. `None` when there is no such column.
    pub fn insert_pane(
        &mut self,
        col: usize,
        row: Option<usize>,
        pane: Pane,
        working_height: f64,
    ) -> Option<PaneEffect> {
        let gaps = self.options.gaps;
        let column = self.columns.get_mut(col)?;
        let row = row.map_or(column.panes.len(), |row| row.min(column.panes.len()));
        column.make_room_for_one_more(working_height, gaps);
        column.add_pane_at(row, pane);
        Some(PaneEffect::Inserted { col, row })
    }

    /// Take the pane at row `row` of column `col` out. Taking a column's last pane takes the
    /// column with it, and the effect says so. `None` when there is no such pane.
    pub fn take_pane_from(&mut self, col: usize, row: usize) -> Option<(Pane, SpaceEffect)> {
        let count = self.columns.get(col)?.panes.len();
        if row >= count {
            return None;
        }
        if count == 1 {
            let (mut column, effect) = self.take_column(col)?;
            return column.panes.pop().map(|pane| (pane, SpaceEffect::Column(effect)));
        }
        let pane = self.columns[col].remove_pane(row)?;
        Some((pane, SpaceEffect::Pane(PaneEffect::Removed { col, row })))
    }

    /// Move the pane at row `row` of column `from_col` to the bottom of column `to_col`, making
    /// room for it there. `to_col` names the column as it is now, before the move. `None` when
    /// nothing moves.
    pub fn move_pane_between(
        &mut self,
        from_col: usize,
        row: usize,
        to_col: usize,
        working_height: f64,
    ) -> Option<Vec<SpaceEffect>> {
        if from_col == to_col || to_col >= self.columns.len() {
            return None;
        }
        let (pane, taken) = self.take_pane_from(from_col, row)?;
        let to = after_taking(taken, to_col);
        let inserted = self.insert_pane(to, None, pane, working_height)?;
        Some(vec![taken, SpaceEffect::Pane(inserted)])
    }

    /// Take the pane at row `row` of column `col` into a new column `new_column` at `at`, at the
    /// layout's default width. `at` names the place as it is now, before the pane leaves. `None`
    /// when there is no such pane.
    pub fn extract_pane(
        &mut self,
        col: usize,
        row: usize,
        new_column: ColumnId,
        at: usize,
    ) -> Option<Vec<SpaceEffect>> {
        let (pane, taken) = self.take_pane_from(col, row)?;
        let at = after_taking(taken, at);
        let column = self.new_column(new_column, pane);
        Some(vec![taken, SpaceEffect::Column(self.insert_column(at, column))])
    }

    /// Trade the panes at rows `a` and `b` of column `col`; the column's active pane travels with
    /// the swap when it is one of the two. `None` when either is not there or they are the same.
    pub fn swap_panes_in_column(&mut self, col: usize, a: usize, b: usize) -> Option<PaneEffect> {
        let column = self.columns.get_mut(col)?;
        let len = column.panes.len();
        if a >= len || b >= len || a == b {
            return None;
        }
        column.active_pane_idx = match column.active_pane_idx {
            i if i == a => b,
            i if i == b => a,
            i => i,
        };
        column.panes.swap(a, b);
        Some(PaneEffect::Swapped { col, a, b })
    }
}

/// Where column `idx` is once `taken` has happened: one place back when a column before it went.
fn after_taking(taken: SpaceEffect, idx: usize) -> usize {
    match taken {
        SpaceEffect::Column(ColumnEffect::Removed { idx: gone }) if gone < idx => idx - 1,
        _ => idx,
    }
}

impl ScrollView {
    /// **Show a change in this window**: what one effect did, or what several did together (a
    /// move is a removal and an insertion). `space` is the content after the change; `before` is
    /// [`ScrollingRef::positions`] taken before it. Which column and pane to follow is not
    /// decided here: that is the window's own choice ([`follow`](Self::follow)).
    pub fn react_to(&mut self, space: &ScrollingSpace, effects: &[SpaceEffect], before: &Positions) {
        match effects {
            [SpaceEffect::Column(effect)] => self.react(space, *effect, before),
            [SpaceEffect::Pane(effect)] => self.react_to_pane(space, *effect, before),
            _ => self.react_to_move(space, effects, before),
        }
    }

    /// Make column `col` the active one and bring it on screen, as focusing it does.
    pub fn follow(&mut self, space: &ScrollingSpace, col: usize) {
        self.activate_column(space, col);
    }

    fn react_to_pane(&mut self, space: &ScrollingSpace, effect: PaneEffect, before: &Positions) {
        let rows = match effect {
            PaneEffect::Inserted { col, row } => vec![(col, row)],
            PaneEffect::Removed { .. } => Vec::new(),
            PaneEffect::Swapped { col, a, b } => vec![(col, a), (col, b)],
        };
        self.slide_panes_from(space, &pane_ids(space, &rows), before);
        self.slide_to_new_positions(space, before);
    }

    /// Several effects at once: keep the active column on the column it was and the view where it
    /// was, then slide every pane that arrived and every column from where it was.
    fn react_to_move(&mut self, space: &ScrollingSpace, effects: &[SpaceEffect], before: &Positions) {
        let mut arrived = Vec::new();
        for effect in effects {
            match *effect {
                SpaceEffect::Column(ColumnEffect::Inserted { idx }) => {
                    if idx <= self.active_column {
                        self.active_column += 1;
                    }
                    arrived.push((idx, 0));
                }
                SpaceEffect::Column(ColumnEffect::Removed { idx }) if idx < self.active_column => {
                    self.active_column -= 1;
                }
                SpaceEffect::Pane(PaneEffect::Inserted { col, row }) => arrived.push((col, row)),
                _ => {}
            }
        }
        self.active_column = self.active_column.min(space.columns.len().saturating_sub(1));
        // The scroll is measured from the active column, which may now sit somewhere else.
        self.keep_view_in_place(space, before);
        self.slide_panes_from(space, &pane_ids(space, &arrived), before);
        self.slide_to_new_positions(space, before);
    }

    /// Slide each of `panes` from its slot in `before` to its slot now. A pane that was not in a
    /// column before, or has not moved, does not slide.
    fn slide_panes_from(
        &mut self,
        space: &ScrollingSpace,
        panes: &[PaneId],
        before: &Positions,
    ) {
        for &id in panes {
            let (Some(was), Some(now)) = (before.pane(id), space.through(self).pane_slot(id)) else {
                continue;
            };
            let from = Point::new(was.x - now.x, was.y - now.y);
            if from.x.abs() > 0.5 || from.y.abs() > 0.5 {
                self.motion.slide_pane(id, from, AnimationConfig::default());
            }
        }
    }
}

/// The panes at these `(column, row)` places, skipping a place with none.
fn pane_ids(space: &ScrollingSpace, places: &[(usize, usize)]) -> Vec<PaneId> {
    places
        .iter()
        .filter_map(|&(col, row)| space.columns.get(col)?.panes.get(row).map(|p| p.id))
        .collect()
}

impl ScrollingMut<'_> {
    /// Show `effects` in this window, given where things were `before` them.
    pub fn show(&mut self, effects: &[SpaceEffect], before: &Positions) {
        self.view.react_to(&*self.space, effects, before);
    }
}
