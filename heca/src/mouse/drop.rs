//! Mouse drop and pane placement helpers.
//!
//! This module owns detached-pane reinsertion, sidebar drop targeting, and
//! sidebar-drag move/swap behavior.

use crate::app_state::{AppState, DragState};
use heca_core::layout::types::{InsertPosition, Point};
use heca_core::layout::{Column, ColumnId, ColumnWidth};

/// Handle a drop during sidebar drag.
/// Removes the pane from its original position and inserts it at the target
/// (sidebar item or content area), with animation from original to new position.
/// If `swap` is true, performs a swap instead of a move.
pub(super) fn sidebar_drag_drop(
    state: &mut AppState,
    pane_id: u64,
    original_ws: usize,
    swap: bool,
    pos: (f32, f32),
) {
    state.mouse.drag_state = DragState::None;
    state.mouse.drag_hover_sidebar_fi = None;
    state.mouse.sidebar_drag_source_fi = None;
    state.mouse.sidebar_drag_label = None;

    if swap {
        let (_win_w, win_h) = super::window_logical_size(state);
        let chrome = super::chrome_config(state);
        let sidebar_top = chrome.tab_bar_height;
        let sidebar_bottom = win_h - chrome.status_bar_height;
        let sw = if state.sidebar.left_visible {
            chrome.left_sidebar_width
        } else {
            40.0
        };

        if pos.0 >= 0.0 && pos.0 <= sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom {
            let sidebar_h = sidebar_bottom - sidebar_top;
            if let Some(fi) = crate::sidebar::sidebar_hit_test(
                &state.sidebar_tree,
                sidebar_top,
                sidebar_h,
                sw,
                pos.1,
            ) && let Some(item) = state.sidebar_tree.flat_items.get(fi).cloned()
                && let crate::sidebar::SidebarItem::Pane {
                    pane_id: target_pid,
                } = item
            {
                crate::handlers::handle_swap_param(
                    state,
                    &crate::input::WmAction::Swap {
                        a_id: pane_id,
                        b_id: target_pid,
                    },
                );
                crate::sync_focus(state);
                state.needs_redraw = true;
                return;
            }
        }
    }

    let old_rect = state.session.workspaces.get(original_ws).and_then(|ws| {
        ws.scrolling
            .panes_with_positions()
            .into_iter()
            .find(|(pid, _)| *pid == heca_core::layout::PaneId(pane_id))
            .map(|(_, r)| r)
    });

    let removed_pane = {
        let ws = match state.session.workspaces.get_mut(original_ws) {
            Some(ws) => ws,
            None => return,
        };
        let mut found = None;
        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
            if let Some(pi) = col.panes.iter().position(|p| p.id.0 == pane_id) {
                found = Some((ci, pi));
                break;
            }
        }
        match found {
            Some((ci, pi)) => ws.scrolling.remove_pane(ci, pi),
            None => return,
        }
    };

    let Some(removed_pane) = removed_pane else {
        return;
    };

    let (_win_w, win_h) = super::window_logical_size(state);
    let chrome = super::chrome_config(state);
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;
    let sw = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        40.0
    };

    let on_sidebar = pos.0 >= 0.0 && pos.0 <= sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom;

    if on_sidebar {
        let sidebar_h = sidebar_bottom - sidebar_top;
        let fi = crate::sidebar::sidebar_hit_test(
            &state.sidebar_tree,
            sidebar_top,
            sidebar_h,
            sw,
            pos.1,
        );

        if let Some(fi) = fi {
            if let Some(item) = state.sidebar_tree.flat_items.get(fi).cloned() {
                match item {
                    crate::sidebar::SidebarItem::Pane {
                        pane_id: target_pid,
                    } => {
                        if let Some((t_ws, t_col, t_pi)) =
                            crate::find_pane_location(&state.session, target_pid)
                        {
                            if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                                let insert_idx =
                                    (t_pi + 1).min(ws.scrolling.columns[t_col].panes.len());
                                ws.scrolling.add_pane_to_column(
                                    t_col,
                                    Some(insert_idx),
                                    removed_pane,
                                    true,
                                );
                            }
                        } else {
                            state.session.add_pane(removed_pane, None, true);
                        }
                        state.focused_pane = Some(target_pid);
                    }
                    crate::sidebar::SidebarItem::Workspace { ws_idx } => {
                        if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                            let width = state
                                .session
                                .options
                                .default_column_width
                                .unwrap_or(heca_core::layout::ColumnWidth::Proportion(0.85));
                            ws.add_pane(removed_pane, None, true, width);
                        }
                    }
                    crate::sidebar::SidebarItem::Column { ws_idx, col_idx } => {
                        if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                            let target_col =
                                col_idx.min(ws.scrolling.columns.len().saturating_sub(1));
                            ws.scrolling
                                .add_pane_to_column(target_col, None, removed_pane, true);
                        }
                    }
                    crate::sidebar::SidebarItem::FloatingPane { .. } => {
                        let width = state
                            .session
                            .options
                            .default_column_width
                            .unwrap_or(heca_core::layout::ColumnWidth::Proportion(0.85));
                        if let Some(ws) = state.session.workspaces.get_mut(original_ws) {
                            ws.add_pane(removed_pane, None, true, width);
                        }
                    }
                }
            }
        } else {
            state.session.add_pane(removed_pane, None, true);
        }
    } else {
        state.session.add_pane(removed_pane, None, true);
    }

    let new_ws = state.session.active_workspace_idx;
    if let Some(old_rect) = old_rect
        && let Some((_, new_rect)) = state.session.workspaces.get(new_ws).and_then(|ws| {
            ws.scrolling
                .panes_with_positions()
                .into_iter()
                .find(|(pid, _)| *pid == heca_core::layout::PaneId(pane_id))
        })
    {
        let dx = old_rect.loc.x - new_rect.loc.x;
        let dy = old_rect.loc.y - new_rect.loc.y;
        if let Some(ws) = state.session.workspaces.get_mut(new_ws) {
            for col in &mut ws.scrolling.columns {
                for pane in &mut col.panes {
                    if pane.id.0 == pane_id {
                        pane.animate_move_from(
                            Point::new(dx, dy),
                            heca_core::layout::animation::AnimationConfig::default(),
                        );
                        break;
                    }
                }
            }
        }
    }

    crate::sync_focus(state);
    state.needs_redraw = true;
}

