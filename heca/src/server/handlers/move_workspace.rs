//! Moving a workspace to another place in the list.
//!
//! The order is content every window shares, so a move is a layout change: each window keeps
//! showing the workspace it showed (it holds it by identity), wherever it now sits.

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// Move workspace `from` (unset: the asker's) to `to`, and say so when anything moved.
fn moved(
    cx: &mut ServerCx<'_>,
    from: Option<usize>,
    to: impl FnOnce(usize) -> Option<usize>,
) -> Vec<Change> {
    let from = from.unwrap_or(cx.asker.workspace);
    match to(from).is_some_and(|to| cx.layout.move_workspace(from, to)) {
        true => vec![Change::LayoutChanged],
        false => Vec::new(),
    }
}

/// Move a workspace to a position in the list.
pub(super) fn move_workspace(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MoveWorkspace { ws_idx, to_idx } = action else {
        return Vec::new();
    };
    moved(cx, *ws_idx, |_| Some(*to_idx))
}

/// Move a workspace one place up; the first stays.
pub(super) fn move_workspace_up(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MoveWorkspaceUp { ws_idx } = action else {
        return Vec::new();
    };
    moved(cx, *ws_idx, |from| from.checked_sub(1))
}

/// Move a workspace one place down; the last stays.
pub(super) fn move_workspace_down(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MoveWorkspaceDown { ws_idx } = action else {
        return Vec::new();
    };
    let count = cx.layout.workspaces.len();
    moved(cx, *ws_idx, |from| (from + 1 < count).then_some(from + 1))
}
