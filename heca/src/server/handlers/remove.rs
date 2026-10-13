//! Removing for good: a pane, a column with its panes, a workspace with everything in it.

use heca_core::layout::{PaneId, Removed};

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// End the processes of the panes a removal took out, and say what it did; nothing when nothing
/// was there.
fn removed(cx: &mut ServerCx<'_>, removed: Option<Removed>) -> Vec<Change> {
    let Some(removed) = removed else {
        return Vec::new();
    };
    cx.terminals.end(removed.panes.iter().copied());
    Change::after_removal(removed)
}

/// Close `pane`, wherever it is, and end what ran in it.
pub(super) fn close_pane(cx: &mut ServerCx<'_>, pane: PaneId) -> Vec<Change> {
    let gone = cx.layout.remove_pane(pane);
    removed(cx, gone)
}

/// Close the pane the asker has focused.
pub(super) fn close(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let Some(pane) = cx.asker.focused_pane else {
        return Vec::new();
    };
    close_pane(cx, pane)
}

/// Close a pane by id, wherever it is.
pub(super) fn close_pane_by_id(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ClosePaneById { pane_id } = action else {
        return Vec::new();
    };
    close_pane(cx, *pane_id)
}

/// Delete a column and all its panes.
pub(super) fn delete_column(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::DeleteColumn { ws_idx, col_idx } = action else {
        return Vec::new();
    };
    let gone = cx.layout.remove_column(*ws_idx, *col_idx);
    removed(cx, gone)
}

/// Delete a workspace and everything in it.
pub(super) fn delete_workspace(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::DeleteWorkspace { ws_idx } = action else {
        return Vec::new();
    };
    let gone = cx.layout.remove_workspace_with_panes(*ws_idx);
    removed(cx, gone)
}
