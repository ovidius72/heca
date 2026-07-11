//! Left sidebar drag surface implementation.
//!
//! Implements all surface dispatch functions for `DragSurfaceId::LeftSidebar`:
//! hit testing, click routing, hover updates, and drop acceptance.
//!
//! This module absorbs the former `sidebar.rs` (click routing) and
//! `sidebar_drop.rs` (drop logic) into a single surface handler.

use crate::app::pane_ops::{insert_pane_at_position, remove_pane_by_id};
use crate::app_state::{AppState, InteractiveMovePhase};
use crate::chrome::default_column_width;
use crate::input::WmAction;
use heca_core::layout::types::Point;
use heca_core::layout::{ColumnId, ColumnWidth, PaneId};
use heca_grid_ui::drag::{DragItemId, DragSurfaceId, DropSide};

// ═══════════════════════════════════════════════════════════════════════════════
//  Geometry
// ═══════════════════════════════════════════════════════════════════════════════

/// Returns true if the cursor is within the left sidebar bounds.
pub(crate) fn contains(state: &AppState, pos: (f32, f32)) -> bool {
    let (sx, sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
    pos.0 >= sx && pos.0 <= sx + sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom
}

/// Returns the flat index of the sidebar item at the cursor position.
pub(crate) fn item_at(state: &AppState, pos: (f32, f32)) -> Option<DragItemId> {
    let (_sx, _sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
    let sidebar_h = sidebar_bottom - sidebar_top;
    crate::sidebar::sidebar_hit_test(&state.sidebar_tree, sidebar_top, sidebar_h, pos.1)
        .map(DragItemId::new)
}

/// Helper: compute sidebar geometry (width, top, bottom).
fn sidebar_bounds(state: &AppState) -> (f32, f32, f32, f32) {
    let chrome = super::chrome_config(state);
    let (_win_w, win_h) = super::window_logical_size(state);
    // `left_sidebar_width` is 0 when Hidden (no icon rail), so the bounds collapse
    // to nothing and no click lands in the region — see `docs/sidebar-provider-modes.md`.
    let total_w = chrome.left_sidebar_width;
    let gap = chrome.sidebar_gap.max(0.0);
    let sx = gap.min(total_w * 0.5);
    let sw = (total_w - sx * 2.0).max(0.0);
    let sidebar_top = chrome.tab_bar_height + gap;
    let sidebar_bottom = (win_h - chrome.status_bar_height - gap).max(sidebar_top);
    (sx, sw, sidebar_top, sidebar_bottom)
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Click routing
// ═══════════════════════════════════════════════════════════════════════════════

/// Handle a click (press+release without drag) in the left sidebar.
///
/// Routes to button clicks, workspace switches, or pane focus actions.
pub(crate) fn click_action(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    let (sx, sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);

    if pos.0 >= sx && pos.0 <= sx + sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom {
        // Button hits (close, delete) take priority.
        if let Some((btn_idx, button)) =
            crate::sidebar::sidebar_button_hit_test(&state.sidebar_tree, pos.0, pos.1)
        {
            let is_delete = matches!(
                &button,
                WmAction::DeleteWorkspace { .. } | WmAction::DeleteColumn { .. }
            );
            if is_delete {
                // Centralized: confirm (per config) or delete now, message generated
                // by the chokepoint.
                crate::handlers::request_destructive(state, button, true);
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

        // The sidebar is Expanded whenever it is visible (there is no collapsed rail):
        // dispatch the press into the retained grid-ui chrome tree so widget callbacks
        // route their own intents through the app event loop.
        if state.chrome_state.left_visible() {
            crate::chrome::chrome_dispatch_press(state, pos);
        }
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Hover
// ═══════════════════════════════════════════════════════════════════════════════

/// Update the hover item for the left sidebar during a drag.
pub(crate) fn update_hover(state: &mut AppState) {
    let (sx, sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
    let pos = state.mouse.pos;
    let left = state
        .mouse
        .drag_ctx
        .surface_mut(DragSurfaceId::LeftSidebar)
        .expect("LeftSidebar pre-populated in DragContext::default");
    if pos.0 >= sx && pos.0 <= sx + sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom {
        let sidebar_h = sidebar_bottom - sidebar_top;
        let fi = crate::sidebar::sidebar_hit_test(&state.sidebar_tree, sidebar_top, sidebar_h, pos.1);
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
pub(crate) fn can_accept(
    state: &AppState,
    _source_pane_id: PaneId,
    target_fi: usize,
    _swap: bool,
) -> bool {
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
    pane_id: PaneId,
    original_ws: usize,
    swap: bool,
    pos: (f32, f32),
) {
    // Drop target + side resolved from the RETAINED chrome tree's bounds (F4.5), not
    // the legacy fixed-row geometry. The side (Before/Onto/After, from vertical thirds)
    // decides which edge of the target the source lands on. This is the PANE drop path,
    // so only pane targets count; dropping a pane onto a column/workspace falls through
    // to "re-add to the active workspace" (pane→column placement is a later enhancement).
    //
    let target =
        crate::chrome::sidebar_drop_target(state, pos, crate::chrome::DragSourceKind::Pane)
            .and_then(|(item, side)| match item {
                crate::chrome::ChromeDragItem::Pane(pid) => {
                    Some((crate::sidebar::SidebarItem::Pane { pane_id: pid }, side))
                }
                // Dropping a pane on a workspace's header/empty area moves it INTO that
                // workspace — the only way to reach an *empty* workspace (which has no pane
                // card to aim at). Columns can't be empty, so they need no pane-drop target.
                crate::chrome::ChromeDragItem::Workspace { ws } => {
                    Some((crate::sidebar::SidebarItem::Workspace { ws_idx: ws }, side))
                }
                crate::chrome::ChromeDragItem::Column { .. } => None,
            });

    // Now clear all drag state.
    state.mouse.drag_ctx.cancel_all();
    state.mouse.interactive_move = None;

    // Swap applies only to a pane-on-pane drop; a workspace target falls through to a
    // plain move into that workspace.
    if swap
        && let Some((
            crate::sidebar::SidebarItem::Pane {
                pane_id: target_pid,
            },
            _side,
        )) = &target
    {
        let target_pid = *target_pid;
        if target_pid != pane_id {
            crate::handlers::handle_swap_param(
                state,
                &WmAction::Swap {
                    a_id: pane_id,
                    b_id: target_pid,
                },
            );
            crate::app::mutations::after_layout_change(state);
        }
        return;
    }

    let old_rect = state.session.workspaces.get(original_ws).and_then(|ws| {
        ws.scrolling
            .panes_with_positions()
            .into_iter()
            .find(|(pid, _)| *pid == pane_id)
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

    match target {
        // Dropped onto a pane card → place relative to it.
        Some((
            crate::sidebar::SidebarItem::Pane {
                pane_id: target_pid,
            },
            side,
        )) if target_pid != pane_id => {
            place_pane_at_sidebar_target(
                state,
                original_ws,
                removed_pane,
                crate::sidebar::SidebarItem::Pane {
                    pane_id: target_pid,
                },
                side,
            );
        }
        // Dropped onto a workspace (its header/empty area) → move into that workspace.
        Some((item @ crate::sidebar::SidebarItem::Workspace { .. }, side)) => {
            place_pane_at_sidebar_target(state, original_ws, removed_pane, item, side);
        }
        // Off any card (or onto itself) → re-add to the active workspace.
        _ => {
            state.session.add_pane(removed_pane, None, true);
        }
    }

    let new_ws = state.session.active_workspace_idx;
    if let Some(old_rect) = old_rect
        && let Some(ws) = state.session.workspaces.get_mut(new_ws)
        && let Some((ci, pi)) = crate::app::pane_ops::find_pane_indices_in_workspace(ws, pane_id)
    {
        let new_rects = ws.scrolling.panes_with_positions();
        if let Some((_, new_rect)) = new_rects.into_iter().find(|(pid, _)| *pid == pane_id) {
            let dx = old_rect.loc.x - new_rect.loc.x;
            let dy = old_rect.loc.y - new_rect.loc.y;
            ws.scrolling.columns[ci].panes[pi].animate_move_from(
                Point::new(dx, dy),
                heca_core::layout::animation::AnimationConfig::default(),
            );
        }
    }

    crate::app::mutations::after_layout_change(state);
}

/// Place a pane at a sidebar target location.
///
/// Uses [`insert_pane_at_position`] for all target types.
fn place_pane_at_sidebar_target(
    state: &mut AppState,
    original_ws: usize,
    pane: heca_core::layout::Pane,
    item: crate::sidebar::SidebarItem,
    side: DropSide,
) {
    let default_width = state
        .session
        .options
        .default_column_width
        .unwrap_or(ColumnWidth::Proportion(0.85));

    match item {
        crate::sidebar::SidebarItem::Pane {
            pane_id: target_pid,
        } => {
            if let Some((t_ws, t_col, t_pi)) = crate::find_pane_location(&state.session, target_pid)
            {
                let new_col_id = ColumnId(state.session.next_id());
                // Before → insert above the target; Onto/After → below it.
                let pane_idx = if side == DropSide::Before {
                    t_pi
                } else {
                    t_pi + 1
                };
                let position = heca_core::layout::types::PaneInsertTarget::InColumn {
                    col_idx: t_col,
                    pane_idx,
                };
                if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                    let _ = insert_pane_at_position(
                        ws,
                        pane,
                        position,
                        new_col_id,
                        default_column_width(),
                        true,
                    );
                }
            } else {
                state.session.add_pane(pane, None, true);
            }
            state.focused_pane = Some(target_pid);
        }
        crate::sidebar::SidebarItem::Workspace { ws_idx } => {
            let new_col_id = ColumnId(state.session.next_id());
            if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                // Dropping a pane on a workspace makes a NEW column at the end (appending
                // into an existing column is what dropping on a column/pane card is for).
                // For an empty workspace this also creates its first column.
                let insert_at = ws.scrolling.columns.len();
                let _ = insert_pane_at_position(
                    ws,
                    pane,
                    heca_core::layout::types::PaneInsertTarget::NewColumn(insert_at),
                    new_col_id,
                    default_width,
                    true,
                );
            }
        }
        crate::sidebar::SidebarItem::Column { ws_idx, col_idx } => {
            let new_col_id = ColumnId(state.session.next_id());
            if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                let target_col = col_idx.min(ws.scrolling.columns.len().saturating_sub(1));
                let pane_idx = ws.scrolling.columns[target_col].panes.len();
                let _ = insert_pane_at_position(
                    ws,
                    pane,
                    heca_core::layout::types::PaneInsertTarget::InColumn {
                        col_idx: target_col,
                        pane_idx,
                    },
                    new_col_id,
                    default_width,
                    true,
                );
            }
        }
        crate::sidebar::SidebarItem::FloatingPane { .. } => {
            let new_col_id = ColumnId(state.session.next_id());
            if let Some(ws) = state.session.workspaces.get_mut(original_ws) {
                let active_col = ws.scrolling.active_column_idx;
                let pane_idx = ws
                    .scrolling
                    .columns
                    .get(active_col)
                    .map(|c| c.panes.len())
                    .unwrap_or(0);
                let _ = insert_pane_at_position(
                    ws,
                    pane,
                    heca_core::layout::types::PaneInsertTarget::InColumn {
                        col_idx: active_col,
                        pane_idx,
                    },
                    new_col_id,
                    default_width,
                    true,
                );
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
    let (sx, sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
    if pos.0 < sx || pos.0 >= sx + sw || pos.1 < sidebar_top || pos.1 >= sidebar_bottom {
        return false;
    }

    let sidebar_h = sidebar_bottom - sidebar_top;
    let fi = match crate::sidebar::sidebar_hit_test(&state.sidebar_tree, sidebar_top, sidebar_h, pos.1)
    {
        Some(fi) => fi,
        None => return false,
    };

    let item = match state.sidebar_tree.flat_items.get(fi).cloned() {
        Some(item) => item,
        None => return false,
    };

    // Source pane is in the layout. Get its ID from the drag state.
    let source_id = match state.mouse.interactive_move {
        Some(InteractiveMovePhase::Moving { pane_id, .. }) => pane_id,
        _ => return false,
    };

    let original_ws = state.session.active_workspace_idx;

    // Reset drag offset so layout positions are correct for removal.
    crate::mouse::interactive::reset_interactive_move_offset(state);

    let shift_held = state.modifiers.shift_key();

    match item {
        crate::sidebar::SidebarItem::Pane {
            pane_id: target_pid,
        } if shift_held => {
            // Swap: delegate to shared swap handler.
            // The source pane is still in the layout (interactive move keeps it there).
            crate::handlers::handle_swap_param(
                state,
                &WmAction::Swap {
                    a_id: source_id,
                    b_id: target_pid,
                },
            );
            state.focused_pane = Some(target_pid);
        }
        _ => {
            // Move: remove source pane from layout first.
            let pane = match state.session.workspaces.get_mut(original_ws) {
                Some(ws) => match remove_pane_by_id(ws, source_id) {
                    Some(r) => r.pane,
                    None => return false,
                },
                None => return false,
            };

            match item {
                crate::sidebar::SidebarItem::Pane {
                    pane_id: target_pid,
                } => {
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
                                default_column_width(),
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
