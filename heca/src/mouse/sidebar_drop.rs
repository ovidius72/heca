//! Mouse sidebar drag/drop helpers.
//!
//! This module owns sidebar-targeted drop and sidebar-drag move/swap behavior.

use crate::app::pane_ops::{insert_pane_at_position, remove_pane_by_id};
use crate::app_state::{AppState, InteractiveMovePhase};
use crate::chrome::DEFAULT_COLLAPSED_SIDEBAR_WIDTH;
use heca_grid_ui::drag::DragSurfaceId;
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
    state.mouse.drag_ctx.cancel_all();
    state.mouse.interactive_move = None;
    if let Some(s) = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar) {
        s.hover_item = None;
        s.source_item = None;
        s.ghost_label = None;
    }

    if swap {
        let (_win_w, win_h) = super::window_logical_size(state);
        let chrome = super::chrome_config(state);
        let sidebar_top = chrome.tab_bar_height;
        let sidebar_bottom = win_h - chrome.status_bar_height;
        let sw = if state.sidebar.left_visible {
            chrome.left_sidebar_width
        } else {
            DEFAULT_COLLAPSED_SIDEBAR_WIDTH
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
        DEFAULT_COLLAPSED_SIDEBAR_WIDTH
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
/// In the new model, the pane stays in the layout (no detach). We remove it
/// from its current position and re-insert at the sidebar target.
pub(super) fn handle_drop(state: &mut AppState, pos: (f32, f32)) -> bool {
    let (_win_w, win_h) = super::window_logical_size(state);
    let chrome = super::chrome_config(state);
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;

    let sw = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        DEFAULT_COLLAPSED_SIDEBAR_WIDTH
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

    // Source pane is in the layout. Get its ID from the drag state
    // and find its current position before removing it.
    let source_id = match state.mouse.interactive_move {
        Some(InteractiveMovePhase::Moving { _pane_id, .. }) => _pane_id,
        _ => return false,
    };

    // Find source position before removal.
    let source_loc = crate::find_pane_location(&state.session, source_id);
    let original_ws = state.session.active_workspace_idx;

    // Reset drag offset so layout positions are correct for removal.
    crate::mouse::drag::reset_interactive_move_offset(state);

    // Remove the pane from its current position.
    let removed_pane = match state.session.workspaces.get_mut(original_ws) {
        Some(ws) => remove_pane_by_id(ws, source_id),
        None => return false,
    };
    let Some(removed) = removed_pane else {
        return false;
    };
    let pane = removed.pane;

    // Re-derive source column id for reinsertion in swap case.
    let source_col_id = source_loc.and_then(|(ws_idx, col_idx, _)| {
        state.session.workspaces.get(ws_idx)
            .and_then(|ws| ws.scrolling.columns.get(col_idx).map(|c| c.id))
    });
    let source_pane_idx = source_loc.map(|(_, _, pi)| pi);

    let shift_held = state.modifiers.shift_key();
    let new_col_id = ColumnId(state.session.next_id());

    match item {
        crate::sidebar::SidebarItem::Pane { pane_id: target_pid } => {
            if shift_held {
                // Swap: remove target pane too, insert source at target, re-insert target at source.
                if let Some((t_ws, t_col, t_pi)) =
                    crate::find_pane_location(&state.session, target_pid)
                {
                    let removed_target = state.session.workspaces[t_ws]
                        .scrolling
                        .remove_pane(t_col, t_pi);

                    if let Some(removed_target) = removed_target {
                        // Insert source pane at target position.
                        let position = if t_col < state.session.workspaces[t_ws].scrolling.columns.len() {
                            PaneInsertTarget::InColumn { col_idx: t_col, pane_idx: t_pi }
                        } else {
                            PaneInsertTarget::NewColumn(t_col)
                        };
                        if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                            let _ = insert_pane_at_position(
                                ws,
                                pane,
                                position,
                                new_col_id,
                                ColumnWidth::Proportion(0.5),
                                true,
                            );
                        }

                        // Re-insert target pane at source position.
                        let new_col_id2 = ColumnId(state.session.next_id());
                        if let Some(ws) = state.session.workspaces.get_mut(original_ws) {
                            if let Some(src_col_idx) = ws.scrolling.columns.iter().position(|c| Some(&c.id) == source_col_id.as_ref()) {
                                let src_pi = source_pane_idx.unwrap_or(0).min(ws.scrolling.columns[src_col_idx].panes.len());
                                let _ = insert_pane_at_position(
                                    ws,
                                    removed_target,
                                    PaneInsertTarget::InColumn { col_idx: src_col_idx, pane_idx: src_pi },
                                    new_col_id2,
                                    ColumnWidth::Proportion(0.5),
                                    true,
                                );
                            } else {
                                let width = state
                                    .session
                                    .options
                                    .default_column_width
                                    .unwrap_or(heca_core::layout::ColumnWidth::Proportion(0.85));
                                ws.add_pane(removed_target, None, true, width);
                            }
                        } else {
                            state.session.add_pane(removed_target, None, true);
                        }
                    } else {
                        place_pane_at_sidebar_target(state, original_ws, pane, &item);
                    }
                } else {
                    place_pane_at_sidebar_target(state, original_ws, pane, &item);
                }
            } else {
                // Move: insert source pane after the target pane.
                if let Some((ws_idx, col_idx, pane_idx)) =
                    crate::find_pane_location(&state.session, target_pid)
                {
                    let position = PaneInsertTarget::InColumn {
                        col_idx,
                        pane_idx: pane_idx + 1,
                    };
                    let col_id = ColumnId(state.session.next_id());
                    if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                        let _ = insert_pane_at_position(
                            ws,
                            pane,
                            position,
                            col_id,
                            ColumnWidth::Proportion(0.5),
                            true,
                        );
                    }
                } else {
                    state.session.add_pane(pane, None, true);
                }
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
                ws.add_pane(pane, None, true, width);
            }
        }
        crate::sidebar::SidebarItem::Column { ws_idx, col_idx } => {
            if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                let target_col = col_idx.min(ws.scrolling.columns.len().saturating_sub(1));
                ws.scrolling
                    .add_pane_to_column(target_col, None, pane, true);
            }
        }
        crate::sidebar::SidebarItem::FloatingPane { .. } => {
            let width = state
                .session
                .options
                .default_column_width
                .unwrap_or(heca_core::layout::ColumnWidth::Proportion(0.85));
            if let Some(ws) = state.session.workspaces.get_mut(original_ws) {
                ws.add_pane(pane, None, true, width);
            }
        }
    }

    state.mouse.drag_ctx.cancel_all();
    state.mouse.interactive_move = None;
    state.mouse.insert_hint = None;
    if let Some(s) = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar) {
        s.hover_item = None;
    }
    crate::app::mutations::after_layout_change(state);
    true
}

/// Place a pane at a sidebar target location (workspace, column, or pane item).
fn place_pane_at_sidebar_target(
    state: &mut AppState,
    _original_ws: usize,
    pane: heca_core::layout::Pane,
    item: &crate::sidebar::SidebarItem,
) {
    match item {
        crate::sidebar::SidebarItem::Workspace { ws_idx } => {
            if let Some(ws) = state.session.workspaces.get_mut(*ws_idx) {
                let width = state
                    .session
                    .options
                    .default_column_width
                    .unwrap_or(heca_core::layout::ColumnWidth::Proportion(0.85));
                ws.add_pane(pane, None, true, width);
            }
        }
        crate::sidebar::SidebarItem::Column { ws_idx, col_idx } => {
            if let Some(ws) = state.session.workspaces.get_mut(*ws_idx) {
                let target_col = (*col_idx).min(ws.scrolling.columns.len() - 1);
                ws.scrolling.add_pane_to_column(target_col, None, pane, true);
            }
        }
        _ => {
            state.session.add_pane(pane, None, true);
        }
    }
}
