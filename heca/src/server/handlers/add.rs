//! Making a pane: in a column, or in a column of its own. Nothing runs in it yet.

use heca_core::layout::Added;

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// What a pane that was made says.
fn added(added: Option<Added>) -> Vec<Change> {
    match added {
        Some(a) => vec![Change::PaneAdded {
            pane: a.pane,
            workspace: a.workspace,
            column: a.column,
        }],
        None => Vec::new(),
    }
}

/// Make a pane at the bottom of a column of a workspace.
pub(super) fn add_pane_to_column(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::AddPaneToColumn { ws_idx, col_idx } = action else {
        return Vec::new();
    };
    added(cx.layout.add_pane_to_column(*ws_idx, *col_idx))
}

/// Make a pane in a new column of a workspace.
pub(super) fn add_column_to_workspace(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::AddColumnToWorkspace { ws_idx } = action else {
        return Vec::new();
    };
    added(cx.layout.add_column(*ws_idx))
}
