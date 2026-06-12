//! App-level mutation helpers.
//!
//! These helpers coordinate multi-step pane/workspace mutations that span
//! session layout state, focus bookkeeping, backend lifecycle, and the shared
//! post-mutation hooks introduced in Phase 2.

use crate::app::backend_factory::create_terminal_backend;
use crate::app::focus::sync_focus;
use crate::app_state::AppState;
use crate::chrome;
use heca_core::layout::{Column, ColumnId, FocusDomain, Pane, PaneId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationKind {
    Layout,
    Focus,
    Config,
}

fn after_mutation_change_inner(state: &mut AppState, kind: MutationKind) {
    match kind {
        MutationKind::Layout | MutationKind::Focus | MutationKind::Config => {
            sync_focus(state);
            state.needs_redraw = true;
        }
    }
}

pub fn after_layout_change(state: &mut AppState) {
    after_mutation_change_inner(state, MutationKind::Layout);
}

pub fn after_focus_change(state: &mut AppState) {
    after_mutation_change_inner(state, MutationKind::Focus);
}

pub fn after_config_change(state: &mut AppState) {
    after_mutation_change_inner(state, MutationKind::Config);
}

pub fn after_metadata_change(state: &mut AppState) {
    after_mutation_change_inner(state, MutationKind::Focus);
}

pub fn after_mutation_change(state: &mut AppState, kind: MutationKind) {
    after_mutation_change_inner(state, kind);
}

pub(crate) fn close_pane_by_id_anywhere(state: &mut AppState, pane_id: PaneId) -> bool {
    let mut removed_ws_idx = None;

    for (ws_idx, ws) in state.session.workspaces.iter_mut().enumerate() {
        if let Some(removed) = crate::app::pane_ops::remove_pane_by_id(ws, pane_id) {
            state.backends.remove_for_pane(removed.pane.id);
            removed_ws_idx = Some(ws_idx);
            break;
        }

        if let Some(float_idx) = ws.floating_panes.iter().position(|f| f.pane.id == pane_id) {
            let removed = ws.floating_panes.remove(float_idx);
            state.backends.remove_for_pane(removed.pane.id);
            if ws.focus_domain == FocusDomain::Floating && ws.floating_panes.is_empty() {
                ws.deactivate_floating_panes();
                ws.focus_domain = FocusDomain::Tiled;
            }
            removed_ws_idx = Some(ws_idx);
            break;
        }
    }

    let Some(ws_idx) = removed_ws_idx else {
        return false;
    };

    let should_destroy = state
        .session
        .workspaces
        .get(ws_idx)
        .map(|ws| !ws.has_panes())
        .unwrap_or(false)
        && state.session.workspaces.len() > 1;

    if should_destroy {
        destroy_empty_workspace(state, ws_idx);
    }

    after_layout_change(state);
    true
}

/// Move a pane from one workspace into a new/existing column position in another workspace.
pub(crate) fn move_pane_to_workspace_column(
    state: &mut AppState,
    pane_id: PaneId,
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
        crate::app::pane_ops::find_pane_indices_in_workspace(ws, pane_id)
            .and_then(|(ci, pi)| ws.scrolling.remove_pane(ci, pi))
    };

    if let Some(pane) = removed_pane {
        let pane_id = pane.id;
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
                Column::new(new_col_id, pane, chrome::default_column_width()),
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
    pane_id: PaneId,
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
            .and_then(|col| col.panes.iter().position(|p| p.id == pane_id))
        {
            ws.scrolling.remove_pane(src_col, pi)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(pane) = removed_pane {
        let pane_id = pane.id;
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
                let cid = new_col_id.unwrap_or(ColumnId(pane_id.0));
                ws.scrolling.add_column(
                    Some(target_pos),
                    Column::new(cid, pane, chrome::default_column_width()),
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
/// Move a column from its current workspace to a target workspace.
/// If `focus` is true, switches to the target workspace after the move.
/// If the source workspace becomes empty, destroys it or adds a placeholder pane.
pub(crate) fn move_column_to_workspace(
    state: &mut AppState,
    col_idx: usize,
    target_ws: usize,
    focus: bool,
) {
    let current_ws = state.session.active_workspace_idx;
    if current_ws == target_ws {
        return;
    }
    if target_ws >= state.session.workspaces.len() {
        return;
    }

    let removed_column = {
        let ws = match state.session.workspaces.get_mut(current_ws) {
            Some(ws) => ws,
            None => return,
        };
        if col_idx >= ws.scrolling.columns.len() {
            return;
        }
        ws.scrolling.remove_column(col_idx)
    };

    let Some(column) = removed_column else { return };

    let source_empty = state
        .session
        .workspaces
        .get(current_ws)
        .map(|ws| ws.scrolling.columns.is_empty())
        .unwrap_or(false);

    let mut target_ws = target_ws;
    let mut source_destroyed = false;

    if source_empty && state.session.workspaces.len() > 1 {
        if current_ws < target_ws {
            target_ws -= 1;
        }
        destroy_empty_workspace(state, current_ws);
        source_destroyed = true;
    } else if source_empty {
        let next_id = state.session.next_id();
        let placeholder_pane = Pane::new(PaneId(next_id), format!("pane{}", next_id));
        let placeholder_col = Column::new(
            ColumnId(state.session.next_id()),
            placeholder_pane,
            chrome::default_column_width(),
        );
        if let Some(ws) = state.session.workspaces.get_mut(current_ws) {
            ws.scrolling.add_column(None, placeholder_col, true);
        }
        state
            .backends
            .insert_for_pane(
                PaneId(next_id),
                create_terminal_backend(80, 24, &state.theme, Some(&state.event_proxy)),
            );
    }

    state.session.switch_to_workspace(target_ws);
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.add_column(None, column, true);
    }

    if focus {
        sync_focus(state);
    } else {
        let source_ws = if source_destroyed {
            current_ws.min(state.session.workspaces.len().saturating_sub(1))
        } else {
            current_ws
        };
        state.session.switch_to_workspace(source_ws);
        sync_focus(state);
    }

    state.needs_redraw = true;
}

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
