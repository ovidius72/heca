//! Named action handlers — one per `WmAction` variant.
//!
//! Each handler is a plain `fn(&mut AppState, &WmAction)` that performs the
//! action.  Parameterized variants destructure their fields from the enum;
//! unit variants ignore the `_action` parameter.

use crate::app::mutations::{after_focus_change, after_layout_change, after_metadata_change};
use crate::app::pane_ops::{swap_panes_cross_workspace, swap_panes_diff_columns, swap_panes_same_column};
use crate::app::focus::{focus_pane_by_id, sync_focus};
use crate::app_state::{AppState, InputMode, RenameTarget};
use crate::input::WmAction;
use crate::sidebar;
use crate::{
    collect_all_pane_candidates, destroy_empty_workspace, find_pane_location,
    move_pane_to_column, move_pane_to_workspace_column, pane_name, switch_workspace_tracked,
    update_session_viewport,
};
use heca_core::backend::FakeBackend;
use crate::chrome;
use heca_core::layout::{Column, ColumnId, ColumnWidth, FocusDomain, Pane as LayoutPane, PaneId};
use crate::app::interaction::{pane_is_floating, focused_pane_id};

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
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
    let backend_id = next_id;
    state.session.add_pane(pane, None, true);
    state
        .backends
        .insert_for_pane(backend_id, Box::new(FakeBackend::new(80, 24)));
    after_layout_change(state);
}

pub fn handle_split_vertical(state: &mut AppState, _action: &WmAction) {
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
    let backend_id = next_id;
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.add_pane_to_column(col_idx, None, pane, true);
    }
    state
        .backends
        .insert_for_pane(backend_id, Box::new(FakeBackend::new(80, 24)));
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

pub fn handle_move_pane_left(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_active_pane_left();
    }
    after_layout_change(state);
}

