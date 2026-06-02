//! Mouse interaction system.
//!
//! Handles focus-follows-mouse, click-to-focus, interactive drag-and-drop,
//! and edge scrolling. All WM actions (focus, sidebat clicks) are returned
//! as `Option<WmAction>` for the caller to dispatch via `registry.execute()`.
//! Drag-and-drop state machine is managed internally since it involves
//! mouse-specific state (detached pane, insert position).

use crate::app_state::{AppState, DetachedPane, DragState};
use crate::chrome::ChromeConfig;
use crate::input::WmAction;
use heca_core::layout::types::{InsertPosition, Point, Size};
use heca_core::layout::{Column, ColumnId, ColumnWidth};
use winit::event::{ElementState, MouseButton};

/// Handle cursor movement. Returns a `WmAction` if one should be dispatched
/// (e.g. focus-follows-mouse triggered), or `None` for internal state updates.
pub fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    state.mouse.pos = pos;

    // ── Phase 1: rubberband (Starting) ──
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

        // Apply rubberband offset using the ORIGINAL workspace (not active, which
        // may have changed if the user clicked a sidebar entry during drag).
        if let Some(ws) = state.session.workspaces.get_mut(original_ws)
            && let Some((ci, pi)) = find_pane_in_workspace(ws, pane_id)
        {
            let factor = rubberband(sq_dist / threshold_sq);
            ws.scrolling.columns[ci].panes[pi].interactive_move_offset =
                Point::new((dx * factor) as f64, (dy * factor) as f64);
        }

        if sq_dist > threshold_sq {
            should_transition = true;
        }
    }

    if should_transition
        && let DragState::InteractiveMoveStarting { pane_id, .. } = state.mouse.drag_state {
            transition_to_moving(state, pane_id, pos);
        }

    // ── Phase 2: detached (Moving) ──
    if matches!(state.mouse.drag_state, DragState::InteractiveMove { .. }) {
        let offset = match state.mouse.drag_state {
            DragState::InteractiveMove { offset, .. } => offset,
            _ => unreachable!(),
        };
        let (cx, cy) = content_area_origin(state);
        let pointer_in = (pos.0 - cx, pos.1 - cy);

        if let Some(det) = &mut state.mouse.detached_pane {
            det.render_pos = Point::new(
                (pointer_in.0 - offset.0) as f64,
                (pointer_in.1 - offset.1) as f64,
            );
        }

        // Compute insert hint from space coords using ACTIVE workspace
        // (where the cursor currently is, which is where we'll drop).
        if let Some(ws) = state.session.active_workspace() {
            let space = Point::new(
                (pointer_in.0 as f64) + ws.scrolling.view_pos(),
                pointer_in.1 as f64,
            );
            state.mouse.insert_hint = Some(ws.scrolling.insert_position(space));
        }
    }

    // ── Drag: track sidebar hover for highlighting ──
    if !matches!(state.mouse.drag_state, DragState::None) {
        update_sidebar_drag_hover(state);
    }

    // ── Sidebar drag: update ghost label position ──
    if let DragState::SidebarDrag { .. } = state.mouse.drag_state
        && let Some(label) = &mut state.mouse.sidebar_drag_label {
            label.x = pos.0;
            label.y = pos.1;
        }

    // ── Button hover detection ──
    state.mouse.sidebar_hovered_btn_idx = crate::sidebar::sidebar_button_hit_test(&state.sidebar_tree, pos.0, pos.1).map(|(i, _)| i);

    None
}

