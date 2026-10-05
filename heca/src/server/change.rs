//! **What the server tells a window changed** — plain data a window drains and reacts to.
//!
//! The server never reaches into a window. It reports what happened to the state it owns, and the
//! window does what only a window can: redraw, rebuild what it drew, re-flow. A [`Change`] says
//! *what* changed, never what to do about it — the same rule events follow everywhere else.

/// Something that changed in the state every window shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    /// A new pane now sits in column `column` of workspace `workspace`. Nothing runs in it yet.
    PaneAdded {
        pane: heca_core::layout::PaneId,
        workspace: usize,
        column: usize,
    },
    /// The pane is gone from the layout. Whatever ran in it is the window's to end.
    PaneRemoved { pane: heca_core::layout::PaneId },
    /// A column now sits at index `column` of workspace `workspace` (numbered as they are now).
    ColumnMoved { workspace: usize, column: usize },
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
    /// What a removal says: each pane that went, then the workspace that went with them — or, when
    /// no workspace did, that the layout changed.
    pub(crate) fn after_removal(removed: heca_core::layout::Removed) -> Vec<Self> {
        let panes = removed.panes.into_iter().map(|pane| Self::PaneRemoved { pane });
        let last = match removed.removed_workspace {
            Some(index) => Self::WorkspaceRemoved { index },
            None => Self::LayoutChanged,
        };
        panes.chain([last]).collect()
    }

    /// What a move that landed says: the workspace it emptied and removed first, if any, then
    /// where it landed.
    pub(crate) fn after_move(moved: heca_core::layout::Moved, landed: Self) -> Vec<Self> {
        let removed = moved
            .removed_workspace
            .map(|index| Self::WorkspaceRemoved { index });
        removed.into_iter().chain([landed]).collect()
    }
}
