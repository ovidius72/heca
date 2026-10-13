//! Making a pane: in a column, or in a column of its own. Nothing runs in it yet.

use heca_core::layout::Added;

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// What a pane that was made says: what making it did, then where it is.
fn added(added: Option<Added>) -> Vec<Change> {
    let Some(a) = added else {
        return Vec::new();
    };
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

/// Make a pane at the bottom of a column of a workspace.
pub(super) fn add_pane_to_column(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::AddPaneToColumn { ws_idx, col_idx } = action else {
        return Vec::new();
    };
    let height = cx.asker.area.size.h;
    added(cx.layout.add_pane_to_column(*ws_idx, *col_idx, height))
}

/// Make a pane in a new column of a workspace: after the asker's active column in the workspace it
/// shows, at the end of any other.
pub(super) fn add_column_to_workspace(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::AddColumnToWorkspace { ws_idx } = action else {
        return Vec::new();
    };
    let at = match *ws_idx == cx.asker.workspace {
        true => cx.asker.column + 1,
        false => usize::MAX,
    };
    added(cx.layout.add_column(*ws_idx, at))
}
