//! Window event dispatch helpers.
//!
//! This module keeps per-event routing out of `main.rs` while preserving the
//! existing winit-driven behavior.

use crate::actions::ActionRegistry;
use crate::app::input::{KeyInputContext, handle_keyboard_input};
use crate::app::interaction::{InteractionIntent, InteractionSource, dispatch_action};
use crate::app::keyboard::{build_event_combo, is_prefix_match};
use crate::app::mutations::{MutationKind, after_mutation_change};
use crate::app::render::{render_frame, update_session_viewport};
use crate::app::terminal_host::{
    forward_mouse_button, forward_mouse_move, forward_mouse_wheel, notify_window_focus_changed,
};
use crate::app::terminal_metrics::refresh_terminal_cell_size;
use crate::app_state::AppState;
use crate::input::{FontZoomStep, WmAction};
use crate::keymap::{KeyCombo, KeymapRegistry};
use crate::mouse;
use heca_core::layout::Point;
use heca_grid_ui::reactive::SignalGet;
use heca_grid_ui::{Component, Event, GridKey};
use std::collections::HashMap;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
#[derive(Clone, Debug)]
pub enum AppEvent {
    BackendWake,
    RequestRedraw,
    ChromeIntent {
        source: InteractionSource,
        intent: InteractionIntent,
    },
}

