//! **What a terminal's input means** — the policy behind the typed messages a terminal sends
//! (`chrome::terminal::TerminalInput`): the wheel, buttons and moves, and the window gestures that
//! may have claimed the same press.

use crate::actions::ActionRegistry;
use crate::app::interaction::{InteractionSource, dispatch_action};
use crate::app::selection_model::{SelectionOwner, SelectionSource};
use crate::app_state::{AppState, InputMode};
use crate::chrome::terminal::Cell;
use crate::input::{FontZoomStep, WmAction};
use heca_core::backend::{
    BackendModifiers, BackendMouseButton, BackendMouseEvent, BackendMouseEventKind,
};
use heca_core::layout::PaneId;
use heca_grid_ui::PointerButton;
use winit::event::MouseScrollDelta;

use super::selection::{begin_terminal_selection_at, visible_row_to_stable_row};

/// Default cell height in logical pixels for PixelDelta → line conversion
/// fallback when the terminal backend cannot be queried for real cell metrics.
const DEFAULT_CELL_H: f64 = 14.0;

/// **The presses a terminal's program has heard and not yet seen released**, so a release is passed
/// on only where its press was: one that ends a divider resize, or a selection, is not the
/// program's.
#[derive(Default)]
pub(crate) struct HeardPresses(Vec<(PaneId, BackendMouseButton)>);

impl HeardPresses {
    /// The program heard this press.
    pub(crate) fn heard(&mut self, pane: PaneId, button: BackendMouseButton) {
        if !self.0.contains(&(pane, button)) {
            self.0.push((pane, button));
        }
    }

    /// A release came: was its press heard? Forgets it either way.
    pub(crate) fn released(&mut self, pane: PaneId, button: BackendMouseButton) -> bool {
        match self.0.iter().position(|p| *p == (pane, button)) {
            Some(at) => {
                self.0.swap_remove(at);
                true
            }
            None => false,
        }
    }
}

/// **A button went down on a terminal.** What it means needs state — the selection, the settings,
/// the window gesture that may have claimed the same press — so it is decided here.
///
/// The terminal does not take the press, so a window gesture (a divider, the move modifier) has
/// already decided about it by the time this runs, and what it decided is in the state.
fn on_press(
    state: &mut AppState,
    pane_id: PaneId,
    button: PointerButton,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    // A plain left-click while a selection is active clears it and leaves selection mode — a click
    // in a terminal puts you back in the program. Placing a caret at the click instead would trap
    // the user in selection mode until Esc (the same as most terminal emulators).
    if button == PointerButton::Left && !modifiers.shift && state.selection.is_active() {
        state.selection.clear();
        if matches!(state.input_mode, InputMode::Selection) {
            state.input_mode = InputMode::Normal;
        }
        state.needs_redraw = true;
    }
    if window_gesture_has_it(state) {
        return;
    }
    // **Host selection: Shift + left press.** It goes to the shared selection model instead of the
    // program, which keeps a program's own mouse use (a plain click) intact while giving the host an
    // explicit entry gesture.
    if button == PointerButton::Left && modifiers.shift {
        let Some(cell) = cell else {
            return;
        };
        // Only a pane whose backend has a cell grid; a future browser or GUI pane would use its
        // own selection.
        let has_grid = state
            .server
            .backends
            .get(pane_id)
            .and_then(|backend| backend.terminal_snapshot())
            .is_some();
        if has_grid {
            begin_terminal_selection_at(
                state,
                pane_id,
                cell.row,
                cell.col,
                SelectionSource::MouseDrag,
            );
        }
        return;
    }
    let Some(button) = backend_button(button) else {
        return;
    };
    let Some(cell) = cell else {
        return;
    };
    let event = backend_mouse_event(BackendMouseEventKind::Press, button, cell, modifiers);
    if let Some(backend) = state.server.backends.get_mut(pane_id) {
        let _ = backend.process_mouse_event(&event);
        // The release that pairs with this press is the program's to hear.
        state.terminal_presses.heard(pane_id, button);
    }
}

