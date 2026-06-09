//! Mouse drag state transitions.
//!
//! This module owns drag initiation, drag-motion updates, and cancellation for
//! interactive move and sidebar drag workflows.

use heca_core::layout::Point;

use crate::app_state::{AppState, DragState};

/// Route cursor movement to the active drag phase handler.
///
/// Dispatches to the appropriate phase handler based on `DragState`,
/// then updates sidebar hover highlighting if a drag is active.
pub(crate) fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) {
    state.mouse.pos = pos;

    handle_interactive_move_starting(state, pos);
    handle_sidebar_drag_starting(state, pos);
    handle_interactive_move_drag(state, pos);
    handle_sidebar_drag_move(state, pos);

    if !matches!(state.mouse.drag_state, DragState::None) {
        update_sidebar_drag_hover(state);
    }
}

// ── Interactive move starting (rubberband phase) ──────────────────────────

/// Handle rubberband displacement during the threshold phase of interactive move.
/// If the drag distance exceeds the threshold, transitions to full drag mode.
fn handle_interactive_move_starting(state: &mut AppState, pos: (f32, f32)) {
    let mut should_transition = false;
    if let DragState::InteractiveMoveStarting {
        pane_id,
        original_ws,
        start_mouse,
        threshold_sq,
        ..
    } = state.mouse.drag_state
    {
        let dx = pos.0 - start_mouse.0;
        let dy = pos.1 - start_mouse.1;
        let sq_dist = dx * dx + dy * dy;

        if let Some(ws) = state.session.workspaces.get_mut(original_ws)
            && let Some((ci, pi)) = super::find_pane_in_workspace(ws, pane_id)
        {
            let factor = super::rubberband(sq_dist / threshold_sq);
            ws.scrolling.columns[ci].panes[pi].interactive_move_offset =
                Point::new((dx * factor) as f64, (dy * factor) as f64);
        }

        if sq_dist > threshold_sq {
            should_transition = true;
        }
    }

    if should_transition
        && let DragState::InteractiveMoveStarting { pane_id, swap, .. } = state.mouse.drag_state
    {
        transition_to_moving(state, pane_id, pos, swap);
    }
}

// ── Sidebar drag starting (threshold phase) ──────────────────────────────

/// Handle threshold detection for sidebar drag. On threshold exceeded,
/// transitions to active sidebar drag mode and sets up ghost label + source highlight.
fn handle_sidebar_drag_starting(state: &mut AppState, pos: (f32, f32)) {
    if let DragState::SidebarDragStarting {
        pane_id,
        original_ws,
        start_mouse,
        threshold_sq,
        swap,
        ..
    } = state.mouse.drag_state
    {
        let dx = pos.0 - start_mouse.0;
        let dy = pos.1 - start_mouse.1;
        let sq_dist = dx * dx + dy * dy;

        if sq_dist > threshold_sq {
            let label = state.sidebar_tree.flat_items.iter()
                .find(|item| matches!(item, crate::sidebar::SidebarItem::Pane { pane_id: pid } if *pid == pane_id))
                .and_then(|item| {
                    if let crate::sidebar::SidebarItem::Pane { pane_id: pid } = item {
                        for ws in &state.sidebar_tree.workspaces {
                            for col in &ws.columns {
                                for p in &col.panes {
                                    if p.pane_id == *pid {
                                        return Some(p.name.clone());
                                    }
                                }
                            }
                        }
                    }
                    None
                })
                .unwrap_or_else(|| format!("pane{}", pane_id));

            let chrome = super::chrome_config(state);
            let (_win_w, win_h) = super::window_logical_size(state);
            let sw = if state.sidebar.left_visible {
                chrome.left_sidebar_width
            } else {
                40.0
            };
            let sidebar_top = chrome.tab_bar_height;
            let sidebar_bottom = win_h - chrome.status_bar_height;

            state.mouse.sidebar_drag_source_fi = crate::sidebar::sidebar_hit_test(
                &state.sidebar_tree,
                sidebar_top,
                sidebar_bottom - sidebar_top,
                sw,
                pos.1,
            );
            state.mouse.sidebar_drag_label = Some(crate::app_state::SidebarDragLabel {
                text: label,
                x: pos.0,
                y: pos.1,
                width: sw,
                _height: 20.0,
            });

            state.mouse.drag_state = DragState::SidebarDrag {
                pane_id,
                original_ws,
                swap,
            };
        }
    }
}