pub(crate) fn handle_window_event(
    event_loop: &ActiveEventLoop,
    registry: &ActionRegistry,
    keymap: &KeymapRegistry,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    mode_triggers: &HashMap<String, (KeyCombo, bool)>,
    state: &mut AppState,
    event: WindowEvent,
) {
    match event {
        WindowEvent::CloseRequested => event_loop.exit(),
        WindowEvent::Resized(phys) if phys.width > 0 && phys.height > 0 => {
            state.surface_config.width = phys.width;
            state.surface_config.height = phys.height;
            state
                .surface
                .configure(&state.device, &state.surface_config);
            let log_w = phys.width as f32 / state.scale_factor as f32;
            let log_h = phys.height as f32 / state.scale_factor as f32;
            state.text_renderer.set_target_size(phys.width, phys.height);
            state.grid_renderer.set_target_size(phys.width, phys.height);
            state
                .primitive_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state
                .text_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state
                .grid_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state
                .compositor
                .resize(&state.device, phys.width, phys.height);
            state.blur.resize(&state.device, phys.width, phys.height);
            state
                .background
                .resize(&state.device, phys.width, phys.height);
            update_session_viewport(state);
            after_mutation_change(state, MutationKind::Config);
        }
        WindowEvent::RedrawRequested => {
            state.needs_redraw = true;
            render_frame(state);
        }
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            state.scale_factor = scale_factor;
            state.text_renderer.set_scale_factor(scale_factor);
            state.grid_renderer.set_scale_factor(scale_factor);
            state.terminal_layers.clear();
            refresh_terminal_cell_size(state);
            state.mark_full_redraw();
        }
        WindowEvent::Focused(focused) => {
            state.window_focused = focused;
            if focused {
                state.window.request_user_attention(None);
            }
            notify_window_focus_changed(state, focused);
            state.mark_full_redraw();
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed {
                return;
            }
            state.mark_full_redraw();

            // An open context menu captures the keyboard: navigation (↑/↓/Enter/Esc)
            // and quick-pick letters drive it; any other key is swallowed so it does
            // not leak to the focused terminal while the menu is up.
            if context_menu_open(state) {
                if let Some(grid_key) = winit_key_to_grid_key(&event.logical_key) {
                    if let Some(menu) = state.context_menu.as_mut() {
                        let _ = menu.event(&Event::Key {
                            key: grid_key,
                            pressed: true,
                        });
                    }
                    settle_context_menu(state, registry, InteractionSource::Keyboard);
                }
                return;
            }

            // A visible modal **layer** (overlay dialog) owns the keyboard: a letter shortcut
            // fires its action via `SubmitOverlay`; every other key is forwarded to the dialog
            // root (Tab / ←→ / Enter / Space / Esc — the `Dialog` self-handles them + fires its
            // `on_dismiss`). Any key is swallowed so nothing leaks to the pane behind the scrim.
            if let Some(overlay) = crate::chrome::top_modal(state) {
                if let Some(grid_key) = winit_key_to_grid_key(&event.logical_key) {
                    if let GridKey::Char(c) = grid_key
                        && let Some(action) = crate::chrome::shortcut_action(state, overlay, c)
                    {
                        dispatch_action(
                            state,
                            registry,
                            InteractionSource::Keyboard,
                            &WmAction::SubmitOverlay { overlay, action },
                        );
                        state.mark_full_redraw();
                        return;
                    }
                    if let Some(root) = state.layers.top_modal_root_mut() {
                        let _ = root.event(&Event::Key { key: grid_key, pressed: true });
                    }
                }
                state.mark_full_redraw();
                return;
            }

            let is_ctrl = state.modifiers.control_key();
            let is_shift = state.modifiers.shift_key();
            let log_key = &event.logical_key;
            let key_text = log_key.to_text().unwrap_or("").to_string();
            let event_combo = build_event_combo(
                &event.logical_key,
                &event.physical_key,
                &key_text,
                state.modifiers,
            );
            let is_prefix = is_prefix_match(&event_combo, &state.prefix_combo, log_key, &key_text);

            handle_keyboard_input(
                registry,
                keymap,
                mode_keymaps,
                mode_triggers,
                state,
                KeyInputContext {
                    logical_key: &event.logical_key,
                    physical_key: &event.physical_key,
                    key_text: &key_text,
                    event_combo: &event_combo,
                    is_prefix,
                    is_ctrl,
                    is_shift,
                },
            );
        }
        WindowEvent::ModifiersChanged(new_mods) => {
            state.modifiers = new_mods.state();
            mouse::on_modifiers_changed(state);
            // Refresh the cursor affordance: pressing/releasing Cmd over a link
            // toggles the pointer cue even without pointer movement.
            mouse::update_cursor(state, state.mouse.pos);
            state.mark_full_redraw();
        }
        WindowEvent::CursorMoved { position, .. } => {
            let pos = (
                position.x as f32 / state.scale_factor as f32,
                position.y as f32 / state.scale_factor as f32,
            );
            state.mouse.pos = pos;
            // An open context menu owns the pointer: route moves to it (row hover)
            // and stop here so the move does not also drive focus/drag/terminal.
            if context_menu_open(state) {
                if let Some(menu) = state.context_menu.as_mut() {
                    let _ = menu.event(&Event::PointerMoved {
                        pos: Point::new(pos.0 as f64, pos.1 as f64),
                    });
                }
                state.mark_full_redraw();
                return;
            }
            // A visible modal layer owns the pointer while open (hover on its buttons).
            if crate::chrome::top_modal(state).is_some() {
                if let Some(root) = state.layers.top_modal_root_mut() {
                    let _ = root.event(&Event::PointerMoved {
                        pos: Point::new(pos.0 as f64, pos.1 as f64),
                    });
                }
                state.mark_full_redraw();
                return;
            }
            if let Some(action) = mouse::on_cursor_moved(state, pos) {
                dispatch_action(state, registry, InteractionSource::MouseContent, &action);
            }
            // Feed the move into the retained chrome tree so sidebar hover affordances
            // (MarkerGroup grip "grab" cue, Row hover) light up — the app otherwise
            // only sends presses. NOT during a drag: otherwise pane rows would light
            // their hover as if droppable, contradicting the source-aware drop
            // indicator (a column drag targets columns, not the panes inside them).
            let mut pane_viewport_over = false;
            if !state.mouse.drag_ctx.is_dragging() && !mouse::is_resizing(state) {
                crate::chrome::chrome_dispatch_move(state, pos);
                // Feed the move into the retained pane-info-bar headers so the action
                // buttons' hover affordance lights up (repaint via mark_full_redraw below).
                crate::chrome::dispatch_pane_header_move(state, pos);
                pane_viewport_over = crate::chrome::dispatch_pane_viewport_move(state, pos);
            }
            // Don't forward moves to the terminal while resizing a divider or while a
            // retained viewport widget (badge / scrollbar) owns the pointer.
            if !mouse::is_resizing(state) && !pane_viewport_over {
                forward_mouse_move(state, pos);
            }
            // Cursor affordance: Grab over a draggable, Grabbing while dragging.
            mouse::update_cursor(state, pos);
            state.mark_full_redraw();
        }
        WindowEvent::MouseInput {
            state: button_state,
            button,
            ..
        } => {
            // An open context menu swallows all button input: a press selects a row
            // (or dismisses on an outside-click); the release is consumed. Routed
            // before any normal mouse handling so clicks land on the menu, not panes.
            if context_menu_open(state) {
                if button_state == ElementState::Pressed {
                    let pos = state.mouse.pos;
                    if let Some(menu) = state.context_menu.as_mut() {
                        let _ = menu.event(&Event::PointerPressed {
                            pos: Point::new(pos.0 as f64, pos.1 as f64),
                        });
                    }
                    settle_context_menu(state, registry, InteractionSource::MouseContent);
                }
                state.mark_full_redraw();
                return;
            }
            // A visible modal layer swallows all button input: a press on a button (its
            // `on_click` emits `SubmitOverlay`) or the scrim (`Dialog::on_dismiss` emits
            // `CloseOverlay`) resolves it; anything else is consumed so clicks don't leak.
            if crate::chrome::top_modal(state).is_some() {
                if button_state == ElementState::Pressed {
                    let pos = state.mouse.pos;
                    if let Some(root) = state.layers.top_modal_root_mut() {
                        let _ = root.event(&Event::PointerPressed {
                            pos: Point::new(pos.0 as f64, pos.1 as f64),
                        });
                    }
                }
                state.mark_full_redraw();
                return;
            }
            // Pane info-bar action **buttons** intercept a plain left-press so a click
            // hits the button (not the terminal). Only an actual button hit is
            // consumed — a press on the empty header band falls through to the normal
            // content/drag/resize paths (the lower pane's band sits on the divider, so
            // consuming it would break divider/resize gestures). A modifier-held press
            // also falls through (meta-drag).
            if button == winit::event::MouseButton::Left
                && button_state == ElementState::Pressed
                && !mouse::interactive_move_modifier_held(state)
                && let Some((_pane_id, true)) =
                    crate::chrome::dispatch_pane_header_press(state, state.mouse.pos)
            {
                mouse::update_cursor(state, state.mouse.pos);
                state.mark_full_redraw();
                return;
            }
            // Terminal viewport widgets (scrollbar / badge) intercept a plain
            // left-press before divider resize/content forwarding.
            if button == winit::event::MouseButton::Left
                && button_state == ElementState::Pressed
                && !mouse::interactive_move_modifier_held(state)
                && crate::chrome::dispatch_pane_viewport_press(state, state.mouse.pos)
            {
                mouse::update_cursor(state, state.mouse.pos);
                state.mark_full_redraw();
                return;
            }
            // Divider resize: a plain left-press on a column/pane divider starts a
            // resize-drag. Only an actual divider hit consumes — a miss falls
            // through to the normal content/drag paths. A modifier-held press
            // (meta-drag) falls through. Checked after the header-button block
            // because the lower pane's header band sits on top of the divider.
            if button == winit::event::MouseButton::Left
                && button_state == ElementState::Pressed
                && !mouse::interactive_move_modifier_held(state)
                && mouse::resize::on_press(state, state.mouse.pos)
            {
                mouse::update_cursor(state, state.mouse.pos);
                state.mark_full_redraw();
                return;
            }
            if button == winit::event::MouseButton::Left
                && button_state == ElementState::Released
                && crate::chrome::dispatch_pane_viewport_release(state, state.mouse.pos)
            {
                mouse::update_cursor(state, state.mouse.pos);
                state.mark_full_redraw();
                return;
            }
            let interactive_before = state.mouse.interactive_move.is_some();
            let resize_before = mouse::is_resizing(state);
            if let Some((action, source)) = mouse::on_mouse_input(state, button, button_state) {
                dispatch_action(state, registry, source, &action);
            }
            let started_interactive_move =
                !interactive_before && state.mouse.interactive_move.is_some();
            // A button event that started, drove, or ended a divider resize (e.g.
            // the right-button fallback press, or a release) must not also reach the
            // terminal — the gesture consumed it.
            let resize_consumed = resize_before || mouse::is_resizing(state);
            // A right-press that just opened the context menu must not also forward
            // to the terminal (it would deliver a stray right-click to the TUI).
            let opened_context_menu = context_menu_open(state);
            if !started_interactive_move && !resize_consumed && !opened_context_menu {
                forward_mouse_button(state, state.mouse.pos, button, button_state, registry);
            }
            // Snap the cursor on press/release (drag start → Grabbing, drop → Grab/Default)
            // without waiting for the next move.
            mouse::update_cursor(state, state.mouse.pos);
            state.mark_full_redraw();
        }
        WindowEvent::MouseWheel { delta, .. } => {
            // Ctrl/Meta+wheel is a font-zoom gesture, resolved by what's under the
            // pointer. It is intercepted at the WM level BEFORE terminal wheel
            // forwarding so the modified wheel never reaches the TUI as a scroll.
            if !handle_wheel_font_zoom(state, registry, state.mouse.pos, delta) {
                forward_mouse_wheel(state, state.mouse.pos, delta);
            }
            state.mark_full_redraw();
        }
        _ => {}
    }
}

