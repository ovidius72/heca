//! **What the server tells a window changed** — plain data a window drains and reacts to.
//!
//! The server never reaches into a window. It reports what happened to the state it owns, and the
//! window does what only a window can: redraw, rebuild what it drew, re-flow. A [`Change`] says
//! *what* changed, never what to do about it — the same rule events follow everywhere else.

/// Something that changed in the state every window shares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    /// The notifications on show, or when they expire, are different.
    NotificationsChanged,
    /// The arrangement of panes or columns is different: a column was resized, a pane was moved.
    LayoutChanged,
    /// The column at `column` in the workspace at `workspace` was zoomed or restored.
    ColumnZoomed { workspace: usize, column: usize },
    /// A pane now sits in column `column` of workspace `workspace` (numbered as they are now).
    PaneMoved {
        pane: heca_core::layout::PaneId,
        workspace: usize,
        column: usize,
    },
    /// A new pane now sits in column `column` of workspace `workspace`, its program running.
    PaneAdded {
        pane: heca_core::layout::PaneId,
        workspace: usize,
        column: usize,
    },
    /// The pane is gone from the layout, and whatever ran in it has ended.
    PaneRemoved { pane: heca_core::layout::PaneId },
    /// The columns or panes of workspace `workspace` changed, as `effects` say: each window shows
    /// it for itself. Several effects are one change (a move is a removal and an insertion).
    Arranged {
        workspace: usize,
        effects: Vec<heca_core::layout::SpaceEffect>,
    },
    /// A column now sits at index `column` of workspace `workspace` (numbered as they are now).
    ColumnMoved { workspace: usize, column: usize },
    /// A workspace now sits at `index`, after the last.
    WorkspaceAdded { index: usize },
    /// The workspace that was at `index` is gone; every later one is one lower.
    WorkspaceRemoved { index: usize },
    /// A pane, column or workspace has a different name.
    NamesChanged,
    /// The server did not do what was asked, and why. Not a change to anything: a window says it
    /// in its own words.
    Refused(Refusal),
}

/// Why the server did not do what it was asked — plain data; the words are the window's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// The pane is already the only one in its column, so taking it out would change nothing.
    OnlyPaneInColumn,
}

impl Change {
    /// What a change to the arrangement says: the workspace it emptied and removed first, if any,
    /// then what changed in each workspace that is left, for every window to show for itself.
    pub(crate) fn after_arranging(arranged: heca_core::layout::Arranged) -> Vec<Self> {
        let removed = arranged.removed_workspace.map(|index| Self::WorkspaceRemoved { index });
        let arranged = arranged
            .effects
            .into_iter()
            .map(|(workspace, effects)| Self::Arranged { workspace, effects });
        removed.into_iter().chain(arranged).collect()
    }

    /// What a removal says: each pane that went, then what taking them out did.
    pub(crate) fn after_removal(removed: heca_core::layout::Removed) -> Vec<Self> {
        let panes = removed.panes.into_iter().map(|pane| Self::PaneRemoved { pane });
        panes.chain(Self::after_arranging(removed.arranged)).collect()
    }

    /// What a move that landed says: what it did to get there, then where it landed.
    pub(crate) fn after_move(moved: heca_core::layout::Moved, landed: Self) -> Vec<Self> {
        Self::after_arranging(moved.arranged).into_iter().chain([landed]).collect()
    }
}
