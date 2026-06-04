//! Application lifecycle tick helpers.
//!
//! This module keeps redraw / animation / backend polling orchestration out of
//! `main.rs`.

use crate::app_state::{AppState, DragState, InputMode};
use crate::mouse;
use std::time::{Duration, Instant};
use winit::event_loop::{ActiveEventLoop, ControlFlow};

pub(crate) fn handle_about_to_wait(event_loop: &ActiveEventLoop, state: &mut AppState) {
    let should_timeout = matches!(
        state.input_mode,
        InputMode::Prefix | InputMode::Chord { .. }
    ) && state
        .prefix_entered_at
        .is_some_and(|entered| entered.elapsed() >= Duration::from_millis(500));
    if should_timeout {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
        state.needs_redraw = true;
    }

    let edge_scrolled = mouse::process_edge_scroll(state);
    if edge_scrolled {
        state.needs_redraw = true;
    }

    if !matches!(state.mouse.drag_state, DragState::None) {
        state.needs_redraw = true;
    }

    state.session.advance_animations();

    let mut backend_has_data = false;
    for backend in state.backends.values_mut() {
        if backend.update() {
            backend_has_data = true;
        }
    }

    let needs_frame =
        state.needs_redraw || backend_has_data || state.session.are_animations_ongoing();
    if needs_frame {
        state.window.request_redraw();
    }

    if state.session.are_animations_ongoing() {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
    } else {
        event_loop.set_control_flow(ControlFlow::Wait);
    }
}
