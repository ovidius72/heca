//! Taking a pane: pulling it out of wherever it is into the active column.

use super::docks::focus_navigable_dock;
use super::pick::{begin_pick, pane_candidates_with_stable_letters};
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;
use heca_core::layout::{ColumnId, FocusDomain, PaneId};

pub fn handle_pane_take(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(
        state,
        InputMode::PaneTake {
            candidates,
            focus_after: false,
        },
    );
}

pub fn handle_pane_take_and_focus(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(
        state,
        InputMode::PaneTake {
            candidates,
            focus_after: true,
        },
    );
}

/// Move a pane from wherever it is to the bottom of the active column.
pub fn handle_take_pane(state: &mut AppState, action: &WmAction) {
    let WmAction::TakePane {
        pane_id,
        focus_after,
    } = action
    else {
        return;
    };
    let target = *pane_id;
    let should_focus = *focus_after;

    if pane_is_already_active_column_tail(state, target) {
        return;
    }

    let active_ws_idx = state.session.active_workspace_idx;
    let Some((src_ws, pane)) = remove_take_pane_source(state, target) else {
        return;
    };
    insert_taken_pane_into_active_column(state, pane, should_focus);
    if src_ws != active_ws_idx {
        crate::destroy_empty_workspace(state, src_ws);
    }
}

fn pane_is_already_active_column_tail(state: &AppState, pane_id: PaneId) -> bool {
    let Some(ws) = state.session.active_workspace() else {
        return false;
    };
    let active_col = ws.scrolling.active_column_idx;
    active_col < ws.scrolling.columns.len()
        && ws.scrolling.columns[active_col].panes.last().map(|p| p.id) == Some(pane_id)
}

fn remove_take_pane_source(
    state: &mut AppState,
    pane_id: PaneId,
) -> Option<(usize, heca_core::layout::column::Pane)> {
    let tiled = crate::find_pane_location(&state.session, pane_id).and_then(
        |(src_ws, src_col, src_idx)| {
            state
                .session
                .workspaces
                .get_mut(src_ws)
                .and_then(|ws| {
                    if src_col < ws.scrolling.columns.len() {
                        ws.scrolling.remove_pane(src_col, src_idx)
                    } else {
                        None
                    }
                })
                .map(|pane| (src_ws, pane))
        },
    );
    tiled.or_else(|| remove_take_pane_from_floating(state, pane_id))
}

fn remove_take_pane_from_floating(
    state: &mut AppState,
    pane_id: PaneId,
) -> Option<(usize, heca_core::layout::column::Pane)> {
    for (ws_idx, ws) in state.session.workspaces.iter_mut().enumerate() {
        if let Some(pos) = ws.floating_panes.iter().position(|f| f.pane.id == pane_id) {
            let fp = ws.floating_panes.remove(pos);
            ws.deactivate_floating_panes();
            ws.focus_domain = FocusDomain::Tiled;
            return Some((ws_idx, fp.pane));
        }
    }
    None
}

fn insert_taken_pane_into_active_column(
    state: &mut AppState,
    pane: heca_core::layout::column::Pane,
    should_focus: bool,
) {
    let active_col = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    let new_col_id = ColumnId(state.session.next_id());
    let Some(ws) = state.session.active_workspace_mut() else {
        return;
    };
    if active_col < ws.scrolling.columns.len() {
        ws.scrolling
            .add_pane_to_column(active_col, None, pane, should_focus);
    } else if ws.scrolling.columns.is_empty() {
        let col = ws.scrolling.new_column(new_col_id, pane);
        ws.scrolling.add_column(None, col, should_focus);
    } else {
        let last = ws.scrolling.columns.len() - 1;
        ws.scrolling
            .add_pane_to_column(last, None, pane, should_focus);
    }
}
