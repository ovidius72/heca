//! Mouse drop and pane placement helpers.
//!
//! This module owns detached-pane reinsertion, sidebar drop targeting, and
//! sidebar-drag move/swap behavior.

use crate::app_state::{AppState, DragState};
use heca_core::layout::types::InsertPosition;
use heca_core::layout::{Column, ColumnId, ColumnWidth};

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
                let target_col_id = state
                    .session
                    .workspaces
                    .get(t_ws)
                    .and_then(|ws| ws.scrolling.columns.get(t_col).map(|c| c.id));

                let removed_target = state.session.workspaces[t_ws]
                    .scrolling
                    .remove_pane(t_col, t_pi);

                if let Some(removed_target) = removed_target {
                    if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                        if let Some(tc_id) = target_col_id {
                            if let Some(target_idx) =
                                ws.scrolling.columns.iter().position(|c| c.id == tc_id)
                            {
                                let insert_idx =
                                    t_pi.min(ws.scrolling.columns[target_idx].panes.len());
                                ws.scrolling.add_pane_to_column(
                                    target_idx,
                                    Some(insert_idx),
                                    det.pane,
                                    true,
                                );
                            } else {
                                ws.scrolling.add_column(
                                    None,
                                    Column::new(
                                        new_col_detached,
                                        det.pane,
                                        ColumnWidth::Proportion(0.5),
                                    ),
                                    true,
                                );
                            }
                        } else if t_col < ws.scrolling.columns.len() {
                            ws.scrolling
                                .add_pane_to_column(t_col, Some(t_pi), det.pane, true);
                        } else {
                            ws.scrolling.add_column(
                                Some(t_col),
                                Column::new(
                                    new_col_detached,
                                    det.pane,
                                    ColumnWidth::Proportion(0.5),
                                ),
                                true,
                            );
                        }
                    } else {
                        state.session.add_pane(det.pane, None, true);
                    }

                    let orig_ws = det.original_ws;
                    if let Some(ws) = state.session.workspaces.get_mut(orig_ws) {
                        if let Some(orig_idx) = ws
                            .scrolling
                            .columns
                            .iter()
                            .position(|c| c.id == det.original_col_id)
                        {
                            let orig_pi = det
                                .original_pane
                                .min(ws.scrolling.columns[orig_idx].panes.len());
                            ws.scrolling.add_pane_to_column(
                                orig_idx,
                                Some(orig_pi),
                                removed_target,
                                true,
                            );
                        } else {
                            ws.scrolling.add_column(
                                None,
                                Column::new(
                                    new_col_removed,
                                    removed_target,
                                    ColumnWidth::Proportion(0.5),
                                ),
                                true,
                            );
                        }
                    } else {
                        state.session.add_pane(removed_target, None, true);
                    }
                } else {
                    let fresh_id = state.session.next_id();
                    let fresh_id_2 = state.session.next_id();
                    if let Some(ws) = state.session.active_workspace_mut() {
                        match hint {
                            InsertPosition::NewColumn(col_idx) => {
                                let col = Column::new(
                                    ColumnId(fresh_id),
                                    det.pane,
                                    ColumnWidth::Proportion(0.5),
                                );
                                ws.scrolling.add_column(Some(col_idx), col, true);
                            }
                            InsertPosition::InColumn { col_idx, pane_idx } => {
                                if col_idx < ws.scrolling.columns.len() {
                                    ws.scrolling.add_pane_to_column(
                                        col_idx,
                                        Some(
                                            pane_idx.min(ws.scrolling.columns[col_idx].panes.len()),
                                        ),
                                        det.pane,
                                        true,
                                    );
                                } else {
                                    let col = Column::new(
                                        ColumnId(fresh_id_2),
                                        det.pane,
                                        ColumnWidth::Proportion(0.5),
                                    );
                                    ws.scrolling.add_column(None, col, true);
                                }
                            }
                        }
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
            match hint {
                InsertPosition::NewColumn(col_idx) => {
                    let col =
                        Column::new(ColumnId(fresh_id), det.pane, ColumnWidth::Proportion(0.5));
                    ws.scrolling.add_column(Some(col_idx), col, true);
                }
                InsertPosition::InColumn { col_idx, pane_idx } => {
                    if col_idx < ws.scrolling.columns.len() {
                        ws.scrolling.add_pane_to_column(
                            col_idx,
                            Some(pane_idx.min(ws.scrolling.columns[col_idx].panes.len())),
                            det.pane,
                            true,
                        );
                    } else {
                        let col =
                            Column::new(ColumnId(fresh_id), det.pane, ColumnWidth::Proportion(0.5));
                        ws.scrolling.add_column(None, col, true);
                    }
                }
            }
        }
    }

    state.mouse.drag_state = DragState::None;
    state.mouse.drag_hover_sidebar_fi = None;
    crate::app::mutations::after_layout_change(state);
}
