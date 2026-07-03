//! Named action handlers — one per `WmAction` variant.
//!
//! Each handler is a plain `fn(&mut AppState, &WmAction)` that performs the
//! action.  Parameterized variants destructure their fields from the enum;
//! unit variants ignore the `_action` parameter.

use crate::app::backend_factory::{
    create_command_backend_for_state, create_terminal_backend_for_state,
    terminal_grid_for_workspace,
};
use crate::app::focus::{focus_pane_by_id, sync_focus};
use crate::app::interaction::{focused_pane_id, pane_is_floating};
use crate::app::mutations::{after_focus_change, after_layout_change, after_metadata_change};
use crate::app::pane_ops::{
    swap_panes_cross_workspace, swap_panes_diff_columns, swap_panes_same_column,
};
use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionSource};
use crate::app::terminal_host::{
    ensure_caret_visible, enter_selection_mode_for_focused_terminal,
    move_focused_terminal_selection,
};
use crate::app_state::{AppState, InputMode, RenameTarget, WorkspacePickTarget};
use crate::chrome;
use crate::input::{FontZoomStep, SpawnKind, WmAction};
use crate::sidebar;
use crate::{
    collect_all_pane_candidates, destroy_empty_workspace, find_pane_location, move_pane_to_column,
    move_pane_to_workspace_column, pane_name, switch_workspace_tracked, update_session_viewport,
};
use heca_core::layout::{Column, ColumnId, ColumnWidth, FocusDomain, Pane as LayoutPane, PaneId};

// ── Navigation ──

pub fn handle_focus_left(state: &mut AppState, _action: &WmAction) {
    state.session.focus_left();
    after_focus_change(state);
}

pub fn handle_focus_right(state: &mut AppState, _action: &WmAction) {
    state.session.focus_right();
    after_focus_change(state);
}

pub fn handle_focus_up(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.focus_up();
    }
    after_focus_change(state);
}

pub fn handle_focus_down(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.focus_down();
    }
    after_focus_change(state);
}

pub fn handle_next_pane(state: &mut AppState, _action: &WmAction) {
    state.session.focus_right();
    after_focus_change(state);
}

pub fn handle_prev_pane(state: &mut AppState, _action: &WmAction) {
    state.session.focus_left();
    after_focus_change(state);
}

pub fn handle_workspace_next(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    let next = (current_ws + 1).min(state.session.workspaces.len().saturating_sub(1));
    if next != current_ws {
        switch_workspace_tracked(state, next);
        after_focus_change(state);
    }
}

pub fn handle_workspace_prev(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    let prev = current_ws.saturating_sub(1);
    if prev != current_ws {
        switch_workspace_tracked(state, prev);
        after_focus_change(state);
    }
}

pub fn handle_focus_toggle_local(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    if let Some(prev_pane) = state
        .last_visited_pane_per_ws
        .get(ws_idx)
        .copied()
        .flatten()
    {
        if Some(prev_pane) != state.focused_pane {
            focus_pane_by_id(state, prev_pane);
        }
        state.needs_redraw = true;
    }
}

pub fn handle_focus_toggle_global(state: &mut AppState, _action: &WmAction) {
    if let Some(prev_ws) = state.last_visited_ws_idx {
        let current_ws = state.session.active_workspace_idx;
        if prev_ws != current_ws {
            switch_workspace_tracked(state, prev_ws);
            after_focus_change(state);
        } else {
            state.needs_redraw = true;
        }
    }
}

pub fn handle_focus_pane(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusPane { pane_id } = action else {
        return;
    };
    focus_pane_by_id(state, *pane_id);
}

pub fn handle_focus_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusWorkspace { ws_idx } = action else {
        return;
    };
    if *ws_idx < state.session.workspaces.len() {
        switch_workspace_tracked(state, *ws_idx);
        after_focus_change(state);
    }
}

// ── Layout ──

pub fn handle_split_horizontal(state: &mut AppState, _action: &WmAction) {
    let active_ws = state.session.active_workspace_idx;
    let (cols, rows) = terminal_grid_for_workspace(state, active_ws);
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    let backend_id = PaneId(next_id);
    state.session.add_pane(pane, None, true);
    state.backends.insert_for_pane(
        backend_id,
        create_terminal_backend_for_state(state, cols, rows),
    );
    after_layout_change(state);
}

pub fn handle_split_vertical(state: &mut AppState, _action: &WmAction) {
    let active_ws = state.session.active_workspace_idx;
    let (cols, rows) = terminal_grid_for_workspace(state, active_ws);
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    let backend_id = PaneId(next_id);
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.add_pane_to_column(col_idx, None, pane, true);
    }
    state.backends.insert_for_pane(
        backend_id,
        create_terminal_backend_for_state(state, cols, rows),
    );
    after_layout_change(state);
}

pub fn handle_resize_increase(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_active_column(0.05);
    }
    after_layout_change(state);
}

pub fn handle_resize_decrease(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_active_column(-0.05);
    }
    after_layout_change(state);
}

pub fn handle_zoom_column(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.toggle_active_column_zoom();
    }
    after_layout_change(state);
}

/// Pan the horizontal view left/right by a quarter of the viewport, to reach
/// column overflow / content scrolled past an edge. View-only (no focus change).
pub fn handle_scroll_view_left(state: &mut AppState, _action: &WmAction) {
    scroll_view_by(state, -1.0);
}

pub fn handle_scroll_view_right(state: &mut AppState, _action: &WmAction) {
    scroll_view_by(state, 1.0);
}

fn scroll_view_by(state: &mut AppState, sign: f64) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let step = ws.scrolling.working_area.size.w * 0.25;
        ws.scrolling.scroll_view(sign * step);
    }
    after_layout_change(state);
}

pub fn handle_pane_height_increase(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.columns.get_mut(col_idx) {
            let h = ws.scrolling.working_area.size.h;
            let gaps = ws.scrolling.options.gaps;
            col.resize_active_pane_height(40.0, h, gaps);
        }
    }
    after_layout_change(state);
}

pub fn handle_pane_height_decrease(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.columns.get_mut(col_idx) {
            let h = ws.scrolling.working_area.size.h;
            let gaps = ws.scrolling.options.gaps;
            col.resize_active_pane_height(-40.0, h, gaps);
        }
    }
    after_layout_change(state);
}

pub fn handle_swap_left(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_column_left();
        ws.scrolling.align_view_to_active_column();
    }
    after_layout_change(state);
}

pub fn handle_swap_right(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_column_right();
        ws.scrolling.align_view_to_active_column();
    }
    after_layout_change(state);
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
    after_layout_change(state);
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
    after_layout_change(state);
}

pub fn handle_move_pane_left(state: &mut AppState, action: &WmAction) {
    // `Some(id)` (pane-header button / RPC) targets a specific pane; focus it first
    // so the active-pane move below operates on it. `None` (keyboard) = active pane.
    if let WmAction::MovePaneLeft { pane_id: Some(id) } = action {
        focus_pane_by_id(state, *id);
    }
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_active_pane_left();
    }
    after_layout_change(state);
}

