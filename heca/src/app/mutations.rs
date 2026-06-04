//! App-level mutation helpers.
//!
//! These helpers coordinate multi-step pane/workspace mutations that span
//! session layout state, focus bookkeeping, and backend lifecycle.

use crate::app::focus::sync_focus;
use crate::app_state::AppState;
use heca_core::layout::{Column, ColumnId, ColumnWidth};

/// Move a pane from one workspace into a new/existing column position in another workspace.
pub(crate) fn move_pane_to_workspace_column(
    state: &mut AppState,
    pane_id: u64,
    target_ws: usize,
    target_col: usize,
) {
    let current_ws = state.session.active_workspace_idx;
    if current_ws == target_ws {
        return;
    }

    let removed_pane = {
        let ws = match state.session.workspaces.get_mut(current_ws) {
            Some(ws) => ws,
            None => return,
        };
        let mut removed = None;
        for ci in 0..ws.scrolling.columns.len() {
            if let Some(pi) = ws.scrolling.columns[ci]
                .panes
                .iter()
                .position(|p| p.id.0 == pane_id)
            {
                removed = ws.scrolling.remove_pane(ci, pi);
                break;
            }
        }
        removed
    };

    if let Some(pane) = removed_pane {
        state.session.switch_to_workspace(target_ws);

        // Insert the pane into the target workspace. Treat `target_col` as the desired
        // insertion index for a new column so moving between workspaces preserves
        // the pane-as-single-column layout (rather than joining an existing column).
        // Generate a fresh ColumnId before mutably borrowing the workspace.
        let new_col_id = ColumnId(state.session.next_id());
        if let Some(ws) = state.session.active_workspace_mut() {
            let insert_pos = target_col.min(ws.scrolling.columns.len());
            ws.scrolling.add_column(
                Some(insert_pos),
                Column::new(new_col_id, pane, ColumnWidth::Proportion(0.5)),
                true,
            );
            state.focused_pane = Some(pane_id);
        }

        destroy_empty_workspace(state, current_ws);
    }

    sync_focus(state);
}

/// Move a pane from one column to another within the same workspace.
/// Handles column removal when a column becomes empty after the move.
pub(crate) fn move_pane_to_column(
    state: &mut AppState,
    pane_id: u64,
    src_col: usize,
    dst_col: usize,
) {
    if src_col == dst_col {
        return;
    }

    let ws_idx = state.session.active_workspace_idx;

    // Validate indices before mutation. Allow `dst_col == col_count_before` to mean "append/new column".
    let col_count_before = state
        .session
        .workspaces
        .get(ws_idx)
        .map(|ws| ws.scrolling.columns.len())
        .unwrap_or(0);
    if src_col >= col_count_before || dst_col > col_count_before {
        return;
    }

    let removed_pane = if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
        if let Some(pi) = ws
            .scrolling
            .columns
            .get(src_col)
            .and_then(|col| col.panes.iter().position(|p| p.id.0 == pane_id))
        {
            ws.scrolling.remove_pane(src_col, pi)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(pane) = removed_pane {
        // Check if the source column was removed (became empty after removal).
        let col_count_after = state
            .session
            .workspaces
            .get(ws_idx)
            .map(|ws| ws.scrolling.columns.len())
            .unwrap_or(0);
        let col_removed = col_count_after < col_count_before;

        let adjusted_dst = if col_removed && src_col < dst_col {
            dst_col.saturating_sub(1)
        } else {
            dst_col
        };

        // Determine current column count (immutable borrow) so we can call next_id() if we need to create a column.
        let col_count_now = state
            .session
            .workspaces
            .get(ws_idx)
            .map(|ws| ws.scrolling.columns.len())
            .unwrap_or(0);
        // Clamp adjusted_dst to [0..=col_count_now]
        let target_pos = if adjusted_dst <= col_count_now {
            adjusted_dst
        } else {
            col_count_now
        };
        let need_new_column = target_pos >= col_count_now;
        let new_col_id = if need_new_column {
            Some(ColumnId(state.session.next_id()))
        } else {
            None
        };

        if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
            if target_pos < ws.scrolling.columns.len() {
                // Insert into existing column at target_pos
                ws.scrolling
                    .add_pane_to_column(target_pos, None, pane, true);
            } else {
                // Create a new column at target_pos (append if equal to current len)
                let cid = new_col_id.unwrap_or(ColumnId(pane.id.0));
                ws.scrolling.add_column(
                    Some(target_pos),
                    Column::new(cid, pane, ColumnWidth::Proportion(0.5)),
                    true,
                );
            }
            state.focused_pane = Some(pane_id);
        }
    }

    sync_focus(state);
}

/// Remove a workspace if it is empty and there are other workspaces.
/// Adjusts tracking indices after removal.
pub(crate) fn destroy_empty_workspace(state: &mut AppState, ws_idx: usize) {
    let is_empty = state
        .session
        .workspaces
        .get(ws_idx)
        .map(|ws| ws.scrolling.columns.iter().all(|c| c.panes.is_empty()))
        .unwrap_or(true);

    if is_empty && state.session.workspaces.len() > 1 {
        state.session.remove_workspace(ws_idx);

        // Fix up last_visited_ws_idx if it pointed to the removed workspace.
        if state.last_visited_ws_idx == Some(ws_idx) {
            state.last_visited_ws_idx = None;
        } else if let Some(ref mut idx) = state.last_visited_ws_idx
            && *idx > ws_idx
        {
            *idx -= 1;
        }

        // Fix up last_visited_pane_per_ws — remove the entry for the removed workspace.
        if ws_idx < state.last_visited_pane_per_ws.len() {
            state.last_visited_pane_per_ws.remove(ws_idx);
        }
    }
}