/// Update sidebar drag hover — find which sidebar flat item (if any)
/// is under the cursor during drag, for visual highlight in sidebar render.
fn update_sidebar_drag_hover(state: &mut AppState) {
    let (_win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
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
        state.mouse.drag_hover_sidebar_fi = crate::sidebar::sidebar_hit_test(
            &state.sidebar_tree,
            sidebar_top,
            sidebar_h,
            sw,
            pos.1,
        );
    } else {
        state.mouse.drag_hover_sidebar_fi = None;
    }
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
            // Meta+click on sidebar pane → start drag from sidebar.
            if interactive_move_modifier_held(state)
                && let Some(pane_id) = sidebar_pane_hit_test(state, pos) {
                start_sidebar_drag(state, pane_id, pos);
                return None;
            }

            // Meta+click on content pane → start drag from content.
            if interactive_move_modifier_held(state)
                && let Some(pane_id) = hit_test_pane(state, pos) {
                    start_interactive_move(state, pane_id, pos);
                    return None;
                }

            // Sidebar click (no modifier) → focus.
            if let Some(action) = sidebar_click(state, pos) {
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
                    cancel_interactive_move(state);
                }
                DragState::InteractiveMove { .. } => {
                    // Check if dropping on a sidebar entry (workspace, column, or pane).
                    // This must be done BEFORE drop_pane since the pane is detached.
                    if sidebar_handle_drop(state, pos) {
                        // Sidebar drop handled; detached pane already placed.
                    } else if state.mouse.insert_hint.is_some() {
                        drop_pane(state);
                    } else {
                        cancel_interactive_move(state);
                    }
                }
                DragState::SidebarDrag { pane_id, original_ws } => {
                    sidebar_drag_drop(state, pane_id, original_ws, pos);
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

/// Find which pane (if any) is under the cursor.
/// Floating panes are tested before scrolling panes.
pub fn hit_test_pane(state: &AppState, pos: (f32, f32)) -> Option<u64> {
    let (win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
    let pane_area = chrome.content_rect(win_w, win_h);

    // Skip if in sidebar or chrome.
    let sidebar_left_w = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        40.0
    };
    if pos.0 < sidebar_left_w {
        return None;
    }
    if pos.1 < chrome.tab_bar_height || pos.1 > win_h - chrome.status_bar_height {
        return None;
    }
    if pos.0 > win_h / 2.0
        && state.sidebar.right_visible
        && pos.0 > win_w - chrome.right_sidebar_width
    {
        return None;
    }

    let ws_offset = state
        .session
        .workspace_geometries()
        .first()
        .map(|(_, r)| (r.loc.x as f32, r.loc.y as f32))
        .unwrap_or((0.0, 0.0));

    // Floating panes first (they render on top).
    if let Some(ws) = state.session.active_workspace() {
        for float in &ws.floating_panes {
            let fx = pane_area.x + ws_offset.0 + float.position.x as f32;
            let fy = pane_area.y + ws_offset.1 + float.position.y as f32;
            let fw = float.size.w as f32;
            let fh = float.size.h as f32;
            if pos.0 >= fx && pos.0 < fx + fw && pos.1 >= fy && pos.1 < fy + fh {
                return Some(float.pane.id.0);
            }
        }
    }

    // Scrolling panes.
    let pane_positions = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.panes_with_positions())
        .unwrap_or_default();

    for (pane_id, rect) in &pane_positions {
        let px = pane_area.x + ws_offset.0 + rect.loc.x as f32;
        let py = pane_area.y + ws_offset.1 + rect.loc.y as f32;
        let pw = rect.size.w as f32;
        let ph = rect.size.h as f32;
        // Exclusive bounds (< not <=) so border points aren't in two panes.
        if pos.0 >= px && pos.0 < px + pw && pos.1 >= py && pos.1 < py + ph {
            return Some(pane_id.0);
        }
    }

    None
}

/// Check if a sidebar pane item is under the cursor. Returns pane_id if found.
fn sidebar_pane_hit_test(state: &AppState, pos: (f32, f32)) -> Option<u64> {
    let (_win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;
    let sw = if state.sidebar.left_visible { chrome.left_sidebar_width } else { 40.0 };

    if pos.0 < 0.0 || pos.0 >= sw || pos.1 < sidebar_top || pos.1 >= sidebar_bottom {
        return None;
    }

    let sidebar_h = sidebar_bottom - sidebar_top;
    let fi = crate::sidebar::sidebar_hit_test(&state.sidebar_tree, sidebar_top, sidebar_h, sw, pos.1)?;
    let item = state.sidebar_tree.flat_items.get(fi);
    match item? {
        crate::sidebar::SidebarItem::Pane { pane_id } => Some(*pane_id),
        crate::sidebar::SidebarItem::Column { ws_idx, col_idx } => {
            // Drag the first pane in this column.
            state.session.workspaces.get(*ws_idx)
                .and_then(|ws| ws.scrolling.columns.get(*col_idx))
                .and_then(|col| col.panes.first())
                .map(|p| p.id.0)
        }
        _ => None,
    }
}

/// Start a drag from a sidebar pane item.
/// Switches to the pane's source workspace if needed, then starts the same
/// interactive move state machine as content-initiated drags.
fn start_sidebar_drag(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32)) {
    // Cancel any previous incomplete drag.
    if matches!(state.mouse.drag_state, DragState::InteractiveMoveStarting { .. } | DragState::InteractiveMove { .. }) {
        cancel_interactive_move(state);
    }
    if matches!(state.mouse.drag_state, DragState::SidebarDrag { .. }) {
        state.mouse.drag_state = DragState::None;
    }

    // Find which workspace contains this pane.
    let (ws_idx, _col_idx, _pane_idx) = match crate::find_pane_location(&state.session, pane_id) {
        Some(loc) => loc,
        None => return,
    };

    // Get the label text from the sidebar tree.
    let label = state.sidebar_tree.flat_items.iter()
        .find(|item| matches!(item, crate::sidebar::SidebarItem::Pane { pane_id: pid } if *pid == pane_id))
        .and_then(|item| {
            if let crate::sidebar::SidebarItem::Pane { pane_id: pid } = item {
                // Find the pane name from the tree.
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

    eprintln!("[sidebar-drag] starting drag for pane_id={} from ws={} label='{}'", pane_id, ws_idx, label);

    // Switch to the source workspace if needed.
    if state.session.active_workspace_idx != ws_idx {
        eprintln!("[sidebar-drag] switching to source workspace {}", ws_idx);
        crate::switch_workspace_tracked(state, ws_idx);
    }

    // Record the sidebar flat index and drag label for visual effect.
    let chrome = chrome_config(state);
    let sidebar_top = chrome.tab_bar_height;
    let (_win_w, win_h) = window_logical_size(state);
    let sidebar_bottom = win_h - chrome.status_bar_height;
    let sw = if state.sidebar.left_visible { chrome.left_sidebar_width } else { 40.0 };
    state.mouse.sidebar_drag_source_fi = crate::sidebar::sidebar_hit_test(
        &state.sidebar_tree, sidebar_top, sidebar_bottom - sidebar_top, sw, mouse_pos.1,
    );
    state.mouse.sidebar_drag_label = Some(crate::app_state::SidebarDragLabel {
        text: label,
        x: mouse_pos.0,
        y: mouse_pos.1,
        width: sw,
        height: 20.0, // approximate item height
    });

    // Enter sidebar drag state — pane stays in layout, no floating ghost.
    state.mouse.drag_state = DragState::SidebarDrag {
        pane_id,
        original_ws: ws_idx,
    };
}

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
        if let Some((btn_idx, button)) = crate::sidebar::sidebar_button_hit_test(&state.sidebar_tree, pos.0, pos.1) {
            // Switch to the target workspace if specified.
            if let Some(hitbox) = state.sidebar_tree.button_hitboxes.get(btn_idx)
                && let Some(ws_idx) = hitbox.ws_idx
                    && ws_idx != state.session.active_workspace_idx {
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
            }
        }
    }

    None
}

/// Handle a drop during sidebar drag.
/// Removes the pane from its original position and inserts it at the target
/// (sidebar item or content area), with animation from original to new position.
fn sidebar_drag_drop(state: &mut AppState, pane_id: u64, original_ws: usize, pos: (f32, f32)) {
    state.mouse.drag_state = DragState::None;
    state.mouse.drag_hover_sidebar_fi = None;
    state.mouse.sidebar_drag_source_fi = None;
    state.mouse.sidebar_drag_label = None;

    // 1. Capture original position for animation BEFORE removing.
    let old_rect = state.session.workspaces.get(original_ws)
        .and_then(|ws| ws.scrolling.panes_with_positions().into_iter()
            .find(|(pid, _)| *pid == heca_core::layout::PaneId(pane_id))
            .map(|(_, r)| r));

    // 2. Remove the pane from its original workspace.
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
        eprintln!("[sidebar-drag-drop] remove_pane returned None for pane_id={}", pane_id);
        return;
    };

    eprintln!("[sidebar-drag-drop] removed pane_id={} from ws={}", pane_id, original_ws);

    // 3. Determine drop target.
    let (_win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;
    let sw = if state.sidebar.left_visible { chrome.left_sidebar_width } else { 40.0 };

    let on_sidebar = pos.0 >= 0.0 && pos.0 <= sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom;

    if on_sidebar {
        // Drop on sidebar item.
        let sidebar_h = sidebar_bottom - sidebar_top;
        let fi = crate::sidebar::sidebar_hit_test(
            &state.sidebar_tree, sidebar_top, sidebar_h, sw, pos.1,
        );

        if let Some(fi) = fi {
            if let Some(item) = state.sidebar_tree.flat_items.get(fi).cloned() {
                match item {
                    crate::sidebar::SidebarItem::Pane { pane_id: target_pid } => {
                        // Insert after the target pane in the same column/workspace.
                        if let Some((t_ws, t_col, t_pi)) = crate::find_pane_location(&state.session, target_pid) {
                            if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                                let insert_idx = (t_pi + 1).min(ws.scrolling.columns[t_col].panes.len());
                                ws.scrolling.add_pane_to_column(t_col, Some(insert_idx), removed_pane, true);
                                eprintln!("[sidebar-drag-drop] inserted after target pane at ws={} col={} idx={}", t_ws, t_col, insert_idx);
                            }
                        } else {
                            state.session.add_pane(removed_pane, None, true);
                        }
                        state.focused_pane = Some(target_pid);
                    }
                    crate::sidebar::SidebarItem::Workspace { ws_idx } => {
                        if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                            let width = state.session.options.default_column_width
                                .unwrap_or(heca_core::layout::ColumnWidth::Proportion(0.85));
                            ws.add_pane(removed_pane, None, true, width);
                        }
                        eprintln!("[sidebar-drag-drop] added to workspace ws={}", ws_idx);
                    }
                    crate::sidebar::SidebarItem::Column { ws_idx, col_idx } => {
                        if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                            let target_col = col_idx.min(ws.scrolling.columns.len().saturating_sub(1));
                            ws.scrolling.add_pane_to_column(target_col, None, removed_pane, true);
                        }
                        eprintln!("[sidebar-drag-drop] added to column ws={} col={}", ws_idx, col_idx);
                    }
                }
            }
        } else {
            state.session.add_pane(removed_pane, None, true);
        }
    } else {
        // Drop in content area — add to active workspace.
        state.session.add_pane(removed_pane, None, true);
        eprintln!("[sidebar-drag-drop] added to active workspace (content drop)");
    }

    // 4. Animate the pane from its old position to new position.
    let new_ws = state.session.active_workspace_idx;
    if let Some(old_rect) = old_rect
        && let Some((_, new_rect)) = state.session.workspaces.get(new_ws)
            .and_then(|ws| ws.scrolling.panes_with_positions().into_iter()
                .find(|(pid, _)| *pid == heca_core::layout::PaneId(pane_id))) {
            let dx = old_rect.loc.x - new_rect.loc.x;
            let dy = old_rect.loc.y - new_rect.loc.y;
            if let Some(ws) = state.session.workspaces.get_mut(new_ws) {
                for col in &mut ws.scrolling.columns {
                    for pane in &mut col.panes {
                        if pane.id.0 == pane_id {
                            pane.animate_move_from(Point::new(dx, dy), heca_core::layout::animation::AnimationConfig::default());
                            eprintln!("[sidebar-drag-drop] animated pane from ({:.1},{:.1}) to ({:.1},{:.1})", old_rect.loc.x, old_rect.loc.y, new_rect.loc.x, new_rect.loc.y);
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
fn sidebar_handle_drop(state: &mut AppState, pos: (f32, f32)) -> bool {
    let (_win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
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

    // Capture detached pane id for logging (det.pane will be moved later).
    let detached_id = det.pane.id.0;
    // Pre-generate column ids for recreated columns so we don't reuse pane ids as column ids.
    let new_col_detached = ColumnId(state.session.next_id());
    let new_col_removed = ColumnId(state.session.next_id());

    match item {
        crate::sidebar::SidebarItem::Pane { pane_id } => {
            // Drop on sidebar pane.
            // If Shift+modifier held → true swap (detached ↔ target positions).
            // Otherwise → move: insert detached just after target pane.
            let shift_held = state.modifiers.shift_key();

            if shift_held {
                // SWAP: detached ↔ target pane.
                eprintln!("[mouse-swap-sidebar] detached id={} orig_ws={} orig_col_id={:?} orig_pi={}", detached_id, det.original_ws, det.original_col_id, det.original_pane);
                if let Some((t_ws, t_col, t_pi)) = crate::find_pane_location(&state.session, pane_id) {
                    eprintln!("[mouse-swap-sidebar] target pane {} at ws={} col={} idx={}", pane_id, t_ws, t_col, t_pi);
                    if t_ws < state.session.workspaces.len() {
                        // Remember column count before removal to detect if a column was deleted.
                        let col_count_before = state.session.workspaces[t_ws].scrolling.columns.len();

                        // Remove target pane.
                        let removed_target = state.session.workspaces[t_ws]
                            .scrolling
                            .remove_pane(t_col, t_pi);

                        let col_count_after = state.session.workspaces[t_ws].scrolling.columns.len();
                        let column_removed = col_count_after < col_count_before;
                        eprintln!("[mouse-swap-sidebar] removed_target.is_some={} column_removed={}", removed_target.is_some(), column_removed);

                        if let Some(removed_target) = removed_target {
                            let removed_target_id = removed_target.id.0;
                            eprintln!("[mouse-swap-sidebar] removed target id={}", removed_target_id);
                            // Insert detached pane at target location (create column if needed).
                            if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                                if t_col < ws.scrolling.columns.len() {
                                    ws.scrolling.add_pane_to_column(t_col, Some(t_pi), det.pane, true);
                                    eprintln!("[mouse-swap-sidebar] inserted detached id={} at ws={} col={} idx={}", detached_id, t_ws, t_col, t_pi);
                                } else {
                                    ws.scrolling.add_column(Some(t_col), Column::new(new_col_detached, det.pane, ColumnWidth::Proportion(0.5)), true);
                                    eprintln!("[mouse-swap-sidebar] created column and inserted detached id={} at ws={} new_col_idx={}", detached_id, t_ws, t_col);
                                }
                            } else {
                                // Fallback: add to active workspace
                                state.session.add_pane(det.pane, None, true);
                                eprintln!("[mouse-swap-sidebar] fallback inserted detached to active workspace");
                            }

                            // Insert removed target at detached's original location (or create new column).
                            let orig_ws = det.original_ws;
                            if let Some(ws) = state.session.workspaces.get_mut(orig_ws) {
                                // Find the original column by ColumnId so re-insertion is robust to
                                // index shifts while the pane was detached.
                                if let Some(orig_idx) = ws.scrolling.columns.iter().position(|c| c.id == det.original_col_id) {
                                    let orig_pi = det.original_pane.min(ws.scrolling.columns[orig_idx].panes.len());
                                    ws.scrolling.add_pane_to_column(orig_idx, Some(orig_pi), removed_target, true);
                                    eprintln!("[mouse-swap-sidebar] inserted removed target id={} at ws={} col={} idx={}", removed_target_id, orig_ws, orig_idx, orig_pi);
                                } else {
                                    // Original column was removed — create a new column with the target pane.
                                    ws.scrolling.add_column(None, Column::new(new_col_removed, removed_target, ColumnWidth::Proportion(0.5)), true);
                                    eprintln!("[mouse-swap-sidebar] original column missing — created new column with target id={}", removed_target_id);
                                }
                            } else {
                                state.session.add_pane(removed_target, None, true);
                                eprintln!("[mouse-swap-sidebar] fallback added removed target to active workspace");
                            }
                        } else {
                            // Couldn't remove target — fallback: add detached to active workspace.
                            state.session.add_pane(det.pane, None, true);
                            eprintln!("[mouse-swap-sidebar] remove_pane returned None; fallback insert detached");
                        }
                    } else {
                        // Invalid target workspace → fallback.
                        state.session.add_pane(det.pane, None, true);
                        eprintln!("[mouse-swap-sidebar] invalid target workspace {}; fallback inserted detached", t_ws);
                    }
                } else {
                    // Target not found → fallback.
                    state.session.add_pane(det.pane, None, true);
                    eprintln!("[mouse-swap-sidebar] target not found in find_pane_location; fallback inserted detached");
                }
            } else {
                // MOVE: insert detached just after target pane.
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
                        let insert_idx = (pane_idx + 1).min(ws.scrolling.columns[col_idx].panes.len());
                        ws.scrolling.add_pane_to_column(col_idx, Some(insert_idx), det.pane, true);
                    }
                } else {
                    // Fallback: add to active workspace.
                    state.session.add_pane(det.pane, None, true);
                }
            }
            // Focus the target pane.
            state.focused_pane = Some(pane_id);
        }
        crate::sidebar::SidebarItem::Workspace { ws_idx } => {
            // Drop on sidebar workspace → add pane directly to that workspace.
            // Session::add_pane always goes to active workspace, so we bypass it.
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
            // Drop on sidebar column → add pane to that column in the target workspace.
            if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
                let target_col = col_idx.min(ws.scrolling.columns.len().saturating_sub(1));
                ws.scrolling
                    .add_pane_to_column(target_col, None, det.pane, true);
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

// ═══════════════════════════════════════════════════════════════════════════════
//  Interactive move — state machine
// ═══════════════════════════════════════════════════════════════════════════════

fn start_interactive_move(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32)) {
    state.mouse.drag_state = DragState::InteractiveMoveStarting {
        pane_id,
        original_ws: state.session.active_workspace_idx,
        start_mouse: mouse_pos,
        threshold_sq: 64.0, // 8px squared — matches NIRI
    };
}

fn transition_to_moving(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32)) {
    // Capture content origin and original workspace BEFORE borrowing ws.
    let (cx, cy) = content_area_origin(state);
    let original_ws = state.session.active_workspace_idx;

    let ws = match state.session.workspaces.get_mut(original_ws) {
        Some(ws) => ws,
        None => return,
    };

    // Find and remove pane from layout.
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

    // Record original position for the detached pane.
    // Capture view_pos BEFORE remove_pane (which can change active_column_idx).
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

    // Clear rubberband before removal.
    if let Some(col) = ws.scrolling.columns.get_mut(col_idx)
        && let Some(pane) = col.panes.get_mut(pane_idx) {
        pane.interactive_move_offset = Point::default();
    }

    // Capture the original column id (for robust re-insertion even if
    // column indices change while the pane is detached). Capture BEFORE removal.
    let original_col_id = ws.scrolling.columns.get(col_idx).map(|c| c.id).unwrap_or(ColumnId(0));

    // Remove the pane from layout.
    let _old_render_x = col_x;
    let _old_render_y = pane_y;
    let removed = match ws.scrolling.remove_pane(col_idx, pane_idx) {
        Some(p) => p,
        None => return,
    };

    // Compute pointer offset within the pane.
    // Convert col_x from space coords to screen coords by subtracting view_pos.
    // Uses the pre-removal view_pos so the detached pane appears at the correct
    // screen position even though remove_pane may have shifted active_column_idx.
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
        original_col: col_idx,
        original_col_id,
        original_pane: pane_idx,
    });

    state.mouse.drag_state = DragState::InteractiveMove {
        pane_id,
        original_ws,
        offset,
    };
}

fn cancel_interactive_move(state: &mut AppState) {
    if let Some(det) = state.mouse.detached_pane.take() {
        // Restore pane to its ORIGINAL workspace (not active, which may differ).
        let ws_idx = det
            .original_ws
            .min(state.session.workspaces.len().saturating_sub(1));
        if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
            // Prefer to find the original column by ColumnId so re-insertion
            // survives index shifts while the pane was detached.
            if let Some(orig_idx) = ws.scrolling.columns.iter().position(|c| c.id == det.original_col_id) {
                let pane_idx = det.original_pane.min(ws.scrolling.columns[orig_idx].panes.len());
                ws.scrolling.add_pane_to_column(orig_idx, Some(pane_idx), det.pane, true);
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

fn drop_pane(state: &mut AppState) {
    let hint = match state.mouse.insert_hint.take() {
        Some(h) => h,
        None => {
            cancel_interactive_move(state);
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

    // Capture detached pane id early because det.pane will be moved when
    // reinserting; use detached_id in diagnostic logs after move.
    let detached_id = det.pane.id.0;
    // Pre-generate new ColumnIds for any recreated columns used during reinsertion.
    let new_col_detached = ColumnId(state.session.next_id());
    let new_col_removed = ColumnId(state.session.next_id());

    let shift_held = state.modifiers.shift_key();

    if shift_held {
        // SWAP: Shift+drop in content area.
        eprintln!("[mouse-swap] detached id={} orig_ws={} orig_col_id={:?} orig_pi={}", detached_id, det.original_ws, det.original_col_id, det.original_pane);
        let target_pane_id = hit_test_pane(state, state.mouse.pos);
        if let Some(target_id) = target_pane_id {
            eprintln!("[mouse-swap] hit_test_pane -> target_id={}", target_id);
            if let Some((t_ws, t_col, t_pi)) = crate::find_pane_location(&state.session, target_id) {
                eprintln!("[mouse-swap] target located at ws={} col={} idx={}", t_ws, t_col, t_pi);
                let target_col_id = state
                    .session
                    .workspaces
                    .get(t_ws)
                    .and_then(|ws| ws.scrolling.columns.get(t_col).map(|c| c.id));

                let col_count_before = state.session.workspaces[t_ws].scrolling.columns.len();
                let removed_target = state.session.workspaces[t_ws]
                    .scrolling
                    .remove_pane(t_col, t_pi);
                let col_count_after = state.session.workspaces[t_ws].scrolling.columns.len();
                eprintln!("[mouse-swap] remove_pane returned is_some={} col_count_before={} col_count_after={}", removed_target.is_some(), col_count_before, col_count_after);

                if let Some(removed_target) = removed_target {
                    let removed_target_id = removed_target.id.0;
                    eprintln!("[mouse-swap] removed target id={}", removed_target_id);
                    if let Some(ws) = state.session.workspaces.get_mut(t_ws) {
                        if let Some(tc_id) = target_col_id {
                            if let Some(target_idx) = ws.scrolling.columns.iter().position(|c| c.id == tc_id) {
                                let insert_idx = t_pi.min(ws.scrolling.columns[target_idx].panes.len());
                                ws.scrolling.add_pane_to_column(target_idx, Some(insert_idx), det.pane, true);
                                eprintln!("[mouse-swap] inserted detached id={} at ws={} col={} idx={}", detached_id, t_ws, target_idx, insert_idx);
                            } else {
                                ws.scrolling.add_column(None, Column::new(new_col_detached, det.pane, ColumnWidth::Proportion(0.5)), true);
                                eprintln!("[mouse-swap] target column id missing; created new column and inserted detached id={}", detached_id);
                            }
                        } else {
                            if t_col < ws.scrolling.columns.len() {
                                ws.scrolling.add_pane_to_column(t_col, Some(t_pi), det.pane, true);
                                eprintln!("[mouse-swap] fallback numeric insert detached id={} at ws={} col={} idx={}", detached_id, t_ws, t_col, t_pi);
                            } else {
                                ws.scrolling.add_column(Some(t_col), Column::new(new_col_detached, det.pane, ColumnWidth::Proportion(0.5)), true);
                                eprintln!("[mouse-swap] fallback created column and inserted detached id={}", detached_id);
                            }
                        }
                    } else {
                        state.session.add_pane(det.pane, None, true);
                        eprintln!("[mouse-swap] fallback added detached to active workspace");
                    }

                    // Re-insert removed target into original location identified by ColumnId
                    let orig_ws = det.original_ws;
                    if let Some(ws) = state.session.workspaces.get_mut(orig_ws) {
                        if let Some(orig_idx) = ws.scrolling.columns.iter().position(|c| c.id == det.original_col_id) {
                            let orig_pi = det.original_pane.min(ws.scrolling.columns[orig_idx].panes.len());
                            ws.scrolling.add_pane_to_column(orig_idx, Some(orig_pi), removed_target, true);
                            eprintln!("[mouse-swap] reinserted removed target id={} at ws={} col={} idx={}", removed_target_id, orig_ws, orig_idx, orig_pi);
                        } else {
                            ws.scrolling.add_column(None, Column::new(new_col_removed, removed_target, ColumnWidth::Proportion(0.5)), true);
                            eprintln!("[mouse-swap] original column missing; created new column with target id={}", removed_target_id);
                        }
                    } else {
                        state.session.add_pane(removed_target, None, true);
                        eprintln!("[mouse-swap] fallback added removed target to active workspace");
                    }
                } else {
                    eprintln!("[mouse-swap] remove_pane failed; performing fallback insert of detached");
                    let fresh_id = state.session.next_id();
                    let fresh_id_2 = state.session.next_id();
                    if let Some(ws) = state.session.active_workspace_mut() {
                        match hint {
                            InsertPosition::NewColumn(col_idx) => {
                                let col = Column::new(ColumnId(fresh_id), det.pane, ColumnWidth::Proportion(0.5));
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
                                    let col = Column::new(ColumnId(fresh_id_2), det.pane, ColumnWidth::Proportion(0.5));
                                    ws.scrolling.add_column(None, col, true);
                                }
                            }
                        }
                    }
                }
            } else {
                eprintln!("[mouse-swap] target not found on find_pane_location; cancelling");
                cancel_interactive_move(state);
                state.needs_redraw = true;
                return;
            }
        } else {
            eprintln!("[mouse-swap] shift+drop on empty space; cancelling");
            cancel_interactive_move(state);
            state.needs_redraw = true;
            return;
        }
    } else {
        // Regular insert (no shift): use InsertPosition.
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

// ═══════════════════════════════════════════════════════════════════════════════
//  Rendering helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Render the detached pane during interactive move.
pub fn render_detached_pane(state: &mut AppState, pane_area: (f32, f32, f32, f32)) {
    let det = match &state.mouse.detached_pane {
        Some(d) => d,
        None => return,
    };

    let px = pane_area.0 + det.render_pos.x as f32;
    let py = pane_area.1 + det.render_pos.y as f32;
    let pw = det.size.w as f32;
    let ph = det.size.h as f32;
    let accent = state.theme.accent.to_f32x4();

    // Draw translucent background.
    state
        .primitive_renderer
        .draw_rect(px, py, pw, ph, [0.118, 0.118, 0.180, 0.7]);

    // Draw pane name centered.
    let pane_name = &det.pane.title;
    let name_size = (pw.min(ph) * 0.25).clamp(24.0, 72.0);
    let name_w = name_size * pane_name.len() as f32 * 0.6;
    let name_x = px + (pw - name_w) / 2.0;
    let name_y = py + (ph - name_size) / 2.0;
    state
        .text_renderer
        .queue_text(pane_name, name_x, name_y, name_size, [1.0, 1.0, 1.0, 0.9]);

    // 3× accent border.
    let border_w = state.theme.border_width * 3.0;
    state
        .primitive_renderer
        .draw_border(px, py, pw, ph, accent, border_w);
}

/// Render the insert hint (placeholder rectangle) during interactive move.
pub fn render_insert_hint(state: &mut AppState, pane_area: (f32, f32, f32, f32)) {
    let hint = match state.mouse.insert_hint {
        Some(h) => h,
        None => return,
    };
    let ws = match state.session.active_workspace() {
        Some(w) => w,
        None => return,
    };

    let wa = ws.scrolling.working_area;
    let gaps = ws.scrolling.options.gaps;
    let view_pos = ws.scrolling.view_pos();

    let (rx, ry, rw, rh) = match hint {
        InsertPosition::NewColumn(col_idx) => {
            // Compute the x position in space coordinates for the new column insertion.
            let space_x = if col_idx == 0 {
                0.0
            } else if col_idx < ws.scrolling.columns.len() {
                let prev_x = ws.scrolling.column_x(col_idx - 1);
                let prev_w = ws.scrolling.column_widths.get(col_idx - 1).copied().unwrap_or(0.0);
                prev_x + prev_w + gaps * 0.5
            } else if ws.scrolling.columns.is_empty() {
                0.0
            } else {
                let last_x = ws.scrolling.column_x(ws.scrolling.columns.len() - 1);
                let last_w = ws.scrolling.column_widths.last().copied().unwrap_or(0.0);
                last_x + last_w - gaps * 0.5
            };

            let x = space_x - view_pos;
            let y = wa.loc.y + wa.size.h * 0.2;
            let w = 24.0f64;
            let h = wa.size.h * 0.6;
            (x, y, w, h)
        }
        InsertPosition::InColumn { col_idx, pane_idx } => {
            // Full column width × 24px at the insertion point.
            let col_len = ws.scrolling.columns.len();
            let space_x = if col_idx < col_len {
                ws.scrolling.column_x(col_idx)
            } else if col_len == 0 {
                0.0
            } else {
                ws.scrolling.column_x(col_len - 1)
            };

            let col_w = ws.scrolling.column_widths.get(col_idx).copied().unwrap_or(0.0);

            let space_y = if col_idx < ws.scrolling.columns.len() {
                let col = &ws.scrolling.columns[col_idx];
                if pane_idx == col.panes.len() && pane_idx > 0 {
                    let last_pane_idx = pane_idx - 1;
                    let last_pane_y = ws.scrolling.pane_y_in_column(col_idx, last_pane_idx);
                    let last_pane_h = col.pane_sizes.get(last_pane_idx).map(|s| s.h).unwrap_or(0.0);
                    last_pane_y + last_pane_h
                } else {
                    ws.scrolling.pane_y_in_column(col_idx, pane_idx)
                }
            } else {
                0.0
            };

            let x = space_x - view_pos;
            let y = space_y;
            let w = col_w;
            let h = 24.0f64;
            (x, y, w, h)
        }
    };

    let mut render_x = pane_area.0 + rx as f32;
    let mut render_y = pane_area.1 + ry as f32;
    let mut render_w = rw as f32;
    let mut render_h = rh as f32;

    // Inset by border_width + 1px so the hint sits clearly inside
    // the pane's visible area (the pane border is drawn inside its rect).
    let inset = (state.theme.border_width * 2.0).max(2.0);
    render_x = (render_x + inset).min(pane_area.0 + pane_area.2 - 4.0);
    render_y = (render_y + inset).min(pane_area.1 + pane_area.3 - 4.0);
    render_w = (render_w - 2.0 * inset).max(4.0);
    render_h = (render_h - 2.0 * inset).max(4.0);

    // Clamp to content area as a safety net.
    let ca_r = pane_area.0 + pane_area.2;
    let ca_b = pane_area.1 + pane_area.3;
    if render_x < pane_area.0 {
        let overflow = pane_area.0 - render_x;
        render_w = (render_w - overflow).max(4.0);
        render_x = pane_area.0;
    }
    if render_x >= ca_r {
        return;
    }
    if render_x + render_w > ca_r {
        render_w = (ca_r - render_x).max(4.0);
    }
    if render_y < pane_area.1 {
        let overflow = pane_area.1 - render_y;
        render_h = (render_h - overflow).max(4.0);
        render_y = pane_area.1;
    }
    if render_y >= ca_b {
        return;
    }
    if render_y + render_h > ca_b {
        render_h = (ca_b - render_y).max(4.0);
    }

    let accent = state.theme.accent.to_f32x4();
    state.primitive_renderer.draw_rect(
        render_x,
        render_y,
        render_w,
        render_h,
        [accent[0], accent[1], accent[2], 0.15],
    );
    state.primitive_renderer.draw_border(
        render_x,
        render_y,
        render_w,
        render_h,
        [accent[0], accent[1], accent[2], 0.6],
        2.0,
    );
}

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
