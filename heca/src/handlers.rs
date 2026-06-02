//! Named action handlers — one per `WmAction` variant.
//!
//! Each handler is a plain `fn(&mut AppState, &WmAction)` that performs the
//! action.  Parameterized variants destructure their fields from the enum;
//! unit variants ignore the `_action` parameter.

use crate::app_state::{AppState, InputMode, RenameTarget};
use crate::input::WmAction;
use crate::sidebar;
use crate::{
    collect_all_pane_candidates, destroy_empty_workspace,
    find_pane_location, focus_pane_by_id, move_pane_to_column, move_pane_to_workspace_column,
    pane_name, switch_workspace_tracked, sync_focus, update_session_viewport,
};
use heca_core::backend::FakeBackend;
use heca_core::layout::animation::AnimationConfig;
use heca_core::layout::{Column, ColumnId, ColumnWidth, Pane as LayoutPane, PaneId};

// ── Navigation ──

pub fn handle_focus_left(state: &mut AppState, _action: &WmAction) {
    state.session.focus_left();
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_focus_right(state: &mut AppState, _action: &WmAction) {
    state.session.focus_right();
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_focus_up(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.focus_up();
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_focus_down(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.focus_down();
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_next_pane(state: &mut AppState, _action: &WmAction) {
    state.session.focus_right();
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_prev_pane(state: &mut AppState, _action: &WmAction) {
    state.session.focus_left();
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_workspace_next(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    let next = (current_ws + 1).min(state.session.workspaces.len().saturating_sub(1));
    if next != current_ws {
        switch_workspace_tracked(state, next);
        sync_focus(state);
        state.needs_redraw = true;
    }
}

pub fn handle_workspace_prev(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    let prev = current_ws.saturating_sub(1);
    if prev != current_ws {
        switch_workspace_tracked(state, prev);
        sync_focus(state);
        state.needs_redraw = true;
    }
}

pub fn handle_focus_toggle_local(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    if let Some(prev_pane) = state.last_visited_pane_per_ws.get(ws_idx).copied().flatten() {
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
            sync_focus(state);
        }
        state.needs_redraw = true;
    }
}

pub fn handle_focus_pane(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusPane { pane_id } = action else { return };
    focus_pane_by_id(state, *pane_id);
}

pub fn handle_focus_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusWorkspace { ws_idx } = action else { return };
    if *ws_idx < state.session.workspaces.len() {
        switch_workspace_tracked(state, *ws_idx);
        sync_focus(state);
        state.needs_redraw = true;
    }
}

// ── Layout ──

pub fn handle_split_horizontal(state: &mut AppState, _action: &WmAction) {
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
    let backend_id = next_id;
    state.session.add_pane(pane, None, true);
    state.backends.insert(backend_id, Box::new(FakeBackend::new(80, 24)));
    sync_focus(state);
    state.needs_redraw = true;
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
        ws.scrolling
            .add_pane_to_column(col_idx, None, pane, true);
    }
    state.backends.insert(backend_id, Box::new(FakeBackend::new(80, 24)));
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_resize_increase(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_active_column(0.05);
    }
    state.needs_redraw = true;
}

pub fn handle_resize_decrease(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_active_column(-0.05);
    }
    state.needs_redraw = true;
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
    state.needs_redraw = true;
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
    state.needs_redraw = true;
}

pub fn handle_swap_left(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_column_left();
        ws.scrolling.align_view_to_active_column();
    }
    state.needs_redraw = true;
}

pub fn handle_swap_right(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_column_right();
        ws.scrolling.align_view_to_active_column();
    }
    state.needs_redraw = true;
}

pub fn handle_swap_up(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.active_column() {
            let pane_idx = col.active_pane_idx;
            let swap_with = pane_idx.saturating_sub(1);
            if swap_with != pane_idx
                && let Some(col) = ws.scrolling.columns.get_mut(col_idx)
            {
                let h_above = col
                    .pane_sizes
                    .get(pane_idx.min(swap_with))
                    .map(|s| s.h)
                    .unwrap_or(0.0);
                let h_below = col
                    .pane_sizes
                    .get(pane_idx.max(swap_with))
                    .map(|s| s.h)
                    .unwrap_or(0.0);
                let gap = ws.scrolling.options.gaps;
                let up_offset = h_above + gap;
                let down_offset = -(h_below + gap);
                col.panes[pane_idx].animate_move_y_from(up_offset, AnimationConfig::default());
                col.panes[swap_with].animate_move_y_from(down_offset, AnimationConfig::default());
                col.panes.swap(pane_idx, swap_with);
                col.active_pane_idx = swap_with;
                col.compute_pane_sizes(ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
            }
        }
    }
    sync_focus(state);
    if let Some(pane_id) = state.focused_pane {
        crate::focus_pane_by_id(state, pane_id);
    }
    state.needs_redraw = true;
}

