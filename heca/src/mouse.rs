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
pub(crate) mod resize;
mod surface_left;
mod target;

use crate::app::interaction::InteractionSource;
use crate::app::terminal_host::should_intercept_selection_gesture;
use crate::app_state::{AppDragPayload, AppState, InteractiveMovePhase};
use crate::chrome::ChromeConfig;
use crate::input::WmAction;
use heca_core::layout::PaneId;
use heca_grid_ui::drag::{DEFAULT_DRAG_THRESHOLD_SQ, DragItemId, DragPhase, DragSurfaceId};
use winit::event::{ElementState, MouseButton};

/// Handle cursor movement. Returns a `WmAction` if one should be dispatched
/// (e.g. focus-follows-mouse triggered), or `None` for internal state updates.
pub fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    // A divider resize-drag takes priority over DnD/focus-follow: it emits a
    // parameterized resize action for the caller to dispatch through the registry.
    if state.mouse.resize.is_some() {
        return resize::on_drag_move(state, pos);
    }
    drag::on_cursor_moved(state, pos);
    None
}

/// Whether a divider resize-drag is currently in flight. Callers gate the normal
/// hover/forward paths on this (mirrors the `drag_ctx.is_dragging()` guard).
pub(crate) fn is_resizing(state: &AppState) -> bool {
    state.mouse.resize.is_some()
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
    // The cursor only signals grabbable/grabbed (there is no "swap" cursor); the
    // move-vs-swap distinction lives on the drag ghost + the on-target indicator.
    let icon = if state.mouse.drag_ctx.is_dragging() {
        CursorIcon::Grabbing
    } else if let Some(resize_icon) = resize::cursor_for(state, pos) {
        // Active resize-drag → the drag axis; otherwise the divider under the cursor.
        resize_icon
    } else if crate::chrome::sidebar_drag_source(state, pos).is_some() {
        CursorIcon::Grab
    } else if link_hover(state, pos) {
        // Cmd held over a terminal hyperlink → signal the click-to-open affordance.
        CursorIcon::Pointer
    } else {
        CursorIcon::Default
    };
    if state.current_cursor != icon {
        state.current_cursor = icon;
        state.window.set_cursor(icon);
    }
}

/// Whether the pointer is over a terminal hyperlink while the open modifier
/// (Cmd, the interactive-move modifier) is held — the exact condition under
/// which a left-click opens the link. Drives the pointer cursor affordance so
/// the cue and the action stay in lockstep. terminal-task-18.
fn link_hover(state: &AppState, pos: (f32, f32)) -> bool {
    interactive_move_modifier_held(state)
        && hit_test_pane(state, pos)
            .is_some_and(|pane_id| {
                crate::app::terminal_host::hyperlink_uri_at_position(state, pane_id, pos).is_some()
            })
}

