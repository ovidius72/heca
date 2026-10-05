//! Moving a column: to another workspace, or to another place among the columns.

use heca_core::layout::Moved;

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// What a column that landed says.
fn landed(moved: Option<Moved>) -> Vec<Change> {
    match moved {
        Some(m) => Change::after_move(
            m,
            Change::ColumnMoved {
                workspace: m.workspace,
                column: m.column,
            },
        ),
        None => Vec::new(),
    }
}

/// Move the asker's active column to the workspace `to` of the one it is in.
fn move_active_column(cx: &mut ServerCx<'_>, to: impl FnOnce(usize, usize) -> Option<usize>) -> Vec<Change> {
    let layout = cx.layout.reader();
    let from = layout.active_workspace_idx();
    let Some(column) = layout.active_workspace().map(|ws| ws.scroll().active_column_idx()) else {
        return Vec::new();
    };
    let count = layout.session().workspaces.len();
    let Some(target) = to(from, count) else {
        return Vec::new();
    };
    landed(cx.layout.move_column_to_workspace(from, column, target))
}

pub(super) fn move_column_up(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    move_active_column(cx, |from, _| from.checked_sub(1))
}

pub(super) fn move_column_down(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    move_active_column(cx, |from, count| (from + 1 < count).then_some(from + 1))
}

/// Move a column of the asker's workspace to another workspace.
pub(super) fn move_column_to_workspace(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MoveColumnToWorkspace { col_idx, ws_idx, .. } = action else {
        return Vec::new();
    };
    let from = cx.layout.reader().active_workspace_idx();
    landed(cx.layout.move_column_to_workspace(from, *col_idx, *ws_idx))
}

/// Move a column to an index of a workspace, which may be its own.
pub(super) fn move_column(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MoveColumn {
        src_ws,
        src_col,
        dst_ws,
        dst_idx,
        focus,
    } = action
    else {
        return Vec::new();
    };
    landed(cx.layout.move_column(*src_ws, *src_col, *dst_ws, *dst_idx, *focus))
}
