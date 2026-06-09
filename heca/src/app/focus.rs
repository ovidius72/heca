//! Focus-tracking helpers.
//!
//! These functions bridge session/workspace focus state into app-level UI state
//! such as `focused_pane`, focus history, and sidebar projection rebuilds.

use crate::app_state::AppState;
use heca_core::layout::Session;

/// Find which workspace contains a pane (by ID). Returns workspace index or None.
fn find_pane_workspace(session: &Session, pane_id: u64) -> Option<usize> {
    let target = heca_core::layout::PaneId(pane_id);
    session
        .workspaces
        .iter()
        .position(|ws| ws.find_pane(target).is_some())
}

/// The single canonical way to focus a pane.
///
/// 1. Switches workspace if the pane is in a different workspace.
/// 2. Activates the pane in the session (scrolling column + pane indices).
/// 3. Calls `sync_focus()` to update `focused_pane`, track history, and rebuild the sidebar.
///
/// **All** focus changes must go through this function — mouse clicks, keyboard nav,
/// sidebar selection, pane select, swap-and-focus, etc.
pub(crate) fn focus_pane_by_id(state: &mut AppState, pane_id: u64) {
    // Switch workspace if the target pane is not in the current workspace.
    if let Some(target_ws) = find_pane_workspace(&state.session, pane_id)
        && target_ws != state.session.active_workspace_idx
    {
        switch_workspace_tracked(state, target_ws);
    }

    if let Some(ws) = state.session.active_workspace_mut() {
        // Find location first (immutable scan), then mutate.
        let mut found = None;
        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
            for (pi, pane) in col.panes.iter().enumerate() {
                if pane.id.0 == pane_id {
                    found = Some((ci, pi));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }

        if let Some((ci, pi)) = found {
            ws.deactivate_floating_panes();
            ws.floating_is_active = false;
            ws.scrolling.activate_column(ci);
            if let Some(col) = ws.scrolling.columns.get_mut(ci) {
                col.activate_pane(pi);
            }
        } else {
            // Not in scrolling columns — check floating panes.
            ws.activate_floating_pane(heca_core::layout::PaneId(pane_id));
        }
    }

    // sync_focus reads the session's active pane, updates AppState, records history,
    // and rebuilds the sidebar tree. This is the ONLY place the sidebar rebuilds.
    sync_focus(state);
}

/// The single canonical way to switch workspaces.
/// Updates last_visited_ws_idx so Prefix+Shift+l can toggle back.
/// Does NOT touch last_visited_pane_per_ws — that field is reserved for
/// same-workspace pane toggle (Prefix+i) and must not be overwritten by
/// workspace switches.
pub(crate) fn switch_workspace_tracked(state: &mut AppState, new_idx: usize) {
    let current_ws = state.session.active_workspace_idx;
    if current_ws == new_idx {
        return;
    }
    state.last_visited_ws_idx = Some(current_ws);
    state.session.switch_to_workspace(new_idx);
}

/// Sync `focused_pane` from session active state and rebuild dependent UI state.
pub(crate) fn sync_focus(state: &mut AppState) {
    let prev_focused = state.focused_pane;
    let prev_ws = state.session.active_workspace_idx;

    // Update focused_pane from session state.
    state.focused_pane = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id.0);

    let focus_changed = prev_focused != state.focused_pane;
    let current_ws = state.session.active_workspace_idx;

    // Only record per-workspace data for same-workspace focus changes.
    // If prev_focused doesn't belong to current_ws, a workspace switch
    // happened and switch_workspace_tracked() already recorded it.
    let pane_belongs_to_current_ws = prev_focused.is_some_and(|pid| {
        state
            .session
            .workspaces
            .get(current_ws)
            .map(|ws| ws.find_pane(heca_core::layout::PaneId(pid)).is_some())
            .unwrap_or(false)
    });
    if focus_changed
        && prev_focused.is_some()
        && prev_ws == current_ws
        && pane_belongs_to_current_ws
    {
        while state.last_visited_pane_per_ws.len() <= current_ws {
            state.last_visited_pane_per_ws.push(None);
        }
        state.last_visited_pane_per_ws[current_ws] = prev_focused;
    }

    // Track global last_focused (for Prefix+Shift+l toggle).
    if focus_changed && prev_focused.is_some() {
        state.last_focused = prev_focused;
    }

    // Sync sidebar tree from session state.
    state.sidebar_tree.sync_from_session(
        &state.session,
        state.last_visited_ws_idx,
        state.focused_pane,
        &state.last_visited_pane_per_ws,
    );
}