pub fn handle_move_pane_right(state: &mut AppState, action: &WmAction) {
    if let WmAction::MovePaneRight { pane_id: Some(id) } = action {
        focus_pane_by_id(state, *id);
    }
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_active_pane_right();
    }
    after_layout_change(state);
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
    after_layout_change(state);
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
    state.needs_redraw = true;
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
    state.needs_redraw = true;
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
    state.needs_redraw = true;
}

pub fn handle_resize(state: &mut AppState, action: &WmAction) {
    let WmAction::Resize {
        target,
        axis,
        amount,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        match (target, axis) {
            (crate::input::ResizeTarget::Column, crate::input::ResizeAxis::X) => {
                let delta_f = *amount / 1000.0;
                ws.scrolling.resize_active_column(delta_f);
            }
            (crate::input::ResizeTarget::Pane, crate::input::ResizeAxis::Y) => {
                let h = ws.scrolling.working_area.size.h;
                let gaps = ws.scrolling.options.gaps;
                if let Some(col) = ws.scrolling.active_column_mut() {
                    col.resize_active_pane_height(*amount, h, gaps);
                }
            }
            _ => {} // Column-Y and Pane-X are not yet implemented
        }
    }
    state.needs_redraw = true;
}

/// Resize a specific column's width (mouse divider-drag / RPC). `delta` is a
/// proportion delta (or fraction of the working width for fixed columns), so a
/// pixel drag maps as `dx / working_area.width`.
pub fn handle_resize_column_by(state: &mut AppState, action: &WmAction) {
    let WmAction::ResizeColumnBy { col_idx, delta } = action else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_column(*col_idx, *delta);
    }
    after_layout_change(state);
}

/// Resize a specific stacked pane's height (mouse divider-drag / RPC). `delta`
/// is logical px (drag down ⇒ taller).
pub fn handle_resize_pane_height_by(state: &mut AppState, action: &WmAction) {
    let WmAction::ResizePaneHeightBy {
        col_idx,
        pane_idx,
        delta,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_pane_height(*col_idx, *pane_idx, *delta);
    }
    after_layout_change(state);
}

pub fn handle_resize_to(state: &mut AppState, action: &WmAction) {
    let WmAction::ResizeTo {
        target,
        width,
        height,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        match target {
            crate::input::ResizeTarget::Column => {
                if let Some(col) = ws.scrolling.active_column_mut() {
                    col.width = ColumnWidth::Fixed(*width);
                    ws.scrolling.update_all_column_widths();
                }
            }
            crate::input::ResizeTarget::Pane => {
                let h = ws.scrolling.working_area.size.h;
                let gaps = ws.scrolling.options.gaps;
                if let Some(col) = ws.scrolling.active_column_mut() {
                    let pane_idx = col.active_pane_idx;
                    if let Some(size) = col.pane_sizes.get_mut(pane_idx) {
                        size.h = *height;
                    }
                    col.compute_pane_sizes(h, gaps);
                }
            }
        }
    }
    state.needs_redraw = true;
}

// ── Pane ──

pub fn handle_float(state: &mut AppState, _action: &WmAction) {
    let pane_id = match focused_pane_id(state) {
        Some(id) => id,
        None => return,
    };
    let is_flt = pane_is_floating(&state.session, pane_id);

    if let Some(ws) = state.session.active_workspace_mut() {
        let wa = ws.scrolling.working_area;

        if is_flt {
            if let Some(idx) = ws.floating_panes.iter().position(|f| f.pane.id == pane_id) {
                let float = ws.floating_panes.remove(idx);
                let orig_col = float.original_column_idx;
                let orig_pane = float.original_pane_idx;
                ws.deactivate_floating_panes();
                if let Some(col_idx) = orig_col {
                    if col_idx < ws.scrolling.columns.len() {
                        let target_idx = orig_pane
                            .unwrap_or(0)
                            .min(ws.scrolling.columns[col_idx].panes.len());
                        ws.scrolling.add_pane_to_column(
                            col_idx,
                            Some(target_idx),
                            float.pane,
                            true,
                        );
                    } else {
                        ws.scrolling.add_column(
                            None,
                            Column::new(
                                ColumnId(pane_id.0),
                                float.pane,
                                chrome::default_column_width(),
                            ),
                            true,
                        );
                    }
                } else {
                    ws.scrolling.add_column(
                        None,
                        Column::new(
                            ColumnId(pane_id.0),
                            float.pane,
                            chrome::default_column_width(),
                        ),
                        true,
                    );
                }
                ws.focus_domain = FocusDomain::Tiled;
            }
        } else {
            let found = crate::app::pane_ops::find_pane_indices_in_workspace(ws, pane_id);
            if let Some((col_idx, pane_idx)) = found
                && let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx)
            {
                let fw = wa.size.w * 0.95;
                let fh = wa.size.h * 0.95;
                let fx = wa.loc.x + (wa.size.w - fw) / 2.0;
                let fy = wa.loc.y + (wa.size.h - fh) / 2.0;
                ws.deactivate_floating_panes();
                ws.floating_panes
                    .push(heca_core::layout::workspace::FloatingPane {
                        pane: removed,
                        position: heca_core::layout::types::Point::new(fx, fy),
                        size: heca_core::layout::types::Size::new(fw, fh),
                        is_active: true,
                        original_column_idx: Some(col_idx),
                        original_pane_idx: Some(pane_idx),
                    });
                ws.focus_domain = FocusDomain::Floating;
            }
        }
    }
    after_layout_change(state);
}

/// Close the currently focused pane.
///
/// Delegates to the floating or tiled close path based on the focused pane's
/// domain. After removal, destroys the workspace if it's empty and others
/// remain, or leaves it empty if it's the only one.
///
/// # Floating domain
///
/// - Removes the floating pane and its backend.
/// - Switches to `FocusDomain::Tiled` only when no floating panes remain.
/// - Focuses the last visited tiled pane (or syncs from session state).
///
/// # Tiled domain
///
/// - Removes the active pane from the active column.
/// - Removes its backend.
///
/// # Empty workspace handling
///
/// If the workspace becomes empty after closing and there are multiple
/// workspaces, the empty one is destroyed and focus switches. If it's
/// the only workspace, it's left empty — the user can repopulate it via
/// the normal split bindings: `prefix+Enter` (new pane in a new column)
/// or `prefix+v` (new pane in the current column). In an empty workspace,
/// either binding effectively creates the first pane again.
pub fn handle_close_pane(state: &mut AppState, _action: &WmAction) {
    let Some(pane_id) = focused_pane_id(state) else {
        return;
    };
    let is_floating = pane_is_floating(&state.session, pane_id);

    if is_floating {
        close_floating_pane(state, pane_id);
    } else {
        close_tiled_pane(state);
    }
    close_workspace_if_empty(state);
    after_layout_change(state);
}