/// Handle a drop on the sidebar during interactive move.
/// Returns true if the drop was handled (pane placed in target workspace/column).
/// The pane is detached from layout, so we insert it directly into the target.
pub(super) fn sidebar_handle_drop(state: &mut AppState, pos: (f32, f32)) -> bool {
    let (_win_w, win_h) = super::window_logical_size(state);
    let chrome = super::chrome_config(state);
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;

    let sw = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        40.0
    };

    if pos.0 < 0.0 || pos.0 >= sw || pos.1 < sidebar_top || pos.1 >= sidebar_bottom {
        return false;
    }

    let sidebar_h = sidebar_bottom - sidebar_top;
    let fi = match crate::sidebar::sidebar_hit_test(
        &state.sidebar_tree,
        sidebar_top,
        sidebar_h,
        sw,
        pos.1,
    ) {
        Some(fi) => fi,
        None => return false,
    };

    let item = match state.sidebar_tree.flat_items.get(fi).cloned() {
        Some(item) => item,
        None => return false,
    };

    let det = match state.mouse.detached_pane.take() {
        Some(d) => d,
        None => {
            state.mouse.drag_state = DragState::None;
            state.mouse.drag_hover_sidebar_fi = None;
            state.mouse.sidebar_drag_source_fi = None;
            state.mouse.sidebar_drag_label = None;
            return true;
        }
    };

    let new_col_detached = ColumnId(state.session.next_id());
    let new_col_removed = ColumnId(state.session.next_id());

    match item {
        crate::sidebar::SidebarItem::Pane { pane_id } => {
            let shift_held = state.modifiers.shift_key();

            if shift_held {
                if let Some((t_ws, t_col, t_pi)) =
                    crate::find_pane_location(&state.session, pane_id)
                {
                    if t_ws < state.session.workspaces.len() {
                        let removed_target = state.session.workspaces[t_ws]
                            .scrolling
                            .remove_pane(t_col, t_pi);

                        if let Some(removed_target) = removed_target {
                            if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                                if t_col < ws.scrolling.columns.len() {
                                    ws.scrolling.add_pane_to_column(
                                        t_col,
                                        Some(t_pi),
                                        det.pane,
                                        true,
                                    );
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
                            state.session.add_pane(det.pane, None, true);
                        }
                    } else {
                        state.session.add_pane(det.pane, None, true);
                    }
                } else {
                    state.session.add_pane(det.pane, None, true);
                }
            } else {
                let target = {
                    let mut found = None;
                    for (wi, ws) in state.session.workspaces.iter().enumerate() {
                        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
                            for (pi, pane) in col.panes.iter().enumerate() {
                                if pane.id.0 == pane_id {
                                    found = Some((wi, ci, pi));
                                }
                            }
                        }
                    }
                    found
                };
                if let Some((ws_idx, col_idx, pane_idx)) = target {
                    if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                        let insert_idx =
                            (pane_idx + 1).min(ws.scrolling.columns[col_idx].panes.len());
                        ws.scrolling
                            .add_pane_to_column(col_idx, Some(insert_idx), det.pane, true);
                    }
                } else {
                    state.session.add_pane(det.pane, None, true);
                }
            }
            state.focused_pane = Some(pane_id);
        }
        crate::sidebar::SidebarItem::Workspace { ws_idx } => {
            if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                let width = state
                    .session
                    .options
                    .default_column_width
                    .unwrap_or(heca_core::layout::ColumnWidth::Proportion(0.85));
                ws.add_pane(det.pane, None, true, width);
            }
        }
        crate::sidebar::SidebarItem::Column { ws_idx, col_idx } => {
            if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                let target_col = col_idx.min(ws.scrolling.columns.len().saturating_sub(1));
                ws.scrolling
                    .add_pane_to_column(target_col, None, det.pane, true);
            }
        }
        crate::sidebar::SidebarItem::FloatingPane { .. } => {
            let width = state
                .session
                .options
                .default_column_width
                .unwrap_or(heca_core::layout::ColumnWidth::Proportion(0.85));
            if let Some(ws) = state.session.workspaces.get_mut(det.original_ws) {
                ws.add_pane(det.pane, None, true, width);
            }
        }
    }

    state.mouse.drag_state = DragState::None;
    state.mouse.insert_hint = None;
    state.mouse.drag_hover_sidebar_fi = None;
    state.needs_redraw = true;
    crate::sync_focus(state);
    true
}

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
    crate::sync_focus(state);
    state.needs_redraw = true;
}
