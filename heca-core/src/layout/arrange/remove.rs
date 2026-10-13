//! Taking things out of the session for good: a pane, a column with its panes, a workspace with
//! everything in it. Each says which panes went, so whatever ran in them can be ended.

use super::{Arranged, Record};
use crate::layout::scrolling::SpaceEffect;
use crate::layout::{PaneId, Session};

/// What a removal took out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removed {
    /// The panes that are gone.
    pub panes: Vec<PaneId>,
    /// What taking them out did, workspace by workspace — the workspace that went with them too.
    pub arranged: Arranged,
}

impl Session {
    /// Remove a pane — tiled or floating, in whichever workspace it is. A workspace it leaves with
    /// nothing in it goes too, when another remains. `None` when the pane is nowhere.
    pub fn remove_pane(&mut self, pane: PaneId) -> Option<Removed> {
        let ws = self.workspace_holding(pane)?;
        let id = self.workspaces[ws].id;
        let mut record = Record::default();
        let (taken, effect) = self.workspaces[ws].take_pane(pane)?;
        record.note(id, effect);
        record.remove_if_empty(self, ws);
        Some(Removed {
            panes: vec![taken.id],
            arranged: record.finish(self),
        })
    }

    /// Remove column `col` of workspace `ws` with its panes; `col` past the last means the last.
    /// The workspace stays, empty or not. `None` when there is no such workspace or column.
    pub fn remove_column(&mut self, ws: usize, col: usize) -> Option<Removed> {
        let workspace = self.workspaces.get_mut(ws)?;
        let last = workspace.scrolling.columns.len().checked_sub(1)?;
        let (column, effect) = workspace.scrolling.take_column(col.min(last))?;
        let mut record = Record::default();
        record.note(workspace.id, [SpaceEffect::Column(effect)]);
        let panes = column.panes.iter().map(|p| p.id).collect();
        Some(Removed {
            panes,
            arranged: record.finish(self),
        })
    }

    /// Remove workspace `ws` with everything in it, the last one too — the session is then empty
    /// until a workspace is made. `None` when there is no such workspace.
    pub fn remove_workspace_with_panes(&mut self, ws: usize) -> Option<Removed> {
        let workspace = self.workspaces.get(ws)?;
        let tiled = workspace
            .scrolling
            .columns
            .iter()
            .flat_map(|c| c.panes.iter().map(|p| p.id));
        let floating = workspace.floating_panes.iter().map(|f| f.pane.id);
        let panes = tiled.chain(floating).collect();
        self.remove_workspace(ws).then(|| Removed {
            panes,
            arranged: Arranged {
                removed_workspace: Some(ws),
                effects: Vec::new(),
            },
        })
    }
}