/// Handle a `Ctrl`/`Meta`+wheel font-zoom gesture. Returns `true` when the wheel
/// event was consumed as a zoom (so the caller must NOT forward it to terminal
/// scroll). Resolution is pointer-based: over a pane → zoom that pane; over
/// chrome/empty space → app-wide zoom. Scroll up zooms in, down zooms out.
fn handle_wheel_font_zoom(
    state: &mut AppState,
    registry: &ActionRegistry,
    pos: (f32, f32),
    delta: MouseScrollDelta,
) -> bool {
    if !state.mouse_wheel_change_font_size {
        return false;
    }
    let mods = state.modifiers;
    if !(mods.control_key() || mods.super_key()) {
        return false;
    }

    // Consume the gesture regardless of direction so a modified wheel never leaks
    // to the TUI; only dispatch when there is a usable vertical direction.
    let vertical = match delta {
        MouseScrollDelta::LineDelta(_, y) => y,
        MouseScrollDelta::PixelDelta(p) => p.y as f32,
    };
    if vertical == 0.0 {
        return true;
    }
    let step = if vertical > 0.0 {
        FontZoomStep::In
    } else {
        FontZoomStep::Out
    };

    let action = match mouse::hit_test_pane(state, pos) {
        Some(pane_id) => WmAction::PaneTerminalFontZoom {
            pane_id: Some(pane_id),
            step,
        },
        None => WmAction::AppFontZoom { step },
    };
    dispatch_action(state, registry, InteractionSource::MouseContent, &action);
    true
}

