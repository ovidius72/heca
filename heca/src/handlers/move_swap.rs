//! Every move and swap — of a pane or a column, within a workspace or to another one.

use super::docks::focus_navigable_dock;
use super::pick::{act_refusal, begin_pick, pane_candidates_with_stable_letters};
use crate::app::focus::focus_pane_by_id;
use crate::app::pane_ops::{
    swap_panes_cross_workspace, swap_panes_diff_columns, swap_panes_same_column,
};
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;
use crate::{
    find_pane_location, move_pane_to_column, move_pane_to_workspace_column, pane_name,
    switch_workspace_tracked,
};
use heca_core::layout::{ColumnId, PaneId};

pub fn handle_swap_left(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_column_left();
        ws.scrolling.align_view_to_active_column();
    }
}

pub fn handle_swap_right(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_column_right();
        ws.scrolling.align_view_to_active_column();
    }
}

pub fn handle_swap_up(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.active_column() {
            let pane_idx = col.active_pane_idx;
            let swap_with = pane_idx.saturating_sub(1);
            if swap_with != pane_idx {
                let _ = swap_panes_same_column(ws, col_idx, pane_idx, swap_with);
            }
        }
    }
}

pub fn handle_swap_down(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.active_column() {
            let pane_idx = col.active_pane_idx;
            let swap_with = (pane_idx + 1).min(col.panes.len().saturating_sub(1));
            if swap_with != pane_idx {
                let _ = swap_panes_same_column(ws, col_idx, pane_idx, swap_with);
            }
        }
    }
}

pub fn handle_move_pane_left(state: &mut AppState, action: &WmAction) {
    // `Some(id)` (pane-header button / RPC) targets a specific pane; focus it first
    // so the active-pane move below operates on it. `None` (keyboard) = active pane.
    if let WmAction::MovePaneLeft { pane_id: Some(id) } = action {
        focus_pane_by_id(state, *id);
    }
    // Allocated before the workspace is borrowed; spent only if the move creates a column.
    let new_column_id = heca_core::layout::ColumnId(state.session.next_id());
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_active_pane_left(new_column_id);
    }
}

pub fn handle_move_pane_right(state: &mut AppState, action: &WmAction) {
    if let WmAction::MovePaneRight { pane_id: Some(id) } = action {
        focus_pane_by_id(state, *id);
    }
    let new_column_id = heca_core::layout::ColumnId(state.session.next_id());
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_active_pane_right(new_column_id);
    }
}

pub fn handle_move_column_up(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    if current_ws == 0 {
        return;
    }
    let target_ws = current_ws - 1;
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    crate::move_column_to_workspace(state, col_idx, target_ws, true);
}

pub fn handle_move_column_down(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    if current_ws >= state.session.workspaces.len().saturating_sub(1) {
        return;
    }
    let target_ws = current_ws + 1;
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    crate::move_column_to_workspace(state, col_idx, target_ws, true);
}

pub fn handle_move_column_to_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::MoveColumnToWorkspace {
        col_idx,
        ws_idx,
        focus,
    } = action
    else {
        return;
    };
    crate::move_column_to_workspace(state, *col_idx, *ws_idx, *focus);
}

pub fn handle_move_column(state: &mut AppState, action: &WmAction) {
    let WmAction::MoveColumn {
        src_ws,
        src_col,
        dst_ws,
        dst_idx,
        focus,
    } = action
    else {
        return;
    };
    crate::app::mutations::move_column(state, *src_ws, *src_col, *dst_ws, *dst_idx, *focus);
}

pub fn handle_swap_columns(state: &mut AppState, action: &WmAction) {
    let WmAction::SwapColumns {
        a_ws,
        a_col,
        b_ws,
        b_col,
    } = action
    else {
        return;
    };
    crate::app::mutations::swap_columns_at(state, *a_ws, *a_col, *b_ws, *b_col);
}

