//! Interactive move — content-area drag logic.
//!
//! Handles the full lifecycle of interactive pane move/swap:
//! starting (rubberband), moving (offset tracking), cancellation,
//! and swap-mode synchronization.
//!
//! This is separate from the surface drag system (`DragContext`) because
//! interactive move detaches a pane from the layout, shows a ghost pane
//! following the cursor, and computes an insert hint — all content-area
//! concepts that don't apply to sidebar/inspector surfaces.

use heca_core::layout::Point;
use heca_grid_ui::drag::SurfaceDragPhase;

use crate::app_state::{AppState, InteractiveMovePhase};

/// Start an interactive move from a content-area pane.
///
/// Enters the rubberband threshold phase. The pane stays in layout with
/// a dampened offset until the cursor moves beyond the threshold.
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

/// Cancel the active interactive move.
///
/// Re-inserts any detached pane at its original position, clears all
/// drag state, and triggers layout change notification.
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
///
/// This clears any rubberband displacement from the
/// `InteractiveMovePhase::Starting` phase.
pub(super) fn reset_interactive_move_offset(state: &mut AppState) {
    if let Some(ws) = state.session.active_workspace_mut() {
        for col in &mut ws.scrolling.columns {
            for pane in &mut col.panes {
                pane.interactive_move_offset = Point::default();
            }
        }
    }
}

/// Keep the current drag operation in sync with the Shift modifier.
///
/// This updates both the content-area drag states and the surface drag
/// states so the operation can switch live while the pointer is held down.
pub(super) fn sync_drag_swap_mode(state: &mut AppState) {
    set_drag_swap_mode(state, state.modifiers.shift_key());
}

// ── Cursor-move handlers (called from drag::on_cursor_moved) ─────────────

/// Handle rubberband displacement during the threshold phase of interactive move.
/// If the drag distance exceeds the threshold, transitions to full drag mode.
pub(super) fn handle_interactive_move_starting(state: &mut AppState, pos: (f32, f32)) {
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

/// Handle cursor movement during an active interactive drag.
/// Updates detached pane position, in-layout offset tracking, and insert hint.
pub(super) fn handle_interactive_move_drag(state: &mut AppState, pos: (f32, f32)) {
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
        ws.scrolling.columns[ci].panes[pi].interactive_move_offset = Point::new(
            pointer_in.0 as f64 - offset.0 as f64 - col_x,
            pointer_in.1 as f64 - offset.1 as f64 - pane_y,
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

/// Transition from rubberband (`InteractiveMovePhase::Starting`) to active drag
/// (`InteractiveMovePhase::Moving`). Both move and swap modes keep the pane
/// in the layout and track it via `interactive_move_offset`.
fn transition_to_moving(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32), swap: bool) {
    let (cx, cy) = super::content_area_origin(state);
    let original_ws = state.session.active_workspace_idx;

    // Find the pane and zero any rubberband offset from the starting phase.
    let (_col_ci, _col_pi, col_x, pane_y) = {
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