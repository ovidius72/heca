//! **Scrollback search** — entering the query, running it, and stepping through the matches.

use super::selection::ensure_caret_visible;
use crate::app::selection_model::SelectionOwner;
use crate::app_state::{AppState, InputMode};
use heca_grid_ui::Component as _;

/// Enter scrollback-search query entry for the selection's (or focused) pane.
/// No-op when there is no terminal-backed pane to search. terminal-task-19.
pub(crate) fn enter_scrollback_search(state: &mut AppState) {
    let Some(pane_id) = state.search_target_pane() else {
        return;
    };
    if state
        .backends
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
        .is_none()
    {
        return;
    }
    // Re-entering search on a pane that already has one RESUMES it: the query, its
    // matches and the caret are all still there. Inserting a fresh state here wiped
    // whatever had been typed, so `/` after Enter was indistinguishable from having
    // no way back into the field at all.
    if let Some(existing) = state.searches.get(&pane_id) {
        existing.input.borrow().base().focus(false);
        state.input_mode = InputMode::Search;
        state.needs_redraw = true;
        return;
    }
    state.searches.insert(
        pane_id,
        crate::app_state::SearchState {
            // Focused so the caret shows and the field accepts editing keys.
            input: std::cell::RefCell::new({
                let field = heca_grid_ui::widgets::Input::new();
                field.base().focus(false);
                field
            }),
            matches: Vec::new(),
            current: None,
        },
    );
    state.input_mode = InputMode::Search;
    state.needs_redraw = true;
}

/// Re-run the search for the current query, refresh the match list, focus the match
/// nearest at/above the caret (else the last), and jump to it.
pub(crate) fn run_scrollback_search(state: &mut AppState) {
    // The pane whose query the keyboard is editing — same resolution the search was
    // started with, so edits land on the entry that exists.
    let Some(pane_id) = state.search_target_pane() else {
        return;
    };
    let Some(query) = state
        .search_for(pane_id)
        .map(|s| s.input.borrow().value_str())
    else {
        return;
    };
    let cols = match state
        .backends
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
    {
        Some(snap) => snap.cols,
        None => return,
    };
    let caret_row = state.selection.cursor_cell().map(|(_, row, _)| row);
    let matches = state
        .backends
        .get(pane_id)
        .map(|b| b.search_scrollback(&query, cols))
        .unwrap_or_default();
    let current = if matches.is_empty() {
        None
    } else {
        let caret = caret_row.unwrap_or(isize::MAX);
        // Nearest match at/above the caret, else fall back to the last match.
        Some(
            matches
                .iter()
                .rposition(|m| m.stable_row <= caret)
                .unwrap_or(matches.len() - 1),
        )
    };
    if let Some(search) = state.searches.get_mut(&pane_id) {
        search.matches = matches;
        search.current = current;
    }
    jump_to_current_match(state);
}

/// Move the focused match by one (wrapping) and jump to it. `forward` = next match.
pub(crate) fn search_step(state: &mut AppState, forward: bool) {
    let stepped = state.active_search_mut().and_then(|search| {
        let n = search.matches.len();
        if n == 0 {
            return None;
        }
        let cur = search.current.unwrap_or(0);
        let next = if forward {
            (cur + 1) % n
        } else {
            (cur + n - 1) % n
        };
        search.current = Some(next);
        Some(())
    });
    if stepped.is_some() {
        jump_to_current_match(state);
    }
}

/// Place the selection caret on the focused match's first cell and scroll it into
/// view. No-op when no match is focused.
fn jump_to_current_match(state: &mut AppState) {
    let Some(pane_id) = state.search_target_pane() else {
        return;
    };
    let Some(m) = state
        .search_for(pane_id)
        .and_then(|s| s.current.and_then(|i| s.matches.get(i)).cloned())
    else {
        return;
    };
    if let Some(terminal) = state.backends.identity_of(pane_id) {
        state
            .selection
            .set_caret(SelectionOwner(terminal), m.stable_row, m.start_col);
    }
    if let Some(snapshot) = state
        .backends
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
        && let Some(terminal) = state.backends.identity_of(pane_id)
    {
        ensure_caret_visible(state, terminal, m.stable_row, &snapshot);
    }
    state.needs_redraw = true;
}
