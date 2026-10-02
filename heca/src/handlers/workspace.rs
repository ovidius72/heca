//! Workspaces and columns as things: create, delete, add to, collapse and expand.

use super::confirm::request_destructive;
use super::split_resize::handle_split_horizontal;
use crate::app::interaction::focused_pane_id;
use crate::app_state::AppState;
use crate::input::WmAction;
use crate::{find_pane_location, pane_name, switch_workspace_tracked};
use heca_core::layout::{Pane as LayoutPane, PaneId};

pub fn handle_create_workspace(state: &mut AppState, _action: &WmAction) {
    let working_area = state
        .session
        .active_workspace()
        .map(|ws| {
            heca_core::layout::types::Rectangle::new(
                ws.scrolling.working_area.loc,
                ws.scrolling.working_area.size,
            )
        })
        .unwrap_or_else(|| {
            heca_core::layout::types::Rectangle::new(
                heca_core::layout::types::Point::default(),
                state.session.viewport_size,
            )
        });
    state.session.add_workspace(working_area);
    let new_idx = state.session.workspaces.len() - 1;
    switch_workspace_tracked(state, new_idx);
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    state.session.add_pane(pane, None, true);
    state.start_shell_in(PaneId(next_id), new_idx);
    while state.last_visited_pane_per_ws.len() <= new_idx {
        state.last_visited_pane_per_ws.push(None);
    }
    while state.expose_cursor_per_ws.len() <= new_idx {
        state.expose_cursor_per_ws.push(None);
    }
}

/// Add a pane to a specific column in a specific workspace.
/// Switches to the target workspace first.
pub fn handle_add_pane_to_column(state: &mut AppState, action: &WmAction) {
    let WmAction::AddPaneToColumn { ws_idx, col_idx } = action else {
        return;
    };
    let target_ws = *ws_idx;
    if target_ws >= state.session.workspaces.len() {
        return;
    }
    // Switch to target workspace if needed
    if state.session.active_workspace_idx != target_ws {
        crate::switch_workspace_tracked(state, target_ws);
    }
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    let backend_id = PaneId(next_id);
    let col = *col_idx;
    if let Some(ws) = state.session.active_workspace_mut() {
        let capped_col = col.min(ws.scrolling.columns.len().saturating_sub(1));
        ws.scrolling
            .add_pane_to_column(capped_col, None, pane, true);
    }
    state.start_shell_in(backend_id, target_ws);
}

/// Add a new column to a specific workspace (sidebar right-click context menu).
/// Switches to the target workspace if needed, then creates a new column via a
/// horizontal split. Explicit target, so unlike `sidebar_create_column` it does not
/// depend on the sidebar-nav mode or cursor.
pub fn handle_add_column_to_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::AddColumnToWorkspace { ws_idx } = action else {
        return;
    };
    let target_ws = *ws_idx;
    if target_ws >= state.session.workspaces.len() {
        return;
    }
    if state.session.active_workspace_idx != target_ws {
        crate::switch_workspace_tracked(state, target_ws);
    }
    handle_split_horizontal(state, &WmAction::SplitHorizontal);
}

/// Delete a column and all its panes (destructive).
pub fn handle_delete_column(state: &mut AppState, action: &WmAction) {
    let WmAction::DeleteColumn { ws_idx, col_idx } = action else {
        return;
    };
    let target_ws = *ws_idx;
    if target_ws >= state.session.workspaces.len() {
        return;
    }
    // Collect pane IDs from the column, remove backends, then remove the column.
    let pane_ids: Vec<PaneId> = state
        .session
        .workspaces
        .get(target_ws)
        .and_then(|ws| ws.scrolling.columns.get(*col_idx))
        .map(|col| col.panes.iter().map(|p| p.id).collect())
        .unwrap_or_default();

    for &id in &pane_ids {
        state.clear_search(id);
    }
    state.server.backends.kill_all(pane_ids);

    // Remove the column
    if let Some(ws) = state.session.workspaces.get_mut(target_ws) {
        let capped_col = (*col_idx).min(ws.scrolling.columns.len().saturating_sub(1));
        ws.scrolling.remove_column(capped_col);
    }
}