/// Remove a floating pane by ID, switch domain to Tiled if no floats remain,
/// and focus the last visited tiled pane.
fn close_floating_pane(state: &mut AppState, pane_id: PaneId) {
    if let Some(ws) = state.session.active_workspace_mut() {
        if let Some(float_idx) = ws.floating_panes.iter().position(|f| f.pane.id == pane_id) {
            let removed = ws.floating_panes.remove(float_idx);
            state.backends.remove_for_pane(removed.pane.id);
        } else {
            // pane_is_floating returned true but the pane was not found in
            // floating_panes — this indicates a state inconsistency.
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] warning: pane {} reported as floating but not found in floating_panes",
                pane_id
            );
        }
        // Only switch back to tiled domain if no floating panes remain.
        if ws.floating_panes.is_empty() {
            ws.deactivate_floating_panes();
            ws.focus_domain = FocusDomain::Tiled;
        }
    }
    // Focus the last visited pane in the tiled area.
    let last_tiled = state
        .last_visited_pane_per_ws
        .get(state.session.active_workspace_idx)
        .copied()
        .flatten();
    if let Some(target_id) = last_tiled {
        focus_pane_by_id(state, target_id);
    } else {
        // No last-visited pane recorded; sync focus from session state.
        sync_focus(state);
    }
}

/// Remove the active pane from the active column in the scrolling layout.
fn close_tiled_pane(state: &mut AppState) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.active_column() {
            let pane_idx = col.active_pane_idx;
            if let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx) {
                state.backends.remove_for_pane(removed.id);
            }
        }
    }
}

/// Destroy the active workspace if it's empty and other workspaces exist.
/// If it's the only workspace, leave it empty — the user can repopulate it
/// via the normal split bindings: `prefix+Enter` (new pane in a new column)
/// or `prefix+v` (new pane in the current column). In an empty workspace,
/// either binding effectively creates the first pane again.
fn close_workspace_if_empty(state: &mut AppState) {
    let current_ws = state.session.active_workspace_idx;
    let ws_is_empty = state
        .session
        .workspaces
        .get(current_ws)
        .map(|ws| !ws.has_panes())
        .unwrap_or(true);
    if ws_is_empty && state.session.workspaces.len() > 1 {
        destroy_empty_workspace(state, current_ws);
        let new_idx = current_ws.min(state.session.workspaces.len().saturating_sub(1));
        state.session.switch_to_workspace(new_idx);
    }
    // If workspace is empty and it's the only one, leave it empty.
}

pub fn handle_pane_select(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        handle_sidebar_focus(state, &WmAction::SidebarFocus);
        return;
    }
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSelect { candidates };
        state.needs_redraw = true;
    }
}

pub fn handle_follow_link(state: &mut AppState, _action: &WmAction) {
    let candidates = crate::app::terminal_host::collect_link_hints(state);
    if !candidates.is_empty() {
        state.input_mode = InputMode::FollowLink { candidates };
        state.needs_redraw = true;
    }
}

pub fn handle_swap_pane(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        handle_sidebar_focus(state, &WmAction::SidebarFocus);
        return;
    }
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSwap {
            candidates,
            focus_after: false,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_swap_and_focus_pane(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        handle_sidebar_focus(state, &WmAction::SidebarFocus);
        return;
    }
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSwap {
            candidates,
            focus_after: true,
        };
        state.needs_redraw = true;
    }
}

/// Enter the "move active column → workspace" letter pick: assign a letter to each
/// workspace (shown as a `KeyHint` over its dock); the next keypress moves the active
/// column into that workspace. No-ops if there are no workspaces.
pub fn handle_move_column_to_workspace_pick(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    let candidates = crate::app::selection::collect_workspace_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::WorkspacePick {
            candidates,
            target: WorkspacePickTarget::Column { ws_idx, col_idx },
        };
        state.needs_redraw = true;
    }
}

/// Enter the "move active pane → workspace" letter pick (see
/// [`handle_move_column_to_workspace_pick`]). No-ops without a focused pane.
pub fn handle_move_pane_to_workspace_pick(state: &mut AppState, _action: &WmAction) {
    let Some(pane_id) = state.focused_pane else {
        return;
    };
    let candidates = crate::app::selection::collect_workspace_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::WorkspacePick {
            candidates,
            target: WorkspacePickTarget::Pane(pane_id),
        };
        state.needs_redraw = true;
    }
}