/// Whether a right-click context menu is currently open.
fn context_menu_open(state: &AppState) -> bool {
    state
        .context_menu
        .as_ref()
        .is_some_and(|m| m.open_signal().get_untracked())
}

/// After feeding an event into the open menu: dispatch whatever action the chosen
/// entry queued in the sink, then drop the menu if it closed itself (entry run or
/// outside-click). One settle path shared by the mouse + keyboard routes.
fn settle_context_menu(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
) {
    let action = state.context_menu_action.borrow_mut().take();
    if let Some(action) = action {
        // Destructive menu picks go through the centralized confirm chokepoint (so a
        // right-click delete honours the same `confirm_*` config as the keyboard);
        // everything else dispatches normally.
        if matches!(
            action,
            WmAction::ClosePaneById { .. }
                | WmAction::DeleteColumn { .. }
                | WmAction::DeleteWorkspace { .. }
        ) {
            crate::handlers::request_destructive(state, action, false);
        } else {
            dispatch_action(state, registry, source, &action);
        }
    }
    let closed = state
        .context_menu
        .as_ref()
        .is_none_or(|m| !m.open_signal().get_untracked());
    if closed {
        state.context_menu = None;
    }
    state.mark_full_redraw();
}

/// Map a winit key to the grid-ui [`GridKey`] the menu understands. Returns `None`
/// for keys the menu ignores (still swallowed while it is open).
fn winit_key_to_grid_key(key: &winit::keyboard::Key) -> Option<GridKey> {
    use winit::keyboard::{Key, NamedKey};
    match key {
        Key::Named(NamedKey::Escape) => Some(GridKey::Escape),
        Key::Named(NamedKey::Enter) => Some(GridKey::Enter),
        Key::Named(NamedKey::ArrowUp) => Some(GridKey::ArrowUp),
        Key::Named(NamedKey::ArrowDown) => Some(GridKey::ArrowDown),
        Key::Character(s) => s.chars().next().map(GridKey::Char),
        _ => None,
    }
}
