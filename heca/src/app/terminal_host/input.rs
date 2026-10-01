//! **What a terminal's input means** — the policy behind the typed messages a terminal sends
//! (`chrome::terminal::TerminalInput`): buttons and moves, and the window gestures that
//! may have claimed the same press.

use crate::actions::ActionRegistry;
use crate::app::interaction::{InteractionSource, dispatch_action};
use crate::app::selection_model::{SelectionOwner, SelectionSource};
use crate::app_state::{AppState, InputMode};
use crate::chrome::terminal::Cell;
use crate::input::WmAction;
use heca_core::backend::{
    BackendModifiers, BackendMouseButton, BackendMouseEvent, BackendMouseEventKind,
};
use heca_core::layout::PaneId;
use heca_grid_ui::PointerButton;

use super::selection::{begin_terminal_selection_at, visible_row_to_stable_row};

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
    if let Some(backend) = state.backends.get_mut(pane_id) {
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
    if let Some(backend) = state.backends.get_mut(pane_id) {
        let _ = backend.process_mouse_event(&event);
    }
}

/// **A terminal asked for a different grid.** Its process takes the cell size and the grid together,
/// so the picture and the program agree on what a row is.
fn on_resize(state: &mut AppState, pane_id: PaneId, grid: crate::chrome::terminal::Grid) {
    if let Some(backend) = state.backends.get_mut(pane_id) {
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
    if let Some(backend) = state.backends.get_mut(pane_id) {
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
/// the library drops a capture when the pointer leaves — until P084(F004)/T534 lands, this one
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
        } => super::wheel::on_wheel(state, registry, pane_id, ((x, y), pixels), cell, modifiers),
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

/// **The backend's mouse event for a pointer at `cell`.** Everything it needs is in the arguments:
/// the terminal worked out the cell, and the modifiers rode in with the event.
pub(super) fn backend_mouse_event(
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

#[cfg(test)]
mod tests {
    use super::*;

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
