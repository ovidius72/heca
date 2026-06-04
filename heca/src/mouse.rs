//! Mouse interaction system.
//!
//! Handles focus-follows-mouse, click-to-focus, interactive drag-and-drop,
//! and edge scrolling. All WM actions (focus, sidebat clicks) are returned
//! as `Option<WmAction>` for the caller to dispatch via `registry.execute()`.
//! Drag-and-drop state machine is managed internally since it involves
//! mouse-specific state (detached pane, insert position).

mod drag;
mod drop;
mod hit_test;
mod render;

use crate::app_state::{AppState, DragState};
use crate::chrome::ChromeConfig;
use crate::input::WmAction;
use winit::event::{ElementState, MouseButton};

/// Handle cursor movement. Returns a `WmAction` if one should be dispatched
/// (e.g. focus-follows-mouse triggered), or `None` for internal state updates.
pub fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    drag::on_cursor_moved(state, pos);
    None
}

/// Check the focus-follows-mouse timer on every frame (even without cursor movement).
/// Call from `about_to_wait` for frame-rate-independent debounce.
/// Handle mouse button events. Returns a `WmAction` if one should be dispatched.
pub fn on_mouse_input(
    state: &mut AppState,
    button: MouseButton,
    button_state: ElementState,
) -> Option<WmAction> {
    if !state.mouse_enabled {
        return None;
    }

    let pos = state.mouse.pos;

    match (button, button_state) {
        (MouseButton::Left, ElementState::Pressed) => {
            // Meta+click on content pane → start drag from content.
            if interactive_move_modifier_held(state)
                && let Some(pane_id) = hit_test_pane(state, pos)
            {
                drag::start_interactive_move(state, pane_id, pos);
                return None;
            }

            // Sidebar click.
            let sidebar_action = sidebar_click(state, pos);

            // Check if this is a sidebar pane hit (no button) → start drag detection.
            let is_pane_item = sidebar_pane_hit_test(state, pos).is_some();

            if is_pane_item {
                // Start drag detection — if mouse moves beyond threshold it becomes SidebarDrag.
                if let Some(pane_id) = sidebar_pane_hit_test(state, pos) {
                    let (ws_idx, _, _) = match crate::find_pane_location(&state.session, pane_id) {
                        Some(loc) => loc,
                        None => return sidebar_action,
                    };
                    let swap = state.modifiers.shift_key();
                    let click_action = sidebar_action
                        .clone()
                        .unwrap_or(WmAction::FocusPane { pane_id });
                    state.mouse.drag_state = DragState::SidebarDragStarting {
                        pane_id,
                        original_ws: ws_idx,
                        start_mouse: pos,
                        threshold_sq: 100.0, // 10px threshold
                        swap,
                        click_action: Box::new(click_action),
                    };
                    return None;
                }
            }

            // Sidebar button clicks / non-pane item clicks dispatch immediately.
            if let Some(action) = sidebar_action {
                return Some(action);
            }

            // Content click → focus.
            if let Some(pane_id) = hit_test_pane(state, pos) {
                return Some(WmAction::FocusPane { pane_id });
            }
        }
        (MouseButton::Left, ElementState::Released) => {
            match state.mouse.drag_state {
                DragState::InteractiveMoveStarting { .. } => {
                    drag::cancel_interactive_move(state);
                }
                DragState::InteractiveMove { .. } => {
                    // Check if dropping on a sidebar entry (workspace, column, or pane).
                    // This must be done BEFORE drop_pane since the pane is detached.
                    if drop::sidebar_handle_drop(state, pos) {
                        // Sidebar drop handled; detached pane already placed.
                    } else if state.mouse.insert_hint.is_some() {
                        drop::drop_pane(state);
                    } else {
                        drag::cancel_interactive_move(state);
                    }
                }
                DragState::SidebarDrag {
                    pane_id,
                    original_ws,
                    swap,
                } => {
                    drop::sidebar_drag_drop(state, pane_id, original_ws, swap, pos);
                }
                DragState::SidebarDragStarting { .. } => {
                    // Released before threshold: execute the click action.
                    if let DragState::SidebarDragStarting { click_action, .. } =
                        &state.mouse.drag_state
                    {
                        let action = click_action.as_ref().clone();
                        state.mouse.drag_state = DragState::None;
                        return Some(action);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }

    None
}

/// Process edge scrolling in `about_to_wait`. Should be called every frame
/// while a drag is active. Returns true if the view was scrolled.
pub fn process_edge_scroll(state: &mut AppState) -> bool {
    if !state.auto_scroll_edge {
        return false;
    }

    let is_dragging = matches!(
        state.mouse.drag_state,
        DragState::InteractiveMove { .. } | DragState::InteractiveMoveStarting { .. }
    );
    if !is_dragging {
        return false;
    }

    let pos = state.mouse.pos;
    let (win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
    let pane_area = chrome.content_rect(win_w, win_h);

    // During drag, scroll when near any edge:
    // - Absolute window edge (150px): covers sidebar area, fast (1000 px/s)
    // - Content area edge (80px): reaches hidden panes, fast (1000 px/s)
    let content_trigger = 80.0f32;

    // Compute normalized delta, covering the full horizontal range without gaps.
    // Left side: trigger from window edge through sidebar into content area.
    // Right side: trigger from content area edge through sidebar to window edge.
    let raw = if pos.0 < pane_area.x {
        // In the left sidebar/activity area — use distance from content edge.
        let dist = pane_area.x - pos.0;
        -(content_trigger + dist) / content_trigger
    } else if pos.0 < pane_area.x + content_trigger {
        // Near the left content area edge.
        -(content_trigger - (pos.0 - pane_area.x)) / content_trigger
    } else if pos.0 > pane_area.x + pane_area.w - content_trigger
        && pos.0 < pane_area.x + pane_area.w
    {
        // Near the right content area edge.
        (pos.0 - (pane_area.x + pane_area.w - content_trigger)) / content_trigger
    } else if pos.0 >= pane_area.x + pane_area.w {
        // In the right sidebar/activity area — use distance from content edge.
        let dist = pos.0 - (pane_area.x + pane_area.w);
        (content_trigger + dist) / content_trigger
    } else {
        0.0
    };

    if raw == 0.0 {
        return false;
    }

    // Compute dt for frame-rate independence.
    let now = std::time::Instant::now();
    let dt = state
        .mouse
        .last_edge_scroll_time
        .map(|t| now.duration_since(t).as_secs_f64())
        .unwrap_or(1.0 / 60.0)
        .min(0.05); // cap at 50ms to avoid jumps
    state.mouse.last_edge_scroll_time = Some(now);

    // During drag always use fast scroll speed.
    let actual_speed = 1000.0f32;

    let scroll_px = raw as f64 * actual_speed as f64 * dt;

    if let Some(ws) = state.session.active_workspace_mut() {
        let current = ws.scrolling.view_offset.current();
        let active_idx = ws.scrolling.active_column_idx;
        let total_w: f64 = ws.scrolling.column_widths.iter().sum();
        let gaps = ws.scrolling.options.gaps;
        let col_gaps = gaps * ws.scrolling.columns.len().max(1) as f64;
        let content_w = total_w + col_gaps;
        let vp_w = ws.scrolling.working_area.size.w;
        // Left extent: allow scrolling to reveal all columns before the active one.
        let before_w: f64 = ws.scrolling.column_widths.iter().take(active_idx).sum();
        let before_gaps = active_idx as f64 * gaps;
        let min_view = -(before_w + before_gaps + gaps);
        // Right extent: allow scrolling until the last content edge aligns
        // with the right viewport edge (plus a gap of padding).
        let max_view = (content_w - vp_w + gaps).max(min_view);
        let new_off = (current + scroll_px).clamp(min_view, max_view);
        let current = ws.scrolling.view_offset.current();
        let delta = new_off - current;
        ws.scrolling.view_offset.offset(delta);
    }

    true
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Hit testing
// ═══════════════════════════════════════════════════════════════════════════════

pub(crate) use hit_test::hit_test_pane;
use hit_test::sidebar_pane_hit_test;

/// Handle a click on the sidebar. Returns a WmAction if the click targets
/// a pane or workspace, or None if the click missed the sidebar.
fn sidebar_click(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    let (_win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;

    // Check if inside left sidebar.
    let sw = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        40.0
    };
    if pos.0 >= 0.0 && pos.0 <= sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom {
        // Check buttons first.
        if let Some((btn_idx, button)) =
            crate::sidebar::sidebar_button_hit_test(&state.sidebar_tree, pos.0, pos.1)
        {
            // Destructive delete actions require confirmation.
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
                    _ => unreachable!(),
                };
                state.input_mode = crate::app_state::InputMode::ConfirmDelete {
                    message,
                    action: Box::new(button),
                };
                return None;
            }

            // Switch to the target workspace if specified.
            if let Some(hitbox) = state.sidebar_tree.button_hitboxes.get(btn_idx)
                && let Some(ws_idx) = hitbox.ws_idx
                && ws_idx != state.session.active_workspace_idx
            {
                crate::switch_workspace_tracked(state, ws_idx);
            }
            return Some(button);
        }

        let sidebar_h = sidebar_bottom - sidebar_top;
        if let Some(fi) =
            crate::sidebar::sidebar_hit_test(&state.sidebar_tree, sidebar_top, sidebar_h, sw, pos.1)
        {
            state.sidebar_tree.cursor = fi;
            let item = state.sidebar_tree.current_item().cloned();
            match item? {
                crate::sidebar::SidebarItem::Pane { pane_id } => {
                    return Some(WmAction::FocusPane { pane_id });
                }
                crate::sidebar::SidebarItem::Workspace { ws_idx } => {
                    return Some(WmAction::FocusWorkspace { ws_idx });
                }
                crate::sidebar::SidebarItem::Column { .. } => {
                    // Columns are not directly clickable — no-op.
                }
                crate::sidebar::SidebarItem::FloatingPane { .. } => {
                    // Clicking a floating pane does nothing for now.
                }
            }
        }
    }

    None
}

pub(crate) use render::{render_detached_pane, render_insert_hint};

// ═══════════════════════════════════════════════════════════════════════════════
//  Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// NIRI rubberband formula: `(1.0 - (1.0 / (x * c / d + 1.0))) * d`
/// with `c = 1.0`, `d = 0.5`.
fn rubberband(x: f32) -> f32 {
    let c = 1.0;
    let d = 0.5;
    (1.0 - (1.0 / (x * c / d + 1.0))) * d
}

fn content_area_origin(state: &AppState) -> (f32, f32) {
    let (win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
    let r = chrome.content_rect(win_w, win_h);
    (r.x, r.y)
}

fn chrome_config(state: &AppState) -> ChromeConfig {
    ChromeConfig {
        tab_bar_height: 32.0,
        status_bar_height: 24.0,
        left_sidebar_width: if state.sidebar.left_visible {
            state.sidebar.left_width
        } else {
            40.0
        },
        right_sidebar_width: if state.sidebar.right_visible {
            state.sidebar.right_width
        } else {
            40.0
        },
    }
}

fn window_logical_size(state: &AppState) -> (f32, f32) {
    let phys = state.window.inner_size();
    let scale = state.scale_factor as f32;
    (phys.width as f32 / scale, phys.height as f32 / scale)
}

fn find_pane_in_workspace(
    ws: &mut heca_core::layout::workspace::Workspace,
    pane_id: u64,
) -> Option<(usize, usize)> {
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        for (pi, pane) in col.panes.iter().enumerate() {
            if pane.id.0 == pane_id {
                return Some((ci, pi));
            }
        }
    }
    None
}

/// Check if the configured interactive move modifier is currently held.
fn interactive_move_modifier_held(state: &AppState) -> bool {
    let modifiers = state.modifiers;
    match state.interactive_move_modifier {
        heca_config::theme::ModifierKey::Super => modifiers.super_key(),
        heca_config::theme::ModifierKey::Alt => modifiers.alt_key(),
        heca_config::theme::ModifierKey::Ctrl => modifiers.control_key(),
        heca_config::theme::ModifierKey::Shift => modifiers.shift_key(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::types::{LayoutOptions, Point, Rectangle, Size};
    use heca_core::layout::workspace::Workspace;
    use heca_core::layout::{Column, ColumnId, ColumnWidth, Pane, PaneId};

    #[test]
    fn test_rubberband_zero() {
        // At threshold (x = 1.0), rubberband = (1 - 1/(c/d + 1)) * d
        // c = 1.0, d = 0.5, so c/d = 2.0
        // x = 1.0: (1 - 1/(2 + 1)) * 0.5 = (1 - 1/3) * 0.5 = 2/3 * 0.5 = 1/3 ≈ 0.333
        let result = rubberband(1.0);
        let expected = (1.0 - (1.0 / (1.0 * 1.0 / 0.5 + 1.0))) * 0.5;
        assert!((result - expected).abs() < 1e-6);
    }

    #[test]
    fn test_rubberband_small() {
        let r = rubberband(0.25);
        assert!(r > 0.0 && r < 0.5);
    }

    #[test]
    fn test_rubberband_large() {
        let r = rubberband(100.0);
        assert!((r - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_rubberband_negative() {
        // Negative input (cursor moving opposite direction).
        // The formula has a singularity at x=-0.5, so we only test outside that range.
        let r = rubberband(-1.0);
        assert!(r.is_finite(), "rubberband(-1.0) should be finite");
    }

    #[test]
    fn test_find_pane_found() {
        let mut ws = Workspace::new(
            heca_core::layout::WorkspaceId(1),
            Rectangle::new(Point::default(), Size::new(1280.0, 800.0)),
            2.0,
            LayoutOptions::default(),
        );
        ws.scrolling.add_column(
            None,
            Column::new(
                ColumnId(42),
                Pane::new(PaneId(100), "test-pane"),
                ColumnWidth::Proportion(1.0),
            ),
            false,
        );
        assert_eq!(find_pane_in_workspace(&mut ws, 100), Some((0, 0)));
    }

    #[test]
    fn test_find_pane_not_found() {
        let mut ws = Workspace::new(
            heca_core::layout::WorkspaceId(1),
            Rectangle::new(Point::default(), Size::new(1280.0, 800.0)),
            2.0,
            LayoutOptions::default(),
        );
        ws.scrolling.add_column(
            None,
            Column::new(
                ColumnId(1),
                Pane::new(PaneId(10), "a"),
                ColumnWidth::Proportion(1.0),
            ),
            false,
        );
        assert_eq!(find_pane_in_workspace(&mut ws, 999), None);
    }

    #[test]
    fn test_find_pane_empty() {
        let mut ws = Workspace::new(
            heca_core::layout::WorkspaceId(1),
            Rectangle::new(Point::default(), Size::new(1280.0, 800.0)),
            2.0,
            LayoutOptions::default(),
        );
        assert_eq!(find_pane_in_workspace(&mut ws, 1), None);
    }

    #[test]
    fn test_find_pane_multi_column() {
        let mut ws = Workspace::new(
            heca_core::layout::WorkspaceId(1),
            Rectangle::new(Point::default(), Size::new(1280.0, 800.0)),
            2.0,
            LayoutOptions::default(),
        );
        ws.scrolling.add_column(
            None,
            Column::new(
                ColumnId(10),
                Pane::new(PaneId(1), "a"),
                ColumnWidth::Proportion(0.5),
            ),
            false,
        );
        ws.scrolling.add_column(
            None,
            Column::new(
                ColumnId(20),
                Pane::new(PaneId(2), "b"),
                ColumnWidth::Proportion(0.5),
            ),
            false,
        );
        ws.scrolling
            .add_pane_to_column(0, Some(1), Pane::new(PaneId(3), "c"), false);

        assert_eq!(find_pane_in_workspace(&mut ws, 1), Some((0, 0)));
        assert_eq!(find_pane_in_workspace(&mut ws, 3), Some((0, 1)));
        assert_eq!(find_pane_in_workspace(&mut ws, 2), Some((1, 0)));
        assert_eq!(find_pane_in_workspace(&mut ws, 999), None);
    }

    #[test]
    fn test_chrome_content_rect_left_sidebar() {
        let cfg = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 200.0,
            right_sidebar_width: 0.0,
        };
        let r = cfg.content_rect(1280.0, 800.0);
        assert_eq!(r.x, 200.0);
        assert_eq!(r.y, 32.0);
        assert_eq!(r.w, 1080.0);
        assert_eq!(r.h, 744.0);
    }

    #[test]
    fn test_chrome_content_rect_no_sidebars() {
        let cfg = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 40.0,
            right_sidebar_width: 0.0,
        };
        let r = cfg.content_rect(1280.0, 800.0);
        assert_eq!(r.x, 40.0);
        assert_eq!(r.y, 32.0);
        assert_eq!(r.w, 1240.0);
        assert_eq!(r.h, 744.0);
    }

    #[test]
    fn test_rubberband_niri_ref() {
        // NIRI reference: c=1.0, d=0.5
        // x=0 → 0; x=0.5 → 0.25; x=1 → 1/3; x=2 → 0.4
        assert!((rubberband(0.0) - 0.0).abs() < 1e-6);
        assert!((rubberband(0.5) - 0.25).abs() < 1e-6);
        assert!((rubberband(1.0) - 1.0 / 3.0).abs() < 1e-6);
        assert!((rubberband(2.0) - 0.4).abs() < 1e-6);
    }
}
