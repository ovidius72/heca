//! **Changing what is where in the session, with no window in sight** — moving, swapping, making,
//! naming and removing panes, columns and workspaces on the shared content alone.
//!
//! Each change says what it did, workspace by workspace, as the effects a window shows for itself
//! ([`LayoutMut::show_change`](super::LayoutMut::show_change)), and which workspace — if any — it
//! emptied and removed ([`LayoutMut::workspace_removed`](super::LayoutMut::workspace_removed)).
//! Which workspace a window shows, which column is active in it and where it is scrolled are never
//! read or moved here.

use super::scrolling::{ColumnEffect, SpaceEffect};
use super::types::WorkspaceId;
use super::Session;

mod add;
mod moves;
mod names;
mod remove;
mod swap;
#[cfg(test)]
mod tests;

pub use add::Added;
pub use moves::Moved;
pub use remove::Removed;

/// What a change did to the workspaces: the one it emptied and removed, and what it did to the
/// columns and panes of each workspace that is left.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Arranged {
    /// The workspace the change emptied and removed, numbered as it was before the change.
    pub removed_workspace: Option<usize>,
    /// What changed in each workspace that is left, numbered as they are now. A workspace's effects
    /// are in the order they happened, and are shown together.
    pub effects: Vec<(usize, Vec<SpaceEffect>)>,
}

/// What a change does, kept by workspace identity while it happens: numbers shift when a
/// workspace goes, identities do not.
#[derive(Default)]
struct Record {
    effects: Vec<(WorkspaceId, Vec<SpaceEffect>)>,
    removed: Option<usize>,
}

impl Record {
    /// Workspace `ws` changed as `effects` say.
    fn note(&mut self, ws: WorkspaceId, effects: impl IntoIterator<Item = SpaceEffect>) {
        let mut effects = effects.into_iter().peekable();
        if effects.peek().is_none() {
            return;
        }
        match self.effects.iter_mut().find(|(id, _)| *id == ws) {
            Some((_, noted)) => noted.extend(effects),
            None => self.effects.push((ws, effects.collect())),
        }
    }

    /// Workspace `idx` of `session` went, if it was left with nothing in it and is not the last.
    fn remove_if_empty(&mut self, session: &mut Session, idx: usize) {
        if session.remove_workspace_if_empty(idx) {
            self.removed = Some(idx);
        }
    }

    /// What the change did, numbered as `session` is now. A workspace that is gone has nothing to
    /// show.
    fn finish(self, session: &Session) -> Arranged {
        let effects = self
            .effects
            .into_iter()
            .filter_map(|(id, effects)| Some((session.index_of(id)?, effects)))
            .collect();
        Arranged { removed_workspace: self.removed, effects }
    }
}

impl Session {
    /// Where workspace `id` is now. `None` when it is gone.
    pub fn index_of(&self, id: WorkspaceId) -> Option<usize> {
        self.workspaces.iter().position(|ws| ws.id == id)
    }

    /// Remove the workspace at `idx`, the last one too. `false` for an out-of-range index. A
    /// window showing a workspace learns of it through
    /// [`LayoutMut::workspace_removed`](super::LayoutMut::workspace_removed).
    pub fn remove_workspace(&mut self, idx: usize) -> bool {
        if idx >= self.workspaces.len() {
            return false;
        }
        self.workspaces.remove(idx);
        true
    }

    /// Remove workspace `idx` when nothing is left in it and another remains. Answers whether it
    /// was removed.
    pub fn remove_workspace_if_empty(&mut self, idx: usize) -> bool {
        let empty = self.workspaces.get(idx).is_some_and(|ws| !ws.has_panes());
        empty && self.workspaces.len() > 1 && self.remove_workspace(idx)
    }
}

/// Where column `idx` of a workspace is once `taken` has happened there: one place back when a
/// column before it went.
fn after_taking(taken: Option<SpaceEffect>, idx: usize) -> usize {
    match taken {
        Some(SpaceEffect::Column(ColumnEffect::Removed { idx: gone })) if gone < idx => idx - 1,
        _ => idx,
    }
}
