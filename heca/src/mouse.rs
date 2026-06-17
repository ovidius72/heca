//! Mouse interaction system.
//!
//! Handles focus-follows-mouse, click-to-focus, interactive drag-and-drop,
//! and edge scrolling. All WM actions (focus, sidebar clicks) are returned
//! as `Option<WmAction>` for the caller to dispatch via `registry.execute()`.
//! Drag-and-drop state machine is managed internally since it involves
//! mouse-specific state (detached pane, insert position).

mod drag;
mod hit_test;
mod interactive;
mod release;
mod render;
mod surface_left;
mod target;

use crate::app::interaction::InteractionSource;
use crate::app::terminal_host::should_intercept_selection_gesture;
use crate::app_state::{AppDragPayload, AppState, InteractiveMovePhase};
use heca_core::layout::PaneId;
use crate::chrome::{ChromeConfig, DEFAULT_TAB_BAR_HEIGHT, DEFAULT_STATUS_BAR_HEIGHT, DEFAULT_COLLAPSED_SIDEBAR_WIDTH};
use crate::input::WmAction;
use heca_grid_ui::drag::{DragItemId, DragPhase, DragSurfaceId, DEFAULT_DRAG_THRESHOLD_SQ};
use winit::event::{ElementState, MouseButton};

/// Handle cursor movement. Returns a `WmAction` if one should be dispatched
/// (e.g. focus-follows-mouse triggered), or `None` for internal state updates.
pub fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    drag::on_cursor_moved(state, pos);
    None
}

/// Cursor policy: pick the OS cursor for the current state and apply it to the
/// window — but only when it changes (cursor-moved fires very often). While a sidebar
/// drag is in flight → `Grabbing`; hovering a draggable source (a pane card or a column
/// grip, via [`sidebar_drag_source`](crate::chrome::sidebar_drag_source)) → `Grab`;
/// otherwise the default arrow. `heca-grid-ui` stays cursor-free (it only emits a
/// `Scene`) — the OS cursor is a host concern. General by design: add text/resize
/// cursors here as more affordances arrive.
pub(crate) fn update_cursor(state: &mut AppState, pos: (f32, f32)) {
    use winit::window::CursorIcon;
    let icon = if state.mouse.drag_ctx.is_dragging() {
        CursorIcon::Grabbing
    } else if crate::chrome::sidebar_drag_source(state, pos).is_some() {
        CursorIcon::Grab
    } else {
        CursorIcon::Default
    };
    if state.current_cursor != icon {
        state.current_cursor = icon;
        state.window.set_cursor(icon);
    }
}

/// Sync the current drag mode with modifier state changes.
///
/// This keeps move/swap behavior live while the user presses or releases Shift.
pub fn on_modifiers_changed(state: &mut AppState) {
    interactive::sync_drag_swap_mode(state);
    if state.mouse.drag_ctx.is_dragging() || state.mouse.interactive_move.is_some() {
        drag::on_cursor_moved(state, state.mouse.pos);
    }
}

