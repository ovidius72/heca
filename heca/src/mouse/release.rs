//! Mouse release handlers for drag-and-drop.
//!
//! Extracted from `on_mouse_input()` for clarity. Each function handles
//! one release scenario: interactive move, sidebar drag, or sidebar drag starting.

use crate::app::interaction::InteractionSource;
use crate::app_state::{AppState, InteractiveMovePhase};
use crate::chrome::ChromeDragItem;
use crate::input::WmAction;
use heca_core::layout::PaneId;
use heca_grid_ui::drag::{DragSurfaceId, DropSide};

/// Handle release during an active interactive move (content-area drag).
///
/// Dispatches between swap mode (swap with target) and move mode (reinsert at
/// drop target), with sidebar drop as a fallback.
pub(super) fn handle_interactive_move_release(state: &mut AppState, pos: (f32, f32)) {
    let (swap, source_id) = match state.mouse.interactive_move {
        Some(InteractiveMovePhase::Moving { swap, pane_id, .. }) => (swap, pane_id),
        _ => return,
    };

    if swap {
        // Swap mode: pane stays in layout. If the pointer is over
        // a sidebar target, fall back to normal move semantics.
        // Otherwise, swap with the content-area target pane.
        super::interactive::reset_interactive_move_offset(state);
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
        } else if super::target::surface_interactive_move_drop(state, DragSurfaceId::LeftSidebar, pos) {
            // Sidebar drop handled as a move.
        } else if let Some(hint) = state.mouse.insert_hint.take() {
            // Fallback: move semantics in the content area.
            handle_content_move(state, source_id, hint);
        } else {
            super::interactive::cancel_interactive_move(state);
        }
    } else if super::target::surface_interactive_move_drop(state, DragSurfaceId::LeftSidebar, pos) {
        // Sidebar drop handled.
    } else if let Some(hint) = state.mouse.insert_hint.take() {
        // Move mode: pane is still in layout. Remove it and
        // re-insert at the drop target position.
        super::interactive::reset_interactive_move_offset(state);
        handle_content_move(state, source_id, hint);
    } else {
        super::interactive::cancel_interactive_move(state);
    }

    state.mouse.interactive_move = None;
    state.mouse.insert_hint = None;
}

/// Handle release during an active sidebar drag.
pub(super) fn handle_sidebar_drag_release(
    state: &mut AppState,
    pane_id: PaneId,
    original_ws: usize,
    swap: bool,
    pos: (f32, f32),
) {
    super::target::surface_accept_drop(state, DragSurfaceId::LeftSidebar, pane_id, original_ws, swap, pos);
}

/// Handle release during an active sidebar **column** drag (F4.5 step 2).
///
/// Resolves the drop target (source-aware: a column drag only hits columns /
/// workspaces), then dispatches the matching action:
/// - onto another **column** → `MoveColumn` (Before/After = which side), or
///   `SwapColumns` when Shift is held;
/// - onto a **workspace** → `MoveColumn` to the end of that workspace.
///
/// Resolves BEFORE cancelling — the source-aware filter reads the live payload.
pub(super) fn handle_sidebar_column_drag_release(
    state: &mut AppState,
    src_ws: usize,
    src_col: usize,
    swap: bool,
    pos: (f32, f32),
) {
    let target = crate::chrome::sidebar_drop_target(state, pos);
    state.mouse.drag_ctx.cancel_all();

    let Some((item, side)) = target else {
        return;
    };

    let action = match item {
        ChromeDragItem::Column { ws: dst_ws, col: dst_col } => {
            if swap {
                if dst_ws == src_ws && dst_col == src_col {
                    return; // swap with self
                }
                WmAction::SwapColumns { a_ws: src_ws, a_col: src_col, b_ws: dst_ws, b_col: dst_col }
            } else {
                if dst_ws == src_ws && dst_col == src_col {
                    return; // dropped on itself
                }
                let before = side == DropSide::Before; // Onto/After both insert after
                let dst_idx = column_move_dst_idx(src_ws, src_col, dst_ws, dst_col, before);
                WmAction::MoveColumn { src_ws, src_col, dst_ws, dst_idx, focus: true }
            }
        }
        // Drop on a workspace → move the column to the end of that workspace
        // (usize::MAX clamps to the end inside the handler). Swap is meaningless here.
        ChromeDragItem::Workspace { ws: dst_ws } => {
            WmAction::MoveColumn { src_ws, src_col, dst_ws, dst_idx: usize::MAX, focus: true }
        }
        // The source-aware filter never yields a pane target for a column drag.
        ChromeDragItem::Pane(_) => return,
    };

    match &action {
        WmAction::SwapColumns { .. } => crate::handlers::handle_swap_columns(state, &action),
        _ => crate::handlers::handle_move_column(state, &action),
    }
    crate::app::mutations::after_layout_change(state);
}

