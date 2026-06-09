//! Mouse drag state transitions.
//!
//! This module owns drag initiation, drag-motion updates, and cancellation for
//! interactive move and sidebar drag workflows.

use heca_core::layout::Point;
use heca_grid_ui::drag::{DragItemKind, DragItemId, DragLabel, DragSurfaceId, SurfaceDragPhase};

use crate::app_state::{AppState, InteractiveMovePhase};
use crate::chrome::DEFAULT_COLLAPSED_SIDEBAR_WIDTH;

/// Route cursor movement to the active drag phase handler.
///
/// Dispatches to the appropriate phase handler based on drag state,
/// then updates sidebar hover highlighting if a drag is active.
pub(crate) fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) {
    state.mouse.pos = pos;

    handle_interactive_move_starting(state, pos);
    handle_sidebar_drag_starting(state, pos);
    handle_interactive_move_drag(state, pos);
    handle_sidebar_drag_move(state, pos);

    if state.mouse.drag_ctx.is_dragging() || state.mouse.interactive_move.is_some() {
        update_sidebar_drag_hover(state);
    }
}

// ── Interactive move starting (rubberband phase) ──────────────────────────

/// Handle rubberband displacement during the threshold phase of interactive move.
/// If the drag distance exceeds the threshold, transitions to full drag mode.
fn handle_interactive_move_starting(state: &mut AppState, pos: (f32, f32)) {
    let mut should_transition = false;
    if let Some(InteractiveMovePhase::Starting {
        pane_id,
        original_ws,
        start_mouse,
        threshold_sq,
        ..
    }) = state.mouse.interactive_move
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
        && let Some(InteractiveMovePhase::Starting { pane_id, swap, .. }) = state.mouse.interactive_move
    {
        transition_to_moving(state, pane_id, pos, swap);
    }
}

// ── Sidebar drag starting (threshold phase) ──────────────────────────────

/// Handle threshold detection for sidebar drag. On threshold exceeded,
/// transitions to active sidebar drag mode and sets up ghost label + source highlight.
fn handle_sidebar_drag_starting(state: &mut AppState, pos: (f32, f32)) {
    let left = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar).expect("LeftSidebar pre-populated in DragContext::default");
    let phase = std::mem::replace(&mut left.phase, SurfaceDragPhase::Idle);
    if let SurfaceDragPhase::Starting {
        pane_id: Some(pane_id),
        original_ws,
        start_pos,
        threshold_sq,
        swap,
        ..
    } = phase
    {
        let dx = pos.0 - start_pos.0;
        let dy = pos.1 - start_pos.1;
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
            let sw = if state.sidebar.left_visible {
                chrome.left_sidebar_width
            } else {
                DEFAULT_COLLAPSED_SIDEBAR_WIDTH
            };

            let left = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar).expect("LeftSidebar pre-populated in DragContext::default");
            left.ghost_label = Some(DragLabel {
                text: label,
                x: pos.0,
                y: pos.1,
                width: sw,
                height: 20.0,
            });
            left.phase = SurfaceDragPhase::Dragging {
                kind: DragItemKind::Pane,
                pane_id: Some(pane_id),
                original_ws,
                swap,
            };
            // source_item was already set when the Starting phase began
        } else {
            // Threshold not exceeded — restore the Starting phase
            state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar).expect("LeftSidebar pre-populated in DragContext::default").phase = SurfaceDragPhase::Starting {
                kind: DragItemKind::Pane,
                pane_id: Some(pane_id),
                original_ws,
                start_pos,
                threshold_sq,
                swap,
            };
        }
    } else {
        // Not a Starting phase — restore whatever it was
        state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar).expect("LeftSidebar pre-populated in DragContext::default").phase = phase;
    }
}

// ── Interactive move drag (active phase) ─────────────────────────────────

/// Handle cursor movement during an active interactive drag.
/// Updates detached pane position, in-layout offset tracking, and insert hint.
fn handle_interactive_move_drag(state: &mut AppState, pos: (f32, f32)) {
    let Some(InteractiveMovePhase::Moving { offset, .. }) = state.mouse.interactive_move else {
        return;
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
    let source_id = match state.mouse.interactive_move {
        Some(InteractiveMovePhase::Moving { _pane_id, .. }) => _pane_id,
        _ => return,
    };

    if state.mouse.detached_pane.is_none()
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
    let left = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar).expect("LeftSidebar pre-populated in DragContext::default");
    if let SurfaceDragPhase::Dragging { .. } = left.phase
        && let Some(label) = &mut left.ghost_label
    {
        label.x = pos.0;
        label.y = pos.1;
    }
}

// ── Public API ───────────────────────────────────────────────────────────

pub(super) fn start_interactive_move(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32)) {
    let swap = state.modifiers.shift_key();
    state.mouse.interactive_move = Some(InteractiveMovePhase::Starting {
        pane_id,
        original_ws: state.session.active_workspace_idx,
        start_mouse: mouse_pos,
        threshold_sq: 64.0,
        swap,
    });
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
    // Interactive move
    match &mut state.mouse.interactive_move {
        Some(InteractiveMovePhase::Starting { swap: s, .. }) => *s = swap,
        Some(InteractiveMovePhase::Moving { swap: s, .. }) => *s = swap,
        None => {}
    }
    // Surface drags
    for surface_state in state.mouse.drag_ctx.surfaces.values_mut() {
        match &mut surface_state.phase {
            SurfaceDragPhase::Starting { swap: s, .. } => *s = swap,
            SurfaceDragPhase::Dragging { swap: s, .. } => *s = swap,
            SurfaceDragPhase::Idle => {}
        }
    }
}

/// Transition from rubberband (InteractiveMovePhase::Starting) to active drag
/// (InteractiveMovePhase::Moving). Both move and swap modes keep the pane in the layout
/// and track it via interactive_move_offset.
fn transition_to_moving(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32), swap: bool) {
    let (cx, cy) = super::content_area_origin(state);
    let original_ws = state.session.active_workspace_idx;

    // Find the pane and zero any rubberband offset from the starting phase.
    let (_, _, col_x, pane_y) = {
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

    state.mouse.interactive_move = Some(InteractiveMovePhase::Moving {
        _pane_id: pane_id,
        _original_ws: original_ws,
        offset,
        swap,
    });
}

pub(super) fn cancel_interactive_move(state: &mut AppState) {
    // Reset any rubberband offset from InteractiveMovePhase::Starting phase.
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
    state.mouse.interactive_move = None;
    state.mouse.insert_hint = None;
    state.mouse.drag_ctx.cancel_all();
    crate::app::mutations::after_layout_change(state);
}

/// Reset interactive_move_offset on all panes in the active workspace.
/// This clears any rubberband displacement from the InteractiveMovePhase::Starting phase.
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
        DEFAULT_COLLAPSED_SIDEBAR_WIDTH
    };
    let left = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar).expect("LeftSidebar pre-populated in DragContext::default");
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
                left.hover_item = None;
            } else {
                left.hover_item = Some(DragItemId::new(fi));
            }
        } else {
            left.hover_item = None;
        }
    } else {
        left.hover_item = None;
    }
}