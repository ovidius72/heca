//! Left sidebar drag surface implementation.
//!
//! Implements all surface dispatch functions for `DragSurfaceId::LeftSidebar`:
//! hit testing, click routing, hover updates, and drop acceptance.
//!
//! This module absorbs the former `sidebar.rs` (click routing) and
//! `sidebar_drop.rs` (drop logic) into a single surface handler.

use crate::app::pane_ops::{insert_pane_at_position, remove_pane_by_id};
use crate::app_state::{AppState, InteractiveMovePhase};
use crate::chrome::{ChromeDragItem, default_column_width};
use crate::input::WmAction;
use heca_core::layout::types::Point;
use heca_core::layout::{ColumnId, ColumnWidth, PaneId};
use heca_grid_ui::drag::{DragSurfaceId, DropSide};

// ═══════════════════════════════════════════════════════════════════════════════
//  Geometry
// ═══════════════════════════════════════════════════════════════════════════════

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
/// The press goes into the **retained** chrome tree, whose widgets route their own intents through
/// the app event loop. Nothing here resolves a row: the tree knows where its rows are.
pub(crate) fn click_action(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    let (sx, sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);

    if pos.0 >= sx
        && pos.0 <= sx + sw
        && pos.1 >= sidebar_top
        && pos.1 <= sidebar_bottom
        // The sidebar is Expanded whenever it is visible (there is no collapsed rail).
        && state.chrome_state.left_visible()
    {
        let _ = crate::chrome::chrome_dispatch_press(state, pos);
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Hover
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
//  Accept / Drop
// ═══════════════════════════════════════════════════════════════════════════════

/// Execute a drop during sidebar drag: move or swap the pane.
///
/// Ported from the former `sidebar_drop::drag_drop()`.
pub(crate) fn accept_drop(
    state: &mut AppState,
    pane_id: PaneId,
    original_ws: usize,
    swap: bool,
    target: Option<(crate::providers::workspaces::WorkspaceRow, DropSide)>,
) {
    // **The target arrives resolved.** It used to be hunted for under the pointer here, at the
    // moment of release — which is how the drop came to be tied to one sidebar: the hunt was keyed
    // to it. The gesture now says what it landed on (F003/P097/T496).
    state.mouse.interactive_move = None;

    // Swap applies only to a pane-on-pane drop; a workspace target falls through to a
    // plain move into that workspace.
    if swap
        && let Some((
            crate::providers::workspaces::WorkspaceRow::Pane {
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
            crate::providers::workspaces::WorkspaceRow::Pane {
                pane_id: target_pid,
            },
            side,
        )) if target_pid != pane_id => {
            place_pane_at_sidebar_target(
                state,
                original_ws,
                removed_pane,
                crate::providers::workspaces::WorkspaceRow::Pane {
                    pane_id: target_pid,
                },
                side,
            );
        }
        // Dropped onto a workspace (its header/empty area) → move into that workspace.
        Some((item @ crate::providers::workspaces::WorkspaceRow::Workspace { .. }, side)) => {
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
    item: crate::providers::workspaces::WorkspaceRow,
    side: DropSide,
) {
    let default_width = state
        .session
        .options
        .default_column_width
        .unwrap_or(ColumnWidth::Proportion(0.85));

    match item {
        crate::providers::workspaces::WorkspaceRow::Pane {
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
        crate::providers::workspaces::WorkspaceRow::Workspace { ws_idx, .. } => {
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
        crate::providers::workspaces::WorkspaceRow::Column { ws_idx, col_idx, .. } => {
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
        crate::providers::workspaces::WorkspaceRow::FloatingPane { .. } => {
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

    // Resolved against the **retained** tree's real laid-out bounds, like every other row
    // resolution — not by arithmetic on a fixed row height, which stopped describing this sidebar
    // the moment its rows became widgets of varying height (F003/P085/T356).
    let item = match crate::chrome::sidebar_item_at(state, pos) {
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
        ChromeDragItem::Pane(target_pid) if shift_held => {
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
                // A **floating** pane's card resolves here too: it is a pane row like any other,
                // and `find_pane_location` simply does not find it in the tiled layout, so it takes
                // the same fallback the old floating-only arm did — `Session::add_pane` uses the
                // very default width that arm spelled out, on the same workspace.
                ChromeDragItem::Pane(target_pid) => {
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
                ChromeDragItem::Workspace { ws: ws_idx } => {
                    let width = state
                        .session
                        .options
                        .default_column_width
                        .unwrap_or(ColumnWidth::Proportion(0.85));
                    // Allocated before the workspace is borrowed; dropping onto a workspace always
                    // lands the pane in a column of its own.
                    let new_column_id = heca_core::layout::ColumnId(state.session.next_id());
                    if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                        ws.add_pane(pane, None, true, width, new_column_id);
                    }
                }
                ChromeDragItem::Column { ws: ws_idx, col: col_idx } => {
                    if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                        let target_col = col_idx.min(ws.scrolling.columns.len().saturating_sub(1));
                        ws.scrolling
                            .add_pane_to_column(target_col, None, pane, true);
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