/// Check the focus-follows-mouse timer on every frame (even without cursor movement).
/// Call from `about_to_wait` for frame-rate-independent debounce.
/// Handle mouse button events. Returns a `WmAction` if one should be dispatched.
pub fn on_mouse_input(
    state: &mut AppState,
    button: MouseButton,
    button_state: ElementState,
) -> Option<(WmAction, InteractionSource)> {
    if !state.mouse_enabled {
        return None;
    }

    let pos = state.mouse.pos;

    match (button, button_state) {
        (MouseButton::Left, ElementState::Pressed) => {
            // Meta+click on content pane → start drag from content.
            if !should_intercept_selection_gesture(state, pos, button, button_state)
                && interactive_move_modifier_held(state)
                && let Some(pane_id) = hit_test_pane(state, pos)
            {
                interactive::start_interactive_move(state, pane_id, pos);
                return None;
            }

            // When a floating pane is modal, block all sidebar interaction.
            // No clicks, no drags, no mode changes.
            if crate::app::interaction::is_floating_domain(&state.session) {
                return None;
            }

            // Resolve the drag source FIRST — `surface_click_action` dispatches into
            // (and discards) the retained chrome tree, so it must run after this.
            let drag_source = crate::chrome::sidebar_drag_source(state, pos);
            // Sidebar click.
            let sidebar_action = target::surface_click_action(state, DragSurfaceId::LeftSidebar, pos);

            // Sidebar pane press → start drag-detection, resolving the source pane
            // from the RETAINED chrome tree's real bounds (F4.5), not the legacy
            // fixed-row geometry. If the threshold isn't crossed it falls back to the
            // pending click action (stored below); see `mouse/drag.rs`.
            if let Some(item) = drag_source {
                let swap = state.modifiers.shift_key();
                // Build the drag payload from the source kind. Workspaces are
                // drop-targets only (never `.draggable`), so `source_at` can only
                // return a pane or a column here.
                let (payload, source_item) = match item {
                    crate::chrome::ChromeDragItem::Pane(pane_id) => {
                        let origin_ws = match crate::find_pane_location(&state.session, pane_id) {
                            Some((ws_idx, _, _)) => ws_idx,
                            None => {
                                return sidebar_action.map(|a| (a, InteractionSource::MouseLeftSidebar))
                            }
                        };
                        (
                            AppDragPayload::Pane { pane_id, origin_ws, swap },
                            // Legacy collapsed-rail dim id (pane-only); see render.rs.
                            Some(DragItemId::new(pane_id.0 as usize)),
                        )
                    }
                    crate::chrome::ChromeDragItem::Column { ws, col } => {
                        (AppDragPayload::Column { ws, col, swap }, None)
                    }
                    crate::chrome::ChromeDragItem::Workspace { .. } => {
                        return sidebar_action.map(|a| (a, InteractionSource::MouseLeftSidebar))
                    }
                };
                state.mouse.pending_click_action = sidebar_action.clone();
                if let Some(left) = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar) {
                    left.phase = DragPhase::Starting {
                        payload,
                        start_pos: pos,
                        threshold_sq: DEFAULT_DRAG_THRESHOLD_SQ,
                    };
                    left.source_item = source_item;
                }
                state.mouse.drag_ctx.set_active(DragSurfaceId::LeftSidebar);
                return None;
            }

            // Sidebar button clicks / non-pane item clicks dispatch immediately.
            if let Some(action) = sidebar_action {
                return Some((action, InteractionSource::MouseLeftSidebar));
            }

            // Content click → focus.
            if let Some(pane_id) = hit_test_pane(state, pos) {
                return Some((WmAction::FocusPane { pane_id }, InteractionSource::MouseContent));
            }
        }
        (MouseButton::Left, ElementState::Released) => {
            // Check for interactive move release first.
            if let Some(InteractiveMovePhase::Starting { .. }) = state.mouse.interactive_move {
                interactive::cancel_interactive_move(state);
                return None;
            }
            if let Some(InteractiveMovePhase::Moving { .. }) = state.mouse.interactive_move {
                release::handle_interactive_move_release(state, pos);
                return None;
            }
            // Then check surface drags.
            if let Some(active) = state.mouse.drag_ctx.active_surface {
                match active {
                    DragSurfaceId::LeftSidebar => {
                        let left = state.mouse.drag_ctx.surface_mut(DragSurfaceId::LeftSidebar).expect("LeftSidebar pre-populated in DragContext::default");
                        let phase = std::mem::replace(&mut left.phase, DragPhase::Idle);
                        match phase {
                            DragPhase::Dragging { payload } => match payload {
                                AppDragPayload::Pane { pane_id, origin_ws, swap } => {
                                    release::handle_sidebar_drag_release(state, pane_id, origin_ws, swap, pos);
                                }
                                AppDragPayload::Column { ws, col, swap } => {
                                    release::handle_sidebar_column_drag_release(state, ws, col, swap, pos);
                                }
                            },
                            DragPhase::Starting { .. } => {
                                return release::handle_sidebar_drag_starting_release(state);
                            }
                            DragPhase::Idle => {}
                        }
                    }
                }
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
        state.mouse.interactive_move,
        Some(InteractiveMovePhase::Moving { .. }) | Some(InteractiveMovePhase::Starting { .. })
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
    let content_trigger = crate::chrome::EDGE_SCROLL_TRIGGER;

    // Compute normalized delta, covering the full horizontal range without gaps.
    // Left side: trigger from window edge through sidebar into content area.
    // Right side: trigger from content area edge through sidebar to window edge.
    let raw = if pos.0 < pane_area.loc.x as f32 {
        // In the left sidebar/activity area — use distance from content edge.
        let dist = pane_area.loc.x as f32 - pos.0;
        -(content_trigger + dist) / content_trigger
    } else if pos.0 < pane_area.loc.x as f32 + content_trigger {
        // Near the left content area edge.
        -(content_trigger - (pos.0 - pane_area.loc.x as f32)) / content_trigger
    } else if pos.0 > pane_area.loc.x as f32 + pane_area.size.w as f32 - content_trigger
        && pos.0 < pane_area.loc.x as f32 + pane_area.size.w as f32
    {
        // Near the right content area edge.
        (pos.0 - (pane_area.loc.x as f32 + pane_area.size.w as f32 - content_trigger)) / content_trigger
    } else if pos.0 >= pane_area.loc.x as f32 + pane_area.size.w as f32 {
        // In the right sidebar/activity area — use distance from content edge.
        let dist = pos.0 - (pane_area.loc.x as f32 + pane_area.size.w as f32);
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

pub(crate) use hit_test::{hit_test_pane, hit_test_pane_excluding};

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
    (r.loc.x as f32, r.loc.y as f32)
}

fn chrome_config(state: &AppState) -> ChromeConfig {
    ChromeConfig {
        tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
        status_bar_height: DEFAULT_STATUS_BAR_HEIGHT,
        left_sidebar_width: if state.chrome_state.left_visible() {
            state.chrome_state.left_size()
        } else {
            DEFAULT_COLLAPSED_SIDEBAR_WIDTH
        },
        right_sidebar_width: if state.chrome_state.right_visible() {
            state.chrome_state.right_size()
        } else {
            DEFAULT_COLLAPSED_SIDEBAR_WIDTH
        },
        sidebar_gap: state.appearance.effective_sidebar_gap(&state.theme),
    }
}

fn window_logical_size(state: &AppState) -> (f32, f32) {
    let phys = state.window.inner_size();
    let scale = state.scale_factor as f32;
    (phys.width as f32 / scale, phys.height as f32 / scale)
}

fn find_pane_in_workspace(
    ws: &mut heca_core::layout::workspace::Workspace,
    pane_id: PaneId,
) -> Option<(usize, usize)> {
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        for (pi, pane) in col.panes.iter().enumerate() {
            if pane.id == pane_id {
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
mod tests;
