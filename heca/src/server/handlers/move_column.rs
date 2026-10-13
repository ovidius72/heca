//! Moving a column: to another workspace, or to another place among the columns.

use heca_core::layout::Moved;

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// What a column that landed says. Whether the asker goes where it went is the asker's to decide.
fn landed(moved: Option<Moved>) -> Vec<Change> {
    match moved {
        Some(m) => {
            let landed = Change::ColumnMoved { workspace: m.workspace, column: m.column };
            Change::after_move(m, landed)
        }
        None => Vec::new(),
    }
}

/// Move the asker's active column to the workspace `to` of the one it is in.
fn move_active_column(cx: &mut ServerCx<'_>, to: impl FnOnce(usize, usize) -> Option<usize>) -> Vec<Change> {
    let (from, column) = (cx.asker.workspace, cx.asker.column);
    let count = cx.layout.workspaces.len();
    let Some(target) = to(from, count) else {
        return Vec::new();
    };
    to_the_end(cx, from, column, target)
}

/// Move column `column` of workspace `from` to the end of workspace `to`.
fn to_the_end(cx: &mut ServerCx<'_>, from: usize, column: usize, to: usize) -> Vec<Change> {
    match from == to {
        true => Vec::new(),
        false => landed(cx.layout.move_column(from, column, to, usize::MAX)),
    }
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
    let from = cx.asker.workspace;
    to_the_end(cx, from, *col_idx, *ws_idx)
}

/// Move a column to an index of a workspace, which may be its own.
pub(super) fn move_column(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MoveColumn { src_ws, src_col, dst_ws, dst_idx, .. } = action else {
        return Vec::new();
    };
    landed(cx.layout.move_column(*src_ws, *src_col, *dst_ws, *dst_idx))
}