/// Final insert index for a column **move** onto target column `dst_col` in `dst_ws`,
/// given a `before` (vs after) drop side. Within the same workspace the source column
/// is removed first, so the target's index shifts left by one when the source sat to
/// its left — `MoveColumn`'s within-ws path (`reorder_column`) inserts at this final
/// index. Cross-workspace leaves `dst_ws` untouched, so it's a plain before/after.
/// Assumes `(src_ws, src_col) != (dst_ws, dst_col)` (self-drops are handled earlier).
fn column_move_dst_idx(
    src_ws: usize,
    src_col: usize,
    dst_ws: usize,
    dst_col: usize,
    before: bool,
) -> usize {
    if src_ws == dst_ws {
        let target_final = if src_col < dst_col { dst_col - 1 } else { dst_col };
        if before { target_final } else { target_final + 1 }
    } else if before {
        dst_col
    } else {
        dst_col + 1
    }
}

/// Handle release during sidebar drag starting (threshold not exceeded).
///
/// If a pending click action was stored, dispatch it. Otherwise, just clear the drag state.
pub(super) fn handle_sidebar_drag_starting_release(state: &mut AppState) -> Option<(WmAction, InteractionSource)> {
    let click_action = state.mouse.pending_click_action.take();
    state.mouse.drag_ctx.cancel_all();

    if let Some(action) = click_action {
        if matches!(action, WmAction::FocusPane { .. })
            && matches!(state.input_mode, crate::app_state::InputMode::SidebarNav)
        {
            state.input_mode = crate::app_state::InputMode::Normal;
        }
        return Some((action, InteractionSource::MouseLeftSidebar));
    }
    None
}

// ── Internal helpers ─────────────────────────────────────────────────────

/// Move a pane from its current position and re-insert at the given insert hint.
fn handle_content_move(
    state: &mut AppState,
    source_id: PaneId,
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
                crate::chrome::default_column_width(),
                true,
            );
        }
    }
    state.focused_pane = Some(source_id);
    crate::app::mutations::after_layout_change(state);
}
#[cfg(test)]
mod tests {
    use super::column_move_dst_idx;

    #[test]
    fn within_ws_before_after_account_for_source_removal_shift() {
        // [A,B,C,D] (src=A@0). "Before C@2" → A lands at index 1 → [B,A,C,D].
        assert_eq!(column_move_dst_idx(0, 0, 0, 2, true), 1);
        // "After C@2" → A lands at index 2 → [B,C,A,D].
        assert_eq!(column_move_dst_idx(0, 0, 0, 2, false), 2);
        // src to the RIGHT of target: drag D@3 "Before B@1" → index 1 (no left shift).
        assert_eq!(column_move_dst_idx(0, 3, 0, 1, true), 1);
        assert_eq!(column_move_dst_idx(0, 3, 0, 1, false), 2);
    }

    #[test]
    fn cross_ws_is_a_plain_before_after_insert() {
        // Different workspace: dst is untouched by the source removal.
        assert_eq!(column_move_dst_idx(0, 1, 1, 2, true), 2);
        assert_eq!(column_move_dst_idx(0, 1, 1, 2, false), 3);
    }
}
