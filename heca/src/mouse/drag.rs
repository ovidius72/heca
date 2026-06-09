//! Mouse drag state transitions.
//!
//! This module owns the `on_cursor_moved` router and sidebar-drag handlers.
//! Interactive move logic lives in `mouse/interactive.rs`.

use heca_grid_ui::drag::{DragItemKind, DragItemId, DragLabel, DragSurfaceId, SurfaceDragPhase};

use crate::app_state::AppState;
use crate::chrome::DEFAULT_COLLAPSED_SIDEBAR_WIDTH;

/// Route cursor movement to the active drag phase handler.
///
/// Dispatches to the appropriate phase handler based on drag state,
/// then updates sidebar hover highlighting if a drag is active.
pub(crate) fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) {
    state.mouse.pos = pos;

    super::interactive::handle_interactive_move_starting(state, pos);
    handle_sidebar_drag_starting(state, pos);
    super::interactive::handle_interactive_move_drag(state, pos);
    handle_sidebar_drag_move(state, pos);

    if state.mouse.drag_ctx.is_dragging() || state.mouse.interactive_move.is_some() {
        update_sidebar_drag_hover(state);
    }
}

// ── Sidebar drag starting (threshold phase) ──────────────────────────────

/// Handle threshold detection for sidebar drag. On threshold exceeded,
/// transitions to active sidebar drag mode and sets up ghost label + source highlight.
fn handle_sidebar_drag_starting(state: &mut AppState, pos: (f32, f32)) {
    let left = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar)
        .expect("LeftSidebar pre-populated in DragContext::default");
    let phase = std::mem::replace(&mut left.phase, SurfaceDragPhase::Idle);
    if let SurfaceDragPhase::Starting {
        pane_id: Some(pane_id),
        original_ws,
        start_pos,
        threshold_sq,
        swap,
        ..
    } = phase
    {
        let dx = pos.0 - start_pos.0;
        let dy = pos.1 - start_pos.1;
        let sq_dist = dx * dx + dy * dy;

        if sq_dist > threshold_sq {
            let label = state.sidebar_tree.flat_items.iter()
                .find(|item| matches!(item, crate::sidebar::SidebarItem::Pane { pane_id: pid } if *pid == pane_id))
                .and_then(|item| {
                    if let crate::sidebar::SidebarItem::Pane { pane_id: pid } = item {
                        for ws in &state.sidebar_tree.workspaces {
                            for col in &ws.columns {
                                for p in &col.panes {
                                    if p.pane_id == *pid {
                                        return Some(p.name.clone());
                                    }
                                }
                            }
                        }
                    }
                    None
                })
                .unwrap_or_else(|| format!("pane{}", pane_id));

            let chrome = super::chrome_config(state);
            let sw = if state.sidebar.left_visible {
                chrome.left_sidebar_width
            } else {
                DEFAULT_COLLAPSED_SIDEBAR_WIDTH
            };

            let left = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar)
                .expect("LeftSidebar pre-populated in DragContext::default");
            left.ghost_label = Some(DragLabel {
                text: label,
                x: pos.0,
                y: pos.1,
                width: sw,
                height: 20.0,
            });
            left.phase = SurfaceDragPhase::Dragging {
                kind: DragItemKind::Pane,
                pane_id: Some(pane_id),
                original_ws,
                swap,
            };
            // source_item was already set when the Starting phase began
        } else {
            // Threshold not exceeded — restore the Starting phase
            state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar)
                .expect("LeftSidebar pre-populated in DragContext::default")
                .phase = SurfaceDragPhase::Starting {
                    kind: DragItemKind::Pane,
                    pane_id: Some(pane_id),
                    original_ws,
                    start_pos,
                    threshold_sq,
                    swap,
                };
        }
    } else {
        // Not a Starting phase — restore whatever it was
        state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar)
            .expect("LeftSidebar pre-populated in DragContext::default")
            .phase = phase;
    }
}

// ── Sidebar drag move (active phase) ────────────────────────────────────

/// Handle cursor movement during an active sidebar drag.
/// Updates the ghost label position to follow the cursor.
fn handle_sidebar_drag_move(state: &mut AppState, pos: (f32, f32)) {
    let left = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar)
        .expect("LeftSidebar pre-populated in DragContext::default");
    if let SurfaceDragPhase::Dragging { .. } = left.phase
        && let Some(label) = &mut left.ghost_label
    {
        label.x = pos.0;
        label.y = pos.1;
    }
}

// ── Sidebar drag hover ──────────────────────────────────────────────────

fn update_sidebar_drag_hover(state: &mut AppState) {
    let (_win_w, win_h) = super::window_logical_size(state);
    let chrome = super::chrome_config(state);
    let pos = state.mouse.pos;
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;
    let sw = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        DEFAULT_COLLAPSED_SIDEBAR_WIDTH
    };
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