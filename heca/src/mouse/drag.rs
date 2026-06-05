//! Mouse drag state transitions.
//!
//! This module owns drag initiation, drag-motion updates, and cancellation for
//! interactive move and sidebar drag workflows.

use crate::app_state::{AppState, DetachedPane, DragState};
use heca_core::layout::{ColumnId, Point, Size};

pub(crate) fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) {
    state.mouse.pos = pos;

    let mut should_transition = false;
    if let DragState::InteractiveMoveStarting {
        pane_id,
        original_ws,
        start_mouse,
        threshold_sq,
    } = state.mouse.drag_state
    {
        let dx = pos.0 - start_mouse.0;
        let dy = pos.1 - start_mouse.1;
        let sq_dist = dx * dx + dy * dy;

        if let Some(ws) = state.session.workspaces.get_mut(original_ws)
            && let Some((ci, pi)) = super::find_pane_in_workspace(ws, pane_id)
        {
            let factor = super::rubberband(sq_dist / threshold_sq);
            ws.scrolling.columns[ci].panes[pi].interactive_move_offset =
                Point::new((dx * factor) as f64, (dy * factor) as f64);
        }

        if sq_dist > threshold_sq {
            should_transition = true;
        }
    }

    if should_transition
        && let DragState::InteractiveMoveStarting { pane_id, .. } = state.mouse.drag_state
    {
        transition_to_moving(state, pane_id, pos);
    }

    if let DragState::SidebarDragStarting {
        pane_id,
        original_ws,
        start_mouse,
        threshold_sq,
        swap,
        ..
    } = state.mouse.drag_state
    {
        let dx = pos.0 - start_mouse.0;
        let dy = pos.1 - start_mouse.1;
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
                        Some(format!("pane{}", pid))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| format!("pane{}", pane_id));

            let chrome = super::chrome_config(state);
            let (_win_w, win_h) = super::window_logical_size(state);
            let sw = if state.sidebar.left_visible {
                chrome.left_sidebar_width
            } else {
                40.0
            };
            let sidebar_top = chrome.tab_bar_height;
            let sidebar_bottom = win_h - chrome.status_bar_height;

            state.mouse.sidebar_drag_source_fi = crate::sidebar::sidebar_hit_test(
                &state.sidebar_tree,
                sidebar_top,
                sidebar_bottom - sidebar_top,
                sw,
                pos.1,
            );
            state.mouse.sidebar_drag_label = Some(crate::app_state::SidebarDragLabel {
                text: label,
                x: pos.0,
                y: pos.1,
                width: sw,
                _height: 20.0,
            });

            state.mouse.drag_state = DragState::SidebarDrag {
                pane_id,
                original_ws,
                swap,
            };
        }
    }

    if let DragState::SidebarDrag { .. } = state.mouse.drag_state
        && let Some(label) = &mut state.mouse.sidebar_drag_label
    {
        label.x = pos.0;
        label.y = pos.1;
    }

    if matches!(state.mouse.drag_state, DragState::InteractiveMove { .. }) {
        let offset = match state.mouse.drag_state {
            DragState::InteractiveMove { offset, .. } => offset,
            _ => unreachable!(),
        };
        let (cx, cy) = super::content_area_origin(state);
        let pointer_in = (pos.0 - cx, pos.1 - cy);

        if let Some(det) = &mut state.mouse.detached_pane {
            det.render_pos = Point::new(
                (pointer_in.0 - offset.0) as f64,
                (pointer_in.1 - offset.1) as f64,
            );
        }

        if let Some(ws) = state.session.active_workspace() {
            let space = Point::new(
                (pointer_in.0 as f64) + ws.scrolling.view_pos(),
                pointer_in.1 as f64,
            );
            state.mouse.insert_hint = Some(ws.scrolling.insert_position(space));
        }
    }

    if !matches!(state.mouse.drag_state, DragState::None) {
        update_sidebar_drag_hover(state);
    }

    if let DragState::SidebarDrag { .. } = state.mouse.drag_state
        && let Some(label) = &mut state.mouse.sidebar_drag_label
    {
        label.x = pos.0;
        label.y = pos.1;
    }

    state.mouse.sidebar_hovered_btn_idx =
        crate::sidebar::sidebar_button_hit_test(&state.sidebar_tree, pos.0, pos.1).map(|(i, _)| i);
}