// ── Interactive move drag (active phase) ─────────────────────────────────

/// Handle cursor movement during an active interactive drag.
/// Updates detached pane position, in-layout offset tracking, and insert hint.
fn handle_interactive_move_drag(state: &mut AppState, pos: (f32, f32)) {
    if !matches!(state.mouse.drag_state, DragState::InteractiveMove { .. }) {
        return;
    }

    let offset = match state.mouse.drag_state {
        DragState::InteractiveMove { offset, .. } => offset,
        // SAFETY: guarded by matches!() check above
        _ => unreachable!("InteractiveMove variant guaranteed by outer matches guard"),
    };
    let (cx, cy) = super::content_area_origin(state);
    let pointer_in = (pos.0 - cx, pos.1 - cy);

    if let Some(det) = &mut state.mouse.detached_pane {
        det.render_pos = Point::new(
            (pointer_in.0 - offset.0) as f64,
            (pointer_in.1 - offset.1) as f64,
        );
    }

    // Swap mode: update interactive_move_offset so the pane follows the cursor.
    // Move mode (non-detached): same — pane follows cursor via offset.
    // This reuses the same mechanism as the rubberband starting phase,
    // but with a direct 1:1 tracking instead of damping.
    if state.mouse.detached_pane.is_none()
        && let DragState::InteractiveMove { swap: _, _pane_id: source_id, .. } = state.mouse.drag_state
        && let Some(ws) = state.session.workspaces.get_mut(state.session.active_workspace_idx)
        && let Some((ci, pi)) = super::find_pane_in_workspace(ws, source_id)
    {
        let col_x = ws.scrolling.column_x(ci) - ws.scrolling.view_pos();
        let pane_y = ws.scrolling.working_area.loc.y
            + ws.scrolling.pane_y_in_column(ci, pi);
        let pane_layout_x = col_x;
        let pane_layout_y = pane_y;
        ws.scrolling.columns[ci].panes[pi].interactive_move_offset = Point::new(
            pointer_in.0 as f64 - offset.0 as f64 - pane_layout_x,
            pointer_in.1 as f64 - offset.1 as f64 - pane_layout_y,
        );
    }

    if let Some(ws) = state.session.active_workspace() {
        let space = Point::new(
            (pointer_in.0 as f64) + ws.scrolling.view_pos(),
            pointer_in.1 as f64,
        );
        state.mouse.insert_hint = Some(ws.scrolling.insert_position(space));
    }
}

// ── Sidebar drag move (active phase) ────────────────────────────────────

/// Handle cursor movement during an active sidebar drag.
/// Updates the ghost label position to follow the cursor.
fn handle_sidebar_drag_move(state: &mut AppState, pos: (f32, f32)) {
    if let DragState::SidebarDrag { .. } = state.mouse.drag_state
        && let Some(label) = &mut state.mouse.sidebar_drag_label
    {
        label.x = pos.0;
        label.y = pos.1;
    }
}

// ── Public API ───────────────────────────────────────────────────────────

pub(super) fn start_interactive_move(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32)) {
    let swap = state.modifiers.shift_key();
    state.mouse.drag_state = DragState::InteractiveMoveStarting {
        pane_id,
        original_ws: state.session.active_workspace_idx,
        start_mouse: mouse_pos,
        threshold_sq: 64.0,
        swap,
    };
}

/// Keep the current drag operation in sync with the Shift modifier.
///
/// This updates both the content-area drag states and the sidebar drag states so
/// the operation can switch live while the pointer is held down.
pub(super) fn sync_drag_swap_mode(state: &mut AppState) {
    set_drag_swap_mode(state, state.modifiers.shift_key());
}

// ── Internal helpers ─────────────────────────────────────────────────────

fn set_drag_swap_mode(state: &mut AppState, swap: bool) {
    match &mut state.mouse.drag_state {
        DragState::InteractiveMoveStarting { swap: s, .. }
        | DragState::InteractiveMove { swap: s, .. }
        | DragState::SidebarDragStarting { swap: s, .. }
        | DragState::SidebarDrag { swap: s, .. } => {
            *s = swap;
        }
        DragState::None => {}
    }
}