/// Enter the "move active pane → column" letter pick: assign a letter to each column
/// in the active workspace (shown as a `KeyHint` over its sidebar column); the next
/// keypress moves the active pane into that column (stacking with its panes). No-ops
/// without a focused pane or columns.
pub fn handle_move_pane_to_column_pick(state: &mut AppState, _action: &WmAction) {
    let Some(pane_id) = state.focused_pane else {
        return;
    };
    let candidates = crate::app::selection::collect_column_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::ColumnPick {
            candidates,
            pane_id,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_rename_pane(state: &mut AppState, _action: &WmAction) {
    if let Some(pane_id) = focused_pane_id(state) {
        // Seed with the existing custom name (so editing a rename keeps it); empty when
        // the pane is still tracking the process name.
        let current_name = state
            .session
            .active_workspace()
            .and_then(|ws| ws.find_pane(pane_id))
            .and_then(|p| p.custom_name.clone())
            .unwrap_or_default();
        state.input_mode = InputMode::Rename {
            target: RenameTarget::Pane(pane_id),
            buffer: current_name,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_rename_column(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    if let Some(ws) = state.session.active_workspace() {
        let col_idx = ws.scrolling.active_column_idx;
        let current_name = ws
            .scrolling
            .columns
            .get(col_idx)
            .and_then(|col| col.name.clone())
            .unwrap_or_default();
        state.input_mode = InputMode::Rename {
            target: RenameTarget::Column { ws_idx, col_idx },
            buffer: current_name,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_float_at(state: &mut AppState, action: &WmAction) {
    let WmAction::FloatAt {
        pane_id,
        x,
        y,
        width,
        height,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut()
        && let Some((col_idx, pane_idx)) =
            crate::app::pane_ops::find_pane_indices_in_workspace(ws, *pane_id)
        && let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx)
    {
        ws.deactivate_floating_panes();
        ws.floating_panes
            .push(heca_core::layout::workspace::FloatingPane {
                pane: removed,
                position: heca_core::layout::types::Point::new(*x, *y),
                size: heca_core::layout::types::Size::new(*width, *height),
                is_active: true,
                original_column_idx: Some(col_idx),
                original_pane_idx: Some(pane_idx),
            });
        ws.focus_domain = FocusDomain::Floating;
    }
    after_layout_change(state);
}

/// Close a specific pane by ID (RPC-style).
///
/// Handles both tiled and floating panes. After removal, destroys the
/// workspace if it's empty and other workspaces remain. If it's the only
/// workspace, it stays empty — the user can repopulate it via the normal
/// split bindings: `prefix+Enter` (new pane in a new column) or `prefix+v`
/// (new pane in the current column). In an empty workspace, either binding
/// effectively creates the first pane again.
pub fn handle_close_pane_by_id(state: &mut AppState, action: &WmAction) {
    let WmAction::ClosePaneById { pane_id } = action else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        if let Some((ci, pi)) = crate::app::pane_ops::find_pane_indices_in_workspace(ws, *pane_id) {
            if let Some(removed) = ws.scrolling.remove_pane(ci, pi) {
                state.backends.remove_for_pane(removed.id);
            }
        } else if let Some(float_idx) = ws.floating_panes.iter().position(|f| f.pane.id == *pane_id)
        {
            let removed = ws.floating_panes.remove(float_idx);
            state.backends.remove_for_pane(removed.pane.id);
            if ws.focus_domain == FocusDomain::Floating && ws.floating_panes.is_empty() {
                ws.deactivate_floating_panes();
                ws.focus_domain = FocusDomain::Tiled;
            }
        }
    }
    close_workspace_if_empty(state);
    after_layout_change(state);
}

pub fn handle_rename_target(state: &mut AppState, action: &WmAction) {
    let WmAction::RenameTarget { pane_id, name } = action else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut()
        && let Some(pane) = ws.find_pane_mut(*pane_id)
    {
        pane.title = if name.is_empty() {
            format!("pane{pane_id}")
        } else {
            name.clone()
        };
        after_metadata_change(state);
    }
}

// ── Workspace ──

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
    let (cols, rows) = terminal_grid_for_workspace(state, target_ws);
    state.backends.insert_for_pane(
        backend_id,
        create_terminal_backend_for_state(state, cols, rows),
    );
    after_layout_change(state);
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

    state.backends.remove_all(pane_ids);

    // Remove the column
    if let Some(ws) = state.session.workspaces.get_mut(target_ws) {
        let capped_col = (*col_idx).min(ws.scrolling.columns.len().saturating_sub(1));
        ws.scrolling.remove_column(capped_col);
    }

    after_layout_change(state);
}

/// Delete a workspace and all its columns/panes (destructive).
/// The last workspace cannot be deleted.
pub fn handle_delete_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::DeleteWorkspace { ws_idx } = action else {
        return;
    };
    let target_ws = *ws_idx;
    if target_ws >= state.session.workspaces.len() || state.session.workspaces.len() <= 1 {
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

    state.backends.remove_all(pane_ids);

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

    after_layout_change(state);
}

// ── Take pane ──

pub fn handle_pane_take(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        handle_sidebar_focus(state, &WmAction::SidebarFocus);
        return;
    }
    let candidates = crate::collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneTake {
            candidates,
            focus_after: false,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_pane_take_and_focus(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        handle_sidebar_focus(state, &WmAction::SidebarFocus);
        return;
    }
    let candidates = crate::collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneTake {
            candidates,
            focus_after: true,
        };
        state.needs_redraw = true;
    }
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

    after_layout_change(state);
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
        let col = Column::new(new_col_id, pane, ColumnWidth::Proportion(0.85));
        ws.scrolling.add_column(None, col, should_focus);
    } else {
        let last = ws.scrolling.columns.len() - 1;
        ws.scrolling
            .add_pane_to_column(last, None, pane, should_focus);
    }
}

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
    let (cols, rows) = terminal_grid_for_workspace(state, new_idx);
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    state.session.add_pane(pane, None, true);
    state.backends.insert_for_pane(
        PaneId(next_id),
        create_terminal_backend_for_state(state, cols, rows),
    );
    while state.last_visited_pane_per_ws.len() <= new_idx {
        state.last_visited_pane_per_ws.push(None);
    }
    after_layout_change(state);
}

pub fn handle_rename_workspace(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    let current_name = state
        .session
        .active_workspace()
        .and_then(|ws| ws.name.clone())
        .unwrap_or_default();
    state.input_mode = InputMode::Rename {
        target: RenameTarget::Workspace(ws_idx),
        buffer: current_name,
    };
    state.needs_redraw = true;
}

// ── Sidebar / Chrome ──

/// Map a visible flag to a chrome region mode (visible = Expanded, hidden = Hidden).
fn region_mode(visible: bool) -> heca_grid_ui::widgets::RegionMode {
    if visible {
        heca_grid_ui::widgets::RegionMode::Expanded
    } else {
        heca_grid_ui::widgets::RegionMode::Hidden
    }
}

pub fn handle_sidebar_left(state: &mut AppState, _action: &WmAction) {
    let vis = !state.chrome_state.left_visible();
    state.chrome_state.set_left_mode(region_mode(vis));
    update_session_viewport(state);
    after_layout_change(state);
}

pub fn handle_sidebar_right(state: &mut AppState, _action: &WmAction) {
    let vis = !state.chrome_state.right_visible();
    state.chrome_state.set_right_mode(region_mode(vis));
    update_session_viewport(state);
    after_layout_change(state);
}

pub fn handle_sidebar_focus(state: &mut AppState, _action: &WmAction) {
    // Enter sidebar-nav WITHOUT changing the sidebar's mode or width at all: a
    // contracted/collapsed sidebar stays exactly as it is, an expanded one stays
    // expanded (selection-driven). The look is driven by `left_size` (render.rs
    // uses width < SIDEBAR_EXPANDED_THRESHOLD for the rail), so touching neither
    // mode nor size here is what keeps the contracted sidebar contracted.
    state.input_mode = InputMode::SidebarNav;
    update_session_viewport(state);
    after_layout_change(state);
}

pub fn handle_sidebar_up(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let is_collapsed = !state.chrome_state.left_visible()
            || state.chrome_state.left_size() < crate::chrome::SIDEBAR_EXPANDED_THRESHOLD;
        if is_collapsed {
            state.sidebar_tree.cursor_up_collapsed();
        } else {
            state.sidebar_tree.cursor_up();
        }
        state.needs_redraw = true;
    }
}

pub fn handle_sidebar_down(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let is_collapsed = !state.chrome_state.left_visible()
            || state.chrome_state.left_size() < crate::chrome::SIDEBAR_EXPANDED_THRESHOLD;
        if is_collapsed {
            state.sidebar_tree.cursor_down_collapsed();
        } else {
            state.sidebar_tree.cursor_down();
        }
        state.needs_redraw = true;
    }
}

pub fn handle_sidebar_left_nav(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        state.sidebar_tree.collapse(&state.chrome_state.workspaces);
        state.needs_redraw = true;
    }
}

pub fn handle_sidebar_right_nav(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let item = state.sidebar_tree.current_item().cloned();
        match &item {
            Some(sidebar::SidebarItem::Pane { pane_id }) => {
                focus_pane_by_id(state, *pane_id);
                state.input_mode = InputMode::Normal;
            }
            Some(sidebar::SidebarItem::FloatingPane { pane_id, .. }) => {
                focus_pane_by_id(state, *pane_id);
                state.input_mode = InputMode::Normal;
            }
            _ => {
                state.sidebar_tree.expand(&state.chrome_state.workspaces);
            }
        }
        state.needs_redraw = true;
    }
}

pub fn handle_sidebar_expand_toggle(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let item = state.sidebar_tree.current_item().cloned();
        match &item {
            Some(sidebar::SidebarItem::Pane { .. })
            | Some(sidebar::SidebarItem::FloatingPane { .. }) => {}
            _ => {
                state
                    .sidebar_tree
                    .toggle_expand(&state.chrome_state.workspaces);
            }
        }
        state.needs_redraw = true;
    }
}

fn sidebar_selected_workspace_idx(state: &AppState) -> Option<usize> {
    let item = state.sidebar_tree.current_item()?;
    match item {
        sidebar::SidebarItem::Workspace { ws_idx }
        | sidebar::SidebarItem::Column { ws_idx, .. }
        | sidebar::SidebarItem::FloatingPane { ws_idx, .. } => Some(*ws_idx),
        sidebar::SidebarItem::Pane { pane_id } => {
            find_pane_location(&state.session, *pane_id).map(|(ws_idx, _, _)| ws_idx)
        }
    }
}

