//! Moving a pane or a column to another place: another column, another workspace.

use super::{Arranged, Record, after_taking};
use crate::layout::scrolling::{PaneEffect, SpaceEffect};
use crate::layout::{ColumnId, FocusDomain, PaneId, Session};

/// Where a moved pane or column landed, and what the move did to get it there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    /// Its workspace, numbered as they are now.
    pub workspace: usize,
    /// Its column.
    pub column: usize,
    /// What the move did, workspace by workspace.
    pub arranged: Arranged,
}

impl Session {
    /// Move a pane to workspace `dst_ws`. With `join`, and a column at `dst_col`, it stacks at the
    /// bottom of that column; otherwise it becomes a column of its own at `dst_col`
    /// (the end, when that is past the last). Room is made for it in a window `working_height`
    /// tall. `None` when it is already in that workspace or not in a column.
    pub fn move_pane_to_workspace(
        &mut self,
        pane: PaneId,
        dst_ws: usize,
        dst_col: usize,
        join: bool,
        working_height: f64,
    ) -> Option<Moved> {
        let (src_ws, col, row) = self.pane_location(pane)?;
        if src_ws == dst_ws || dst_ws >= self.workspaces.len() {
            return None;
        }
        let new_column = ColumnId(self.next_id());
        let (src_id, dst_id) = (self.workspaces[src_ws].id, self.workspaces[dst_ws].id);
        let mut record = Record::default();
        let (moving, taken) = self.workspaces[src_ws].scrolling.take_pane_from(col, row)?;
        record.note(src_id, [taken]);
        let space = &mut self.workspaces[dst_ws].scrolling;
        let count = space.columns.len();
        let landed = match join && dst_col < count {
            true => space
                .insert_pane(dst_col, None, moving, working_height)
                .map(SpaceEffect::Pane),
            false => {
                let column = space.new_column(new_column, moving);
                Some(SpaceEffect::Column(space.insert_column(dst_col, column)))
            }
        }?;
        record.note(dst_id, [landed]);
        record.remove_if_empty(self, src_ws);
        self.landed(dst_id, landed_column(landed), record)
    }

    /// Move the column at `(src_ws, src_col)` to index `dst_idx` of workspace `dst_ws` (its end,
    /// when that is past the last), which may be its own: that reorders. `None` when nothing moves.
    pub fn move_column(
        &mut self,
        src_ws: usize,
        src_col: usize,
        dst_ws: usize,
        dst_idx: usize,
    ) -> Option<Moved> {
        if dst_ws >= self.workspaces.len() {
            return None;
        }
        let (src_id, dst_id) = (self.workspaces.get(src_ws)?.id, self.workspaces[dst_ws].id);
        let mut record = Record::default();
        if src_ws == dst_ws {
            let effect = self.workspaces[src_ws]
                .scrolling
                .move_column(src_col, dst_idx)?;
            record.note(src_id, [SpaceEffect::Column(effect)]);
            return self.landed(dst_id, landed_column(SpaceEffect::Column(effect)), record);
        }
        let (column, taken) = self.workspaces[src_ws].scrolling.take_column(src_col)?;
        record.note(src_id, [SpaceEffect::Column(taken)]);
        let landed = SpaceEffect::Column(
            self.workspaces[dst_ws]
                .scrolling
                .insert_column(dst_idx, column),
        );
        record.note(dst_id, [landed]);
        record.remove_if_empty(self, src_ws);
        self.landed(dst_id, landed_column(landed), record)
    }

