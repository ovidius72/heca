//! Mouse drop and pane placement helpers.
//!
//! This module owns detached-pane reinsertion, sidebar drop targeting, and
//! sidebar-drag move/swap behavior.

use crate::app::pane_ops::{insert_pane_at_position, remove_pane_by_id};
use crate::app_state::{AppState, DragState};
use heca_core::layout::types::PaneInsertTarget;
use heca_core::layout::{ColumnId, ColumnWidth};

/// Handle drop during interactive move (non-swap mode).
///
/// Currently unused — move mode now keeps the pane in layout and re-inserts
/// at the target position via the mouse.rs drop handler. Kept for potential
/// reversion or future detach-on-move behavior.
#[allow(dead_code)]
pub(super) fn drop_pane(state: &mut AppState) {
    let hint = match state.mouse.insert_hint.take() {
        Some(h) => h,
        None => {
            super::drag::cancel_interactive_move(state);
            return;
        }
    };

    let det = match state.mouse.detached_pane.take() {
        Some(d) => d,
        None => {
            state.mouse.drag_state = DragState::None;
            state.mouse.drag_hover_sidebar_fi = None;
            return;
        }
    };

    let new_col_detached = ColumnId(state.session.next_id());
    let new_col_removed = ColumnId(state.session.next_id());

    let shift_held = state.modifiers.shift_key();

    if shift_held {
        let target_pane_id = super::hit_test_pane(state, state.mouse.pos);
        if let Some(target_id) = target_pane_id {
            if let Some((t_ws, t_col, t_pi)) = crate::find_pane_location(&state.session, target_id)
            {
                let removed_target = state.session.workspaces[t_ws]
                    .scrolling
                    .remove_pane(t_col, t_pi)
                    .or_else(|| {
                        state.session.workspaces.get_mut(t_ws).and_then(|ws| {
                            remove_pane_by_id(ws, target_id).map(|removed| removed.pane)
                        })
                    });

                if let Some(removed_target) = removed_target {
                    if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                        let target_position = if t_col < ws.scrolling.columns.len() {
                            PaneInsertTarget::InColumn {
                                col_idx: t_col,
                                pane_idx: t_pi,
                            }
                        } else {
                            PaneInsertTarget::NewColumn(t_col)
                        };
                        let _ = insert_pane_at_position(
                            ws,
                            det.pane,
                            target_position,
                            new_col_detached,
                            ColumnWidth::Proportion(0.5),
                            true,
                        );
                    } else {
                        state.session.add_pane(det.pane, None, true);
                    }

                    let orig_ws = det.original_ws;
                    if let Some(ws) = state.session.workspaces.get_mut(orig_ws) {
                        let orig_position = if let Some(orig_idx) = ws
                            .scrolling
                            .columns
                            .iter()
                            .position(|c| c.id == det.original_col_id)
                        {
                            PaneInsertTarget::InColumn {
                                col_idx: orig_idx,
                                pane_idx: det.original_pane,
                            }
                        } else {
                            PaneInsertTarget::NewColumn(ws.scrolling.columns.len())
                        };
                        let _ = insert_pane_at_position(
                            ws,
                            removed_target,
                            orig_position,
                            new_col_removed,
                            ColumnWidth::Proportion(0.5),
                            true,
                        );
                    } else {
                        state.session.add_pane(removed_target, None, true);
                    }
                } else {
                    let fresh_id = state.session.next_id();
                    if let Some(ws) = state.session.active_workspace_mut() {
                        let _ = insert_pane_at_position(
                            ws,
                            det.pane,
                            hint,
                            ColumnId(fresh_id),
                            ColumnWidth::Proportion(0.5),
                            true,
                        );
                    }
                }
            } else {
                super::drag::cancel_interactive_move(state);
                state.needs_redraw = true;
                return;
            }
        } else {
            super::drag::cancel_interactive_move(state);
            state.needs_redraw = true;
            return;
        }
    } else {
        let fresh_id = state.session.next_id();
        if let Some(ws) = state.session.active_workspace_mut() {
            let _ = insert_pane_at_position(
                ws,
                det.pane,
                hint,
                ColumnId(fresh_id),
                ColumnWidth::Proportion(0.5),
                true,
            );
        }
    }

    state.mouse.drag_state = DragState::None;
    state.mouse.drag_hover_sidebar_fi = None;
    crate::app::mutations::after_layout_change(state);
}
