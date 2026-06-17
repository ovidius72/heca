//! Window event dispatch helpers.
//!
//! This module keeps per-event routing out of `main.rs` while preserving the
//! existing winit-driven behavior.

use crate::actions::ActionRegistry;
use crate::app::input::{KeyInputContext, handle_keyboard_input};
use crate::app::interaction::{dispatch_action, InteractionSource};
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
            state.needs_redraw = true;
        }
        WindowEvent::Focused(focused) => {
            state.window_focused = focused;
            if focused {
                state.window.request_user_attention(None);
            }
            notify_window_focus_changed(state, focused);
            state.needs_redraw = true;
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed {
                return;
            }
            state.needs_redraw = true;

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
            state.needs_redraw = true;
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
            // only sends presses. Repaint is already requested below.
            crate::chrome::chrome_dispatch_move(state, pos);
            forward_mouse_move(state, pos);
            state.needs_redraw = true;
        }
        WindowEvent::MouseInput {
            state: button_state,
            button,
            ..
        } => {
            let interactive_before = state.mouse.interactive_move.is_some();
            if let Some((action, source)) = mouse::on_mouse_input(state, button, button_state) {
                dispatch_action(state, registry, source, &action);
            }
            let started_interactive_move = !interactive_before && state.mouse.interactive_move.is_some();
            if !started_interactive_move {
                forward_mouse_button(state, state.mouse.pos, button, button_state, registry);
            }
            state.needs_redraw = true;
        }
        WindowEvent::MouseWheel { delta, .. } => {
            forward_mouse_wheel(state, state.mouse.pos, delta);
            state.needs_redraw = true;
        }
        _ => {}
    }
}
