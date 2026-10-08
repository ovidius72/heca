//! What a server handler is given: the shared content to change, and who asked.

use heca_core::layout::{
    ColumnEffect, Layout, PaneId, ScrollingSpace, Size, SpaceEffect, WorkspaceMut,
};

use super::{Change, ServerLayout};

/// Plain data about the window that asked — what a server action needs to know that only the
/// window knows. The server never reads it from a window: with several windows (F012) each request
/// carries its own, and two windows asking at once are two different askers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Asker {
    /// The pane this window has focused, if any. Focus is per window.
    pub(crate) focused_pane: Option<PaneId>,
    /// The workspace this window shows.
    pub(crate) workspace: usize,
    /// The column that is active in it.
    pub(crate) column: usize,
    /// The size of the area this window lays that workspace out in. It is **this window's**, not
    /// a size the session shares: a resize the user does in a window is clamped to what fits that
    /// window, so a request carries it; nothing should read it as the size of anything else.
    pub(crate) area: Size,
}

impl Asker {
    /// The asker for a window that sees the session as `layout` shows it, with `focused_pane`
    /// focused — the one place a window's place is turned into data.
    pub(crate) fn seen_through(layout: Layout<'_>, focused_pane: Option<PaneId>) -> Self {
        Self {
            focused_pane,
            workspace: layout.active_workspace_idx(),
            column: layout
                .active_workspace()
                .map_or(0, |ws| ws.scroll().active_column_idx()),
            area: layout
                .active_workspace()
                .map_or_else(Size::default, |ws| ws.scroll().area().size),
        }
    }
}

/// What a server handler runs against.
pub(crate) struct ServerCx<'a> {
    /// The session, by index and by id. **While standalone heca is one process** the asking
    /// window's view still travels inside it, so a move shifts that window's scroll exactly as
    /// before; 2c removes it.
    pub(crate) layout: ServerLayout<'a>,
    /// Who asked.
    pub(crate) asker: Asker,
}

impl ServerCx<'_> {
    /// Change the layout of the workspace the asker is in, and say the layout changed. Says nothing
    /// when there is no workspace — there was nothing to change.
    pub(crate) fn change_asker_workspace(
        &mut self,
        change: impl FnOnce(WorkspaceMut<'_>),
    ) -> Vec<Change> {
        match self.layout.workspace_mut(self.asker.workspace) {
            Some(ws) => {
                change(ws);
                vec![Change::LayoutChanged]
            }
            None => Vec::new(),
        }
    }
}

impl ServerCx<'_> {
    /// Change the columns of the workspace the asker is in, with no view, and report each effect
    /// as a fact for the window to show. Says nothing when nothing changed.
    pub(crate) fn change_asker_columns(
        &mut self,
        change: impl FnOnce(&mut ScrollingSpace, &Asker) -> Vec<ColumnEffect>,
    ) -> Vec<Change> {
        let (workspace, asker) = (self.asker.workspace, self.asker);
        let Some(space) = self.layout.columns_mut(workspace) else {
            return Vec::new();
        };
        change(space, &asker)
            .into_iter()
            .map(|effect| Change::Arranged { workspace, effects: vec![SpaceEffect::Column(effect)] })
            .collect()
    }

    /// Change the asker's workspace's content with no view and say the layout changed (for what
    /// needs no reaction in a window, such as a pane's height share).
    pub(crate) fn change_asker_content(
        &mut self,
        change: impl FnOnce(&mut ScrollingSpace, &Asker),
    ) -> Vec<Change> {
        let (workspace, asker) = (self.asker.workspace, self.asker);
        match self.layout.columns_mut(workspace) {
            Some(space) => {
                change(space, &asker);
                vec![Change::LayoutChanged]
            }
            None => Vec::new(),
        }
    }
}

impl ServerCx<'_> {
    /// Change workspace `workspace`'s columns and panes with no view, and say what changed. `None`
    /// when there is no such workspace or nothing changed.
    pub(crate) fn arrange(
        &mut self,
        workspace: usize,
        change: impl FnOnce(&mut ScrollingSpace, &Asker) -> Option<Vec<SpaceEffect>>,
    ) -> Option<Vec<SpaceEffect>> {
        let asker = self.asker;
        let space = self.layout.columns_mut(workspace)?;
        change(space, &asker).filter(|effects| !effects.is_empty())
    }
}
