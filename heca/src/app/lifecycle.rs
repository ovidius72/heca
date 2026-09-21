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

/// **Everything that wants the loop back at a TIME rather than on an event.**
///
/// Gathered as data so the choice below is a pure function: the app cannot be built in a test (it
/// needs a window), and this is the part worth asserting.
pub(crate) struct WakeRequests {
    /// Something on screen is mid-animation, so the next frame is due at the frame interval.
    pub(crate) animating: bool,
    /// Seconds until a widget wants drawing — a tooltip revealing under a resting pointer, a caret
    /// blinking. `None` when no tree is waiting on the clock.
    pub(crate) widget_in: Option<f32>,
    /// When the next toast auto-dismisses, or `None` when the stack is holding its deadlines.
    pub(crate) toast_expiry: Option<Instant>,
}

/// What the loop should do next: sleep until [`wake_at`](Wake::wake_at), or until an event when it
/// is `None`.
pub(crate) struct Wake {
    pub(crate) wake_at: Option<Instant>,
    /// The widget deadline to remember, because **arriving at it is itself the reason to draw**:
    /// by then the widget no longer reports a pending wake, so every other reason is false and the
    /// loop would wake and go straight back to sleep with the bubble still unshown.
    pub(crate) widget_frame_due: Option<Instant>,
}

