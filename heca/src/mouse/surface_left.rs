//! Left sidebar drag surface implementation.
//!
//! Implements all surface dispatch functions for `DragSurfaceId::LeftSidebar`:
//! hit testing, click routing, hover updates, and drop acceptance.
//!
//! This module absorbs the former `sidebar.rs` (click routing) and
//! `sidebar_drop.rs` (drop logic) into a single surface handler.

use crate::app::pane_ops::{insert_pane_at_position, remove_pane_by_id};
use crate::app_state::{AppState, InteractiveMovePhase};
use crate::chrome::DEFAULT_COLLAPSED_SIDEBAR_WIDTH;
use crate::input::WmAction;
use heca_core::layout::types::Point;
use heca_core::layout::{ColumnId, ColumnWidth};
use heca_grid_ui::drag::{DragItemId, DragSurfaceId};

// ═══════════════════════════════════════════════════════════════════════════════
//  Geometry
// ═══════════════════════════════════════════════════════════════════════════════

/// Returns true if the cursor is within the left sidebar bounds.
pub(crate) fn contains(state: &AppState, pos: (f32, f32)) -> bool {
    let (sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
    pos.0 >= 0.0 && pos.0 <= sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom
}

/// Returns the flat index of the sidebar item at the cursor position.
pub(crate) fn item_at(state: &AppState, pos: (f32, f32)) -> Option<DragItemId> {
    let (sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
    let sidebar_h = sidebar_bottom - sidebar_top;
    crate::sidebar::sidebar_hit_test(&state.sidebar_tree, sidebar_top, sidebar_h, sw, pos.1)
        .map(DragItemId::new)
}

/// Helper: compute sidebar geometry (width, top, bottom).
fn sidebar_bounds(state: &AppState) -> (f32, f32, f32) {
    let chrome = super::chrome_config(state);
    let (_win_w, win_h) = super::window_logical_size(state);
    let sw = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        DEFAULT_COLLAPSED_SIDEBAR_WIDTH
    };
    (sw, chrome.tab_bar_height, win_h - chrome.status_bar_height)
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Click routing
// ═══════════════════════════════════════════════════════════════════════════════

/// Handle a click (press+release without drag) in the left sidebar.
///
/// Routes to button clicks, workspace switches, or pane focus actions.
pub(crate) fn click_action(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    let (sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
    let was_sidebar_nav = matches!(state.input_mode, crate::app_state::InputMode::SidebarNav);

    if pos.0 >= 0.0 && pos.0 <= sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom {
        // Button hits (close, delete) take priority.
        if let Some((btn_idx, button)) =
            crate::sidebar::sidebar_button_hit_test(&state.sidebar_tree, pos.0, pos.1)
        {
            let is_delete = matches!(
                &button,
                WmAction::DeleteWorkspace { .. } | WmAction::DeleteColumn { .. }
            );
            if is_delete {
                let message = match &button {
                    WmAction::DeleteWorkspace { ws_idx } => {
                        let ws_label = if let Some(ws) = state.session.workspaces.get(*ws_idx)
                            && let Some(ref name) = ws.name
                        {
                            name.clone()
                        } else {
                            format!("workspace {}", ws_idx + 1)
                        };
                        format!("Delete {}? (y/n)", ws_label)
                    }
                    WmAction::DeleteColumn { ws_idx, col_idx } => {
                        let ws_label = if let Some(ws) = state.session.workspaces.get(*ws_idx)
                            && let Some(ref name) = ws.name
                        {
                            name.clone()
                        } else {
                            format!("ws {}", ws_idx + 1)
                        };
                        format!("Delete column {} from {}? (y/n)", col_idx + 1, ws_label)
                    }
                    _ => unreachable!("DeleteWorkspace/DeleteColumn guaranteed by is_delete check"),
                };
                state.input_mode = crate::app_state::InputMode::ConfirmDelete {
                    message,
                    action: Box::new(button),
                    resume_sidebar: true,
                };
                return None;
            }

            state.input_mode = crate::app_state::InputMode::SidebarNav;
            if let Some(hitbox) = state.sidebar_tree.button_hitboxes.get(btn_idx)
                && let Some(ws_idx) = hitbox.ws_idx
                && ws_idx != state.session.active_workspace_idx
            {
                crate::switch_workspace_tracked(state, ws_idx);
            }
            return Some(button);
        }

        // Non-button item hits: pane, workspace, column.
        let sidebar_h = sidebar_bottom - sidebar_top;
        if let Some(fi) =
            crate::sidebar::sidebar_hit_test(&state.sidebar_tree, sidebar_top, sidebar_h, sw, pos.1)
        {
            state.sidebar_tree.cursor = fi;
            state.input_mode = crate::app_state::InputMode::SidebarNav;
            let item = state.sidebar_tree.current_item().cloned();
            match item? {
                crate::sidebar::SidebarItem::Pane { pane_id } => {
                    if was_sidebar_nav {
                        return Some(WmAction::FocusPane { pane_id });
                    }
                }
                crate::sidebar::SidebarItem::FloatingPane { pane_id, .. } => {
                    if was_sidebar_nav {
                        return Some(WmAction::FocusPane { pane_id });
                    }
                }
                crate::sidebar::SidebarItem::Workspace { ws_idx } => {
                    return Some(WmAction::FocusWorkspace { ws_idx });
                }
                crate::sidebar::SidebarItem::Column { ws_idx, .. } => {
                    if ws_idx != state.session.active_workspace_idx {
                        return Some(WmAction::FocusWorkspace { ws_idx });
                    }
                }
            }
        }
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Hover
// ═══════════════════════════════════════════════════════════════════════════════

/// Update the hover item for the left sidebar during a drag.
pub(crate) fn update_hover(state: &mut AppState) {
    let (sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
    let pos = state.mouse.pos;
    let left = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar)
        .expect("LeftSidebar pre-populated in DragContext::default");
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

// ═══════════════════════════════════════════════════════════════════════════════
//  Accept / Drop
// ═══════════════════════════════════════════════════════════════════════════════

/// Check if the left sidebar can accept a drop of the given source pane
/// onto the target item (by flat index).
pub(crate) fn can_accept(state: &AppState, _source_pane_id: u64, target_fi: usize, _swap: bool) -> bool {
    // Floating panes cannot be drop targets (they float over the content area).
    !matches!(
        state.sidebar_tree.flat_items.get(target_fi),
        Some(&crate::sidebar::SidebarItem::FloatingPane { .. })
    )
}

/// Execute a drop during sidebar drag: move or swap the pane.
///
/// Ported from the former `sidebar_drop::drag_drop()`.
pub(crate) fn accept_drop(
    state: &mut AppState,
    pane_id: u64,
    original_ws: usize,
    swap: bool,
    pos: (f32, f32),
) {
    // Clear all drag state first.
    state.mouse.drag_ctx.cancel_all();
    state.mouse.interactive_move = None;
    if let Some(s) = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar) {
        s.hover_item = None;
        s.source_item = None;
        s.ghost_label = None;
    }

    if swap {
        let (sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
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

    let (sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
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
                place_pane_at_sidebar_target(state, original_ws, removed_pane, item);
            } else {
                state.session.add_pane(removed_pane, None, true);
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

/// Place a pane at a sidebar target location.
fn place_pane_at_sidebar_target(
    state: &mut AppState,
    original_ws: usize,
    pane: heca_core::layout::Pane,
    item: crate::sidebar::SidebarItem,
) {
    match item {
        crate::sidebar::SidebarItem::Pane { pane_id: target_pid } => {
            if let Some((t_ws, t_col, t_pi)) =
                crate::find_pane_location(&state.session, target_pid)
            {
                let new_col_id = ColumnId(state.session.next_id());
                let position = heca_core::layout::types::PaneInsertTarget::InColumn {
                    col_idx: t_col,
                    pane_idx: t_pi + 1,
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
            } else {
                state.session.add_pane(pane, None, true);
            }
            state.focused_pane = Some(target_pid);
        }
        crate::sidebar::SidebarItem::Workspace { ws_idx } => {
            if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                let width = state
                    .session
                    .options
                    .default_column_width
                    .unwrap_or(ColumnWidth::Proportion(0.85));
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
                .unwrap_or(ColumnWidth::Proportion(0.85));
            if let Some(ws) = state.session.workspaces.get_mut(original_ws) {
                ws.add_pane(pane, None, true, width);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Interactive-move drop (content-area drag → sidebar target)
// ═══════════════════════════════════════════════════════════════════════════════

/// Handle a drop during interactive move where the cursor is over the sidebar.
///
/// Returns true if the drop was handled (sidebar target found).
/// Ported from the former `sidebar_drop::handle_drop()`.
pub(crate) fn handle_interactive_move_drop(state: &mut AppState, pos: (f32, f32)) -> bool {
    let (sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
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

    // Source pane is in the layout. Get its ID from the drag state.
    let source_id = match state.mouse.interactive_move {
        Some(InteractiveMovePhase::Moving { _pane_id, .. }) => _pane_id,
        _ => return false,
    };

    // Find source position before removal.
    let source_loc = crate::find_pane_location(&state.session, source_id);
    let original_ws = state.session.active_workspace_idx;

    // Reset drag offset so layout positions are correct for removal.
    crate::mouse::interactive::reset_interactive_move_offset(state);

    // Remove the pane from its current position.
    let removed_pane = match state.session.workspaces.get_mut(original_ws) {
        Some(ws) => remove_pane_by_id(ws, source_id),
        None => return false,
    };
    let Some(removed) = removed_pane else {
        return false;
    };
    let pane = removed.pane;

    // Re-derive source column for reinsertion in swap case.
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
                // Swap: remove target pane, insert source at target, re-insert target at source.
                if let Some((t_ws, t_col, t_pi)) =
                    crate::find_pane_location(&state.session, target_pid)
                {
                    let removed_target = state.session.workspaces[t_ws]
                        .scrolling
                        .remove_pane(t_col, t_pi);

                    if let Some(removed_target) = removed_target {
                        // Insert source pane at target position.
                        let position = if t_col < state.session.workspaces[t_ws].scrolling.columns.len() {
                            heca_core::layout::types::PaneInsertTarget::InColumn { col_idx: t_col, pane_idx: t_pi }
                        } else {
                            heca_core::layout::types::PaneInsertTarget::NewColumn(t_col)
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
                                    heca_core::layout::types::PaneInsertTarget::InColumn { col_idx: src_col_idx, pane_idx: src_pi },
                                    new_col_id2,
                                    ColumnWidth::Proportion(0.5),
                                    true,
                                );
                            } else {
                                let width = state
                                    .session
                                    .options
                                    .default_column_width
                                    .unwrap_or(ColumnWidth::Proportion(0.85));
                                ws.add_pane(removed_target, None, true, width);
                            }
                        } else {
                            state.session.add_pane(removed_target, None, true);
                        }
                    } else {
                        place_pane_at_sidebar_target(state, original_ws, pane, crate::sidebar::SidebarItem::Pane { pane_id: target_pid });
                    }
                } else {
                    place_pane_at_sidebar_target(state, original_ws, pane, crate::sidebar::SidebarItem::Pane { pane_id: target_pid });
                }
            } else {
                // Move: insert source pane after the target pane.
                if let Some((ws_idx, col_idx, pane_idx)) =
                    crate::find_pane_location(&state.session, target_pid)
                {
                    let position = heca_core::layout::types::PaneInsertTarget::InColumn {
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
                    .unwrap_or(ColumnWidth::Proportion(0.85));
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
                .unwrap_or(ColumnWidth::Proportion(0.85));
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