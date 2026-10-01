//! **What the wheel means over a terminal** — font zoom, host scrollback, or the wheel handed to the
//! program, decided from the mouse-grab, the settings and the device that turned it.

use crate::actions::ActionRegistry;
use crate::app::interaction::{InteractionSource, dispatch_action};
use crate::app_state::AppState;
use crate::chrome::terminal::Cell;
use crate::input::{FontZoomStep, WmAction};
use heca_core::backend::{BackendMouseButton, BackendMouseEventKind};
use heca_core::layout::PaneId;
use winit::event::MouseScrollDelta;

use super::input::backend_mouse_event;

/// **The wheel as the device reported it**, which is what the scrollback policy is written in: a
/// device that counts pixels (a trackpad) is scrolled by the cell height, one that counts notches by
/// notches. The grid's deltas run the other way round from the device's, so both are negated back.
fn device_delta((x, y): (f32, f32), pixels: Option<(f32, f32)>) -> MouseScrollDelta {
    match pixels {
        Some((px, py)) => MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
            -(px as f64),
            -(py as f64),
        )),
        None => MouseScrollDelta::LineDelta(-x, -y),
    }
}

pub(super) fn on_wheel(
    state: &mut AppState,
    registry: &ActionRegistry,
    pane_id: PaneId,
    ((x, y), pixels): ((f32, f32), Option<(f32, f32)>),
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    // **Ctrl/Meta+wheel is a font-zoom gesture**, resolved by the terminal it turned over. It is
    // consumed whatever the direction, so a modified wheel never reaches the program as a scroll.
    if state.mouse_wheel_change_font_size && (modifiers.ctrl || modifiers.meta) {
        if y != 0.0 {
            let step = if y < 0.0 {
                FontZoomStep::In
            } else {
                FontZoomStep::Out
            };
            dispatch_action(
                state,
                registry,
                InteractionSource::MouseContent,
                &WmAction::PaneTerminalFontZoom {
                    pane_id: Some(pane_id),
                    step,
                },
            );
        }
        return;
    }
    if state.mouse.interactive_move.is_some() || crate::chrome::drag_in_flight(state) {
        return;
    }
    // The grid's lines run the other way round from the device's: positive `y` is content moving
    // down, which is the wheel turned toward you.
    //
    // A device that counts pixels (a trackpad) is scrolled by the cell height, as it always was;
    // one that counts notches, by notches.
    let delta = device_delta((x, y), pixels);

    // ── Host scrollback routing ──
    //
    // Wheel → host scrollback UNLESS the terminal has mouse-grab active.
    // Shift+wheel → always host scrollback (bypasses any grab).
    //
    // Q2 policy:
    //   `shift_held`         → always host scroll (bypasses grab)
    //   `terminal_mouse && !grab` → host scroll
    //   `terminal_mouse && grab`  → forward to terminal
    //   `!terminal_mouse`         → forward to terminal
    //
    // The wheel only scrolls: it never switches to selection mode (Antonio, 2026-09-29 — a user who
    // drives heca without the prefix would be left in a mode they never asked for). Typing snaps
    // the view back to the bottom; selection mode is entered with `enter_selection_mode`.
    let shift_held = modifiers.shift;
    let wants_mouse = state
        .backends
        .get(pane_id)
        .is_some_and(|b| b.is_mouse_grabbed());
    let do_host_scroll = shift_held || (state.terminal_mouse_enabled && !wants_mouse);

    if !do_host_scroll {
        // Forward wheel to the terminal backend.
        forward_wheel_to_terminal(state, pane_id, cell, modifiers, delta);
        return;
    }

    // Host scrollback fallback (`terminal-task-01h`): if the host viewport has
    // no room to move (the backend is in an alternate screen with no retained
    // history, e.g. `nvim`/`less`, or the pane simply has no scrollback yet), the
    // wheel is otherwise wasted. For plain wheel we gracefully forward it to the
    // terminal so non-grabbed TUIs can still react. For `Shift+wheel` this is a
    // documented no-op: Shift's contract is "bypass the TUI, host-only", so we
    // never silently scroll the TUI. wezterm does not expose the main screen's
    // preserved history while the alt screen is active, so host scrollback in
    // alt-screen TUIs is an inherent limitation (see README).
    let host_can_scroll = state
        .backends
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
        .is_some_and(|s| s.scrollback_rows > s.rows);
    if !host_can_scroll {
        if shift_held {
            // Documented limitation: Shift+wheel host scrollback is a no-op
            // while the backend is in an alternate screen (no exposed host
            // history).
            return;
        }
        forward_wheel_to_terminal(state, pane_id, cell, modifiers, delta);
        return;
    }

    let lines = state.terminal_wheel_scroll_lines;
    // Determine number of host-scroll notches from the dominant wheel axis.
    // On many platforms Shift+wheel is remapped to horizontal scroll (`x`) with
    // `y == 0`; the host scrollback policy still wants that gesture to behave as
    // a vertical history scroll, so we fall back to `x` when there is no usable
    // vertical component.
    let signed_notches = host_scroll_notches(
        delta,
        state
            .backends
            .get(pane_id)
            .map(|b| b.cell_size().1 as f64)
            .unwrap_or(state.terminal_cell_size.1 as f64),
    );
    let total = (signed_notches.abs().ceil() as usize) * lines;
    if total == 0 {
        return;
    }
    let delta_i32: i32 = if signed_notches > 0.0 {
        total as i32
    } else {
        -(total as i32)
    };

    if let Some(backend) = state.backends.get_mut(pane_id) {
        backend.scroll_viewport(delta_i32);
    }
    state.needs_redraw = true;
    // Reset prefix timeout on scroll (like keyboard input).
    state.prefix_entered_at = None;
}