pub fn handle_swap_down(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.active_column() {
            let pane_idx = col.active_pane_idx;
            let swap_with = (pane_idx + 1).min(col.panes.len().saturating_sub(1));
            if swap_with != pane_idx
                && let Some(col) = ws.scrolling.columns.get_mut(col_idx)
            {
                let h_above = col
                    .pane_sizes
                    .get(pane_idx.min(swap_with))
                    .map(|s| s.h)
                    .unwrap_or(0.0);
                let h_below = col
                    .pane_sizes
                    .get(pane_idx.max(swap_with))
                    .map(|s| s.h)
                    .unwrap_or(0.0);
                let gap = ws.scrolling.options.gaps;
                let up_offset = h_above + gap;
                let down_offset = -(h_below + gap);
                col.panes[pane_idx].animate_move_y_from(down_offset, AnimationConfig::default());
                col.panes[swap_with].animate_move_y_from(up_offset, AnimationConfig::default());
                col.panes.swap(pane_idx, swap_with);
                col.active_pane_idx = swap_with;
                col.compute_pane_sizes(ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
            }
        }
    }
    sync_focus(state);
    if let Some(pane_id) = state.focused_pane {
        crate::focus_pane_by_id(state, pane_id);
    }
    state.needs_redraw = true;
}

pub fn handle_move_pane_left(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_active_pane_left();
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_move_pane_right(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_active_pane_right();
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_swap_param(state: &mut AppState, action: &WmAction) {
    let WmAction::Swap { a_id, b_id } = action else { return };
    if a_id == b_id { return; }

    // Find both panes' locations.
    let a_loc = find_pane_location(&state.session, *a_id);
    let b_loc = find_pane_location(&state.session, *b_id);
    let ((aws, acol, api), (bws, bcol, bpi)) = match (a_loc, b_loc) {
        (Some(a), Some(b)) => (a, b),
        _ => return,
    };

    // True swap: exchange positions of both panes.
    // Same workspace: remove both (higher index first to avoid shift), then re-insert.
    // Different workspaces: remove A, remove B, insert A at B's pos, insert B at A's pos.
    if aws == bws {
        // Same workspace.
        if acol == bcol {
            // Same column: remove higher index first.
            let (first_pi, second_pi) = if api < bpi { (api, bpi) } else { (bpi, api) };
            if let Some(ws) = state.session.workspaces.get_mut(aws) {
                if acol >= ws.scrolling.columns.len() { return; }
                // Remove pane at higher index first so removal doesn't shift the other.
                let pane_b = ws.scrolling.remove_pane(acol, second_pi);
                let pane_a = ws.scrolling.remove_pane(acol, first_pi);
                // column now has 2 fewer panes at first_pi and second_pi-1
                if let (Some(a), Some(b)) = (pane_a, pane_b) {
                    let insert_b = (second_pi - 2).min(ws.scrolling.columns[acol].panes.len());
                    ws.scrolling.add_pane_to_column(acol, Some(insert_b), b, true);
                    let insert_a = first_pi.min(ws.scrolling.columns[acol].panes.len());
                    ws.scrolling.add_pane_to_column(acol, Some(insert_a), a, true);
                }
            }
        } else {
            // Different columns, same workspace: remove both, then cross-insert.
            // Remove from higher column index first to avoid index shift.
            let (first_col, second_col, first_pi, second_pi) = if acol < bcol {
                (acol, bcol, api, bpi)
            } else {
                (bcol, acol, bpi, api)
            };
            if let Some(ws) = state.session.workspaces.get_mut(aws) {
                let pane2 = ws.scrolling.remove_pane(second_col, second_pi);
                let pane1 = ws.scrolling.remove_pane(first_col, first_pi);
                if let (Some(a), Some(b)) = (pane1, pane2) {
                    let target_col_a = if acol < bcol { bcol - 1 } else { bcol };
                    let target_col_b = if acol < bcol { acol } else { acol - 1 };
                    let tca = target_col_a.min(ws.scrolling.columns.len().saturating_sub(1));
                    let tcb = target_col_b.min(ws.scrolling.columns.len().saturating_sub(1));
                    ws.scrolling.add_pane_to_column(tca, None, a, true);
                    ws.scrolling.add_pane_to_column(tcb, None, b, true);
                }
            }
        }
    } else {
        // Different workspaces: move A to B's workspace, B to A's workspace.
        let pane_a = {
            let ws = state.session.workspaces.get_mut(aws);
            ws.and_then(|ws| {
                if acol < ws.scrolling.columns.len() {
                    ws.scrolling.remove_pane(acol, api)
                } else { None }
            })
        };
        if pane_a.is_none() { return; }
        let pane_b = {
            let ws = state.session.workspaces.get_mut(bws);
            ws.and_then(|ws| {
                if bcol < ws.scrolling.columns.len() {
                    ws.scrolling.remove_pane(bcol, bpi)
                } else { None }
            })
        };
        if pane_b.is_none() { return; }
        if let (Some(a), Some(b)) = (pane_a, pane_b) {
            // Insert A at B's original column in B's workspace.
            if let Some(ws) = state.session.workspaces.get_mut(bws) {
                let col = bcol.min(ws.scrolling.columns.len().saturating_sub(1));
                ws.scrolling.add_pane_to_column(col, None, a, true);
            }
            // Insert B at A's original column in A's workspace.
            if let Some(ws) = state.session.workspaces.get_mut(aws) {
                let col = acol.min(ws.scrolling.columns.len().saturating_sub(1));
                ws.scrolling.add_pane_to_column(col, None, b, true);
            }
        }
    }

    state.needs_redraw = true;
}

pub fn handle_move_param(state: &mut AppState, action: &WmAction) {
    let WmAction::Move { pane_id, target_col } = action else { return };
    if let Some((_ws_idx, col_idx, _)) = find_pane_location(&state.session, *pane_id) {
        move_pane_to_column(state, *pane_id, col_idx, *target_col);
    }
    state.needs_redraw = true;
}

pub fn handle_move_pane_to_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::MovePaneToWorkspace { pane_id, ws_idx } = action else { return };
    if let Some((current_ws, current_col, _)) = find_pane_location(&state.session, *pane_id)
        && current_ws != *ws_idx
    {
        move_pane_to_workspace_column(state, *pane_id, *ws_idx, current_col);
    }
    state.needs_redraw = true;
}

pub fn handle_move_pane_to_column(state: &mut AppState, action: &WmAction) {
    let WmAction::MovePaneToColumn { pane_id, ws_idx, col_idx } = action else { return };
    if let Some((current_ws, current_col, _)) = find_pane_location(&state.session, *pane_id) {
        if current_ws == *ws_idx {
            move_pane_to_column(state, *pane_id, current_col, *col_idx);
        } else {
            move_pane_to_workspace_column(state, *pane_id, *ws_idx, *col_idx);
        }
    }
    state.needs_redraw = true;
}

pub fn handle_resize(state: &mut AppState, action: &WmAction) {
    let WmAction::Resize { target, axis, amount } = action else { return };
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
    let WmAction::ResizeTo { target, width, height } = action else { return };
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
    if let Some(pane_id) = state.focused_pane
        && let Some(ws) = state.session.active_workspace_mut()
    {
        let wa = ws.scrolling.working_area;
        let is_floating = ws.floating_panes.iter().any(|f| f.pane.id.0 == pane_id);

        if is_floating {
            if let Some(idx) = ws.floating_panes.iter().position(|f| f.pane.id.0 == pane_id) {
                let float = ws.floating_panes.remove(idx);
                let orig_col = float.original_column_idx;
                let orig_pane = float.original_pane_idx;
                if let Some(col_idx) = orig_col {
                    if col_idx < ws.scrolling.columns.len() {
                        let target_idx = orig_pane
                            .unwrap_or(0)
                            .min(ws.scrolling.columns[col_idx].panes.len());
                        ws.scrolling.columns[col_idx].panes.insert(target_idx, float.pane);
                        ws.scrolling.columns[col_idx].active_pane_idx = target_idx;
                        ws.scrolling.active_column_idx = col_idx;
                    } else {
                        ws.scrolling.add_column(
                            None,
                            Column::new(
                                ColumnId(pane_id),
                                float.pane,
                                ColumnWidth::Proportion(0.5),
                            ),
                            true,
                        );
                    }
                } else {
                    ws.scrolling.add_column(
                        None,
                        Column::new(
                            ColumnId(pane_id),
                            float.pane,
                            ColumnWidth::Proportion(0.5),
                        ),
                        true,
                    );
                }
                ws.floating_is_active = false;
            }
        } else {
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
            if let Some((col_idx, pane_idx)) = found
                && let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx)
            {
                let fw = wa.size.w * 0.75;
                let fh = wa.size.h * 0.75;
                let fx = wa.loc.x + (wa.size.w - fw) / 2.0;
                let fy = wa.loc.y + (wa.size.h - fh) / 2.0;
                ws.floating_panes.push(heca_core::layout::workspace::FloatingPane {
                    pane: removed,
                    position: heca_core::layout::types::Point::new(fx, fy),
                    size: heca_core::layout::types::Size::new(fw, fh),
                    is_active: true,
                    original_column_idx: Some(col_idx),
                    original_pane_idx: Some(pane_idx),
                });
                ws.floating_is_active = true;
            }
        }
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_close_pane(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.active_column() {
            let pane_idx = col.active_pane_idx;
            if let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx) {
                state.backends.remove(&removed.id.0);
            }
        }
    }

    let ws_is_empty = state
        .session
        .workspaces
        .get(current_ws)
        .map(|ws| ws.scrolling.columns.iter().all(|c| c.panes.is_empty()))
        .unwrap_or(true);

    if ws_is_empty && state.session.workspaces.len() > 1 {
        destroy_empty_workspace(state, current_ws);
        let new_idx = current_ws.min(state.session.workspaces.len().saturating_sub(1));
        state.session.switch_to_workspace(new_idx);
        sync_focus(state);
    } else if ws_is_empty {
        let next_id = state.session.next_id();
        let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
        state.session.add_pane(pane, None, true);
        state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
        sync_focus(state);
    } else {
        sync_focus(state);
    }
    state.needs_redraw = true;
}