/// **The nearest pending deadline wins — that is the whole rule.**
///
/// It used to be an if / else-if chain, and only the winning arm ran, so only the winning arm's
/// deadline was booked: a widget that asked to be woken had its request dropped outright whenever
/// anything was animating, and the toast expiry was outranked by both. Nothing looked wrong,
/// because while something animates the frames arrive anyway — the moment the animation stopped,
/// the deadline was simply gone and the widget waited for the user to nudge something.
///
/// A list has no order to get wrong, and the next subsystem that needs a wake adds a field instead
/// of working out what outranks what. Note the widget's deadline is remembered whatever else wins:
/// waking earlier for another reason is harmless, because the wake is recomputed and re-booked on
/// that frame.
///
/// ⚠️ Every entry must be a deadline something is actually waiting for. An entry that is always
/// `Some` turns an idle window into a poller, which is what the whole design avoids.
pub(crate) fn next_wake(now: Instant, req: WakeRequests) -> Wake {
    let widget_frame_due = req
        .widget_in
        .map(|secs| now + Duration::from_secs_f32(secs.max(0.0)));
    let wake_at = [
        req.animating.then(|| now + crate::chrome::FRAME_INTERVAL),
        widget_frame_due,
        req.toast_expiry,
    ]
    .into_iter()
    .flatten()
    .min();
    Wake {
        wake_at,
        widget_frame_due,
    }
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
    // **Take the standing frame request, so the next one gets through.** Widgets coalesce their
    // asks into one (`heca_grid_ui::request_frame`); this is the point at which that one has been
    // received and a fresh ask is meaningful again. Anything marking itself further down this pass
    // is asking for the *next* frame and must be able to wake us.
    heca_grid_ui::frame_served();
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

    // **What the widgets themselves are waiting for.** A tooltip revealing under a resting pointer,
    // a caret blinking — behaviour due at a time rather than on an event, so nothing would draw it.
    // The widget says when; one question covers every tree the app draws.
    //
    // ⚠️ **Scheduling the wake is only half of it.** Arriving at the deadline, every reason-to-draw
    // above is false — the widget no longer reports a pending wake, because it is due *now* — so the
    // loop would wake and go straight back to sleep, and the bubble would still be waiting for the
    // user to nudge the mouse. The deadline is remembered, and reaching it is itself a reason to
    // draw.
    let widget_wake = crate::chrome::next_redraw_across_trees(state);
    let widget_due = state
        .widget_frame_due
        .is_some_and(|due| Instant::now() >= due);
    if widget_due {
        state.widget_frame_due = None;
    }

    let terminal_animating = backend_poll.terminal_animating;
    // An animated inline image (GIF/APNG) keeps the loop ticking so frames advance.
    let image_animating = state.has_animated_images;
    // **Every reason, by name** — see `frame_reasons`. It was a ten-term `||` chain, which cannot
    // say which term was true, so a loop that spins at 100% with nothing happening had no way to
    // name what was asking. `HECA_LOG_FRAMES=1` now prints exactly that.
    let reasons = super::frame_reasons::FrameReasons {
        marked: state.needs_redraw,
        backend_data: backend_poll.has_data,
        backend_closed: backend_poll.closed_any,
        chrome_runtime: chrome_runtime_changed,
        bell_flashing,
        session_animating: state.session.are_animations_ongoing(),
        terminal_animating,
        image_animating,
        chrome_animating,
        widget_due,
    };
    state.frame_log.record(reasons, Instant::now());
    if reasons.any() {
        state.window.request_redraw();
    }

    let wake_animating = state.session.are_animations_ongoing()
        || terminal_animating
        || image_animating
        || chrome_animating;
    let toast_expiry = match state.notifications.is_hovered() {
        true => None,
        false => state.notifications.next_expiry(),
    };
    state
        .frame_log
        .record_wake(wake_animating, widget_wake, toast_expiry);
    let schedule = next_wake(
        Instant::now(),
        WakeRequests {
            animating: wake_animating,
            widget_in: widget_wake,
            // **Nothing to wake for while the pointer rests on the stack.** The deadlines are
            // frozen, so waking at one would find nothing due and re-arm at the same instant — a
            // spin, for as long as the pointer stayed. What ends the hold is a pointer event,
            // which wakes the loop on its own.
            toast_expiry,
        },
    );
    state.widget_frame_due = schedule.widget_frame_due;
    match schedule.wake_at {
        Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
        None => event_loop.set_control_flow(ControlFlow::Wait),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requests(
        animating: bool,
        widget_in: Option<f32>,
        toast_in: Option<f32>,
        now: Instant,
    ) -> WakeRequests {
        WakeRequests {
            animating,
            widget_in,
            toast_expiry: toast_in.map(|s| now + Duration::from_secs_f32(s)),
        }
    }

    /// **A widget's request survives an animation.** This is the bug the list replaced: the chain
    /// tested "is anything animating" first, so while something moved on screen the widget's
    /// deadline was never even computed — and `widget_frame_due`, which is what makes arriving at
    /// it a reason to draw, stayed empty. It looked fine because the animation was producing
    /// frames; the tooltip went missing the moment the animation stopped.
    #[test]
    fn an_animation_does_not_swallow_a_widget_deadline() {
        let now = Instant::now();
        let wake = next_wake(now, requests(true, Some(0.5), None, now));
        assert_eq!(
            wake.widget_frame_due,
            Some(now + Duration::from_secs_f32(0.5)),
            "the widget asked to be woken and something else was animating",
        );
    }

    /// **The nearest deadline wins, whoever asked for it.** A widget due before the next animation
    /// frame is not made to wait for it, and a toast is not outranked by either.
    #[test]
    fn the_nearest_pending_deadline_is_the_one_chosen() {
        let now = Instant::now();
        let soon = Duration::from_millis(4);
        assert_eq!(
            next_wake(now, requests(true, Some(soon.as_secs_f32()), None, now)).wake_at,
            Some(now + soon),
            "a widget due inside the frame interval",
        );
        assert_eq!(
            next_wake(now, requests(true, None, Some(0.004), now)).wake_at,
            Some(now + soon),
            "a toast due inside the frame interval",
        );
        assert_eq!(
            next_wake(now, requests(true, Some(1.0), Some(2.0), now)).wake_at,
            Some(now + crate::chrome::FRAME_INTERVAL),
            "the animation is the soonest of the three",
        );
    }

    /// **Nothing pending, nothing booked.** The window sleeps until an event; an entry that were
    /// always `Some` would turn an idle app into a poller, which is the thing this design avoids.
    #[test]
    fn an_idle_window_books_no_wake() {
        let now = Instant::now();
        let wake = next_wake(now, requests(false, None, None, now));
        assert_eq!(wake.wake_at, None);
        assert_eq!(wake.widget_frame_due, None);
    }
}
