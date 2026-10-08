//! Removing for good: a pane, a column with its panes, a workspace with everything in it.

use heca_core::layout::Removed;

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// What a removal says; nothing when nothing was there.
fn removed(removed: Option<Removed>) -> Vec<Change> {
    removed.map(Change::after_removal).unwrap_or_default()
}

/// Close the pane the asker has focused.
pub(super) fn close(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let Some(pane) = cx.asker.focused_pane else {
        return Vec::new();
    };
    removed(cx.layout.remove_pane_anywhere(pane))
}

/// Close a pane by id, wherever it is.
pub(super) fn close_pane_by_id(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ClosePaneById { pane_id } = action else {
        return Vec::new();
    };
    removed(cx.layout.remove_pane_anywhere(*pane_id))
}

/// Delete a column and all its panes.
pub(super) fn delete_column(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::DeleteColumn { ws_idx, col_idx } = action else {
        return Vec::new();
    };
    removed(cx.layout.remove_column_with_panes(*ws_idx, *col_idx))
}

/// Delete a workspace and everything in it.
pub(super) fn delete_workspace(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::DeleteWorkspace { ws_idx } = action else {
        return Vec::new();
    };
    removed(cx.layout.remove_workspace_with_panes(*ws_idx))
}