fn sidebar_selected_column_target(state: &AppState) -> Option<(usize, usize)> {
    let item = state.sidebar_tree.current_item()?;
    match item {
        sidebar::SidebarItem::Column { ws_idx, col_idx } => Some((*ws_idx, *col_idx)),
        sidebar::SidebarItem::Pane { pane_id } => find_pane_location(&state.session, *pane_id)
            .map(|(ws_idx, col_idx, _)| (ws_idx, col_idx)),
        sidebar::SidebarItem::Workspace { .. } | sidebar::SidebarItem::FloatingPane { .. } => None,
    }
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

fn sidebar_delete_prompt(state: &AppState) -> Option<(String, WmAction)> {
    let item = state.sidebar_tree.current_item()?.clone();
    match item {
        sidebar::SidebarItem::Workspace { ws_idx } => {
            let ws_label = if let Some(ws) = state.session.workspaces.get(ws_idx)
                && let Some(ref name) = ws.name
            {
                name.clone()
            } else {
                format!("workspace {}", ws_idx + 1)
            };
            Some((
                format!("Delete {}? (y/n)", ws_label),
                WmAction::DeleteWorkspace { ws_idx },
            ))
        }
        sidebar::SidebarItem::Column { ws_idx, col_idx } => {
            let ws_label = if let Some(ws) = state.session.workspaces.get(ws_idx)
                && let Some(ref name) = ws.name
            {
                name.clone()
            } else {
                format!("ws {}", ws_idx + 1)
            };
            Some((
                format!("Delete column {} from {}? (y/n)", col_idx + 1, ws_label),
                WmAction::DeleteColumn { ws_idx, col_idx },
            ))
        }
        sidebar::SidebarItem::Pane { pane_id }
        | sidebar::SidebarItem::FloatingPane { pane_id, .. } => {
            let pane_label = state
                .session
                .workspaces
                .iter()
                .find_map(|ws| ws.find_pane(pane_id))
                .map(|pane| pane.title.clone())
                .unwrap_or_else(|| format!("pane {}", pane_id));
            Some((
                format!("Delete {}? (y/n)", pane_label),
                WmAction::ClosePaneById { pane_id },
            ))
        }
    }
}

pub fn handle_sidebar_create_workspace(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::SidebarNav) {
        return;
    }
    if matches!(
        state.sidebar_tree.current_item(),
        Some(sidebar::SidebarItem::FloatingPane { .. })
    ) {
        return;
    }
    handle_create_workspace(state, &WmAction::CreateWorkspace);
    state.input_mode = InputMode::SidebarNav;
}

pub fn handle_sidebar_create_column(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::SidebarNav) {
        return;
    }
    if matches!(
        state.sidebar_tree.current_item(),
        Some(sidebar::SidebarItem::FloatingPane { .. })
    ) {
        return;
    }
    if let Some(target_ws) = sidebar_selected_workspace_idx(state) {
        if target_ws != state.session.active_workspace_idx {
            switch_workspace_tracked(state, target_ws);
        }
        handle_split_horizontal(state, &WmAction::SplitHorizontal);
        state.input_mode = InputMode::SidebarNav;
    }
}

pub fn handle_sidebar_split_in_column(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::SidebarNav) {
        return;
    }
    if let Some((ws_idx, col_idx)) = sidebar_selected_column_target(state) {
        handle_add_pane_to_column(state, &WmAction::AddPaneToColumn { ws_idx, col_idx });
        state.input_mode = InputMode::SidebarNav;
    }
}

pub fn handle_sidebar_zoom_selected_column(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::SidebarNav) {
        return;
    }
    if let Some((ws_idx, col_idx)) = sidebar_selected_column_target(state) {
        if ws_idx != state.session.active_workspace_idx {
            switch_workspace_tracked(state, ws_idx);
        }
        if let Some(ws) = state.session.active_workspace_mut()
            && col_idx < ws.scrolling.columns.len()
        {
            ws.scrolling.activate_column(col_idx);
        }
        handle_zoom_column(state, &WmAction::ZoomColumn);
        state.input_mode = InputMode::SidebarNav;
    }
}

pub fn handle_sidebar_delete_selected(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::SidebarNav) {
        return;
    }
    if let Some((message, action)) = sidebar_delete_prompt(state) {
        state.input_mode = InputMode::ConfirmDelete {
            message,
            action: Box::new(action),
            resume_sidebar: true,
        };
        state.needs_redraw = true;
    }
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
    state.sidebar_tree.apply_ws_collapsed(&set, Some(ws_idx));
}

pub fn handle_collapse_current_workspace(state: &mut AppState, _action: &WmAction) {
    let Some(ws_idx) = current_active_workspace_idx(state) else {
        return;
    };
    apply_ws_collapse(state, ws_idx, Some(true));
    state.needs_redraw = true;
}

pub fn handle_expand_current_workspace(state: &mut AppState, _action: &WmAction) {
    let Some(ws_idx) = current_active_workspace_idx(state) else {
        return;
    };
    apply_ws_collapse(state, ws_idx, Some(false));
    state.needs_redraw = true;
}

pub fn handle_toggle_current_workspace_collapsed(state: &mut AppState, _action: &WmAction) {
    let Some(ws_idx) = current_active_workspace_idx(state) else {
        return;
    };
    apply_ws_collapse(state, ws_idx, None);
    state.needs_redraw = true;
}

pub fn handle_collapse_current_column(state: &mut AppState, _action: &WmAction) {
    let Some((ws_idx, col_idx)) = current_tiled_column_target(state) else {
        return;
    };
    state.sidebar_tree.collapse_column(ws_idx, col_idx);
    state.needs_redraw = true;
}

pub fn handle_expand_current_column(state: &mut AppState, _action: &WmAction) {
    let Some((ws_idx, col_idx)) = current_tiled_column_target(state) else {
        return;
    };
    state.sidebar_tree.expand_column(ws_idx, col_idx);
    state.needs_redraw = true;
}

pub fn handle_toggle_current_column_collapsed(state: &mut AppState, _action: &WmAction) {
    let Some((ws_idx, col_idx)) = current_tiled_column_target(state) else {
        return;
    };
    state.sidebar_tree.toggle_column_collapsed(ws_idx, col_idx);
    state.needs_redraw = true;
}

// ── System ──

pub fn handle_command_palette(state: &mut AppState, _action: &WmAction) {
    state.needs_redraw = true;
}

// ── External commands ──

