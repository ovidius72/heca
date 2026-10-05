//! Workspaces and columns as things: create, delete, add to, collapse and expand.

use super::confirm::request_destructive;
use crate::app::interaction::focused_pane_id;
use crate::app_state::AppState;
use crate::input::WmAction;
use crate::{find_pane_location, pane_name, switch_workspace_tracked};
use heca_core::layout::{Pane as LayoutPane, PaneId};

pub fn handle_create_workspace(state: &mut AppState, _action: &WmAction) {
    state.layout_mut().add_workspace();
    let new_idx = state.session.workspaces.len() - 1;
    switch_workspace_tracked(state, new_idx);
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    state.layout_mut().add_pane(pane, None, true);
    state.start_shell_in(PaneId(next_id), new_idx);
    while state.last_visited_pane_per_ws.len() <= new_idx {
        state.last_visited_pane_per_ws.push(None);
    }
    while state.expose_cursor_per_ws.len() <= new_idx {
        state.expose_cursor_per_ws.push(None);
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
    let ws_idx = state.layout().active_workspace_idx();
    (ws_idx < state.session.workspaces.len()).then_some(ws_idx)
}

fn current_tiled_column_target(state: &AppState) -> Option<(usize, usize)> {
    let pane_id = focused_pane_id(state)?;
    let (ws_idx, col_idx, _) = find_pane_location(&state.session, pane_id)?;
    (ws_idx == state.layout().active_workspace_idx()).then_some((ws_idx, col_idx))
}