fn wheel_buttons(delta: MouseScrollDelta) -> Vec<BackendMouseButton> {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => axis_wheel_buttons(x as f64, y as f64),
        MouseScrollDelta::PixelDelta(pos) => axis_wheel_buttons(pos.x, pos.y),
    }
}

/// Forward a wheel scroll to the terminal backend as press events.
///
/// Used by [`on_wheel`] both when the policy routes the wheel away
/// from host scrollback (grabbed TUI / `terminal_mouse = false`) and as a
/// graceful fallback when the host viewport has no scrollback room (see
/// `terminal-task-01h`).
fn forward_wheel_to_terminal(
    state: &mut AppState,
    pane_id: PaneId,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
    delta: MouseScrollDelta,
) {
    let Some(cell) = cell else {
        return;
    };
    for button in wheel_buttons(delta) {
        let event = backend_mouse_event(BackendMouseEventKind::Press, button, cell, modifiers);
        if let Some(backend) = state.backends.get_mut(pane_id) {
            let _ = backend.process_mouse_event(&event);
        }
    }
}

fn host_scroll_notches(delta: MouseScrollDelta, cell_h: f64) -> f64 {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => {
            if y.abs() > 0.0 {
                y as f64
            } else {
                x as f64
            }
        }
        MouseScrollDelta::PixelDelta(pos) => {
            let primary = if pos.y.abs() > 0.0 { pos.y } else { pos.x };
            if primary.abs() > 0.0 && cell_h > 0.0 {
                primary / cell_h
            } else {
                0.0
            }
        }
    }
}

fn axis_wheel_buttons(x: f64, y: f64) -> Vec<BackendMouseButton> {
    let mut buttons = Vec::new();
    push_wheel_buttons(
        &mut buttons,
        y,
        BackendMouseButton::WheelUp,
        BackendMouseButton::WheelDown,
    );
    push_wheel_buttons(
        &mut buttons,
        x,
        BackendMouseButton::WheelRight,
        BackendMouseButton::WheelLeft,
    );
    buttons
}

fn push_wheel_buttons(
    buttons: &mut Vec<BackendMouseButton>,
    delta: f64,
    positive: impl Fn(usize) -> BackendMouseButton,
    negative: impl Fn(usize) -> BackendMouseButton,
) {
    let steps = delta.abs().round().max(1.0) as usize;
    if delta > 0.0 {
        buttons.push(positive(steps));
    } else if delta < 0.0 {
        buttons.push(negative(steps));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trackpad_keeps_its_pixels_and_a_wheel_its_notches() {
        // Content moving down (+y) is the device turned toward you (-y).
        assert!(matches!(
            device_delta((0.0, 3.0), None),
            MouseScrollDelta::LineDelta(_, y) if y == -3.0
        ));
        assert!(matches!(
            device_delta((0.0, 0.7), Some((0.0, 14.0))),
            MouseScrollDelta::PixelDelta(p) if p.y == -14.0
        ));
        // And the pixels, not the lines, decide the unit: 14 px is one 14-px cell.
        assert_eq!(
            host_scroll_notches(device_delta((0.0, 0.7), Some((0.0, 14.0))), 14.0),
            -1.0
        );
    }
}
