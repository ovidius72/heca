//! **What a terminal's input means** — the policy behind the typed messages a terminal sends
//! (`chrome::terminal::TerminalInput`): buttons and moves, and the window gestures that
//! may have claimed the same press.

use crate::actions::ActionRegistry;
use crate::app::interaction::{InteractionSource, dispatch_action};
use crate::app::selection_model::{SelectionOwner, SelectionSource};
use crate::app_state::{AppState, InputMode};
use crate::chrome::terminal::{Cell, TerminalId};
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
pub(crate) struct HeardPresses(Vec<(TerminalId, BackendMouseButton)>);

impl HeardPresses {
    /// The program heard this press.
    pub(crate) fn heard(&mut self, terminal: TerminalId, button: BackendMouseButton) {
        if !self.0.contains(&(terminal, button)) {
            self.0.push((terminal, button));
        }
    }

    /// A release came: was its press heard? Forgets it either way.
    pub(crate) fn released(&mut self, terminal: TerminalId, button: BackendMouseButton) -> bool {
        match self.0.iter().position(|p| *p == (terminal, button)) {
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
    terminal: TerminalId,
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
        // Any terminal's, whoever owns it: the selection is owned by the terminal itself.
        let Some(cell) = cell else {
            return;
        };
        begin_terminal_selection_at(
            state,
            terminal,
            cell.row,
            cell.col,
            SelectionSource::MouseDrag,
        );
        return;
    }
    let Some(button) = backend_button(button) else {
        return;
    };
    let Some(cell) = cell else {
        return;
    };
    let event = backend_mouse_event(BackendMouseEventKind::Press, button, cell, modifiers);
    if let Some(backend) = state.server.backends.get_mut_by_id(terminal) {
        let _ = backend.process_mouse_event(&event);
        // The release that pairs with this press is the program's to hear.
        state.terminal_presses.heard(terminal, button);
    }
}

/// **A button came up over a terminal.** The program hears it only if it heard the press — a
/// release that ends a divider resize, or a selection, is not the program's.
fn on_release(
    state: &mut AppState,
    terminal: TerminalId,
    button: PointerButton,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    let Some(button) = backend_button(button) else {
        return;
    };
    if !state.terminal_presses.released(terminal, button) {
        return;
    }
    let Some(cell) = cell else {
        return;
    };
    let event = backend_mouse_event(BackendMouseEventKind::Release, button, cell, modifiers);
    if let Some(backend) = state.server.backends.get_mut_by_id(terminal) {
        let _ = backend.process_mouse_event(&event);
    }
}

/// **A terminal asked for a different grid.** Its process takes the cell size and the grid together,
/// so the picture and the program agree on what a row is.
fn on_resize(
    state: &mut AppState,
    terminal: crate::chrome::terminal::TerminalId,
    grid: crate::chrome::terminal::Grid,
) {
    if let Some(backend) = state.server.backends.get_mut_by_id(terminal) {
        backend.set_cell_size(grid.cell_w, grid.cell_h);
        backend.set_size(grid.cols, grid.rows);
        state.needs_redraw = true;
    }
}

/// **The pointer moved over a terminal.**
fn on_move(
    state: &mut AppState,
    terminal: TerminalId,
    pane: Option<PaneId>,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    if window_gesture_has_it(state) {
        return;
    }
    // A host selection drag follows the pointer, in the pane it started in. Over another pane, or
    // off the grid, it holds where it was — so a release outside still confirms the selection up to
    // the last cell it reached.
    if state.selection.is_selecting()
        && state.selection.source() == Some(SelectionSource::MouseDrag)
    {
        if let Some(SelectionOwner(owner)) = state.selection.owner()
            && owner == terminal
            && let Some(cell) = cell
            && let Some(snapshot) = state
                .server
                .backends
                .get_by_id(terminal)
                .and_then(|backend| backend.terminal_snapshot())
        {
            state
                .selection
                .update_focus(visible_row_to_stable_row(&snapshot, cell.row), cell.col);
            state.needs_redraw = true;
        }
        return;
    }
    // A pane is told where the pointer is only while it has the keyboard; a terminal no pane owns
    // follows the same rule on its own (it says nothing unless it holds focus), so by the time it
    // is heard here there is nothing left to decide.
    if pane.is_some() && state.focused_pane != pane {
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
    if let Some(backend) = state.server.backends.get_mut_by_id(terminal) {
        let _ = backend.process_mouse_event(&event);
    }
}

/// Has a window gesture taken the pointer — a drag in flight, a menu or dialog
/// that just opened? Then the program does not hear it.
fn window_gesture_has_it(state: &AppState) -> bool {
    crate::chrome::drag_in_flight(state) || crate::chrome::top_modal(state).is_some()
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
    terminal: crate::chrome::terminal::TerminalId,
    input: crate::chrome::terminal::TerminalInput,
) {
    use crate::chrome::terminal::TerminalInput;
    // What every terminal does, whoever owns it: its grid and its scrollback.
    match input {
        TerminalInput::Resize(grid) => return on_resize(state, terminal, grid),
        TerminalInput::ScrollToBottom => {
            if let Some(backend) = state.server.backends.get_mut_by_id(terminal) {
                backend.scroll_to_bottom_animated();
                state.needs_redraw = true;
            }
            return;
        }
        TerminalInput::ScrollTo { rows } => {
            if let Some(backend) = state.server.backends.get_mut_by_id(terminal) {
                crate::handlers::scroll_backend_to_offset(backend, rows);
                state.needs_redraw = true;
            }
            return;
        }
        TerminalInput::Wheel {
            x,
            y,
            pixels,
            cell,
            modifiers,
        } => {
            return super::wheel::on_wheel(
                state,
                registry,
                terminal,
                ((x, y), pixels),
                cell,
                modifiers,
            );
        }
        // What was typed into it while it held the keyboard — any terminal's, whoever owns it.
        TerminalInput::Text(text) => return super::typing::on_text(state, terminal, &text),
        TerminalInput::Key { key, modifiers } => {
            return super::typing::on_key(state, terminal, key, modifiers);
        }
        // What it says about its own search: the matches are the host's to find.
        TerminalInput::Search(search) => return super::search::on_search(state, terminal, search),
        _ => {}
    }
    // The pointer's buttons and moves are every terminal's; what only a pane has (its selection)
    // is asked of `pane`, which is `None` for a terminal no pane owns.
    let pane = state.server.backends.pane_of(terminal);
    match input {
        TerminalInput::Wheel { .. }
        | TerminalInput::Resize(_)
        | TerminalInput::ScrollToBottom
        | TerminalInput::ScrollTo { .. }
        | TerminalInput::Text(_)
        | TerminalInput::Key { .. }
        | TerminalInput::Search(_) => {}
        TerminalInput::Press {
            button,
            cell,
            modifiers,
        } => on_press(state, terminal, button, cell, modifiers),
        TerminalInput::Release {
            button,
            cell,
            modifiers,
        } => on_release(state, terminal, button, cell, modifiers),
        TerminalInput::Move { cell, modifiers } => on_move(state, terminal, pane, cell, modifiers),
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
        let (a, b) = (TerminalId(1), TerminalId(2));
        heard.heard(a, BackendMouseButton::Left);
        assert!(
            !heard.released(b, BackendMouseButton::Left),
            "another terminal's"
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
