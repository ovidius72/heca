//! Mouse interaction system.
//!
//! Focus-follows-mouse, click-to-focus, interactive move and edge scrolling. All WM actions (focus,
//! sidebar clicks) are returned as `Option<WmAction>` for the caller to dispatch via
//! `registry.execute()`.
//!
//! **Dragging a row is not here.** A row declares that it can be dragged and `heca-grid-ui` runs
//! the gesture over the same tree it lays out and paints; the drop comes back through
//! `chrome::drain_pending_drops`. What is left in this module is the content area's own gesture —
//! interactive move, which detaches a pane — and divider resizing.

mod hit_test;
mod interactive;
pub(crate) mod release;
mod render;
pub(crate) mod resize;
pub(crate) mod surface_left;

use crate::app::interaction::InteractionSource;
use crate::app::terminal_host::should_intercept_selection_gesture;
use crate::app_state::{AppState, InteractiveMovePhase};
use crate::chrome::ChromeConfig;
use crate::input::WmAction;
use heca_core::layout::PaneId;

/// Handle cursor movement. Returns a `WmAction` if one should be dispatched
/// (e.g. focus-follows-mouse triggered), or `None` for internal state updates.
pub fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    // A divider resize-drag takes priority over DnD/focus-follow: it emits a
    // parameterized resize action for the caller to dispatch through the registry.
    if state.mouse.resize.is_some() {
        return resize::on_drag_move(state, pos);
    }
    interactive::on_cursor_moved(state, pos);
    None
}

/// Whether a divider resize-drag is currently in flight. Callers gate the normal
/// hover/forward paths on this (mirrors the `chrome::drag_in_flight` guard).
pub(crate) fn is_resizing(state: &AppState) -> bool {
    state.mouse.resize.is_some()
}