/// Open the right-click context menu for `pane_id` at `pos`: an **Open link**
/// entry when the click cell is a hyperlink, plus the common pane actions. Each
/// entry writes its [`WmAction`] into the shared sink (`state.context_menu_action`),
/// which the event loop drains and dispatches after feeding an event into the
/// menu. terminal-task-18 (context-menu open surface) / app-task-33.
fn open_context_menu(state: &mut AppState, pane_id: PaneId, pos: (f32, f32)) {
    use heca_grid_ui::widgets::{ContextMenu, MenuEntry};

    let sink = state.context_menu_action.clone();
    let mut menu = ContextMenu::new();

    // Each entry's icon comes from the action registry (one source of action
    // iconography, shared with the future command palette); the label + shortcut
    // hint stay menu-local. No quick-pick keycap — the real binding is shown.
    let entry = |catalog: &crate::actions::ActionCatalog,
                 label: &str,
                 icon_action: &str,
                 action: WmAction|
     -> MenuEntry {
        let s = sink.clone();
        let mut e = MenuEntry::new(label, move || {
            *s.borrow_mut() = Some(action.clone());
        });
        if let Some(glyph) = catalog.icon(icon_action) {
            e = e.icon(glyph);
        }
        e
    };

    // "Open link" first, only when the click cell carries a hyperlink (OSC 8 or
    // auto-detected) — same lookup as Cmd+click.
    if let Some(url) = crate::app::terminal_host::hyperlink_uri_at_position(state, pane_id, pos) {
        let s = sink.clone();
        let mut e = MenuEntry::new("Open link", move || {
            *s.borrow_mut() = Some(WmAction::OpenLink { url: url.clone() });
        });
        if let Some(glyph) = state.action_catalog.icon("open_link") {
            e = e.icon(glyph);
        }
        menu = menu.entry(e);
    }

    // Pane actions target the focused pane (the right-press focuses the clicked one).
    menu = menu.entry(
        entry(&state.action_catalog, "New column", "split_horizontal", WmAction::SplitHorizontal).shortcut("prefix+Enter"),
    );
    menu = menu
        .entry(entry(&state.action_catalog, "Split down", "split_vertical", WmAction::SplitVertical).shortcut("prefix+v"));
    menu = menu
        .entry(entry(&state.action_catalog, "Zoom / unzoom", "zoom_column", WmAction::ZoomColumn).shortcut("prefix+z"));
    menu = menu.entry(entry(&state.action_catalog, "Float / unfloat", "float", WmAction::Float).shortcut("prefix+f"));
    menu = menu
        .entry(entry(&state.action_catalog, "Close pane", "close", WmAction::ClosePane).shortcut("prefix+x").danger(true));

    menu = menu
        .anchor(heca_core::layout::Point::new(pos.0 as f64, pos.1 as f64))
        .open(true);
    state.context_menu = Some(menu);
    state.needs_redraw = true;
}

