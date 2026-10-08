//! The picks that start taking a pane. (Taking it is the server's.)

use super::docks::focus_navigable_dock;
use super::pick::{begin_pick, pane_candidates_with_stable_letters};
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;

pub fn handle_pane_take(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(
        state,
        InputMode::PaneTake {
            candidates,
            focus_after: false,
        },
    );
}

pub fn handle_pane_take_and_focus(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(
        state,
        InputMode::PaneTake {
            candidates,
            focus_after: true,
        },
    );
}