/// Transition from rubberband (InteractiveMoveStarting) to active drag
/// (InteractiveMove). Both move and swap modes keep the pane in the layout
/// and track it via interactive_move_offset.
fn transition_to_moving(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32), swap: bool) {
    let (cx, cy) = super::content_area_origin(state);
    let original_ws = state.session.active_workspace_idx;

    // Find the pane and zero any rubberband offset from the starting phase.
    let (_ci, _pi, col_x, pane_y) = {
        let ws = match state.session.workspaces.get_mut(original_ws) {
            Some(ws) => ws,
            None => return,
        };
        let found = super::find_pane_in_workspace(ws, pane_id);
        let (ci, pi) = match found {
            Some(v) => v,
            None => return,
        };
        ws.scrolling.columns[ci].panes[pi].interactive_move_offset = Point::default();
        let col_x = ws.scrolling.column_x(ci);
        let pane_y = ws.scrolling.pane_y_in_column(ci, pi);
        (ci, pi, col_x, pane_y)
    };

    // Compute grab offset: cursor position relative to pane top-left in content coords.
    let pointer_in = ((mouse_pos.0 - cx) as f64, (mouse_pos.1 - cy) as f64);
    let view_pos = state.session.workspaces.get(original_ws)
        .map(|ws| ws.scrolling.view_pos())
        .unwrap_or(0.0);
    let col_screen_x = col_x - view_pos;
    let offset = (
        (pointer_in.0 - col_screen_x) as f32,
        (pointer_in.1 - pane_y) as f32,
    );

    state.mouse.drag_state = DragState::InteractiveMove {
        _pane_id: pane_id,
        _original_ws: original_ws,
        offset,
        swap,
    };
}

pub(super) fn cancel_interactive_move(state: &mut AppState) {
    // Reset any rubberband offset from InteractiveMoveStarting phase.
    reset_interactive_move_offset(state);

    if let Some(det) = state.mouse.detached_pane.take() {
        let ws_idx = det
            .original_ws
            .min(state.session.workspaces.len().saturating_sub(1));
        if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
            if let Some(orig_idx) = ws
                .scrolling
                .columns
                .iter()
                .position(|c| c.id == det.original_col_id)
            {
                let pane_idx = det
                    .original_pane
                    .min(ws.scrolling.columns[orig_idx].panes.len());
                ws.scrolling
                    .add_pane_to_column(orig_idx, Some(pane_idx), det.pane, true);
            } else {
                state.session.add_pane(det.pane, None, true);
            }
        }
    }
    state.mouse.drag_state = DragState::None;
    state.mouse.insert_hint = None;
    state.mouse.sidebar_drag_source_fi = None;
    state.mouse.sidebar_drag_label = None;
    crate::app::mutations::after_layout_change(state);
}

/// Reset interactive_move_offset on all panes in the active workspace.
/// This clears any rubberband displacement from the InteractiveMoveStarting phase.
pub(super) fn reset_interactive_move_offset(state: &mut AppState) {
    if let Some(ws) = state.session.active_workspace_mut() {
        for col in &mut ws.scrolling.columns {
            for pane in &mut col.panes {
                pane.interactive_move_offset = Point::default();
            }
        }
    }
}

fn update_sidebar_drag_hover(state: &mut AppState) {
    let (_win_w, win_h) = super::window_logical_size(state);
    let chrome = super::chrome_config(state);
    let pos = state.mouse.pos;
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;
    let sw = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        40.0
    };
    if pos.0 >= 0.0 && pos.0 <= sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom {
        let sidebar_h = sidebar_bottom - sidebar_top;
        let fi = crate::sidebar::sidebar_hit_test(
            &state.sidebar_tree,
            sidebar_top,
            sidebar_h,
            sw,
            pos.1,
        );
        if let Some(fi) = fi {
            if matches!(
                state.sidebar_tree.flat_items.get(fi),
                Some(&crate::sidebar::SidebarItem::FloatingPane { .. })
            ) {
                state.mouse.drag_hover_sidebar_fi = None;
            } else {
                state.mouse.drag_hover_sidebar_fi = Some(fi);
            }
        } else {
            state.mouse.drag_hover_sidebar_fi = None;
        }
    } else {
        state.mouse.drag_hover_sidebar_fi = None;
    }
}