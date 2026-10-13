//! Making a pane — in a column, in a column of its own, in a new workspace — with the user's shell
//! running in it.

use heca_core::layout::Added;

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// Start the shell of a pane that was made, and say what making it did, then where it is.
fn with_shell(cx: &mut ServerCx<'_>, added: Option<Added>) -> Vec<Change> {
    let Some(a) = added else {
        return Vec::new();
    };
    let (grid, cell) = (cx.asker.grid(), cx.asker.cell);
    cx.terminals.start_shell(a.pane, grid, cell);
    let made = Change::PaneAdded {
        pane: a.pane,
        workspace: a.workspace,
        column: a.column,
    };
    Change::after_arranging(a.arranged)
        .into_iter()
        .chain([made])
        .collect()
}

/// Where a new column of workspace `ws` goes: after the asker's active column in the workspace it
/// shows, at the end of any other.
fn new_column_place(cx: &ServerCx<'_>, ws: usize) -> usize {
    match ws == cx.asker.workspace {
        true => cx.asker.column + 1,
        false => usize::MAX,
    }
}

/// Make a pane at the bottom of a column of a workspace.
pub(super) fn add_pane_to_column(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::AddPaneToColumn { ws_idx, col_idx } = action else {
        return Vec::new();
    };
    let height = cx.asker.area.size.h;
    let added = cx.layout.add_pane_to_column(*ws_idx, *col_idx, height);
    with_shell(cx, added)
}

/// Make a pane in a new column of a workspace.
pub(super) fn add_column_to_workspace(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::AddColumnToWorkspace { ws_idx } = action else {
        return Vec::new();
    };
    let at = new_column_place(cx, *ws_idx);
    let added = cx.layout.add_column(*ws_idx, at);
    with_shell(cx, added)
}

/// Make a pane in a new column right of the asker's.
pub(super) fn split_horizontal(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let ws = cx.asker.workspace;
    let at = new_column_place(cx, ws);
    let added = cx.layout.add_column(ws, at);
    with_shell(cx, added)
}

/// Make a pane at the bottom of the asker's column.
pub(super) fn split_vertical(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let (ws, col, height) = (cx.asker.workspace, cx.asker.column, cx.asker.area.size.h);
    let added = cx.layout.add_pane_to_column(ws, col, height);
    with_shell(cx, added)
}

/// Make a workspace, after the last, with a pane in it.
pub(super) fn create_workspace(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.layout.add_workspace();
    let index = cx.layout.workspaces.len() - 1;
    let added = cx.layout.add_column(index, 0);
    [Change::WorkspaceAdded { index }]
        .into_iter()
        .chain(with_shell(cx, added))
        .collect()
}
