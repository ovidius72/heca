//! Named action handlers — one per `WmAction` variant.
//!
//! Each handler is a plain `fn(&mut AppState, &WmAction)` that performs the
//! action.  Parameterized variants destructure their fields from the enum;
//! unit variants ignore the `_action` parameter.

use crate::app_state::{AppState, InputMode, RenameTarget};
use crate::input::WmAction;
use crate::sidebar;
use crate::{
    collect_all_column_candidates, collect_all_pane_candidates, destroy_empty_workspace,
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
    }
    state.needs_redraw = true;
}

pub fn handle_swap_right(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_column_right();
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
    if let Some((aws, acol, _)) = find_pane_location(&state.session, *a_id)
        && let Some((bws, bcol, _)) = find_pane_location(&state.session, *b_id)
    {
        if aws == bws && acol == bcol {
            // Same column — no-op
        } else if aws == bws {
            move_pane_to_column(state, *a_id, acol, bcol);
        } else {
            move_pane_to_workspace_column(state, *a_id, bws, bcol);
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

pub fn handle_resize(state: &mut AppState, action: &WmAction) {
    let WmAction::Resize { target, delta } = action else { return };
    if let Some(ws) = state.session.active_workspace_mut() {
        match target {
            crate::input::ResizeTarget::Column => {
                let delta_f = *delta as f64 / 1000.0;
                ws.scrolling.resize_active_column(delta_f);
            }
            crate::input::ResizeTarget::Pane => {
                let h = ws.scrolling.working_area.size.h;
                let gaps = ws.scrolling.options.gaps;
                if let Some(col) = ws.scrolling.active_column_mut() {
                    col.resize_active_pane_height(*delta as f64, h, gaps);
                }
            }
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
                        ws.scrolling.update_all_column_widths();
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

pub fn handle_swap_select(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_column_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSwap { candidates };
        state.needs_redraw = true;
    }
}

pub fn handle_swap_and_focus(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_column_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSwap { candidates };
        state.swap_and_focus = true;
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
                state.input_mode = InputMode::Normal;
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

pub fn handle_tab_next(state: &mut AppState, _action: &WmAction) {
    if !state.tab_names.is_empty() {
        state.active_tab = (state.active_tab + 1) % state.tab_names.len();
    }
}

pub fn handle_tab_prev(state: &mut AppState, _action: &WmAction) {
    if !state.tab_names.is_empty() {
        state.active_tab =
            (state.active_tab + state.tab_names.len() - 1) % state.tab_names.len();
    }
}

// ── System ──

pub fn handle_command_palette(state: &mut AppState, _action: &WmAction) {
    eprintln!("Command palette triggered (not yet implemented)");
    state.needs_redraw = true;
}
