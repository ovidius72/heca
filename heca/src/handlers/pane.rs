//! A pane's own lifecycle: float, close, select, and moving its cursor.

use super::docks::focus_navigable_dock;
use super::pick::{begin_pick, pane_candidates_with_stable_letters};
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;

pub fn handle_pane_select(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(state, InputMode::PaneSelect { candidates });
}

/// **Move a mounted container's cursor to the row named by `key`** — the generic form of a click
/// on a row, and of any other gesture that means "the cursor belongs here now".
///
/// It moves the cursor and nothing else. Activating the row, focusing a pane or handing the
/// keyboard back are separate acts with separate names; a gesture that wants one of those says so.
pub fn handle_cursor_to(state: &mut AppState, action: &WmAction) {
    let WmAction::CursorTo { mount, key } = action else {
        return;
    };
    crate::providers::move_provider_cursor(state, mount, key);
}