pub fn handle_spawn_command(state: &mut AppState, action: &WmAction) {
    let WmAction::SpawnCommand {
        command,
        kind,
        float,
        close_policy,
    } = action
    else {
        return;
    };
    match kind {
        SpawnKind::Terminal => {}
        SpawnKind::App | SpawnKind::Plugin => {
            eprintln!("[heca] spawn kind '{kind:?}' not yet implemented");
            return;
        }
    }

    let active_ws = state.session.active_workspace_idx;
    let (cols, rows) = terminal_grid_for_workspace(state, active_ws);
    let next_id = state.session.next_id();
    let mut pane = LayoutPane::new(PaneId(next_id), command.clone());
    pane.close_policy = *close_policy;

    if *float {
        if let Some(ws) = state.session.active_workspace_mut() {
            let wa = ws.scrolling.working_area;
            let fw = wa.size.w * 0.95;
            let fh = wa.size.h * 0.95;
            let fx = wa.loc.x + (wa.size.w - fw) / 2.0;
            let fy = wa.loc.y + (wa.size.h - fh) / 2.0;
            ws.deactivate_floating_panes();
            ws.floating_panes
                .push(heca_core::layout::workspace::FloatingPane {
                    pane,
                    position: heca_core::layout::types::Point::new(fx, fy),
                    size: heca_core::layout::types::Size::new(fw, fh),
                    is_active: true,
                    original_column_idx: None,
                    original_pane_idx: None,
                });
            ws.focus_domain = FocusDomain::Floating;
        }
    } else {
        state.session.add_pane(pane, None, true);
    }
    state.backends.insert_for_pane(
        PaneId(next_id),
        create_command_backend_for_state(state, cols, rows, command),
    );
    after_layout_change(state);
}

// ── Mode ──

pub fn handle_enter_mode(state: &mut AppState, action: &WmAction) {
    let WmAction::EnterMode { name } = action else {
        return;
    };
    state.input_mode = InputMode::Mode { name: name.clone() };
    state.needs_redraw = true;
}

// ── Selection (host capability) ──

/// Enter the host-owned selection input mode.
///
/// For terminal panes, this action starts or resumes a host-grid selection at
/// the focused terminal cursor so keyboard selection works immediately.
/// For panes that do not yet expose a selection adapter, it still falls back
/// to a pure input-mode transition.
///
/// The action only transitions the input mode so that:
/// - status bar shows `SELECTION`
/// - `Esc` / `Enter` semantics become "clear / confirm selection"
/// - other keys are not forwarded to the focused backend
///
/// Pre-existing selections are allowed: a user who confirmed a selection
/// with `Enter` (leaving it in the `Selected` phase) can re-enter selection
/// mode to reposition it. Mode-internal keyboard behavior (`Esc` clears,
/// `Enter` confirms, `prefix` returns to Prefix, movement keys update the
/// focus cell) is owned by
/// `app::input::handle_selection_mode`.
pub fn handle_enter_selection_mode(state: &mut AppState, _action: &WmAction) {
    // If we can't place a caret on the focused terminal, do nothing.
    // Entering selection mode without a caret violates the contract:
    // the user must have a caret position to move and begin selection from.
    let _ = enter_selection_mode_for_focused_terminal(state);
}

pub fn handle_selection_left(state: &mut AppState, _action: &WmAction) {
    let _ = move_focused_terminal_selection(state, 0, -1);
}

pub fn handle_selection_right(state: &mut AppState, _action: &WmAction) {
    let _ = move_focused_terminal_selection(state, 0, 1);
}

pub fn handle_selection_up(state: &mut AppState, _action: &WmAction) {
    let _ = move_focused_terminal_selection(state, -1, 0);
}

pub fn handle_selection_down(state: &mut AppState, _action: &WmAction) {
    let _ = move_focused_terminal_selection(state, 1, 0);
}

/// Begin selection from the caret position.
///
/// When in caret-only state (selection mode entered but no selection started),
/// this starts a selection with anchor and focus both at the caret position.
/// Movement keys will then grow the selection from that point.
///
/// If a selection already exists, this is a no-op — the user should clear
/// and re-enter if they want to restart selection from a different point.
/// This avoids accidental loss of an in-progress selection.
pub fn handle_begin_selection(state: &mut AppState, _action: &WmAction) {
    if state.selection.is_caret() {
        state
            .selection
            .begin_selection_from_caret(SelectionSource::KeyboardMode);
        state.needs_redraw = true;
    }
    // If selection already exists: no-op. Document the policy — user must
    // clear (Esc) and re-enter selection mode to restart from caret.
}

/// Toggle which endpoint of the selection is active (anchor vs focus).
///
/// After toggling, movement keys update the other end of the selection.
/// This lets the keyboard user grow the selection from both ends without
/// restarting.
pub fn handle_toggle_selection_endpoint(state: &mut AppState, _action: &WmAction) {
    state.selection.toggle_selection_endpoint();
    state.needs_redraw = true;
}

/// Clear the active selection and exit selection input mode.
pub fn handle_clear_selection(state: &mut AppState, _action: &WmAction) {
    state.selection.clear();
    if matches!(state.input_mode, InputMode::Selection) {
        state.input_mode = InputMode::Normal;
    }
    state.needs_redraw = true;
}

/// Write text to the system clipboard via `arboard`; shared by selection-copy
/// and the `OSC 52` clipboard-write path. Failures are logged in debug builds.
pub(crate) fn set_system_clipboard(text: &str) {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => {
            if let Err(e) = clipboard.set_text(text) {
                #[cfg(debug_assertions)]
                eprintln!("[heca] clipboard write failed: {e}");
                let _ = e; // Suppress unused warning in release.
            }
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("[heca] clipboard unavailable: {e}");
            let _ = e;
        }
    }
}

/// Copy the active host-grid selection text to the system clipboard.
///
/// Routes through the action registry so it is reachable from keyboard,
/// mouse/UI, and RPC. The extraction uses the shared `SelectionState` and
/// the terminal backend snapshot — no terminal-only selection state.
///
/// # Clipboard contract
///
/// - Uses the system clipboard via `arboard`.
/// - If clipboard write fails, logs in debug builds and keeps app state coherent.
/// - Does NOT clear the selection after copy — the user clears explicitly.
/// - Safe no-op when there is no active selection or the owner is unsupported.
pub fn handle_copy_selection(state: &mut AppState, _action: &WmAction) {
    let text = {
        let active = match state.selection.active() {
            Some(a) => a,
            None => return, // No active selection — safe no-op.
        };
        let SelectionOwner::Pane(pane_id) = active.owner;
        let (start_stable, end_stable) = match &active.region {
            SelectionRegion::HostGrid {
                anchor_stable_row,
                focus_stable_row,
                ..
            } => (
                (*anchor_stable_row).min(*focus_stable_row),
                (*anchor_stable_row).max(*focus_stable_row),
            ),
            SelectionRegion::BackendNative => return, // Unsupported — safe no-op.
        };

        // Get the terminal snapshot for the owning pane.
        let snapshot = match state
            .backends
            .get(pane_id)
            .and_then(|b| b.terminal_snapshot())
        {
            Some(s) => s,
            None => return, // No snapshot — safe no-op.
        };

        // Extract text using the shared extraction logic.
        //
        // Fetch the selection rows by stable-row range so history rows are
        // copyable even when they are no longer in the visible viewport.
        let lines = state
            .backends
            .get(pane_id)
            .map(|b| b.lines_in_stable_range(start_stable, end_stable, snapshot.cols))
            .unwrap_or_default();

        match crate::app::selection_model::extract_selection_text(
            &state.selection,
            &lines,
            start_stable,
            snapshot.cols,
            snapshot.default_bg,
        ) {
            Some(t) => t,
            None => return, // Extraction returned nothing — safe no-op.
        }
    };

    if text.is_empty() {
        return;
    }

    // Write to system clipboard.
    set_system_clipboard(&text);

    // Clear the active selection but stay in selection mode with the caret
    // at the last focus position. This way the user can immediately navigate
    // or start a new selection without the Q5 snap-to-bottom triggering.
    if let Some(active) = state.selection.active()
        && let SelectionRegion::HostGrid {
            focus_stable_row, focus_col, ..
        } = &active.region
    {
        let owner = active.owner;
        state.selection.set_caret(owner, *focus_stable_row, *focus_col);
    } else {
        state.selection.clear();
    }
    state.needs_redraw = true;
}

