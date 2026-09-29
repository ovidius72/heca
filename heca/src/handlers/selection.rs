//! Selection mode, copy and paste.

use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionSource};
use crate::app::terminal_host::{
    enter_selection_mode_for_focused_terminal, move_focused_terminal_selection,
};
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;

/// Enter the host-owned selection input mode.
///
/// For terminal panes, this action starts or resumes a host-grid selection at
/// the focused terminal cursor so keyboard selection works immediately.
/// For panes that do not yet expose a selection adapter, it still falls back
/// to a pure input-mode transition.
///
/// The action only transitions the input mode so that:
/// - status bar shows `SELECTION`
/// - `Esc` / `Enter` semantics become "clear / confirm selection"
/// - other keys are not forwarded to the focused backend
///
/// Pre-existing selections are allowed: a user who confirmed a selection
/// with `Enter` (leaving it in the `Selected` phase) can re-enter selection
/// mode to reposition it. Mode-internal keyboard behavior (`Esc` clears,
/// `Enter` confirms, `prefix` returns to Prefix, movement keys update the
/// focus cell) is owned by
/// `app::input::handle_selection_mode`.
pub fn handle_enter_selection_mode(state: &mut AppState, _action: &WmAction) {
    // If we can't place a caret on the focused terminal, do nothing.
    // Entering selection mode without a caret violates the contract:
    // the user must have a caret position to move and begin selection from.
    let _ = enter_selection_mode_for_focused_terminal(state);
}

pub fn handle_selection_left(state: &mut AppState, _action: &WmAction) {
    let _ = move_focused_terminal_selection(state, 0, -1);
}

pub fn handle_selection_right(state: &mut AppState, _action: &WmAction) {
    let _ = move_focused_terminal_selection(state, 0, 1);
}

pub fn handle_selection_up(state: &mut AppState, _action: &WmAction) {
    let _ = move_focused_terminal_selection(state, -1, 0);
}

pub fn handle_selection_down(state: &mut AppState, _action: &WmAction) {
    let _ = move_focused_terminal_selection(state, 1, 0);
}

/// Begin selection from the caret position.
///
/// When in caret-only state (selection mode entered but no selection started),
/// this starts a selection with anchor and focus both at the caret position.
/// Movement keys will then grow the selection from that point.
///
/// If a selection already exists, this is a no-op — the user should clear
/// and re-enter if they want to restart selection from a different point.
/// This avoids accidental loss of an in-progress selection.
pub fn handle_begin_selection(state: &mut AppState, _action: &WmAction) {
    if state.selection.is_caret() {
        state
            .selection
            .begin_selection_from_caret(SelectionSource::KeyboardMode);
    }
    // If selection already exists: no-op. Document the policy — user must
    // clear (Esc) and re-enter selection mode to restart from caret.
}

/// Toggle which endpoint of the selection is active (anchor vs focus).
///
/// After toggling, movement keys update the other end of the selection.
/// This lets the keyboard user grow the selection from both ends without
/// restarting.
pub fn handle_toggle_selection_endpoint(state: &mut AppState, _action: &WmAction) {
    state.selection.toggle_selection_endpoint();
}

/// Clear the active selection and exit selection input mode.
pub fn handle_clear_selection(state: &mut AppState, _action: &WmAction) {
    state.selection.clear();
    if matches!(state.input_mode, InputMode::Selection) {
        state.input_mode = InputMode::Normal;
    }
}

/// Write text to the system clipboard via `arboard`; shared by selection-copy
/// and the `OSC 52` clipboard-write path. Failures are logged in debug builds.
pub(crate) fn set_system_clipboard(text: &str) {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => {
            if let Err(e) = clipboard.set_text(text) {
                #[cfg(debug_assertions)]
                eprintln!("[heca] clipboard write failed: {e}");
                let _ = e; // Suppress unused warning in release.
            }
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("[heca] clipboard unavailable: {e}");
            let _ = e;
        }
    }
}

