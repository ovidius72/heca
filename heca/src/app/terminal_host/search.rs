//! **Scrollback search, host half** — finding the matches for what a terminal's field says, and
//! stepping through them.
//!
//! The search itself is the terminal's: its bar is its own child, so it hears the field and says
//! what the user did ([`Search`]). The host holds the process, so it is the one that can look
//! through the scrollback — by the terminal's id, whether or not a pane owns it — and it hands the
//! terminal what it found. The terminal keeps the matches and the current one, since it is what
//! shows them: nothing here holds a copy.

use super::selection::ensure_caret_visible;
use crate::app::selection_model::SelectionOwner;
use crate::app_state::{AppState, InputMode};
use crate::chrome::terminal::{Search, TerminalId};
use heca_grid_ui::WidgetIntent;

/// **A terminal said something about its search.**
pub(crate) fn on_search(state: &mut AppState, terminal: TerminalId, search: Search) {
    match search {
        Search::Editing(true) => {
            state.input_mode = InputMode::Search { terminal };
        }
        Search::Editing(false) => leave_search_entry(state, terminal),
        Search::Query(query) => run_search(state, terminal, &query),
        Search::Step { forward } => step(state, terminal, forward),
        Search::Close => leave_search_entry(state, terminal),
    }
    state.needs_redraw = true;
}

/// The field no longer has the keyboard: back to selection (where `n`/`N` step) when a selection is
/// still there, else to plain typing. Another terminal's mode is not ours to leave.
fn leave_search_entry(state: &mut AppState, terminal: TerminalId) {
    if !matches!(state.input_mode, InputMode::Search { terminal: t } if t == terminal) {
        return;
    }
    state.input_mode = if state.selection.owner().is_some() {
        InputMode::Selection
    } else {
        InputMode::Normal
    };
}

/// **Open the search of the terminal the keyboard is on** — the `search_scrollback` action.
///
/// A terminal that holds the keyboard in the window tree (a docked one) answers the intent itself:
/// the host names nothing. Otherwise the terminal is the selection's, or the focused pane's — panes
/// are not in the window's keyboard yet — and its own bar is opened by its handle.
pub(crate) fn enter_scrollback_search(state: &mut AppState) {
    if tree_answers(state, WidgetIntent::Find) {
        return;
    }
    let Some(terminal) = state.search_target() else {
        return;
    };
    if state
        .server
        .backends
        .get_by_id(terminal)
        .and_then(|b| b.terminal_snapshot())
        .is_none()
    {
        return;
    }
    let Some(handle) = state.terminals.get(&terminal) else {
        return;
    };
    handle.find();
    // Typing goes to the field from this key on, not from the frame that draws it.
    state.input_mode = InputMode::Search { terminal };
    state.needs_redraw = true;
}

/// Move to the next (`forward`) or previous match of the keyboard's search — `n` / `N`.
pub(crate) fn search_step(state: &mut AppState, forward: bool) {
    let intent = if forward {
        WidgetIntent::FindNext
    } else {
        WidgetIntent::FindPrevious
    };
    if tree_answers(state, intent) {
        return;
    }
    if let Some(terminal) = state.search_target() {
        step(state, terminal, forward);
    }
}

/// **Did a terminal that holds the keyboard in the window tree take this intent?** Only asked while
/// the keyboard really is in the tree (a dock or a layer in front); the panes' keys are not.
fn tree_answers(state: &mut AppState, intent: WidgetIntent) -> bool {
    crate::app::input::focused_surface(state).delivers_to_tree()
        && crate::chrome::deliver(state, &heca_grid_ui::Event::Widget(intent))
}

/// Re-run the search for `query` in `terminal`, refresh the match list, focus the match nearest
/// at/above the caret (else the last), jump to it, and show the terminal what was found.
fn run_search(state: &mut AppState, terminal: TerminalId, query: &str) {
    let Some(backend) = state.server.backends.get_by_id(terminal) else {
        return;
    };
    let Some(cols) = backend.terminal_snapshot().map(|snap| snap.cols) else {
        return;
    };
    let matches = backend.search_scrollback(query, cols);
    let caret_row = state.selection.cursor_cell().map(|(_, row, _)| row);
    let current = nearest_at_or_above(&matches, caret_row);
    // Handed over once, and the terminal is the one that keeps it.
    if let Some(handle) = state.terminals.get(&terminal)
        && handle.show_matches(matches, current)
    {
        state.needs_redraw = true;
    }
    jump_to_current_match(state, terminal);
}

/// The match nearest at/above `caret_row`, else the last one; `None` when there are none.
fn nearest_at_or_above(
    matches: &[heca_core::backend::SearchMatch],
    caret_row: Option<isize>,
) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    let caret = caret_row.unwrap_or(isize::MAX);
    Some(
        matches
            .iter()
            .rposition(|m| m.stable_row <= caret)
            .unwrap_or(matches.len() - 1),
    )
}

/// Move the terminal's current match by one (wrapping) and jump to it.
fn step(state: &mut AppState, terminal: TerminalId, forward: bool) {
    let moved = state
        .terminals
        .get(&terminal)
        .is_some_and(|handle| handle.step_match(forward));
    if moved {
        state.needs_redraw = true;
        jump_to_current_match(state, terminal);
    }
}

/// Place the selection caret on the focused match's first cell and scroll it into view. No-op
/// when no match is focused.
fn jump_to_current_match(state: &mut AppState, terminal: TerminalId) {
    let Some(m) = state
        .terminals
        .get(&terminal)
        .and_then(|handle| handle.current_match())
    else {
        return;
    };
    state
        .selection
        .set_caret(SelectionOwner(terminal), m.stable_row, m.start_col);
    if let Some(snapshot) = state
        .server
        .backends
        .get_by_id(terminal)
        .and_then(|b| b.terminal_snapshot())
    {
        ensure_caret_visible(state, terminal, m.stable_row, &snapshot);
    }
    state.needs_redraw = true;
}

#[cfg(test)]
mod tests;