pub fn handle_move_pane_right(state: &mut AppState, _action: &WmAction) {
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
            let placeholder_a_id = state.session.next_id();
            let placeholder_b_id = state.session.next_id();
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
        move_pane_to_workspace_column(state, *pane_id, *ws_idx, current_col);
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
            move_pane_to_workspace_column(state, *pane_id, *ws_idx, *col_idx);
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
            if let Some(idx) = ws
                .floating_panes
                .iter()
                .position(|f| f.pane.id.0 == pane_id)
            {
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
                                ColumnId(pane_id),
                                float.pane,
                                chrome::default_column_width(),
                            ),
                            true,
                        );
                    }
                } else {
                    ws.scrolling.add_column(
                        None,
                        Column::new(ColumnId(pane_id), float.pane, chrome::default_column_width()),
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

pub fn handle_close_pane(state: &mut AppState, _action: &WmAction) {
    let pane_id = match focused_pane_id(state) {
        Some(id) => id,
        None => return,
    };
    let is_flt = pane_is_floating(&state.session, pane_id);

    if is_flt {
        // Closing a floating pane: remove it, switch domain to Tiled
        // if no floating panes remain, and focus the last visited tiled pane.
        if let Some(ws) = state.session.active_workspace_mut() {
            if let Some(float_idx) = ws.floating_panes.iter().position(|f| f.pane.id.0 == pane_id) {
                let removed = ws.floating_panes.remove(float_idx);
                state.backends.remove_for_pane(removed.pane.id.0);
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

        // Check if workspace is now empty (no tiled or floating panes).
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
        } else if ws_is_empty {
            let next_id = state.session.next_id();
            let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
            state.session.add_pane(pane, None, true);
            state
                .backends
                .insert_for_pane(next_id, Box::new(FakeBackend::new(80, 24)));
        }
    } else {
        // Closing a tiled pane: existing behavior.
        let current_ws = state.session.active_workspace_idx;
        if let Some(ws) = state.session.active_workspace_mut() {
            let col_idx = ws.scrolling.active_column_idx;
            if let Some(col) = ws.scrolling.active_column() {
                let pane_idx = col.active_pane_idx;
                if let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx) {
                    state.backends.remove_for_pane(removed.id.0);
                }
            }
        }

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
        } else if ws_is_empty {
            let next_id = state.session.next_id();
            let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
            state.session.add_pane(pane, None, true);
            state
                .backends
                .insert_for_pane(next_id, Box::new(FakeBackend::new(80, 24)));
        }
    }
    after_layout_change(state);
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

pub fn handle_rename_pane(state: &mut AppState, _action: &WmAction) {
    if let Some(pane_id) = focused_pane_id(state) {
        let current_title = state
            .session
            .active_workspace()
            .and_then(|ws| ws.find_pane(heca_core::layout::PaneId(pane_id)))
            .map(|p| p.title.clone())
            .unwrap_or_default();
        state.input_mode = InputMode::Rename {
            target: RenameTarget::Pane(pane_id),
            buffer: current_title,
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

pub fn handle_close_pane_by_id(state: &mut AppState, action: &WmAction) {
    let WmAction::ClosePaneById { pane_id } = action else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        if let Some((ci, pi)) =
            crate::app::pane_ops::find_pane_indices_in_workspace(ws, *pane_id)
        {
            if let Some(removed) = ws.scrolling.remove_pane(ci, pi) {
                state.backends.remove_for_pane(removed.id.0);
            }
        } else if let Some(float_idx) = ws.floating_panes.iter().position(|f| f.pane.id.0 == *pane_id)
        {
            let removed = ws.floating_panes.remove(float_idx);
            state.backends.remove_for_pane(removed.pane.id.0);
            if ws.focus_domain == FocusDomain::Floating && ws.floating_panes.is_empty() {
                ws.focus_domain = FocusDomain::Tiled;
            }
        }
    }
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
    } else if ws_is_empty {
        let next_id = state.session.next_id();
        let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
        state.session.add_pane(pane, None, true);
        state
            .backends
            .insert_for_pane(next_id, Box::new(FakeBackend::new(80, 24)));
    }
    after_layout_change(state);
}

pub fn handle_rename_target(state: &mut AppState, action: &WmAction) {
    let WmAction::RenameTarget { pane_id, name } = action else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut()
        && let Some(pane) = ws.find_pane_mut(heca_core::layout::PaneId(*pane_id))
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
    let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
    let backend_id = next_id;
    let col = *col_idx;
    if let Some(ws) = state.session.active_workspace_mut() {
        let capped_col = col.min(ws.scrolling.columns.len().saturating_sub(1));
        ws.scrolling
            .add_pane_to_column(capped_col, None, pane, true);
    }
    state
        .backends
        .insert_for_pane(backend_id, Box::new(FakeBackend::new(80, 24)));
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
    let pane_ids: Vec<u64> = state
        .session
        .workspaces
        .get(target_ws)
        .and_then(|ws| ws.scrolling.columns.get(*col_idx))
        .map(|col| col.panes.iter().map(|p| p.id.0).collect())
        .unwrap_or_default();

    state.backends.remove_all(&pane_ids);

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
    let pane_ids: Vec<u64> = state
        .session
        .workspaces
        .get(target_ws)
        .map(|ws| {
            let mut ids: Vec<u64> = ws
                .scrolling
                .columns
                .iter()
                .flat_map(|col| col.panes.iter().map(|p| p.id.0))
                .collect();
            ids.extend(ws.floating_panes.iter().map(|float| float.pane.id.0));
            ids
        })
        .unwrap_or_default();

    state.backends.remove_all(&pane_ids);

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

    // 1. If already at the bottom of the active column → no-op.
    if let Some(ws) = state.session.active_workspace() {
        let active_col = ws.scrolling.active_column_idx;
        if active_col < ws.scrolling.columns.len()
            && ws.scrolling.columns[active_col]
                .panes
                .last()
                .map(|p| p.id.0)
                == Some(target)
        {
            return;
        }
    }

    let active_ws_idx = state.session.active_workspace_idx;

    // 2. Try to find and remove from scrolling columns.
    let removed =
        crate::find_pane_location(&state.session, target).and_then(|(src_ws, src_col, src_idx)| {
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
        });

    let (src_ws, pane) = match removed {
        Some(r) => r,
        None => {
            // 3. Not in scrolling → try floating panes.
            let mut found: Option<(usize, heca_core::layout::column::Pane)> = None;
            for (ws_idx, ws) in state.session.workspaces.iter_mut().enumerate() {
                if let Some(pos) = ws.floating_panes.iter().position(|f| f.pane.id.0 == target) {
                    let fp = ws.floating_panes.remove(pos);
                    ws.deactivate_floating_panes();
                    ws.focus_domain = FocusDomain::Tiled;
                    found = Some((ws_idx, fp.pane));
                    break;
                }
            }
            match found {
                Some(r) => r,
                None => return,
            }
        }
    };

    // 4. Add to active workspace's active column at the bottom.
    let active_col = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    // Need next_id for potential new column; grab before mutable borrow.
    let new_col_id = ColumnId(state.session.next_id());
    if let Some(ws) = state.session.active_workspace_mut() {
        if active_col < ws.scrolling.columns.len() {
            ws.scrolling
                .add_pane_to_column(active_col, None, pane, should_focus);
        } else if ws.scrolling.columns.is_empty() {
            // No columns at all — create one.
            let col = Column::new(new_col_id, pane, ColumnWidth::Proportion(0.85));
            ws.scrolling.add_column(None, col, should_focus);
        } else {
            // Fallback: add to last column.
            let last = ws.scrolling.columns.len() - 1;
            ws.scrolling
                .add_pane_to_column(last, None, pane, should_focus);
        }
    }

    // 5. Clean up empty source workspace if cross-workspace.
    if src_ws != active_ws_idx {
        crate::destroy_empty_workspace(state, src_ws);
    }

    after_layout_change(state);
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
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
    state.session.add_pane(pane, None, true);
    state
        .backends
        .insert_for_pane(next_id, Box::new(FakeBackend::new(80, 24)));
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

pub fn handle_sidebar_left(state: &mut AppState, _action: &WmAction) {
    state.sidebar.left_visible = !state.sidebar.left_visible;
    update_session_viewport(state);
    after_layout_change(state);
}

pub fn handle_sidebar_right(state: &mut AppState, _action: &WmAction) {
    state.sidebar.right_visible = !state.sidebar.right_visible;
    update_session_viewport(state);
    after_layout_change(state);
}

pub fn handle_sidebar_focus(state: &mut AppState, _action: &WmAction) {
    state.sidebar.left_visible = true;
    state.sidebar.left_width = 200.0;
    state.input_mode = InputMode::SidebarNav;
    update_session_viewport(state);
    after_layout_change(state);
}

pub fn handle_sidebar_up(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let is_collapsed = !state.sidebar.left_visible || state.sidebar.left_width < crate::chrome::SIDEBAR_EXPANDED_THRESHOLD;
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
        let is_collapsed = !state.sidebar.left_visible || state.sidebar.left_width < crate::chrome::SIDEBAR_EXPANDED_THRESHOLD;
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
        state.sidebar_tree.collapse();
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
                state.sidebar_tree.expand();
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
                state.sidebar_tree.toggle_expand();
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
        sidebar::SidebarItem::Pane { pane_id } => {
            find_pane_location(&state.session, *pane_id).map(|(ws_idx, col_idx, _)| (ws_idx, col_idx))
        }
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
                .find_map(|ws| ws.find_pane(heca_core::layout::PaneId(pane_id)))
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

pub fn handle_collapse_current_workspace(state: &mut AppState, _action: &WmAction) {
    let Some(ws_idx) = current_active_workspace_idx(state) else {
        return;
    };
    state.sidebar_tree.collapse_workspace(ws_idx);
    state.needs_redraw = true;
}

pub fn handle_expand_current_workspace(state: &mut AppState, _action: &WmAction) {
    let Some(ws_idx) = current_active_workspace_idx(state) else {
        return;
    };
    state.sidebar_tree.expand_workspace(ws_idx);
    state.needs_redraw = true;
}

pub fn handle_toggle_current_workspace_collapsed(state: &mut AppState, _action: &WmAction) {
    let Some(ws_idx) = current_active_workspace_idx(state) else {
        return;
    };
    state.sidebar_tree.toggle_workspace_collapsed(ws_idx);
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
    let WmAction::SpawnCommand { command } = action else {
        return;
    };
    // Create a new pane with the command as its title.
    // In the future this will spawn a real PTY via portable-pty.
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), command.clone());
    state.session.add_pane(pane, None, true);
    state
        .backends
        .insert_for_pane(next_id, Box::new(FakeBackend::new(80, 24)));
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

// ── Config ──

pub fn handle_reload_config(state: &mut AppState, _action: &WmAction) {
    state.pending_reload = true;
}