/// Copy the active host-grid selection text to the system clipboard.
///
/// Routes through the action registry so it is reachable from keyboard,
/// mouse/UI, and RPC. The extraction uses the shared `SelectionState` and
/// the terminal backend snapshot — no terminal-only selection state.
///
/// # Clipboard contract
///
/// - Uses the system clipboard via `arboard`.
/// - If clipboard write fails, logs in debug builds and keeps app state coherent.
/// - Does NOT clear the selection after copy — the user clears explicitly.
/// - Safe no-op when there is no active selection or the owner is unsupported.
pub fn handle_copy_selection(state: &mut AppState, _action: &WmAction) {
    let text = {
        let active = match state.selection.active() {
            Some(a) => a,
            None => return, // No active selection — safe no-op.
        };
        let SelectionOwner::Pane(pane_id) = active.owner;
        let (start_stable, end_stable) = match &active.region {
            SelectionRegion::HostGrid {
                anchor_stable_row,
                focus_stable_row,
                ..
            } => (
                (*anchor_stable_row).min(*focus_stable_row),
                (*anchor_stable_row).max(*focus_stable_row),
            ),
            SelectionRegion::BackendNative => return, // Unsupported — safe no-op.
        };

        // Get the terminal snapshot for the owning pane.
        let snapshot = match state
            .backends
            .get(pane_id)
            .and_then(|b| b.terminal_snapshot())
        {
            Some(s) => s,
            None => return, // No snapshot — safe no-op.
        };

        // Extract text using the shared extraction logic.
        //
        // Fetch the selection rows by stable-row range so history rows are
        // copyable even when they are no longer in the visible viewport.
        let lines = state
            .backends
            .get(pane_id)
            .map(|b| b.lines_in_stable_range(start_stable, end_stable, snapshot.cols))
            .unwrap_or_default();

        match crate::app::selection_model::extract_selection_text(
            &state.selection,
            &lines,
            start_stable,
            snapshot.cols,
            snapshot.default_bg,
        ) {
            Some(t) => t,
            None => return, // Extraction returned nothing — safe no-op.
        }
    };

    if text.is_empty() {
        return;
    }

    // Write to system clipboard.
    set_system_clipboard(&text);

    // Copying ends the selection, however it was made (Antonio, 2026-09-29: `y` and a Shift+drag
    // release must look the same). Outside selection mode — a mouse selection — that is all; inside
    // it, the caret below stays where the selection ended so the keyboard can carry on.
    if !matches!(state.input_mode, InputMode::Selection) {
        state.selection.clear();
        return;
    }

    // Clear the active selection but stay in selection mode with the caret
    // at the last focus position. This way the user can immediately navigate
    // or start a new selection without the Q5 snap-to-bottom triggering.
    if let Some(active) = state.selection.active()
        && let SelectionRegion::HostGrid {
            focus_stable_row,
            focus_col,
            ..
        } = &active.region
    {
        let owner = active.owner;
        state
            .selection
            .set_caret(owner, *focus_stable_row, *focus_col);
    } else {
        state.selection.clear();
    }
}

/// Paste the system clipboard into the focused pane.
///
/// Reads the OS clipboard via `arboard` and forwards it to the focused backend.
/// The backend wraps the text in bracketed-paste markers when the program
/// enabled that mode (see `PaneBackend::paste`), so editors treat it as literal
/// input.
pub fn handle_paste_clipboard(state: &mut AppState, _action: &WmAction) {
    // Read the system clipboard and forward text into the focused pane.
    let text = match arboard::Clipboard::new() {
        Ok(mut cb) => match cb.get_text() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[heca] clipboard read failed: {e}");
                return;
            }
        },
        Err(e) => {
            eprintln!("[heca] clipboard unavailable: {e}");
            return;
        }
    };

    // Find the active pane.
    let pane_id = match state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|pane| pane.id)
        .or(state.focused_pane)
    {
        Some(id) => id,
        None => return,
    };

    // Forward the text to the pane's backend (bracketed-paste aware).
    if let Some(backend) = state.backends.get_mut(pane_id) {
        backend.paste(&text);
    }
}
