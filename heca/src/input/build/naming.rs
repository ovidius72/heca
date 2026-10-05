//! Actions that name or rename a pane, column or workspace.

use super::{get_string, get_u64, get_usize};
use crate::input::WmAction;
use heca_core::layout::PaneId;

pub(super) fn build_naming(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    match name {
        "rename_target" => Some(WmAction::RenameTarget {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            name: get_string(args, "name")?,
        }),
        "rename_workspace_to" => Some(WmAction::RenameWorkspaceTo {
            ws_idx: get_usize(args, "ws_idx")?,
            name: get_string(args, "name")?,
        }),
        "rename_column_to" => Some(WmAction::RenameColumnTo {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
            name: get_string(args, "name")?,
        }),
        "rename_pane_by_id" => Some(WmAction::RenamePaneById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "rename_workspace_by_idx" => Some(WmAction::RenameWorkspaceByIdx {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "rename_column_by_idx" => Some(WmAction::RenameColumnByIdx {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "reset_pane_name_by_id" => Some(WmAction::ResetPaneNameById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "reset_workspace_name_by_idx" => Some(WmAction::ResetWorkspaceNameByIdx {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        _ => None,
    }
}
