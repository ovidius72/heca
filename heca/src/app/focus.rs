//! Focus-tracking helpers.
//!
//! These functions bridge session/workspace focus state into app-level UI state
//! such as `focused_pane`, focus history, and sidebar projection rebuilds.

use crate::app::terminal_host::notify_focus_changed;
use crate::app_state::AppState;
use heca_core::layout::{FocusDomain, PaneId, Session};

/// Find which workspace contains a pane (by ID). Returns workspace index or None.
pub(crate) fn find_pane_workspace(session: &Session, pane_id: PaneId) -> Option<usize> {
    session
        .workspaces
        .iter()
        .position(|ws| ws.find_pane(pane_id).is_some())
}

/// The single canonical way to focus a pane.
///
/// 1. Switches workspace if the pane is in a different workspace.
/// 2. Activates the pane in the session (scrolling column + pane indices).
/// 3. Calls `sync_focus()` to update `focused_pane`, track history, and rebuild the sidebar.
///
/// **All** focus changes must go through this function — mouse clicks, keyboard nav,
/// sidebar selection, pane select, swap-and-focus, etc.
pub(crate) fn focus_pane_by_id(state: &mut AppState, pane_id: PaneId) {
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
                if pane.id == pane_id {
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
            ws.focus_domain = FocusDomain::Tiled;
            ws.scrolling.activate_column(ci);
            if let Some(col) = ws.scrolling.columns.get_mut(ci) {
                col.activate_pane(pi);
            }
        } else {
            // Not in scrolling columns — check floating panes.
            ws.activate_floating_pane(pane_id);
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
        .map(|p| p.id);
    // Project canonical WM focus onto the chrome's derived selection (write-via-action).
    // The chrome build reads selection from here, not from `Session` — the Phase-2
    // state boundary (`pluggable-chrome-plugin-plan.md` §3.3): the WorkspacesContainer
    // (and future providers) consume `chrome_state`, never `Session` directly.
    #[cfg(debug_assertions)]
    if prev_focused != state.focused_pane {
        eprintln!(
            "[heca] sync_focus {prev_focused:?} -> {:?} (ws {prev_ws} -> {})",
            state.focused_pane, state.session.active_workspace_idx,
        );
    }
    state
        .chrome_state
        .workspaces
        .set_active_pane(state.focused_pane);
    // **The dock cursor follows the active pane — when the active pane CHANGES** (user decision,
    // 2026-07-29; the second half added 2026-07-30, F003/P086/T365).
    //
    // The cursor points at "where you are", so when a pane becomes active you are there — a `prefix+q`
    // pick that leaves the dock highlighting the pane you came *from* is just wrong. Only the active
    // marker followed before, and nothing pushed this direction at all.
    //
    // But `sync_focus` runs after **every** layout change, not only after a focus change, so writing
    // the cursor unconditionally yanked it back to the active pane on any of them: rename a pane your
    // cursor is on but which is not the active one, and the outline jumped to the active row the
    // moment the rename applied. Nothing there moved focus — so nothing should have moved the cursor.
    //
    // Both halves are written because both are read: the domain-typed `nav_selection` is what the
    // tree re-derives its positional cursor from after a rebuild (`apply_nav_selection`, below), and
    // the per-mount `container_cursor` is what the row outlines draw from (F003/P085/T354). The
    // setters are change-guarded, so an unchanged selection costs nothing and emits nothing.
    if let Some(selection) = cursor_follow(prev_focused, state.focused_pane) {
        state.chrome_state.workspaces.set_nav_selection(Some(selection));
        let key = crate::providers::workspaces::selection_nav_key(selection);
        // Every seating of the component, since each keeps its own cursor.
        let mounts: Vec<String> = state
            .chrome_host
            .mounted_providers()
            .filter(|p| p.kind() == "workspaces")
            .map(|p| p.id().to_string())
            .collect();
        for mount in mounts {
            state
                .chrome_state
                .set_container_cursor(&mount, Some(key.clone()));
        }
    }
    notify_focus_changed(state, prev_focused, state.focused_pane);

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
            .map(|ws| ws.find_pane(pid).is_some())
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
    state.chrome_state.workspaces.tree_mut().sync_from_session(
        &state.session,
        state.last_visited_ws_idx,
        state.focused_pane,
        &state.last_visited_pane_per_ws,
    );
    // Re-project the canonical workspace-collapse set (chrome_state) onto the rebuilt
    // tree — sync_from_session defaults to expanded.
    let collapsed = state
        .chrome_state
        .workspaces
        .with_collapsed_ws(|s| s.clone());
    state.chrome_state.workspaces.tree_mut().apply_ws_collapsed(&collapsed, None);
    // Same for the canonical nav selection: the rebuild renumbers `flat_items`, so the
    // positional cursor is re-derived from the selection (which names its row and so
    // survives the rebuild) rather than being left pointing at whatever now sits at that
    // index. Collapse changes `flat_items` too, so this runs after `apply_ws_collapsed`.
    let selection = state.chrome_state.workspaces.nav_selection();
    state.chrome_state.workspaces.tree_mut().apply_nav_selection(selection);
}

/// Where a dock's cursor should move after a focus sync: to the newly active pane, and **only when
/// the active pane actually changed**.
///
/// Split out from [`sync_focus`] so the rule is testable at all — `sync_focus` needs an `AppState`,
/// which cannot be constructed without a window (F003/P086/T365).
fn cursor_follow(
    prev: Option<PaneId>,
    now: Option<PaneId>,
) -> Option<crate::chrome::SidebarSelection> {
    let pane_id = now.filter(|_| prev != now)?;
    Some(crate::chrome::SidebarSelection::Pane { pane_id })
}

#[cfg(test)]
mod tests {
    use super::cursor_follow;
    use crate::chrome::SidebarSelection;
    use heca_core::layout::PaneId;

    /// A new active pane pulls the cursor to it — the `prefix+q` case: picking a pane must not
    /// leave the dock outlining the one you came from.
    #[test]
    fn the_cursor_follows_a_pane_that_just_became_active() {
        assert_eq!(
            cursor_follow(Some(PaneId(1)), Some(PaneId(2))),
            Some(SidebarSelection::Pane { pane_id: PaneId(2) }),
        );
        // …including the first sync, where nothing was active before.
        assert_eq!(
            cursor_follow(None, Some(PaneId(1))),
            Some(SidebarSelection::Pane { pane_id: PaneId(1) }),
        );
    }

    /// **An unchanged active pane moves nothing.** `sync_focus` runs after every layout change, so
    /// writing the cursor unconditionally yanked it to the active row on changes that never touched
    /// focus — renaming a pane the cursor was on, but which was not the active one, was the report.
    #[test]
    fn a_layout_change_that_did_not_move_focus_leaves_the_cursor_alone() {
        assert_eq!(cursor_follow(Some(PaneId(3)), Some(PaneId(3))), None);
        assert_eq!(cursor_follow(None, None), None);
    }

    /// Nothing active leaves the cursor where the user put it, rather than clearing it.
    #[test]
    fn losing_the_active_pane_does_not_clear_the_cursor() {
        assert_eq!(cursor_follow(Some(PaneId(1)), None), None);
    }
}