/// Paste the system clipboard into the focused pane.
///
/// Reads the OS clipboard via `arboard` and forwards it to the focused backend.
/// The backend wraps the text in bracketed-paste markers when the program
/// enabled that mode (see `PaneBackend::paste`), so editors treat it as literal
/// input.
pub fn handle_paste_clipboard(state: &mut AppState, _action: &WmAction) {
    // Read the system clipboard and forward text into the focused pane.
    let text = match arboard::Clipboard::new() {
        Ok(mut cb) => match cb.get_text() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[heca] clipboard read failed: {e}");
                return;
            }
        },
        Err(e) => {
            eprintln!("[heca] clipboard unavailable: {e}");
            return;
        }
    };

    // Find the active pane.
    let pane_id = match state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|pane| pane.id)
        .or(state.focused_pane)
    {
        Some(id) => id,
        None => return,
    };

    // Forward the text to the pane's backend (bracketed-paste aware).
    if let Some(backend) = state.backends.get_mut(pane_id) {
        backend.paste(&text);
    }

    state.needs_redraw = true;
}

// ── Scrollback (host terminal viewport) ──

/// Get the approximate viewport page size for the focused terminal pane, in rows.
/// Falls back to a sensible default (24) when no snapshot is available.
fn focused_terminal_page_rows(state: &AppState) -> usize {
    state.focused_pane.and_then(|pane_id| {
        state
            .backends
            .get(pane_id)
            .and_then(|b| b.terminal_snapshot())
            .map(|s| s.rows)
    }).unwrap_or(24)
}

pub fn handle_scrollback_page_up(state: &mut AppState, _action: &WmAction) {
    let page_rows = focused_terminal_page_rows(state) as isize;
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    let _ = move_focused_terminal_selection(state, -page_rows, 0);
    state.needs_redraw = true;
}

pub fn handle_scrollback_page_down(state: &mut AppState, _action: &WmAction) {
    let page_rows = focused_terminal_page_rows(state) as isize;
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    let _ = move_focused_terminal_selection(state, page_rows, 0);
    state.needs_redraw = true;
}

pub fn handle_scrollback_line_up(state: &mut AppState, action: &WmAction) {
    let WmAction::ScrollbackLineUp { amount } = action else {
        return;
    };
    // `amount` is in notches; multiply by the user-configurable lines-per-notch.
    let lines = (amount * state.terminal_wheel_scroll_lines) as isize;
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    let _ = move_focused_terminal_selection(state, -lines, 0);
    state.needs_redraw = true;
}

pub fn handle_scrollback_line_down(state: &mut AppState, action: &WmAction) {
    let WmAction::ScrollbackLineDown { amount } = action else {
        return;
    };
    // `amount` is in notches; multiply by the user-configurable lines-per-notch.
    let lines = (amount * state.terminal_wheel_scroll_lines) as isize;
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    let _ = move_focused_terminal_selection(state, lines, 0);
    state.needs_redraw = true;
}

pub fn handle_scrollback_to_top(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    // Move caret to the oldest scrollback content row.
    if let Some(pane_id) = state.focused_pane
        && let Some(ref snapshot) = state
            .backends
            .get(pane_id)
            .and_then(|b| b.terminal_snapshot())
    {
        let base = snapshot.viewport_top_stable_row
            + snapshot.viewport_offset as isize
            + snapshot.rows as isize;
        let oldest = base - snapshot.scrollback_rows as isize;
        if state.selection.is_caret() {
            state.selection.move_caret(oldest, 0);
        } else {
            state.selection.update_focus(oldest, 0);
        }
        ensure_caret_visible(state, pane_id, oldest, snapshot);
    }
    state.needs_redraw = true;
}

pub fn handle_scrollback_to_bottom(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    // Scroll to the live bottom immediately, then move the caret to the
    // cursor position (newest terminal text, not the adjusted cursor row).
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_to_bottom();
    }
    if let Some(pane_id) = state.focused_pane
        && let Some(ref snapshot) = state
            .backends
            .get(pane_id)
            .and_then(|b| b.terminal_snapshot())
    {
        let cursor_stable =
            snapshot.viewport_top_stable_row + snapshot.cursor.row as isize;
        if state.selection.is_caret() {
            state.selection.move_caret(cursor_stable, snapshot.cursor.col);
        } else {
            state.selection.update_focus(cursor_stable, snapshot.cursor.col);
        }
    }
    state.needs_redraw = true;
}

pub fn handle_exit_scrollback(state: &mut AppState, _action: &WmAction) {
    // Scroll to live bottom, clear selection, and exit selection mode.
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_to_bottom();
    }
    state.selection.clear();
    // Leaving the copy-mode session also ends any scrollback search.
    state.search = None;
    if matches!(state.input_mode, InputMode::Selection) {
        state.input_mode = InputMode::Normal;
    }
    state.needs_redraw = true;
}

// ── Direct scroll (no selection mode / caret) ──

pub fn handle_scroll_line_up(state: &mut AppState, _action: &WmAction) {
    let lines = state.terminal_wheel_scroll_lines as i32;
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_viewport(lines);
    }
    state.needs_redraw = true;
}

pub fn handle_scroll_line_down(state: &mut AppState, _action: &WmAction) {
    let lines = -(state.terminal_wheel_scroll_lines as i32);
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_viewport(lines);
    }
    state.needs_redraw = true;
}

pub fn handle_scroll_page_up(state: &mut AppState, _action: &WmAction) {
    let page_rows = focused_terminal_page_rows(state) as i32;
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        // Direct page jumps are the primary user-visible discrete scrollback
        // jump path in Normal mode, so they should honor
        // `terminal_scroll_animations`. Caret-follow / selection-mode paths stay
        // immediate elsewhere to avoid lagging the caret behind the content.
        backend.scroll_viewport_animated(page_rows);
    }
    state.needs_redraw = true;
}

pub fn handle_scroll_page_down(state: &mut AppState, _action: &WmAction) {
    let page_rows = -(focused_terminal_page_rows(state) as i32);
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_viewport_animated(page_rows);
    }
    state.needs_redraw = true;
}

pub fn handle_scroll_to_top(state: &mut AppState, _action: &WmAction) {
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_to_top_animated();
    }
    state.needs_redraw = true;
}

pub fn handle_scroll_to_bottom(state: &mut AppState, _action: &WmAction) {
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_to_bottom_animated();
    }
    state.needs_redraw = true;
}

