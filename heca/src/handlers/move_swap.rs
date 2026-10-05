//! The picks that start a swap: choosing which pane to swap with by letter. (The swaps and moves
//! themselves are the server's: `server/handlers/{swap,move_pane,move_column}.rs`.)

use super::docks::focus_navigable_dock;
use super::pick::{begin_pick, pane_candidates_with_stable_letters};
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;

pub fn handle_swap_pane(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(
        state,
        InputMode::PaneSwap {
            candidates,
            focus_after: false,
        },
    );
}

pub fn handle_swap_and_focus_pane(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(
        state,
        InputMode::PaneSwap {
            candidates,
            focus_after: true,
        },
    );
}
