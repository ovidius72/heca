//! Mouse sidebar drag/drop helpers.
//!
//! This module owns sidebar-targeted drop and sidebar-drag move/swap behavior.

use crate::app::pane_ops::{insert_pane_at_position, remove_pane_by_id};
use crate::app_state::{AppState, DragState};
use crate::input::WmAction;
use heca_core::layout::types::{PaneInsertTarget, Point};
use heca_core::layout::{ColumnId, ColumnWidth};

/// Handle a drop during sidebar drag.
/// Removes the pane from its original position and inserts it at the target
/// (sidebar item or content area), with animation from original to new position.
/// If `swap` is true, performs a swap instead of a move.
pub(super) fn drag_drop(
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
                    &WmAction::Swap {
                        a_id: pane_id,
                        b_id: target_pid,
                    },
                );
                crate::app::mutations::after_layout_change(state);
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

    let removed_pane = match state.session.workspaces.get_mut(original_ws) {
        Some(ws) => remove_pane_by_id(ws, pane_id),
        None => return,
    };
    let Some(removed) = removed_pane else {
        return;
    };
    let removed_pane = removed.pane;

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
                            let new_col_id = ColumnId(state.session.next_id());
                            let position = PaneInsertTarget::InColumn {
                                col_idx: t_col,
                                pane_idx: t_pi + 1,
                            };
                            if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                                let _ = insert_pane_at_position(
                                    ws,
                                    removed_pane,
                                    position,
                                    new_col_id,
                                    ColumnWidth::Proportion(0.5),
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

    crate::app::mutations::after_layout_change(state);
}

/// Handle a drop on the sidebar during interactive move.
/// Returns true if the drop was handled (pane placed in target workspace/column).
/// The pane is detached from layout, so we insert it directly into the target.
pub(super) fn handle_drop(state: &mut AppState, pos: (f32, f32)) -> bool {
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
                                let position = if t_col < ws.scrolling.columns.len() {
                                    PaneInsertTarget::InColumn { col_idx: t_col, pane_idx: t_pi }
                                } else {
                                    PaneInsertTarget::NewColumn(t_col)
                                };
                                let _ = insert_pane_at_position(
                                    ws,
                                    det.pane,
                                    position,
                                    new_col_detached,
                                    ColumnWidth::Proportion(0.5),
                                    true,
                                );
                            } else {
                                state.session.add_pane(det.pane, None, true);
                            }

                            let orig_ws = det.original_ws;
                            if let Some(ws) = state.session.workspaces.get_mut(orig_ws) {
                                let orig_col_idx = ws
                                    .scrolling
                                    .columns
                                    .iter()
                                    .position(|c| c.id == det.original_col_id);
                                let position = match orig_col_idx {
                                    Some(idx) => {
                                        let orig_pi = det
                                            .original_pane
                                            .min(ws.scrolling.columns[idx].panes.len());
                                        PaneInsertTarget::InColumn { col_idx: idx, pane_idx: orig_pi }
                                    }
                                    None => PaneInsertTarget::NewColumn(ws.scrolling.columns.len()),
                                };
                                let _ = insert_pane_at_position(
                                    ws,
                                    removed_target,
                                    position,
                                    new_col_removed,
                                    ColumnWidth::Proportion(0.5),
                                    true,
                                );
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
                if let Some((ws_idx, col_idx, pane_idx)) =
                    crate::find_pane_location(&state.session, pane_id)
                {
                    let position = PaneInsertTarget::InColumn {
                        col_idx,
                        pane_idx: pane_idx + 1,
                    };
                    let new_col_id = ColumnId(state.session.next_id());
                    if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                        let _ = insert_pane_at_position(
                            ws,
                            det.pane,
                            position,
                            new_col_id,
                            ColumnWidth::Proportion(0.5),
                            true,
                        );
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
    crate::app::mutations::after_layout_change(state);
    true
}