/// Cursor policy: pick the OS cursor for the current state and apply it to the
/// window — but only when it changes (cursor-moved fires very often). While anything is being
/// dragged → `Grabbing`; hovering something that says it can be dragged (a pane card, a column
/// grip — asked of the tree via [`sidebar_drag_source`](crate::chrome::sidebar_drag_source)) →
/// `Grab`; otherwise the default arrow. `heca-grid-ui` stays cursor-free (it only emits a
/// `Scene`) — the OS cursor is a host concern. General by design: add text/resize
/// cursors here as more affordances arrive.
pub(crate) fn update_cursor(state: &mut AppState, pos: (f32, f32)) {
    use winit::window::CursorIcon;
    // The cursor only signals grabbable/grabbed (there is no "swap" cursor); the
    // move-vs-swap distinction lives on the drag ghost + the on-target indicator.
    let icon = if crate::chrome::drag_in_flight(state) {
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

/// Logical-pixel center of the app window — the anchor for keyboard/RPC-opened menus so a menu
/// opened with no pointer stays centered on screen instead of at a stale cursor position.
pub(crate) fn window_center_logical(state: &AppState) -> (f32, f32) {
    let phys = state.window.inner_size();
    let s = state.scale_factor as f32;
    (phys.width as f32 / s / 2.0, phys.height as f32 / s / 2.0)
}

/// Sync the current drag mode with modifier state changes.
///
/// This keeps move/swap behavior live while the user presses or releases Shift.
pub fn on_modifiers_changed(state: &mut AppState) {
    interactive::sync_drag_swap_mode(state);
    if crate::chrome::drag_in_flight(state) || state.mouse.interactive_move.is_some() {
        interactive::on_cursor_moved(state, state.mouse.pos);
    }
}

/// Check the focus-follows-mouse timer on every frame (even without cursor movement).
/// Call from `about_to_wait` for frame-rate-independent debounce.
/// Handle mouse button events. Returns a `WmAction` if one should be dispatched.
///
/// **Takes the event the loop built, not the winit pair it was built from.** One `Event::Raw` per
/// device event, handed down rather than reconstructed here — a second construction is a second
/// answer, and the one that used to happen further down this file delivered the same press to the
/// chrome tree twice (F003/P097/T496).
pub fn on_mouse_input(
    state: &mut AppState,
    ev: &heca_grid_ui::Event,
) -> Option<(WmAction, InteractionSource)> {
    if !state.mouse_enabled {
        return None;
    }

    let heca_grid_ui::Event::Raw(raw) = ev else {
        return None;
    };
    let pos = state.mouse.pos;

    use heca_grid_ui::PointerButton as Btn;
    use heca_grid_ui::event::RawPointerKind as Kind;
    match (raw.button, raw.kind) {
        (Btn::Left, Kind::Pressed) => {
            // A click on a search field re-enters query entry and places the caret.
            // Checked first: the bar floats above pane content, so a press that lands
            // on it must not also be read as a click into the terminal underneath.
            if search_field_press(state, pos) {
                return None;
            }

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
            if !should_intercept_selection_gesture(state, pos, ev)
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

            // First, and deliberately independent of whether a widget then consumes the press: a
            // click on a scrollbar thumb is still a click *in* that container and must focus it.
            // Each of the branches below returns early, so doing this later would mean repeating it
            // in every one of them and still missing the paths that consume.

            // **The row answers its own press, and there is no branch here per region.** A press
            // goes to the tree like any other, wherever it lands — a scrollbar thumb, a collapse
            // caret, a button, a pane card in either sidebar. A row that says it can be dragged is
            // dragged by the framework, and one whose press never crosses the threshold pairs with
            // its release into a click, which is what focuses the pane.
            //
            // Nothing is asked before the tree. The app used to take the press first to put its own
            // question — is this a drag source? — which is what made dragging exist only where the
            // app had been taught about it (F003/P097/T496).
            if crate::chrome::deliver(state, ev) {
                return None;
            }

            // **Focusing the clicked pane is gone from here, and that is the point.** A pane says
            // what a press on it means, on itself — so it focuses for the left button and the
            // right one alike, and nothing in the mouse layer has to find which pane the cursor is
            // over in order to do it for it.
            //
            // What stood here hit-tested the pane by hand for the left button; the right button had
            // a second copy that opened the menu first and then asked to focus, which is why
            // right-clicking a pane never focused it.
        }
        (Btn::Left, Kind::Released) => {
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
            // **A dragged row's release is not handled here.** The framework pairs the press with
            // the release, ends the gesture and hands back a drop naming what it landed on, which
            // `chrome::drain_pending_drops` acts on. A release that crossed no threshold becomes a
            // click by the same pairing, which is what focuses the pane (F003/P097/T496).
        }
        // Right-button fallback for divider resize: hold right-button on a pane to
        // resize along the nearer axis (for when the thin gap fights the terminal).
        (Btn::Right, Kind::Pressed) if resize::on_right_press(state, pos) => {
            return None;
        }
        (Btn::Right, Kind::Released) if resize::on_release(state) => {
            return None;
        }
        // **The release is what makes it a click.** The framework pairs a press with a release on
        // the same widget and only then emits `RightClick` — which is what an unclaimed right-click
        // turns into a declared context menu (F004/P084/T395). Delivering only the press produced
        // no clicks at all, so no sidebar row opened a menu.
        (Btn::Right, Kind::Released) => {
            // **Both trees, and both halves of the gesture.** A pane's widgets are dispatched
            // separately from the chrome's until the pane joins the one tree
            // tree, and only *left* presses were ever handed to them — so a
            // right-click never reached a pane at all, and the menu a pane declares about itself
            // could not be found.
            //
            // The release matters as much as the press: `RightClick` is synthesised from the pair
            // on the same widget, so handing over only one half produces no click and no menu —
            // which is what `heca/tests/pointer_funnel.rs` exists to keep true.
            let handled =
                crate::chrome::deliver(state, ev) | crate::chrome::deliver_to_panes(state, ev);
            state.needs_redraw |= handled;
            return None;
        }
        // Right-click → the menu for whatever is under the cursor: a container's row, else the
        // content pane. **A right-click aims the keyboard the same way a left-click does**
        // (F003/P086/T365): clicking a container's row focuses that container, and clicking
        // outside every container releases it — so the menu that opens describes the same thing
        // the keyboard is now on.
        //
        // Aiming the keyboard first used to be impossible here: opening a menu captured the input
        // mode to restore afterwards, and moving focus first would have rewritten what it captured.
        // Nothing is restored any more, so the constraint went with it.
        // **And this is the whole of a right-click.** The press goes to the tree, whatever is
        // under it deals with it, and a widget that declared a menu gets it opened by the
        // framework. There is no branch here that knows what a pane is.
        //
        // What stood here hit-tested to find the pane, built its menu by hand and then asked to
        // focus it, in that order — so the menu it had just opened covered the panes and the focus
        // was refused every single time: right-clicking a pane never focused it. The pane declares
        // its own menu now, like every other widget. Deleting the
        // special case deletes the ordering it got wrong, which is the point: swapping two lines
        // would have left the next person the same trap.
        (Btn::Right, Kind::Pressed) => {
            let handled =
                crate::chrome::deliver(state, ev) | crate::chrome::deliver_to_panes(state, ev);
            state.needs_redraw |= handled;
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

/// Route a left-press that lands on a pane's search field into that field: focus it,
/// re-enter query entry, and let the `Input` place the caret (it hit-tests the char
/// position from its own bounds + font, both set when the bar was painted).
///
/// Returns whether the press was consumed.
///
/// Without this the field was a dead end: Enter left query entry, and there was no way
/// back in — clicking it did nothing, because the bar is painted onto the chrome scene
/// and never took part in hit-testing.
fn search_field_press(state: &mut AppState, pos: (f32, f32)) -> bool {
    use heca_grid_ui::Component as _;
    use heca_grid_ui::reactive::SignalUpdate as _;

    let pos = heca_core::layout::Point::new(pos.0 as f64, pos.1 as f64);
    let hit = state.searches.iter().find_map(|(&pane_id, search)| {
        search
            .input
            .borrow()
            .base()
            .bounds
            .contains(pos)
            .then_some(pane_id)
    });
    let Some(pane_id) = hit else {
        return false;
    };
    if let Some(search) = state.searches.get(&pane_id) {
        let mut field = search.input.borrow_mut();
        field.base_mut().focused.set(true);
        heca_grid_ui::dispatch(
            &mut *field,
            &heca_grid_ui::Event::pointer_pressed(pos, heca_grid_ui::PointerButton::Left),
        );
    }
    state.input_mode = crate::app_state::InputMode::Search;
    state.needs_redraw = true;
    true
}
