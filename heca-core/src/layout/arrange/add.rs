//! Making a pane: in a column, or in a column of its own. Nothing runs in it yet.

use super::{Arranged, Record};
use crate::layout::scrolling::SpaceEffect;
use crate::layout::{ColumnId, Pane, PaneId, Rectangle, Session};

/// Where a pane that was made landed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Added {
    /// The new pane.
    pub pane: PaneId,
    /// Its workspace.
    pub workspace: usize,
    /// Its column.
    pub column: usize,
    /// What making it did to its workspace.
    pub arranged: Arranged,
}

impl Session {
    /// Make a pane at the bottom of column `col` of workspace `ws` — the last column, when `col` is
    /// past it; a column of its own when there is none — making room for it in a window
    /// `working_height` tall. `None` when the workspace is not there.
    pub fn add_pane_to_column(
        &mut self,
        ws: usize,
        col: usize,
        working_height: f64,
    ) -> Option<Added> {
        let count = self.workspaces.get(ws)?.scrolling.columns.len();
        match count {
            0 => self.add_column(ws, 0),
            _ => {
                let pane = Pane::new(PaneId(self.next_id()), String::new());
                let id = pane.id;
                let effect = self.workspaces[ws].scrolling.insert_pane(
                    col.min(count - 1),
                    None,
                    pane,
                    working_height,
                )?;
                self.added(ws, id, SpaceEffect::Pane(effect))
            }
        }
    }

    /// Make a pane in a new column of workspace `ws`, at index `at` (the end, when past it).
    /// `None` when the workspace is not there.
    pub fn add_column(&mut self, ws: usize, at: usize) -> Option<Added> {
        self.workspaces.get(ws)?;
        let pane = Pane::new(PaneId(self.next_id()), String::new());
        self.add_column_with(ws, at, pane)
    }

    /// Put `pane` in a new column of workspace `ws`, at index `at` (the end, when past it).
    /// `None` when the workspace is not there.
    pub fn add_column_with(&mut self, ws: usize, at: usize, pane: Pane) -> Option<Added> {
        self.workspaces.get(ws)?;
        let id = pane.id;
        let new_column = ColumnId(self.next_id());
        let space = &mut self.workspaces[ws].scrolling;
        let column = space.new_column(new_column, pane);
        let effect = space.insert_column(at, column);
        self.added(ws, id, SpaceEffect::Column(effect))
    }

    /// Float `pane` in workspace `ws` at `rect`, in front and focused. Nothing in the columns
    /// changes. `None` when the workspace is not there.
    pub fn add_floating(&mut self, ws: usize, pane: Pane, rect: Rectangle) -> Option<Added> {
        let id = pane.id;
        self.workspaces
            .get_mut(ws)?
            .add_floating_pane(pane, rect, None);
        Some(Added {
            pane: id,
            workspace: ws,
            column: 0,
            arranged: Arranged::default(),
        })
    }

    /// What a pane made in workspace `ws` says.
    fn added(&self, ws: usize, pane: PaneId, effect: SpaceEffect) -> Option<Added> {
        let column = SpaceEffect::landing(&[effect]).map_or(0, |(col, _)| col);
        let mut record = Record::default();
        record.note(self.workspaces.get(ws)?.id, [effect]);
        Some(Added {
            pane,
            workspace: ws,
            column,
            arranged: record.finish(self),
        })
    }
}
