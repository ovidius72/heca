//! Actions that name panes, columns and workspaces by index or id: focus, move, swap, resize, float, take, place, add, close, delete, zoom.

use super::{get_enum, get_enum_or_default, get_f64, get_u64, get_usize};
use crate::input::WmAction;
use heca_core::layout::PaneId;

pub(super) fn build_layout(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    match name {
        "focus_pane" => Some(WmAction::FocusPane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "focus_workspace" => Some(WmAction::FocusWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "swap" => Some(WmAction::Swap {
            a_id: PaneId(get_u64(args, "a_id")?),
            b_id: PaneId(get_u64(args, "b_id")?),
        }),
        "move" => Some(WmAction::Move {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            target_col: get_usize(args, "target_col")?,
        }),
        "move_pane_to_workspace" => Some(WmAction::MovePaneToWorkspace {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "move_pane_to_column" => Some(WmAction::MovePaneToColumn {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "place_pane" => Some(WmAction::PlacePane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
            pane_idx: get_usize(args, "pane_idx"),
        }),
        "move_column" => Some(WmAction::MoveColumn {
            src_ws: get_usize(args, "src_ws")?,
            src_col: get_usize(args, "src_col")?,
            dst_ws: get_usize(args, "dst_ws")?,
            dst_idx: get_usize(args, "dst_idx")?,
            focus: args
                .get("focus")
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }),
        "swap_columns" => Some(WmAction::SwapColumns {
            a_ws: get_usize(args, "a_ws")?,
            a_col: get_usize(args, "a_col")?,
            b_ws: get_usize(args, "b_ws")?,
            b_col: get_usize(args, "b_col")?,
        }),
        "move_column_to_workspace" => Some(WmAction::MoveColumnToWorkspace {
            col_idx: get_usize(args, "col_idx")?,
            ws_idx: get_usize(args, "ws_idx")?,
            focus: args
                .get("focus")
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }),
        // `pane_id` is OPTIONAL: omitting it moves the FOCUSED pane, which is what a keybinding
        // means. Declaring it keeps the named path level with the RPC command, which has always
        // accepted an explicit pane.
        "move_pane_left" => Some(WmAction::MovePaneLeft {
            pane_id: get_u64(args, "pane_id").map(PaneId),
        }),
        "move_pane_right" => Some(WmAction::MovePaneRight {
            pane_id: get_u64(args, "pane_id").map(PaneId),
        }),
        "resize_column_by" => Some(WmAction::ResizeColumnBy {
            col_idx: get_usize(args, "col_idx")?,
            delta: get_f64(args, "delta")?,
        }),
        "resize_pane_height_by" => Some(WmAction::ResizePaneHeightBy {
            col_idx: get_usize(args, "col_idx")?,
            pane_idx: get_usize(args, "pane_idx")?,
            delta: get_f64(args, "delta")?,
        }),
        "resize" => Some(WmAction::Resize {
            target: get_enum(args, "target")?,
            amount: get_f64(args, "amount")?,
            edge: get_enum_or_default(args, "edge")?,
        }),
        "resize_to" => Some(WmAction::ResizeTo {
            target: get_enum(args, "target")?,
            width: get_f64(args, "width")?,
            height: get_f64(args, "height")?,
        }),
        "float_at" => Some(WmAction::FloatAt {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            x: get_f64(args, "x")?,
            y: get_f64(args, "y")?,
            width: get_f64(args, "width")?,
            height: get_f64(args, "height")?,
        }),
        "close_pane_by_id" => Some(WmAction::ClosePaneById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "take_pane" => Some(WmAction::TakePane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            focus_after: args
                .get("focus_after")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
        }),
        "add_pane_to_column" => Some(WmAction::AddPaneToColumn {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "delete_column" => Some(WmAction::DeleteColumn {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "zoom_column_at_index" => Some(WmAction::ZoomColumnAtIndex {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "delete_workspace" => Some(WmAction::DeleteWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "add_column_to_workspace" => Some(WmAction::AddColumnToWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        _ => None,
    }
}
