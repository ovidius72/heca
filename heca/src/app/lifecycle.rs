//! Application lifecycle tick helpers.
//!
//! This module keeps redraw / animation / backend polling orchestration out of
//! `main.rs`.

use crate::app::mutations::after_config_change;
use crate::app::mutations::close_pane_by_id_anywhere;
use crate::app_state::{AppState, InputMode};
use heca_core::backend::BackendAlert;
use heca_grid_ui::Component;
use crate::mouse;
use std::time::Instant;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::UserAttentionType;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BackendPollResult {
    pub has_data: bool,
    pub closed_any: bool,
    pub bell_any: bool,
}

pub(crate) fn poll_backends(state: &mut AppState) -> BackendPollResult {
    let mut result = BackendPollResult::default();
    for backend in state.backends.values_mut() {
        if backend.update() {
            result.has_data = true;
        }
        for alert in backend.take_alerts() {
            if matches!(alert, BackendAlert::Bell) {
                result.bell_any = true;
            }
        }
    }
    let closing_panes = state.backends.pane_ids_to_close();
    result.closed_any = !closing_panes.is_empty();
    for pane_id in closing_panes {
        close_pane_by_id_anywhere(state, pane_id);
    }

    result
}

pub(crate) fn handle_about_to_wait(event_loop: &ActiveEventLoop, state: &mut AppState) {
    let should_timeout = matches!(
        state.input_mode,
        InputMode::Prefix | InputMode::Chord { .. }
    ) && state
        .prefix_entered_at
        .is_some_and(|entered| entered.elapsed() >= crate::chrome::PREFIX_TIMEOUT);
    if should_timeout {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
        state.needs_redraw = true;
    }

    let edge_scrolled = mouse::process_edge_scroll(state);
    if edge_scrolled {
        after_config_change(state);
    }

    if state.mouse.drag_ctx.is_dragging() || state.mouse.interactive_move.is_some() {
        state.needs_redraw = true;
    }

    state.session.advance_animations();
    let chrome_animating = if let Some(tree) = state.chrome_tree.as_mut() {
        tree.root.tick(crate::chrome::FRAME_INTERVAL.as_secs_f32())
    } else {
        false
    };

    let backend_poll = poll_backends(state);
    if backend_poll.bell_any && !state.window_focused {
        state
            .window
            .request_user_attention(Some(UserAttentionType::Informational));
    }

    let needs_frame =
        state.needs_redraw
            || backend_poll.has_data
            || backend_poll.closed_any
            || state.session.are_animations_ongoing()
            || chrome_animating;
    if needs_frame {
        state.window.request_redraw();
    }

    if state.session.are_animations_ongoing() || chrome_animating {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + crate::chrome::FRAME_INTERVAL,
        ));
    } else {
        event_loop.set_control_flow(ControlFlow::Wait);
    }
}