/// **A button came up over a terminal.** The program hears it only if it heard the press — a
/// release that ends a divider resize, or a selection, is not the program's.
fn on_release(
    state: &mut AppState,
    pane_id: PaneId,
    button: PointerButton,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    let Some(button) = backend_button(button) else {
        return;
    };
    if !state.terminal_presses.released(pane_id, button) {
        return;
    }
    let Some(cell) = cell else {
        return;
    };
    let event = backend_mouse_event(BackendMouseEventKind::Release, button, cell, modifiers);
    if let Some(backend) = state.server.backends.get_mut(pane_id) {
        let _ = backend.process_mouse_event(&event);
    }
}

/// **A terminal asked for a different grid.** Its process takes the cell size and the grid together,
/// so the picture and the program agree on what a row is.
fn on_resize(state: &mut AppState, pane_id: PaneId, grid: crate::chrome::terminal::Grid) {
    if let Some(backend) = state.server.backends.get_mut(pane_id) {
        backend.set_cell_size(grid.cell_w, grid.cell_h);
        backend.set_size(grid.cols, grid.rows);
        state.needs_redraw = true;
    }
}

/// **The pointer moved over a terminal.**
fn on_move(
    state: &mut AppState,
    pane_id: PaneId,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    if window_gesture_has_it(state) || crate::mouse::is_resizing(state) {
        return;
    }
    // A host selection drag follows the pointer, in the pane it started in. Over another pane, or
    // off the grid, it holds where it was — so a release outside still confirms the selection up to
    // the last cell it reached.
    if state.selection.is_selecting()
        && state.selection.source() == Some(SelectionSource::MouseDrag)
    {
        if let Some(SelectionOwner::Pane(owner)) = state.selection.owner()
            && owner == pane_id
            && let Some(cell) = cell
            && let Some(snapshot) = state
                .server
                .backends
                .get(pane_id)
                .and_then(|backend| backend.terminal_snapshot())
        {
            state
                .selection
                .update_focus(visible_row_to_stable_row(&snapshot, cell.row), cell.col);
            state.needs_redraw = true;
        }
        return;
    }
    // Only the pane that has the keyboard is told where the pointer is.
    if state.focused_pane != Some(pane_id) {
        return;
    }
    let Some(cell) = cell else {
        return;
    };
    let event = backend_mouse_event(
        BackendMouseEventKind::Move,
        BackendMouseButton::None,
        cell,
        modifiers,
    );
    if let Some(backend) = state.server.backends.get_mut(pane_id) {
        let _ = backend.process_mouse_event(&event);
    }
}

/// Has a window gesture taken the pointer — a move of a pane, a drag in flight, a menu or dialog
/// that just opened? Then the program does not hear it.
fn window_gesture_has_it(state: &AppState) -> bool {
    state.mouse.interactive_move.is_some()
        || crate::chrome::drag_in_flight(state)
        || crate::chrome::top_modal(state).is_some()
}

/// **The left button came up, anywhere.** (A release outside the window reaches no widget, because
/// the library drops a capture when the pointer leaves — until P084(F004)/T536 lands, this one
/// window-level rule ends the drag; then the selection captures the pointer like a scrollbar's
/// thumb and this goes.) A host selection drag ends where the button does, even
/// when the pointer has left the terminal it started in: the selection is confirmed up to the last
/// cell it reached and copied, as a complete gesture (select, release, copy) — the same as `y` does.
pub(crate) fn on_left_release(state: &mut AppState, registry: &ActionRegistry) {
    if state.selection.is_selecting()
        && state.selection.source() == Some(SelectionSource::MouseDrag)
    {
        state.selection.end();
        dispatch_action(
            state,
            registry,
            InteractionSource::MouseContent,
            &WmAction::CopySelection,
        );
        state.needs_redraw = true;
    }
}

fn backend_button(button: PointerButton) -> Option<BackendMouseButton> {
    match button {
        PointerButton::Left => Some(BackendMouseButton::Left),
        PointerButton::Middle => Some(BackendMouseButton::Middle),
        PointerButton::Right => Some(BackendMouseButton::Right),
        PointerButton::Other(_) => None,
    }
}

