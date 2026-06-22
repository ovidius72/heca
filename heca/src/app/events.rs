//! Window event dispatch helpers.
//!
//! This module keeps per-event routing out of `main.rs` while preserving the
//! existing winit-driven behavior.

use crate::actions::ActionRegistry;
use crate::app::input::{KeyInputContext, handle_keyboard_input};
use crate::app::interaction::{dispatch_action, InteractionIntent, InteractionSource};
use crate::app::keyboard::{build_event_combo, is_prefix_match};
use crate::app::mutations::{after_mutation_change, MutationKind};
use crate::app::render::{render_frame, update_session_viewport};
use crate::app::terminal_host::{
    forward_mouse_button, forward_mouse_move, forward_mouse_wheel, notify_window_focus_changed,
};
use crate::app::terminal_metrics::refresh_terminal_cell_size;
use crate::app_state::AppState;
use crate::keymap::{KeyCombo, KeymapRegistry};
use crate::mouse;
use std::collections::HashMap;
use winit::event::{ElementState, WindowEvent};
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
            state
                .text_renderer
                .set_target_size(phys.width, phys.height);
            state
                .grid_renderer
                .set_target_size(phys.width, phys.height);
            state
                .primitive_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state
                .text_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state
                .grid_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state.compositor.resize(&state.device, phys.width, phys.height);
            state.blur.resize(&state.device, phys.width, phys.height);
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
            state.mark_full_redraw();
        }
        WindowEvent::CursorMoved { position, .. } => {
            let pos = (
                position.x as f32 / state.scale_factor as f32,
                position.y as f32 / state.scale_factor as f32,
            );
            state.mouse.pos = pos;
            if let Some(action) = mouse::on_cursor_moved(state, pos) {
                dispatch_action(state, registry, InteractionSource::MouseContent, &action);
            }
            // Feed the move into the retained chrome tree so sidebar hover affordances
            // (MarkerGroup grip "grab" cue, Row hover) light up — the app otherwise
            // only sends presses. NOT during a drag: otherwise pane rows would light
            // their hover as if droppable, contradicting the source-aware drop
            // indicator (a column drag targets columns, not the panes inside them).
            if !state.mouse.drag_ctx.is_dragging() && !mouse::is_resizing(state) {
                crate::chrome::chrome_dispatch_move(state, pos);
                // Feed the move into the retained pane-info-bar headers so the action
                // buttons' hover affordance lights up (repaint via mark_full_redraw below).
                crate::chrome::dispatch_pane_header_move(state, pos);
            }
            // Don't forward moves to the terminal while resizing a divider — the
            // gesture owns the pointer until release.
            if !mouse::is_resizing(state) {
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
            let interactive_before = state.mouse.interactive_move.is_some();
            let resize_before = mouse::is_resizing(state);
            if let Some((action, source)) = mouse::on_mouse_input(state, button, button_state) {
                dispatch_action(state, registry, source, &action);
            }
            let started_interactive_move = !interactive_before && state.mouse.interactive_move.is_some();
            // A button event that started, drove, or ended a divider resize (e.g.
            // the right-button fallback press, or a release) must not also reach the
            // terminal — the gesture consumed it.
            let resize_consumed = resize_before || mouse::is_resizing(state);
            if !started_interactive_move && !resize_consumed {
                forward_mouse_button(state, state.mouse.pos, button, button_state, registry);
            }
            // Snap the cursor on press/release (drag start → Grabbing, drop → Grab/Default)
            // without waiting for the next move.
            mouse::update_cursor(state, state.mouse.pos);
            state.mark_full_redraw();
        }
        WindowEvent::MouseWheel { delta, .. } => {
            forward_mouse_wheel(state, state.mouse.pos, delta);
            state.mark_full_redraw();
        }
        _ => {}
    }
}