fn update_sidebar_drag_hover(state: &mut AppState) {
    let (_win_w, win_h) = super::window_logical_size(state);
    let chrome = super::chrome_config(state);
    let pos = state.mouse.pos;
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;
    let sw = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        40.0
    };
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
                state.mouse.drag_hover_sidebar_fi = None;
            } else {
                state.mouse.drag_hover_sidebar_fi = Some(fi);
            }
        } else {
            state.mouse.drag_hover_sidebar_fi = None;
        }
    } else {
        state.mouse.drag_hover_sidebar_fi = None;
    }
}

pub(super) fn start_interactive_move(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32)) {
    state.mouse.drag_state = DragState::InteractiveMoveStarting {
        pane_id,
        original_ws: state.session.active_workspace_idx,
        start_mouse: mouse_pos,
        threshold_sq: 64.0,
    };
}

fn transition_to_moving(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32)) {
    let (cx, cy) = super::content_area_origin(state);
    let original_ws = state.session.active_workspace_idx;

    let ws = match state.session.workspaces.get_mut(original_ws) {
        Some(ws) => ws,
        None => return,
    };

    let mut found = None;
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        for (pi, pane) in col.panes.iter().enumerate() {
            if pane.id.0 == pane_id {
                found = Some((ci, pi));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }

    let (col_idx, pane_idx) = match found {
        Some(v) => v,
        None => return,
    };

    let col_x = ws.scrolling.column_x(col_idx);
    let pane_y = ws.scrolling.pane_y_in_column(col_idx, pane_idx);
    let size = ws
        .scrolling
        .columns
        .get(col_idx)
        .and_then(|c| c.pane_sizes.get(pane_idx))
        .copied()
        .unwrap_or(Size::new(200.0, 100.0));
    let view_pos = ws.scrolling.view_pos();

    if let Some(col) = ws.scrolling.columns.get_mut(col_idx)
        && let Some(pane) = col.panes.get_mut(pane_idx)
    {
        pane.interactive_move_offset = Point::default();
    }

    let original_col_id = ws
        .scrolling
        .columns
        .get(col_idx)
        .map(|c| c.id)
        .unwrap_or(ColumnId(0));

    let removed = match ws.scrolling.remove_pane(col_idx, pane_idx) {
        Some(p) => p,
        None => return,
    };

    let col_screen_x = col_x - view_pos;
    let pointer_in_content = (mouse_pos.0 - cx, mouse_pos.1 - cy);
    let offset = (
        pointer_in_content.0 - col_screen_x as f32,
        pointer_in_content.1 - pane_y as f32,
    );

    state.mouse.detached_pane = Some(DetachedPane {
        pane: removed,
        render_pos: Point::new(col_screen_x, pane_y),
        size,
        original_ws,
        _original_col: col_idx,
        original_col_id,
        original_pane: pane_idx,
    });

    state.mouse.drag_state = DragState::InteractiveMove {
        _pane_id: pane_id,
        _original_ws: original_ws,
        offset,
    };
}

pub(super) fn cancel_interactive_move(state: &mut AppState) {
    if let Some(det) = state.mouse.detached_pane.take() {
        let ws_idx = det
            .original_ws
            .min(state.session.workspaces.len().saturating_sub(1));
        if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
            if let Some(orig_idx) = ws
                .scrolling
                .columns
                .iter()
                .position(|c| c.id == det.original_col_id)
            {
                let pane_idx = det
                    .original_pane
                    .min(ws.scrolling.columns[orig_idx].panes.len());
                ws.scrolling
                    .add_pane_to_column(orig_idx, Some(pane_idx), det.pane, true);
            } else {
                state.session.add_pane(det.pane, None, true);
            }
        }
    }
    state.mouse.drag_state = DragState::None;
    state.mouse.insert_hint = None;
    state.mouse.sidebar_drag_source_fi = None;
    state.mouse.sidebar_drag_label = None;
    crate::sync_focus(state);
    state.needs_redraw = true;
}