    /// Take a pane from wherever it is — a column, or the floating layer, of any workspace — to
    /// the bottom of column `dst_col` of workspace `dst_ws` (the last column, when that is past it;
    /// a column of its own when there is none), making room for it in a window `working_height` tall. `None` when it is not
    /// anywhere, or is already that column's last pane.
    pub fn take_pane_into(
        &mut self,
        pane: PaneId,
        dst_ws: usize,
        dst_col: usize,
        working_height: f64,
    ) -> Option<Moved> {
        let src_ws = self.workspace_holding(pane)?;
        let dst = self.workspaces.get(dst_ws)?;
        let dst_col = dst_col.min(dst.scrolling.columns.len().saturating_sub(1));
        let tail = dst
            .scrolling
            .columns
            .get(dst_col)
            .and_then(|c| c.panes.last());
        if tail.is_some_and(|p| p.id == pane) {
            return None;
        }
        let (src_id, dst_id) = (self.workspaces[src_ws].id, dst.id);
        let new_column = ColumnId(self.next_id());
        let mut record = Record::default();
        let src = &mut self.workspaces[src_ws];
        let was_floating = src.scrolling.pane_indices(pane).is_none();
        let (moving, taken) = src.take_pane(pane)?;
        if was_floating {
            src.deactivate_floating_panes();
            src.focus_domain = FocusDomain::Tiled;
        }
        record.note(src_id, taken);
        let dst_col = match src_ws == dst_ws {
            true => after_taking(taken, dst_col),
            false => dst_col,
        };
        let space = &mut self.workspaces[dst_ws].scrolling;
        let landed = match space.columns.len() {
            0 => {
                let column = space.new_column(new_column, moving);
                Some(SpaceEffect::Column(space.insert_column(0, column)))
            }
            count => space
                .insert_pane(dst_col.min(count - 1), None, moving, working_height)
                .map(SpaceEffect::Pane),
        }?;
        record.note(dst_id, [landed]);
        if src_ws != dst_ws {
            record.remove_if_empty(self, src_ws);
        }
        self.landed(dst_id, landed_column(landed), record)
    }

    /// Put a pane at row `row` of column `col` of workspace `dst_ws` — or, with no row, in a new
    /// column at gap `col` there — making room for it in a window `working_height`
    /// tall. Within its own workspace the place is counted as the workspace is now, with the pane
    /// still in it ([`Workspace::move_pane`](crate::layout::Workspace::move_pane)). Nothing happens,
    /// and the pane is not touched, when the workspace is not there or the pane is not anywhere.
    pub fn place_pane(
        &mut self,
        pane: PaneId,
        dst_ws: usize,
        col: usize,
        row: Option<usize>,
        working_height: f64,
    ) -> Option<Moved> {
        let dst_id = self.workspaces.get(dst_ws)?.id;
        let src_ws = self.workspace_holding(pane)?;
        let new_column = ColumnId(self.next_id());
        let mut record = Record::default();
        if src_ws == dst_ws {
            let effects =
                self.workspaces[dst_ws].move_pane(pane, col, row, new_column, working_height)?;
            record.note(dst_id, effects);
        } else {
            let src_id = self.workspaces[src_ws].id;
            let (moving, taken) = self.workspaces[src_ws].take_pane(pane)?;
            record.note(src_id, taken);
            let placed =
                self.workspaces[dst_ws].place_pane(moving, col, row, new_column, working_height)?;
            record.note(dst_id, [placed]);
        }
        let column = self.workspaces[dst_ws]
            .scrolling
            .pane_indices(pane)
            .map_or(0, |(c, _)| c);
        self.landed(dst_id, column, record)
    }

    /// What a move that put something in column `column` of workspace `ws` says.
    fn landed(
        &self,
        ws: crate::layout::WorkspaceId,
        column: usize,
        record: Record,
    ) -> Option<Moved> {
        Some(Moved {
            workspace: self.index_of(ws)?,
            column,
            arranged: record.finish(self),
        })
    }
}

/// The column the effect that put a pane or column somewhere put it in.
fn landed_column(effect: SpaceEffect) -> usize {
    use crate::layout::ColumnEffect;
    match effect {
        SpaceEffect::Pane(PaneEffect::Inserted { col, .. }) => col,
        SpaceEffect::Column(
            ColumnEffect::Inserted { idx } | ColumnEffect::Moved { to: idx, .. },
        ) => idx,
        _ => 0,
    }
}
