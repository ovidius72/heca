//! Naming: set a name, or clear it so the thing goes back to its default label.

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// What a naming that took says; nothing when there was nothing to name.
fn named(done: bool) -> Vec<Change> {
    match done {
        true => vec![Change::NamesChanged],
        false => Vec::new(),
    }
}

/// Name a pane, wherever it is.
pub(super) fn rename_target(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::RenameTarget { pane_id, name } = action else {
        return Vec::new();
    };
    named(cx.layout.rename_pane(*pane_id, name))
}

/// Name a workspace.
pub(super) fn rename_workspace_to(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::RenameWorkspaceTo { ws_idx, name } = action else {
        return Vec::new();
    };
    named(cx.layout.rename_workspace(*ws_idx, name))
}

/// Name a column.
pub(super) fn rename_column_to(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::RenameColumnTo {
        ws_idx,
        col_idx,
        name,
    } = action
    else {
        return Vec::new();
    };
    named(cx.layout.rename_column(*ws_idx, *col_idx, name))
}

/// Clear the name of the pane the asker has focused.
pub(super) fn reset_pane_name(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let Some(pane) = cx.asker.focused_pane else {
        return Vec::new();
    };
    named(cx.layout.rename_pane(pane, ""))
}

/// Clear the name of a pane.
pub(super) fn reset_pane_name_by_id(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ResetPaneNameById { pane_id } = action else {
        return Vec::new();
    };
    named(cx.layout.rename_pane(*pane_id, ""))
}

/// Clear the name of the workspace the asker is in.
pub(super) fn reset_workspace_name(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let ws = cx.layout.reader().active_workspace_idx();
    named(cx.layout.rename_workspace(ws, ""))
}

/// Clear the name of a workspace.
pub(super) fn reset_workspace_name_by_idx(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ResetWorkspaceNameByIdx { ws_idx } = action else {
        return Vec::new();
    };
    named(cx.layout.rename_workspace(*ws_idx, ""))
}