/// **A terminal said something about its input.** The terminal worked out where in its grid the
/// pointer was; what to *do* about it needs state — the process, the mouse-grab, the settings — so
/// it is decided here, in one place.
pub(crate) fn on_terminal_input(
    state: &mut AppState,
    registry: &ActionRegistry,
    pane_id: PaneId,
    input: crate::chrome::terminal::TerminalInput,
) {
    use crate::chrome::terminal::TerminalInput;
    match input {
        TerminalInput::Wheel {
            x,
            y,
            pixels,
            cell,
            modifiers,
        } => on_wheel(state, registry, pane_id, ((x, y), pixels), cell, modifiers),
        TerminalInput::Press {
            button,
            cell,
            modifiers,
        } => on_press(state, pane_id, button, cell, modifiers),
        TerminalInput::Release {
            button,
            cell,
            modifiers,
        } => on_release(state, pane_id, button, cell, modifiers),
        TerminalInput::Move { cell, modifiers } => on_move(state, pane_id, cell, modifiers),
        TerminalInput::Resize(grid) => on_resize(state, pane_id, grid),
    }
}

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

fn on_wheel(
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
        .server
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
        .server
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
            .server
            .backends
            .get(pane_id)
            .map(|b| b.cell_size().1 as f64)
            .unwrap_or(DEFAULT_CELL_H),
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

    if let Some(backend) = state.server.backends.get_mut(pane_id) {
        backend.scroll_viewport(delta_i32);
    }
    state.needs_redraw = true;
    // Reset prefix timeout on scroll (like keyboard input).
    state.prefix_entered_at = None;
}

pub(crate) fn should_intercept_selection_gesture(
    state: &AppState,
    pos: (f32, f32),
    ev: &heca_grid_ui::Event,
) -> bool {
    use heca_grid_ui::event::RawPointerKind as Kind;
    let heca_grid_ui::Event::Raw(raw) = ev else {
        return false;
    };
    if raw.button != heca_grid_ui::PointerButton::Left
        || raw.kind != Kind::Pressed
        || !state.modifiers.shift_key()
    {
        return false;
    }
    let move_modifier_held = match state.interactive_move_modifier {
        heca_config::theme::ModifierKey::Super => state.modifiers.super_key(),
        heca_config::theme::ModifierKey::Alt => state.modifiers.alt_key(),
        heca_config::theme::ModifierKey::Ctrl => state.modifiers.control_key(),
        heca_config::theme::ModifierKey::Shift => state.modifiers.shift_key(),
    };
    if move_modifier_held {
        return false;
    }
    let Some(pane_id) = crate::mouse::hit_test_pane(state, pos) else {
        return false;
    };
    state
        .server
        .backends
        .get(pane_id)
        .and_then(|backend| backend.terminal_snapshot())
        .is_some()
}

/// **The backend's mouse event for a pointer at `cell`.** Everything it needs is in the arguments:
/// the terminal worked out the cell, and the modifiers rode in with the event.
fn backend_mouse_event(
    kind: BackendMouseEventKind,
    button: BackendMouseButton,
    cell: Cell,
    modifiers: heca_grid_ui::Modifiers,
) -> BackendMouseEvent {
    BackendMouseEvent {
        kind,
        col: cell.col,
        row: cell.row,
        x_pixel_offset: cell.x_offset,
        y_pixel_offset: cell.y_offset,
        button,
        modifiers: BackendModifiers {
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            alt: modifiers.alt,
            super_: modifiers.meta,
        },
    }
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
        if let Some(backend) = state.server.backends.get_mut(pane_id) {
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

    #[test]
    fn a_release_is_the_programs_only_where_its_press_was() {
        let mut heard = HeardPresses::default();
        let (a, b) = (PaneId(1), PaneId(2));
        heard.heard(a, BackendMouseButton::Left);
        assert!(
            !heard.released(b, BackendMouseButton::Left),
            "another pane's"
        );
        assert!(
            !heard.released(a, BackendMouseButton::Right),
            "another button's"
        );
        assert!(heard.released(a, BackendMouseButton::Left));
        assert!(
            !heard.released(a, BackendMouseButton::Left),
            "heard once, released once"
        );
    }
}