pub fn handle_swap_param(state: &mut AppState, action: &WmAction) {
    let WmAction::Swap { a_id, b_id } = action else {
        return;
    };
    if a_id == b_id {
        return;
    }

    // Find both panes' locations.
    let a_loc = find_pane_location(&state.session, *a_id);
    let b_loc = find_pane_location(&state.session, *b_id);
    let ((aws, acol, api), (bws, bcol, bpi)) = match (a_loc, b_loc) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            return;
        }
    };

    if aws == bws {
        // Same workspace.
        if acol == bcol {
            // Same column: delegate to shared helper.
            if let Some(ws) = state.session.workspaces.get_mut(aws) {
                let _ = swap_panes_same_column(ws, acol, api, bpi);
            }
        } else {
            // Different columns, same workspace: use placeholder approach.
            // Pre-generate IDs before mutating the workspace.
            let placeholder_a_id = PaneId(state.session.next_id());
            let placeholder_b_id = PaneId(state.session.next_id());
            let new_col_for_a = ColumnId(state.session.next_id());
            let new_col_for_b = ColumnId(state.session.next_id());

            if let Some(ws) = state.session.workspaces.get_mut(aws) {
                swap_panes_diff_columns(crate::app::pane_ops::SwapDiffColumnsArgs {
                    ws,
                    a_id: *a_id,
                    b_id: *b_id,
                    a_col: acol,
                    a_pi: api,
                    b_col: bcol,
                    b_pi: bpi,
                    placeholder_a_id,
                    placeholder_b_id,
                    new_col_for_a,
                    new_col_for_b,
                    pane_name_fn: &pane_name,
                    viewport_w: state.session.viewport_size.w,
                    viewport_h: state.session.viewport_size.h,
                });
            }
        }
    } else {
        // Different workspaces: delegate to shared helper.
        swap_panes_cross_workspace(crate::app::pane_ops::SwapCrossWorkspaceArgs {
            session: &mut state.session,
            a_id: *a_id,
            b_id: *b_id,
            a_ws: aws,
            a_col: acol,
            a_pi: api,
            b_ws: bws,
            b_col: bcol,
            b_pi: bpi,
        });
    }

    // Update AppState.focused_pane and sidebar after the swap.
}

pub fn handle_move_param(state: &mut AppState, action: &WmAction) {
    let WmAction::Move {
        pane_id,
        target_col,
    } = action
    else {
        return;
    };
    if let Some((ws_idx, col_idx, _)) = find_pane_location(&state.session, *pane_id) {
        // Ensure the source workspace is active before calling move_pane_to_column,
        // which operates on the active workspace.
        if state.session.active_workspace_idx != ws_idx {
            switch_workspace_tracked(state, ws_idx);
        }
        move_pane_to_column(state, *pane_id, col_idx, *target_col);
    }
}

pub fn handle_move_pane_to_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::MovePaneToWorkspace { pane_id, ws_idx } = action else {
        return;
    };
    if let Some((current_ws, current_col, _)) = find_pane_location(&state.session, *pane_id)
        && current_ws != *ws_idx
    {
        if state.session.active_workspace_idx != current_ws {
            switch_workspace_tracked(state, current_ws);
        }
        // Move-to-workspace: the pane becomes its own new column (preserve layout).
        move_pane_to_workspace_column(state, *pane_id, *ws_idx, current_col, false);
    }
}

pub fn handle_move_pane_to_column(state: &mut AppState, action: &WmAction) {
    let WmAction::MovePaneToColumn {
        pane_id,
        ws_idx,
        col_idx,
    } = action
    else {
        return;
    };
    if let Some((current_ws, current_col, _)) = find_pane_location(&state.session, *pane_id) {
        if current_ws == *ws_idx {
            if state.session.active_workspace_idx != current_ws {
                switch_workspace_tracked(state, current_ws);
            }
            move_pane_to_column(state, *pane_id, current_col, *col_idx);
        } else {
            if state.session.active_workspace_idx != current_ws {
                switch_workspace_tracked(state, current_ws);
            }
            // Move-to-column: stack the pane into the existing target column.
            move_pane_to_workspace_column(state, *pane_id, *ws_idx, *col_idx, true);
        }
    }
}

pub fn handle_swap_pane(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(
        state,
        InputMode::PaneSwap {
            candidates,
            focus_after: false,
        },
    );
}

pub fn handle_swap_and_focus_pane(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(
        state,
        InputMode::PaneSwap {
            candidates,
            focus_after: true,
        },
    );
}

/// Enter the "move active pane → column" letter pick: assign a letter to each column
/// in the active workspace (shown as a `KeyHint` over its sidebar column); the next
/// keypress moves the active pane into that column (stacking with its panes). No-ops
/// without a focused pane or columns.
/// **Take the focused pane out of its column into a new one, right of it.**
///
/// The layout change itself lives in `heca-core`'s scrolling space, reached through
/// [`move_pane_to_new_column`](crate::app::mutations::move_pane_to_new_column) — this handler
/// computes no geometry, the same way every other structural action works.
pub fn handle_move_pane_to_new_column(state: &mut AppState, _action: &WmAction) {
    let Some(pane_id) = state.focused_pane else {
        return;
    };
    if crate::app::mutations::move_pane_to_new_column(state, pane_id) {
        return;
    }
    // It is already the only pane in its column: it would leave a column of one and land in a
    // column of one. Say so rather than looking like a key that did not register.
    state.status_note = Some(act_refusal(
        &state.action_catalog,
        "move_pane_to_new_column",
        "it is already the only pane in its column",
    ));
}