pub fn handle_pane_select(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSelect { candidates };
        state.needs_redraw = true;
    }
}

pub fn handle_swap_pane(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSwap { candidates, focus_after: false };
        state.needs_redraw = true;
    }
}

pub fn handle_swap_and_focus_pane(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSwap { candidates, focus_after: true };
        state.needs_redraw = true;
    }
}

pub fn handle_rename_pane(state: &mut AppState, _action: &WmAction) {
    if let Some(pane_id) = state.focused_pane {
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

pub fn handle_float_at(state: &mut AppState, action: &WmAction) {
    let WmAction::FloatAt { pane_id, x, y, width, height } = action else { return };
    if let Some(ws) = state.session.active_workspace_mut() {
        let mut found = None;
        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
            for (pi, pane) in col.panes.iter().enumerate() {
                if pane.id.0 == *pane_id {
                    found = Some((ci, pi));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        if let Some((col_idx, pane_idx)) = found
            && let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx)
        {
            ws.floating_panes.push(heca_core::layout::workspace::FloatingPane {
                pane: removed,
                position: heca_core::layout::types::Point::new(*x, *y),
                size: heca_core::layout::types::Size::new(*width, *height),
                is_active: true,
                original_column_idx: Some(col_idx),
                original_pane_idx: Some(pane_idx),
            });
            ws.floating_is_active = true;
        }
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_close_pane_by_id(state: &mut AppState, action: &WmAction) {
    let WmAction::ClosePaneById { pane_id } = action else { return };
    if let Some(ws) = state.session.active_workspace_mut() {
        let mut found = None;
        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
            if let Some(pi) = col.panes.iter().position(|p| p.id.0 == *pane_id) {
                found = Some((ci, pi));
                break;
            }
        }
        if let Some((ci, pi)) = found
            && let Some(removed) = ws.scrolling.remove_pane(ci, pi)
        {
            state.backends.remove(&removed.id.0);
        }
    }
    let current_ws = state.session.active_workspace_idx;
    let ws_is_empty = state
        .session
        .workspaces
        .get(current_ws)
        .map(|ws| ws.scrolling.columns.iter().all(|c| c.panes.is_empty()))
        .unwrap_or(true);
    if ws_is_empty && state.session.workspaces.len() > 1 {
        destroy_empty_workspace(state, current_ws);
        let new_idx = current_ws.min(state.session.workspaces.len().saturating_sub(1));
        state.session.switch_to_workspace(new_idx);
        sync_focus(state);
    } else if ws_is_empty {
        let next_id = state.session.next_id();
        let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
        state.session.add_pane(pane, None, true);
        state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
        sync_focus(state);
    } else {
        sync_focus(state);
    }
    state.needs_redraw = true;
}

pub fn handle_rename_target(state: &mut AppState, action: &WmAction) {
    let WmAction::RenameTarget { pane_id, name } = action else { return };
    if let Some(ws) = state.session.active_workspace_mut()
        && let Some(pane) = ws.find_pane_mut(heca_core::layout::PaneId(*pane_id))
    {
        pane.title = if name.is_empty() {
            format!("pane{pane_id}")
        } else {
            name.clone()
        };
        sync_focus(state);
        state.needs_redraw = true;
    }
}

// ── Workspace ──

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
    state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
    while state.last_visited_pane_per_ws.len() <= new_idx {
        state.last_visited_pane_per_ws.push(None);
    }
    sync_focus(state);
    state.needs_redraw = true;
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
    state.needs_redraw = true;
}

pub fn handle_sidebar_right(state: &mut AppState, _action: &WmAction) {
    state.sidebar.right_visible = !state.sidebar.right_visible;
    update_session_viewport(state);
    state.needs_redraw = true;
}

pub fn handle_sidebar_focus(state: &mut AppState, _action: &WmAction) {
    state.sidebar.left_visible = true;
    state.sidebar.left_width = 200.0;
    state.input_mode = InputMode::SidebarNav;
    update_session_viewport(state);
    state.sidebar_tree.rebuild(
        &state.session,
        state.last_visited_ws_idx,
        state.focused_pane,
        &state.last_visited_pane_per_ws,
    );
    state.needs_redraw = true;
}

pub fn handle_sidebar_up(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let is_collapsed = !state.sidebar.left_visible || state.sidebar.left_width < 80.0;
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
        let is_collapsed = !state.sidebar.left_visible || state.sidebar.left_width < 80.0;
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
                let target_pane_id = heca_core::layout::PaneId(*pane_id);
                let target_ws = state
                    .session
                    .workspaces
                    .iter()
                    .position(|ws| ws.find_pane(target_pane_id).is_some());
                if let Some(ws_idx) = target_ws {
                    if ws_idx != state.session.active_workspace_idx {
                        switch_workspace_tracked(state, ws_idx);
                    }
                    focus_pane_by_id(state, *pane_id);
                }
                // Stay in sidebar mode; only Enter/Esc exit
            }
            Some(sidebar::SidebarItem::Workspace { .. }) => {
                let ws_idx = state
                    .sidebar_tree
                    .cursor_workspace_index()
                    .unwrap_or(state.session.active_workspace_idx);
                if ws_idx != state.session.active_workspace_idx {
                    switch_workspace_tracked(state, ws_idx);
                }
                let next_id = state.session.next_id();
                let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
                state.session.add_pane(pane, None, true);
                state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
                sync_focus(state);
                // Stay in sidebar mode; only Enter/Esc exit
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
            Some(sidebar::SidebarItem::Pane { pane_id }) => {
                let target_pane_id = heca_core::layout::PaneId(*pane_id);
                let target_ws = state
                    .session
                    .workspaces
                    .iter()
                    .position(|ws| ws.find_pane(target_pane_id).is_some());
                if let Some(ws_idx) = target_ws {
                    if ws_idx != state.session.active_workspace_idx {
                        switch_workspace_tracked(state, ws_idx);
                    }
                    focus_pane_by_id(state, *pane_id);
                }
                state.input_mode = InputMode::Normal;
            }
            _ => {
                state.sidebar_tree.toggle_expand();
            }
        }
        state.needs_redraw = true;
    }
}

// ── System ──

pub fn handle_command_palette(state: &mut AppState, _action: &WmAction) {
    eprintln!("Command palette triggered (not yet implemented)");
    state.needs_redraw = true;
}

// ── External commands ──

pub fn handle_spawn_command(state: &mut AppState, action: &WmAction) {
    let WmAction::SpawnCommand { command } = action else { return };
    // Create a new pane with the command as its title.
    // In the future this will spawn a real PTY via portable-pty.
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), command.clone());
    state.session.add_pane(pane, None, true);
    state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
    sync_focus(state);
    state.needs_redraw = true;
}

// ── Mode ──

pub fn handle_enter_mode(state: &mut AppState, action: &WmAction) {
    let WmAction::EnterMode { name } = action else { return };
    state.input_mode = InputMode::Mode { name: name.clone() };
    state.needs_redraw = true;
}

// ── Config ──

pub fn handle_reload_config(state: &mut AppState, _action: &WmAction) {
    eprintln!("[config] reload requested");
    state.pending_reload = true;
}