pub fn handle_scroll_to_offset(state: &mut AppState, action: &WmAction) {
    let WmAction::ScrollToOffset { rows } = action else {
        return;
    };
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
        && let Some(snapshot) = backend.terminal_snapshot()
    {
        let max_offset = snapshot.scrollback_rows.saturating_sub(snapshot.rows);
        let target = (*rows).min(max_offset);
        let delta = target as i32 - snapshot.viewport_offset as i32;
        if delta != 0 {
            backend.scroll_viewport(delta);
        }
    }
    state.needs_redraw = true;
}

// ── Config ──

pub fn handle_reload_config(state: &mut AppState, _action: &WmAction) {
    state.pending_reload = true;
}

/// Resolve a [`FontZoomStep`] into a signed point delta using the configured step
/// size (`[settings] terminal_font_zoom_step`). `Reset` maps to `0.0`, which both
/// zoom helpers treat as "clear the offset". A non-positive configured step falls
/// back to the built-in default so zoom never becomes a no-op.
fn font_zoom_delta(state: &AppState, step: FontZoomStep) -> f32 {
    use crate::app::terminal_metrics::TERMINAL_FONT_ZOOM_STEP;
    let size = if state.terminal_font_zoom_step > 0.0 {
        state.terminal_font_zoom_step
    } else {
        TERMINAL_FONT_ZOOM_STEP
    };
    match step {
        FontZoomStep::In => size,
        FontZoomStep::Out => -size,
        FontZoomStep::Reset => 0.0,
    }
}

/// App-wide terminal font zoom (the `app-03` base) — steps every pane's size.
pub fn handle_app_font_zoom(state: &mut AppState, action: &WmAction) {
    let WmAction::AppFontZoom { step } = action else {
        return;
    };
    let delta = font_zoom_delta(state, *step);
    crate::app::terminal_metrics::apply_app_font_zoom(state, delta);
}

/// Per-pane terminal font zoom. `pane_id = None` targets the focused pane
/// (keyboard); `Some(id)` targets a specific pane (`Ctrl`/`Meta`+wheel / RPC).
pub fn handle_pane_terminal_font_zoom(state: &mut AppState, action: &WmAction) {
    let WmAction::PaneTerminalFontZoom { pane_id, step } = action else {
        return;
    };
    let Some(target) = pane_id.or(state.focused_pane) else {
        return;
    };
    let delta = font_zoom_delta(state, *step);
    crate::app::terminal_metrics::apply_pane_terminal_font_zoom(state, target, delta);
}

/// Schemes we are willing to hand to the OS opener. OSC 8 links come from
/// terminal output, so we refuse anything that isn't a plain web/file/mail
/// resource (e.g. no `javascript:` / `data:`).
fn link_scheme_allowed(url: &str) -> bool {
    let scheme = url.trim().split_once(':').map(|(s, _)| s.to_ascii_lowercase());
    matches!(
        scheme.as_deref(),
        Some("http" | "https" | "mailto" | "file" | "ftp" | "ftps")
    )
}

/// Open `url` in the OS default handler, detached. Best-effort: a missing opener
/// or a spawn failure is logged (debug) and otherwise ignored.
fn open_url_in_os(url: &str) {
    use std::process::{Command, Stdio};
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = Command::new("cmd");
        // Empty title arg so a quoted URL isn't treated as the window title.
        c.args(["/C", "start", "", url]);
        c
    };
    let result = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if let Err(_e) = result {
        #[cfg(debug_assertions)]
        eprintln!("[heca] failed to open link {url}: {_e}");
    }
}

/// Open an OSC 8 hyperlink target. Shared by every surface (HintKey follow-link,
/// Cmd/Ctrl+click, selection-mode `O`, context menu, RPC).
pub fn handle_open_link(_state: &mut AppState, action: &WmAction) {
    let WmAction::OpenLink { url } = action else {
        return;
    };
    if link_scheme_allowed(url) {
        open_url_in_os(url);
    } else {
        #[cfg(debug_assertions)]
        eprintln!("[heca] refusing to open link with disallowed scheme: {url}");
    }
}

/// Selection-mode `O`: open the hyperlink under the selection caret. Resolves the
/// caret cell (caret-only or the moving focus endpoint of an active selection) →
/// the owning pane's snapshot hyperlink → the shared open path. Safe no-op when
/// there is no caret, no owning pane, or no link under it.
pub fn handle_search_scrollback(state: &mut AppState, _action: &WmAction) {
    crate::app::terminal_host::enter_scrollback_search(state);
}

pub fn handle_search_next_match(state: &mut AppState, _action: &WmAction) {
    crate::app::terminal_host::search_step(state, true);
}

pub fn handle_search_prev_match(state: &mut AppState, _action: &WmAction) {
    crate::app::terminal_host::search_step(state, false);
}

pub fn handle_open_link_at_caret(state: &mut AppState, _action: &WmAction) {
    let Some((owner, stable_row, col)) = state.selection.cursor_cell() else {
        return;
    };
    let SelectionOwner::Pane(pane_id) = owner;
    let Some(url) =
        crate::app::terminal_host::hyperlink_uri_at_stable_cell(state, pane_id, stable_row, col)
    else {
        return;
    };
    handle_open_link(state, &WmAction::OpenLink { url });
}

// ── Chrome container placement (plugin-02, §2.9) ──────────────────────────────
// Host-level container moves against `AppState.chrome_host`. plugin-02 has no
// render path consuming the host yet, so these change placement + emit the
// `ContainerPlacementChanged` event but produce no visible effect until plugin-03.

/// Move a mounted container to another chrome region (validated against the
/// container's `supported_regions`).
pub fn handle_move_container_to_region(state: &mut AppState, action: &WmAction) {
    let WmAction::MoveContainerToRegion {
        container_id,
        region,
    } = action
    else {
        return;
    };
    if let Err(e) = state.chrome_host.move_container(container_id, *region) {
        eprintln!("[heca] move container '{container_id}' failed: {e}");
    }
}

/// Reorder a mounted container within its region, before `before_id` (or to the
/// end when `None`).
pub fn handle_reorder_container_before(state: &mut AppState, action: &WmAction) {
    let WmAction::ReorderContainerBefore {
        container_id,
        before_id,
    } = action
    else {
        return;
    };
    if let Err(e) = state
        .chrome_host
        .reorder(container_id, before_id.as_deref())
    {
        eprintln!("[heca] reorder container '{container_id}' failed: {e}");
    }
}

/// Set a chrome region's host-level visibility.
pub fn handle_set_region_visible(state: &mut AppState, action: &WmAction) {
    let WmAction::SetRegionVisible { region, visible } = action else {
        return;
    };
    state.chrome_host.set_region_visible(*region, *visible);
}

#[cfg(test)]
mod open_link_tests {
    use super::link_scheme_allowed;

    #[test]
    fn allows_web_file_mail_schemes() {
        for url in [
            "https://example.com",
            "http://example.com/path",
            "file:///Users/me/x.txt",
            "mailto:a@b.com",
            "ftp://host/f",
            "FTPS://host/f",
        ] {
            assert!(link_scheme_allowed(url), "should allow {url}");
        }
    }

    #[test]
    fn rejects_dangerous_or_schemeless() {
        for url in [
            "javascript:alert(1)",
            "data:text/html,<script>",
            "vbscript:x",
            "example.com",
            "",
        ] {
            assert!(!link_scheme_allowed(url), "should reject {url}");
        }
    }
}