/// Open the right-click context menu for a sidebar `item` (pane / column /
/// workspace) at `pos`: textual **add / remove** entries acting on that explicit
/// target. Each entry writes its [`WmAction`] into the shared sink
/// (`state.context_menu_action`), drained + dispatched by the event loop — the same
/// path as the content-pane menu. Delete entries dispatch directly (styled
/// `danger`), matching the content menu's "Close pane" convention. Resolves against
/// the expanded grid sidebar only (see [`crate::chrome::sidebar_item_at`]).
fn open_sidebar_context_menu(
    state: &mut AppState,
    item: crate::chrome::ChromeDragItem,
    pos: (f32, f32),
) {
    use heca_grid_ui::widgets::{ContextMenu, MenuEntry};

    let sink = state.context_menu_action.clone();
    let entry = |catalog: &crate::actions::ActionCatalog,
                 label: &str,
                 icon_action: &str,
                 action: WmAction|
     -> MenuEntry {
        let s = sink.clone();
        let mut e = MenuEntry::new(label, move || {
            *s.borrow_mut() = Some(action.clone());
        });
        if let Some(glyph) = catalog.icon(icon_action) {
            e = e.icon(glyph);
        }
        e
    };

    let mut menu = ContextMenu::new();
    match item {
        crate::chrome::ChromeDragItem::Pane(pane_id) => {
            if let Some((ws_idx, col_idx, _)) = crate::find_pane_location(&state.session, pane_id) {
                menu = menu.entry(entry(&state.action_catalog,
                    "New pane",
                    "split_vertical",
                    WmAction::AddPaneToColumn { ws_idx, col_idx },
                ));
            }
            menu = menu.entry(
                entry(&state.action_catalog, "Delete pane", "close", WmAction::ClosePaneById { pane_id }).danger(true),
            );
        }
        crate::chrome::ChromeDragItem::Column { ws, col } => {
            menu = menu.entry(entry(&state.action_catalog,
                "New pane",
                "split_vertical",
                WmAction::AddPaneToColumn {
                    ws_idx: ws,
                    col_idx: col,
                },
            ));
            menu = menu.entry(entry(&state.action_catalog,
                "New column",
                "split_horizontal",
                WmAction::AddColumnToWorkspace { ws_idx: ws },
            ));
            menu = menu.entry(
                entry(&state.action_catalog,
                    "Delete column",
                    "close",
                    WmAction::DeleteColumn {
                        ws_idx: ws,
                        col_idx: col,
                    },
                )
                .danger(true),
            );
        }
        crate::chrome::ChromeDragItem::Workspace { ws } => {
            menu = menu.entry(entry(&state.action_catalog,
                "New column",
                "split_horizontal",
                WmAction::AddColumnToWorkspace { ws_idx: ws },
            ));
            menu = menu.entry(entry(&state.action_catalog,
                "New workspace",
                "create_workspace",
                WmAction::CreateWorkspace,
            ));
            menu = menu.entry(
                entry(&state.action_catalog,
                    "Delete workspace",
                    "close",
                    WmAction::DeleteWorkspace { ws_idx: ws },
                )
                .danger(true),
            );
        }
    }

    menu = menu
        .anchor(heca_core::layout::Point::new(pos.0 as f64, pos.1 as f64))
        .open(true);
    state.context_menu = Some(menu);
    state.needs_redraw = true;
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
            // Cmd+click on a terminal hyperlink → open it. Link-first: checked
            // before the interactive-move gesture (Cmd is the move modifier), so
            // a Cmd+click that lands on a link opens it and consumes the press,
            // while a Cmd+click off any link falls through to interactive-move.
            // terminal-task-18 (mouse open-link surface).
            if interactive_move_modifier_held(state)
                && let Some(pane_id) = hit_test_pane(state, pos)
                && let Some(url) =
                    crate::app::terminal_host::hyperlink_uri_at_position(state, pane_id, pos)
            {
                return Some((WmAction::OpenLink { url }, InteractionSource::MouseContent));
            }

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

            // Right sidebar chrome click (e.g. the collapse toggle). The right sidebar
            // has no drag surface yet (app-task-21); dispatch the press into the retained
            // chrome tree so its widget callbacks (the caret's `on_click`) fire, then
            // consume it so it doesn't leak to the content behind.
            if point_in_right_sidebar(state, pos) {
                crate::chrome::chrome_dispatch_press(state, pos);
                return None;
            }

            // Top bar chrome click (the sidebar collapse toggles, sidebar-fu-14). Same
            // as the right sidebar: dispatch into the retained chrome tree and consume.
            // (No tab-click feature today, so swallowing an empty-band click is harmless.)
            if point_in_top_bar(state, pos) {
                crate::chrome::chrome_dispatch_press(state, pos);
                return None;
            }

            // Resolve the drag source FIRST. For draggable items we defer any click
            // effect until release if the drag threshold is not crossed.
            let drag_source = crate::chrome::sidebar_drag_source(state, pos);

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
                            None => return None,
                        };
                        (
                            AppDragPayload::Pane {
                                pane_id,
                                origin_ws,
                                swap,
                            },
                            // Legacy collapsed-rail dim id (pane-only); see render.rs.
                            Some(DragItemId::new(pane_id.0 as usize)),
                        )
                    }
                    crate::chrome::ChromeDragItem::Column { ws, col } => {
                        (AppDragPayload::Column { ws, col, swap }, None)
                    }
                    crate::chrome::ChromeDragItem::Workspace { .. } => return None,
                };
                state.mouse.pending_click_action = match item {
                    crate::chrome::ChromeDragItem::Pane(pane_id) => {
                        Some(WmAction::FocusPane { pane_id })
                    }
                    crate::chrome::ChromeDragItem::Column { .. } => None,
                    crate::chrome::ChromeDragItem::Workspace { .. } => None,
                };
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

            // Sidebar click.
            let sidebar_action =
                target::surface_click_action(state, DragSurfaceId::LeftSidebar, pos);

            // Sidebar button clicks / non-pane item clicks dispatch immediately.
            if let Some(action) = sidebar_action {
                return Some((action, InteractionSource::MouseLeftSidebar));
            }

            // Content click → focus.
            if let Some(pane_id) = hit_test_pane(state, pos) {
                return Some((
                    WmAction::FocusPane { pane_id },
                    InteractionSource::MouseContent,
                ));
            }
        }
        (MouseButton::Left, ElementState::Released) => {
            // End a divider resize-drag first (left-button gap drag).
            if resize::on_release(state) {
                return None;
            }
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
                        let left = state
                            .mouse
                            .drag_ctx
                            .surface_mut(DragSurfaceId::LeftSidebar)
                            .expect("LeftSidebar pre-populated in DragContext::default");
                        let phase = std::mem::replace(&mut left.phase, DragPhase::Idle);
                        match phase {
                            DragPhase::Dragging { payload } => match payload {
                                AppDragPayload::Pane {
                                    pane_id,
                                    origin_ws,
                                    swap,
                                } => {
                                    release::handle_sidebar_drag_release(
                                        state, pane_id, origin_ws, swap, pos,
                                    );
                                }
                                AppDragPayload::Column { ws, col, swap } => {
                                    release::handle_sidebar_column_drag_release(
                                        state, ws, col, swap, pos,
                                    );
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
        // Right-button fallback for divider resize: hold right-button on a pane to
        // resize along the nearer axis (for when the thin gap fights the terminal).
        (MouseButton::Right, ElementState::Pressed) if resize::on_right_press(state, pos) => {
            return None;
        }
        (MouseButton::Right, ElementState::Released) if resize::on_release(state) => {
            return None;
        }
        // Right-click on a sidebar item (pane / column / workspace) → its add/remove
        // context menu, anchored on the clicked item. Checked before the content
        // path since the sidebar sits outside the pane area anyway.
        (MouseButton::Right, ElementState::Pressed)
            if crate::chrome::sidebar_item_at(state, pos).is_some() =>
        {
            if let Some(item) = crate::chrome::sidebar_item_at(state, pos) {
                open_sidebar_context_menu(state, item, pos);
            }
            return None;
        }
        // Right-click on a content pane (not on a resize divider) → context menu.
        // Focus the clicked pane so the menu's pane actions target it, then open
        // the menu in place. terminal-task-18 (context-menu open surface).
        (MouseButton::Right, ElementState::Pressed) => {
            if let Some(pane_id) = hit_test_pane(state, pos) {
                open_context_menu(state, pane_id, pos);
                return Some((
                    WmAction::FocusPane { pane_id },
                    InteractionSource::MouseContent,
                ));
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
        (pos.0 - (pane_area.loc.x as f32 + pane_area.size.w as f32 - content_trigger))
            / content_trigger
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

/// Whether `pos` (logical window coords) is over the **right sidebar** region — the
/// far-right band `[win_w - right_w, win_w]` between the top and bottom bars. The right
/// sidebar has no drag surface yet (app-task-21); the press handler uses this to route
/// its clicks straight into the retained chrome tree so its widgets (the collapse
/// toggle) receive them. `false` when the right sidebar is hidden / zero-width.
fn point_in_right_sidebar(state: &AppState, pos: (f32, f32)) -> bool {
    let chrome = chrome_config(state);
    let right_w = chrome.right_sidebar_width;
    if right_w <= 0.0 {
        return false;
    }
    let (win_w, win_h) = window_logical_size(state);
    let x_start = (win_w - right_w).max(0.0);
    let top = chrome.tab_bar_height;
    let bottom = (win_h - chrome.status_bar_height).max(top);
    pos.0 >= x_start && pos.0 <= win_w && pos.1 >= top && pos.1 <= bottom
}

/// Whether `pos` is over the **top bar** band `[0, tab_bar_height]`. The top bar hosts
/// the sidebar collapse toggles (sidebar-fu-14) as chrome-tree widgets; the press
/// handler routes its clicks into the tree so those toggles receive them. `false` when
/// the top bar is hidden.
fn point_in_top_bar(state: &AppState, pos: (f32, f32)) -> bool {
    let chrome = chrome_config(state);
    if chrome.tab_bar_height <= 0.0 {
        return false;
    }
    let (win_w, _win_h) = window_logical_size(state);
    pos.0 >= 0.0 && pos.0 <= win_w && pos.1 >= 0.0 && pos.1 <= chrome.tab_bar_height
}

fn chrome_config(state: &AppState) -> ChromeConfig {
    ChromeConfig {
        tab_bar_height: state.tab_bar_height(),
        status_bar_height: state.status_bar_height(),
        left_sidebar_width: state.left_sidebar_width(),
        right_sidebar_width: state.right_sidebar_width(),
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
pub(crate) fn interactive_move_modifier_held(state: &AppState) -> bool {
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
