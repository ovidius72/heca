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
use crate::keymap::Keymaps;
use crate::mouse;
use heca_core::layout::Point;
use heca_grid_ui::{Event, Handled};
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
    /// Request late-bound resolution of the action currently stored for a
    /// visible notification. It transports only the runtime id; dispatch stays
    /// in the app thread through `dispatch_view_intent`.
    NotificationActionRequested { notification_id: u64 },
}

pub(crate) fn handle_window_event(
    event_loop: &ActiveEventLoop,
    registry: &ActionRegistry,
    keymaps: &Keymaps,
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

            // A visible modal **layer** (overlay dialog) owns the keyboard — EXCEPT the universal
            // hint picker, which must still reach the modal's buttons (they're collected as hint
            // targets by the layer system). So the prefix trigger and any in-flight prefix /
            // hint-pick sequence fall through to the keymap machinery below; every other key is
            // just forwarded to the modal's root, which self-handles focus/activation/dismiss
            // (a `Dialog` tracks its own modifier state from the broadcast `ModifiersChanged`).
            let picker_seq = is_prefix
                || matches!(
                    state.input_mode,
                    crate::app_state::InputMode::Prefix
                        | crate::app_state::InputMode::HintPick { .. }
                );
            if !picker_seq && crate::chrome::top_modal(state).is_some() {
                // Overlay key resolution via the single host-owned widget keymap
                // (`widget-keys-config`). `Keymap::dispatch` delivers the raw key to the overlay
                // **field-first** (so an `Input`'s typing / caret and a menu's quick-pick letters
                // win), then the semantic `WidgetIntent`(s) the chord resolves to — a `Dialog`
                // takes `Item*`/`Activate`/`Dismiss`, a menu/`Select` takes `Menu*`, a focused
                // field takes `Edit*` (forwarded field-first by the overlay). One dispatch, one map.
                // Use the already-normalized `event_combo` (which carries the macOS
                // physical-key fallback for `Ctrl+letter`, unlike the raw logical key) so vim
                // `Ctrl+h/j/k/l` resolve to the right chord.
                if let Some((combo_key, mods)) = crate::app::registry::combo_to_grid(&event_combo) {
                    // For plain printable typing, deliver the ACTUAL character (case-preserved,
                    // shifted symbols intact) from `key_text` rather than the lowercased combo key
                    // — otherwise an overlay `Input` can't type uppercase (Shift) or `!@#`. The
                    // combo key stays lowercased only for chord *matching*, which `Keymap::resolve`
                    // does internally anyway. Space stays `GridKey::Space` (so it still activates a
                    // focused button); Ctrl/Meta combos and named keys keep the combo key.
                    let key = {
                        let mut cs = key_text.chars();
                        match (cs.next(), cs.next()) {
                            (Some(c), None)
                                if !mods.ctrl
                                    && !mods.meta
                                    && !c.is_control()
                                    && c != ' ' =>
                            {
                                heca_grid_ui::GridKey::Char(c)
                            }
                            _ => combo_key,
                        }
                    };
                    let keymap = state.widget_keymap.clone();
                    keymap.dispatch(key, mods, |ev| {
                        state
                            .layers
                            .top_modal_root_mut()
                            .map(|root| heca_grid_ui::dispatch(root, ev))
                            .unwrap_or(Handled::No)
                    });
                }
                state.mark_full_redraw();
                return;
            }

            handle_keyboard_input(
                registry,
                keymaps,
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
            // Broadcast to an open modal overlay so a self-contained widget (a `Dialog`) can do
            // Shift+Tab / Ctrl+h-l itself — its `Event::Key` carries no modifiers.
            if crate::chrome::top_modal(state).is_some() {
                let mods = grid_modifiers(state.modifiers);
                if let Some(root) = state.layers.top_modal_root_mut() {
                    let _ = heca_grid_ui::dispatch(root, &Event::ModifiersChanged(mods));
                }
            }
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
            // A visible modal layer owns the pointer while open (hover on its buttons + menu rows,
            // and a thumb drag inside its body).
            if crate::chrome::dispatch_modal_pointer(
                state,
                &Event::PointerMoved {
                    pos: Point::new(pos.0 as f64, pos.1 as f64),
                },
            ) {
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
            // A visible modal layer swallows all button input: a press on a button (its
            // `on_click` emits `SubmitOverlay`) or the scrim (`Dialog::on_dismiss` emits
            // `CloseOverlay`) resolves it; anything else is consumed so clicks don't leak.
            // The **release** goes in too — this branch used to forward the press alone, which is
            // what left a body's scrollbar thumb stuck to the cursor: the widget was still waiting
            // for the end of a gesture the host had decided not to deliver.
            if crate::chrome::top_modal(state).is_some() {
                let pos = Point::new(state.mouse.pos.0 as f64, state.mouse.pos.1 as f64);
                let ev = match button_state {
                    ElementState::Pressed => Event::PointerPressed { pos },
                    ElementState::Released => Event::PointerReleased { pos },
                };
                crate::chrome::dispatch_modal_pointer(state, &ev);
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
            {
                // Every retained tree that could have started a gesture gets the release, whether
                // or not the cursor is still over it — that is what ends a scrollbar drag. The
                // chrome tree is unconditional: it consumes nothing it did not start, and gating a
                // release on position is precisely how a thumb ends up welded to the cursor.
                crate::chrome::chrome_dispatch_release(state, state.mouse.pos);
                crate::chrome::dispatch_pane_header_release(state, state.mouse.pos);
                if crate::chrome::dispatch_pane_viewport_release(state, state.mouse.pos) {
                    mouse::update_cursor(state, state.mouse.pos);
                    state.mark_full_redraw();
                    return;
                }
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
            // A right-press that just opened the context menu (now a host-owned overlay layer)
            // must not also forward to the terminal (it would deliver a stray right-click to the TUI).
            let opened_context_menu = crate::chrome::top_modal(state).is_some();
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
            if handle_wheel_font_zoom(state, registry, state.mouse.pos, delta) {
                state.mark_full_redraw();
                return;
            }
            // An open modal owns the wheel: its body may be a scroll region, and the page
            // behind must stay still either way. This branch did not exist, so a described
            // scroll area inside a modal could not be scrolled at all.
            let wheel = wheel_event(state, delta);
            if crate::chrome::dispatch_modal_pointer(state, &wheel) {
                return;
            }
            // The retained chrome tree next: a hovered scroll region in the sidebar takes it, and
            // the terminal must not also scroll. A region gates on its own hover, so this is a
            // no-op whenever the pointer is over a pane.
            if crate::chrome::chrome_dispatch_wheel(state, &wheel)
                || crate::chrome::dispatch_pane_header_wheel(state, &wheel)
            {
                state.mark_full_redraw();
                return;
            }
            forward_mouse_wheel(state, state.mouse.pos, delta);
            state.mark_full_redraw();
        }
        _ => {}
    }
}

/// A winit wheel delta as grid-ui's [`Event::Scroll`], in **lines** (positive = down / right).
///
/// The host owns the modifier→axis mapping, because `Event::Scroll` carries both axes and a plain
/// mouse wheel only reports the vertical one: `Shift`+wheel is remapped to X. A trackpad already
/// fills X itself, so it is left alone. Same convention as the showcase, deliberately — a region
/// must scroll identically wherever it is mounted.
fn wheel_event(state: &AppState, delta: MouseScrollDelta) -> Event {
    let (dx_raw, dy_raw) = match delta {
        MouseScrollDelta::LineDelta(x, y) => (-x, -y),
        MouseScrollDelta::PixelDelta(p) => (-(p.x as f32) / 20.0, -(p.y as f32) / 20.0),
    };
    let (delta_x, delta_y) = if state.modifiers.shift_key() && dx_raw == 0.0 {
        (dy_raw, 0.0)
    } else {
        (dx_raw, dy_raw)
    };
    Event::Scroll { delta_x, delta_y }
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

/// The grid-ui [`Modifiers`](heca_grid_ui::Modifiers) mirror of the current winit modifier state
/// — broadcast to an open overlay so a self-contained widget (e.g. a modal `Dialog`) can do
/// Shift+Tab / Ctrl+h-l without the host special-casing it.
fn grid_modifiers(m: winit::keyboard::ModifiersState) -> heca_grid_ui::Modifiers {
    heca_grid_ui::Modifiers {
        ctrl: m.control_key(),
        alt: m.alt_key(),
        shift: m.shift_key(),
        meta: m.super_key(),
    }
}
