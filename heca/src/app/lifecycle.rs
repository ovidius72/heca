//! Application lifecycle tick helpers.
//!
//! This module keeps redraw / animation / backend polling orchestration out of
//! `main.rs`.

use crate::app::mutations::after_config_change;
use crate::app::mutations::close_pane_by_id_anywhere;
use crate::app_state::{AppState, InputMode};
use crate::mouse;
use heca_core::backend::BackendAlert;
use heca_grid_ui::Component;
use std::time::{Duration, Instant};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::UserAttentionType;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BackendPollResult {
    pub has_data: bool,
    pub closed_any: bool,
    pub bell_any: bool,
    pub terminal_animating: bool,
}

pub(crate) fn poll_backends(state: &mut AppState) -> BackendPollResult {
    let mut result = BackendPollResult::default();
    for backend in state.backends.values_mut() {
        let anim = backend.tick_animation();
        result.terminal_animating |= anim;
        if backend.update() {
            result.has_data = true;
        }
        for alert in backend.take_alerts() {
            if matches!(alert, BackendAlert::Bell) {
                result.bell_any = true;
            }
        }
        // OSC 52: a program asked to set the system clipboard. wezterm already
        // base64-decoded it; push the text to the OS clipboard.
        for text in backend.take_clipboard_writes() {
            crate::handlers::set_system_clipboard(&text);
        }
    }
    let closing_panes = state.backends.pane_ids_to_close();
    result.closed_any = !closing_panes.is_empty();
    for pane_id in closing_panes {
        close_pane_by_id_anywhere(state, pane_id);
    }

    result
}

/// How long a visual-bell flash takes to fade out. Shared with the render pass
/// (`chrome::paint_bell_flash`) so the fade fraction matches the schedule.
pub(crate) const BELL_FLASH_DURATION: Duration = Duration::from_millis(140);

/// Apply the configured bell policy (`[appearance.terminal]`) for a captured bell:
/// OS window attention (only while unfocused), an on-screen visual flash, and/or an
/// audible system beep — each independently toggled. terminal-task-17.
fn apply_bell_policy(state: &mut AppState) {
    let t = &state.appearance.terminal;
    let (attention, visual, audible) = (t.bell_attention, t.bell_visual, t.bell_audible);
    if attention && !state.window_focused {
        state
            .window
            .request_user_attention(Some(UserAttentionType::Informational));
    }
    if audible {
        ring_system_bell();
    }
    if visual {
        state.bell_flash_until = Some(Instant::now() + BELL_FLASH_DURATION);
        state.mark_full_redraw();
    }
}

#[cfg(target_os = "macos")]
fn ring_system_bell() {
    // AppKit's alert beep — respects the user's chosen system alert sound/volume.
    unsafe { objc2_app_kit::NSBeep() };
}

#[cfg(not(target_os = "macos"))]
fn ring_system_bell() {
    // No portable system beep through winit yet; audible bell is a no-op off macOS.
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
        state.mark_full_redraw();
    }

    let edge_scrolled = mouse::process_edge_scroll(state);
    if edge_scrolled {
        after_config_change(state);
    }

    if crate::chrome::drag_in_flight(state) || state.mouse.interactive_move.is_some() {
        state.mark_full_redraw();
    }

    state.session.advance_animations();
    let dt = crate::chrome::FRAME_INTERVAL.as_secs_f32();
    // **Which surfaces are mid-exit, before anything advances.** The tick below is the one that
    // advances them — they are children of this tree — so the registry has to look either side of
    // it to catch the frame an exit finishes. Reading it afterwards misses that frame entirely.
    let leaving_before = state.layers.leaving_before_tick(&state.window_root);
    let mut chrome_animating = state.window_root.tick(dt);
    // Tick every pane's own tree, so its info bar's action buttons' press flash, hover animation
    // and tooltip reveal advance (and a redraw is requested while they animate) instead of getting
    // stuck. The bar is a child of its pane, so one tick reaches both.
    for pane in state.panes.values_mut() {
        chrome_animating |= pane.root.tick(dt);
    }
    for widgets in state.pane_viewport_widgets.values_mut() {
        chrome_animating |= widgets.badge.tick(dt);
        chrome_animating |= widgets.scrollbar.tick(dt);
    }
    // Every surface — an overlay dialog, a plugin panel, the exposé — advanced in the walk above,
    // because it is a child of that tree. All that is left is to **retire the ones whose exit just
    // finished**, which is the registry's bookkeeping and not an animation pass: a surface that was
    // leaving before the tick and is not leaving now has finished, and its absence needs one more
    // frame to be painted.
    chrome_animating |= state
        .layers
        .retire_finished_exits(&mut state.window_root, &leaving_before);

    // Auto-dismiss notifications past their deadline — F009/T202. `expire_due` only touches the
    // store's own `Signal<Vec<ToastSpec>>` (F009/T208); it is not part of `chrome_runtime_changed`,
    // which tracks the chrome_tree's signature, a tree the toast surface is never part of.
    //
    // **The pointer resting on a card holds the whole stack still**, and the time that costs is
    // handed back when it leaves, so a card resumes with what it had left. The stack reports the
    // hover; what it *means* is decided in the runtime, which is where the lifetime lives.
    let now = Instant::now();
    use heca_grid_ui::reactive::SignalGet;
    let deadlines_moved = state
        .notifications
        .set_hovered(state.notification_hovered.get_untracked(), now);
    let notifications_expired = state.notifications.expire_due(now);
    if notifications_expired || deadlines_moved {
        state.needs_redraw = true;
    }

    let backend_poll = poll_backends(state);
    let chrome_runtime_changed = crate::chrome::sync_chrome_state(state);
    if backend_poll.bell_any {
        apply_bell_policy(state);
    }
    // A visual-bell flash keeps requesting frames until it fades out.
    let bell_flashing = state
        .bell_flash_until
        .is_some_and(|deadline| Instant::now() < deadline);
    if !bell_flashing {
        state.bell_flash_until = None;
    }

    let terminal_animating = backend_poll.terminal_animating;
    // An animated inline image (GIF/APNG) keeps the loop ticking so frames advance.
    let image_animating = state.has_animated_images;
    let needs_frame = state.needs_redraw
        || backend_poll.has_data
        || backend_poll.closed_any
        || chrome_runtime_changed
        || bell_flashing
        || state.session.are_animations_ongoing()
        || terminal_animating
        || image_animating
        || chrome_animating;
    if needs_frame {
        state.window.request_redraw();
    }

    if state.session.are_animations_ongoing()
        || terminal_animating
        || image_animating
        || chrome_animating
    {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + crate::chrome::FRAME_INTERVAL,
        ));
    // **Nothing to wake for while the pointer rests on the stack.** The deadlines are frozen, so
    // waking at one would find nothing due and re-arm at the same instant — a spin, for as long as
    // the pointer stayed. What ends the hold is a pointer event, which wakes the loop on its own.
    // Same contract as below, in the one case that would otherwise break it.
    } else if let Some(next_expiry) = match state.notifications.is_hovered() {
        true => None,
        false => state.notifications.next_expiry(),
    } {
        // No busy-loop (F009/T202's contract): wake exactly once, at the next auto-dismiss
        // deadline, rather than polling every frame while a sticky-free toast is up.
        event_loop.set_control_flow(ControlFlow::WaitUntil(next_expiry));
    } else {
        event_loop.set_control_flow(ControlFlow::Wait);
    }
}
