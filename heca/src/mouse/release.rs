//! Mouse release handlers for drag-and-drop.
//!
//! Extracted from `on_mouse_input()` for clarity. Each function handles
//! one release scenario: interactive move, sidebar drag, or sidebar drag starting.

use crate::app_state::{AppState, InteractiveMovePhase};
use crate::input::WmAction;

/// Handle release during an active interactive move (content-area drag).
///
/// Dispatches between swap mode (swap with target) and move mode (reinsert at
/// drop target), with sidebar drop as a fallback.
pub(super) fn handle_interactive_move_release(state: &mut AppState, pos: (f32, f32)) {
    let (swap, source_id) = match state.mouse.interactive_move {
        Some(InteractiveMovePhase::Moving { swap, _pane_id, .. }) => (swap, _pane_id),
        _ => return,
    };

    if swap {
        // Swap mode: pane stays in layout. If the pointer is over
        // a sidebar target, fall back to normal move semantics.
        // Otherwise, swap with the content-area target pane.
        super::drag::reset_interactive_move_offset(state);
        if let Some(target_id) = super::hit_test::hit_test_pane_excluding(
            state,
            state.mouse.pos,
            Some(source_id),
        ) {
            crate::handlers::handle_swap_param(
                state,
                &WmAction::Swap {
                    a_id: source_id,
                    b_id: target_id,
                },
            );
        } else if super::sidebar_drop::handle_drop(state, pos) {
            // Sidebar drop handled as a move.
        } else if let Some(hint) = state.mouse.insert_hint.take() {
            // Fallback: move semantics in the content area.
            handle_content_move(state, source_id, hint);
        } else {
            super::drag::cancel_interactive_move(state);
        }
    } else if super::sidebar_drop::handle_drop(state, pos) {
        // Sidebar drop handled.
    } else if let Some(hint) = state.mouse.insert_hint.take() {
        // Move mode: pane is still in layout. Remove it and
        // re-insert at the drop target position.
        super::drag::reset_interactive_move_offset(state);
        handle_content_move(state, source_id, hint);
    } else {
        super::drag::cancel_interactive_move(state);
    }

    state.mouse.interactive_move = None;
    state.mouse.insert_hint = None;
}

/// Handle release during an active sidebar drag.
pub(super) fn handle_sidebar_drag_release(
    state: &mut AppState,
    pane_id: u64,
    original_ws: usize,
    swap: bool,
    pos: (f32, f32),
) {
    super::sidebar_drop::drag_drop(state, pane_id, original_ws, swap, pos);
}

/// Handle release during sidebar drag starting (threshold not exceeded).
///
/// If a pending click action was stored, dispatch it. Otherwise, just clear the drag state.
pub(super) fn handle_sidebar_drag_starting_release(state: &mut AppState) -> Option<WmAction> {
    let click_action = state.mouse.pending_click_action.take();
    state.mouse.drag_ctx.cancel_all();

    if let Some(action) = click_action {
        if matches!(action, WmAction::FocusPane { .. })
            && matches!(state.input_mode, crate::app_state::InputMode::SidebarNav)
        {
            state.input_mode = crate::app_state::InputMode::Normal;
        }
        return Some(action);
    }
    None
}

// ── Internal helpers ─────────────────────────────────────────────────────

/// Move a pane from its current position and re-insert at the given insert hint.
fn handle_content_move(
    state: &mut AppState,
    source_id: u64,
    hint: heca_core::layout::types::PaneInsertTarget,
) {
    if let Some((ws_idx, col_idx, pane_idx)) = crate::find_pane_location(&state.session, source_id)
        && let Some(ws) = state.session.workspaces.get_mut(ws_idx)
        && let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx)
    {
        let target_ws = state.session.active_workspace_idx;
        let new_col_id = heca_core::layout::ColumnId(state.session.next_id());
        if let Some(target_ws_mut) = state.session.workspaces.get_mut(target_ws) {
            crate::app::pane_ops::insert_pane_at_position(
                target_ws_mut,
                removed,
                hint,
                new_col_id,
                heca_core::layout::ColumnWidth::Proportion(0.5),
                true,
            );
        }
    }
    state.focused_pane = Some(source_id);
    crate::app::mutations::after_layout_change(state);
}