/// Delete a workspace and all its columns/panes (destructive).
/// The last workspace cannot be deleted.
pub fn handle_delete_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::DeleteWorkspace { ws_idx } = action else {
        return;
    };
    let target_ws = *ws_idx;
    // Deleting the last workspace is allowed (leaves the session empty; recoverable via
    // create-workspace) — only bail on an out-of-range index.
    if target_ws >= state.session.workspaces.len() {
        return;
    }

    // Collect all pane IDs from the workspace to clean up backends
    let pane_ids: Vec<PaneId> = state
        .session
        .workspaces
        .get(target_ws)
        .map(|ws| {
            let mut ids: Vec<PaneId> = ws
                .scrolling
                .columns
                .iter()
                .flat_map(|col| col.panes.iter().map(|p| p.id))
                .collect();
            ids.extend(ws.floating_panes.iter().map(|float| float.pane.id));
            ids
        })
        .unwrap_or_default();

    for &id in &pane_ids {
        state.clear_search(id);
    }
    state.server.backends.kill_all(pane_ids);

    // Remove the workspace
    state.session.remove_workspace(target_ws);

    // Fix up tracking indices (same logic as destroy_empty_workspace)
    if state.last_visited_ws_idx == Some(target_ws) {
        state.last_visited_ws_idx = None;
    } else if let Some(ref mut idx) = state.last_visited_ws_idx
        && *idx > target_ws
    {
        *idx -= 1;
    }
    if target_ws < state.last_visited_pane_per_ws.len() {
        state.last_visited_pane_per_ws.remove(target_ws);
    }
    if target_ws < state.expose_cursor_per_ws.len() {
        state.expose_cursor_per_ws.remove(target_ws);
    }
}

/// Delete the "current" column (and all its panes) — the **focused pane's** column.
///
/// The container's own `workspaces.delete_selected_column` handles the cursor's column
/// (F003/P085/T356); this is the plain global binding, and it no longer branches on a mode that
/// does not exist.
pub fn handle_delete_current_column(state: &mut AppState, _action: &WmAction) {
    let Some((ws_idx, col_idx)) = current_tiled_column_target(state) else {
        return;
    };
    request_destructive(state, WmAction::DeleteColumn { ws_idx, col_idx });
}

/// Apply a workspace collapse change: write the canonical `chrome_state.collapsed_ws`,
/// then project it into the sidebar nav model. `collapse = None` toggles.
pub(crate) fn apply_ws_collapse(state: &mut AppState, ws_idx: usize, collapse: Option<bool>) {
    match collapse {
        Some(c) => state.chrome_state.workspaces.set_ws_collapsed(ws_idx, c),
        None => state.chrome_state.workspaces.toggle_ws_collapsed(ws_idx),
    }
    let set = state
        .chrome_state
        .workspaces
        .with_collapsed_ws(|s| s.clone());
    state
        .chrome_state
        .workspaces
        .tree_mut()
        .apply_ws_collapsed(&set, Some(ws_idx));
}

pub fn handle_collapse_current_workspace(state: &mut AppState, _action: &WmAction) {
    let Some(ws_idx) = current_active_workspace_idx(state) else {
        return;
    };
    apply_ws_collapse(state, ws_idx, Some(true));
}

pub fn handle_expand_current_workspace(state: &mut AppState, _action: &WmAction) {
    let Some(ws_idx) = current_active_workspace_idx(state) else {
        return;
    };
    apply_ws_collapse(state, ws_idx, Some(false));
}

pub fn handle_toggle_current_workspace_collapsed(state: &mut AppState, _action: &WmAction) {
    let Some(ws_idx) = current_active_workspace_idx(state) else {
        return;
    };
    apply_ws_collapse(state, ws_idx, None);
}

pub fn handle_collapse_current_column(state: &mut AppState, _action: &WmAction) {
    let Some((ws_idx, col_idx)) = current_tiled_column_target(state) else {
        return;
    };
    state
        .chrome_state
        .workspaces
        .tree_mut()
        .collapse_column(ws_idx, col_idx);
}

pub fn handle_expand_current_column(state: &mut AppState, _action: &WmAction) {
    let Some((ws_idx, col_idx)) = current_tiled_column_target(state) else {
        return;
    };
    state
        .chrome_state
        .workspaces
        .tree_mut()
        .expand_column(ws_idx, col_idx);
}

pub fn handle_toggle_current_column_collapsed(state: &mut AppState, _action: &WmAction) {
    let Some((ws_idx, col_idx)) = current_tiled_column_target(state) else {
        return;
    };
    state
        .chrome_state
        .workspaces
        .tree_mut()
        .toggle_column_collapsed(ws_idx, col_idx);
}

fn current_active_workspace_idx(state: &AppState) -> Option<usize> {
    let ws_idx = state.session.active_workspace_idx;
    (ws_idx < state.session.workspaces.len()).then_some(ws_idx)
}

fn current_tiled_column_target(state: &AppState) -> Option<(usize, usize)> {
    let pane_id = focused_pane_id(state)?;
    let (ws_idx, col_idx, _) = find_pane_location(&state.session, pane_id)?;
    (ws_idx == state.session.active_workspace_idx).then_some((ws_idx, col_idx))
}
