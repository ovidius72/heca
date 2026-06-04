//! Window event dispatch helpers.
//!
//! This module keeps per-event routing out of `main.rs` while preserving the
//! existing winit-driven behavior.

use crate::actions::ActionRegistry;
use crate::app::input::{KeyInputContext, handle_keyboard_input};
use crate::app::keyboard::{build_event_combo, is_prefix_match};
use crate::app::render::{render_frame, update_session_viewport};
use crate::app_state::AppState;
use crate::keymap::{KeyCombo, KeymapRegistry};
use crate::mouse;
use std::collections::HashMap;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;

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
                .primitive_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state
                .text_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            update_session_viewport(state);
            state.needs_redraw = true;
        }
        WindowEvent::RedrawRequested => {
            state.needs_redraw = true;
            render_frame(state);
        }
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            state.scale_factor = scale_factor;
            state.text_renderer.set_scale_factor(scale_factor);
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
        }
        WindowEvent::CursorMoved { position, .. } => {
            let pos = (
                position.x as f32 / state.scale_factor as f32,
                position.y as f32 / state.scale_factor as f32,
            );
            state.mouse.pos = pos;
            if let Some(action) = mouse::on_cursor_moved(state, pos) {
                registry.execute(&action, state);
            }
            state.needs_redraw = true;
        }
        WindowEvent::MouseInput {
            state: button_state,
            button,
            ..
        } => {
            if let Some(action) = mouse::on_mouse_input(state, button, button_state) {
                registry.execute(&action, state);
            }
            state.needs_redraw = true;
        }
        _ => {}
    }
}
