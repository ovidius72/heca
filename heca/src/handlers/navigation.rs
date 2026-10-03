//! Focus moves: columns, panes and workspaces, and the focus toggles.

use crate::app::focus::focus_pane_by_id;
use crate::app_state::AppState;
use crate::input::WmAction;
use crate::switch_workspace_tracked;

pub fn handle_focus_left(state: &mut AppState, _action: &WmAction) {
    state.session.focus_left();
}

pub fn handle_focus_right(state: &mut AppState, _action: &WmAction) {
    state.session.focus_right();
}

pub fn handle_focus_up(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.focus_up();
    }
}

pub fn handle_focus_down(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.focus_down();
    }
}

pub fn handle_next_pane(state: &mut AppState, _action: &WmAction) {
    state.session.focus_right();
}

pub fn handle_prev_pane(state: &mut AppState, _action: &WmAction) {
    state.session.focus_left();
}

pub fn handle_workspace_next(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    let next = (current_ws + 1).min(state.session.workspaces.len().saturating_sub(1));
    if next != current_ws {
        switch_workspace_tracked(state, next);
    }
}

pub fn handle_workspace_prev(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    let prev = current_ws.saturating_sub(1);
    if prev != current_ws {
        switch_workspace_tracked(state, prev);
    }
}

pub fn handle_focus_toggle_local(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    if let Some(prev_pane) = state
        .last_visited_pane_per_ws
        .get(ws_idx)
        .copied()
        .flatten()
        && Some(prev_pane) != state.focused_pane
    {
        focus_pane_by_id(state, prev_pane);
    }
}

pub fn handle_focus_toggle_global(state: &mut AppState, _action: &WmAction) {
    if let Some(prev_ws) = state.last_visited_ws_idx {
        let current_ws = state.session.active_workspace_idx;
        if prev_ws != current_ws {
            switch_workspace_tracked(state, prev_ws);
        }
    }
}

pub fn handle_focus_pane(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusPane { pane_id } = action else {
        return;
    };
    // Focusing a pane changes which pane is active and **nothing else**. It does not release
    // container focus (F003/P086/T364).
    //
    // F003/P085/T352 put the release here, reasoning that "the keyboard goes to the pane". That is
    // not a property of a pane getting focus — it is a property of *what the user asked for*, and
    // this handler cannot tell the two apart. `Space` (hint) focuses a pane and deliberately keeps
    // the keyboard on the dock; the rule fired anyway and `j`/`k` stopped working.
    //
    // Every way out is now explicit at the site that means it: the component's activate-and-leave
    // queues `unfocus_dock` itself (`providers/workspaces/mod.rs`), `Esc` releases, a click that
    // lands outside every container releases (`mouse.rs`), and another container taking focus
    // takes the keyboard from this one (`tree_focus::focus_dock`).
    focus_pane_by_id(state, *pane_id);
}

pub fn handle_focus_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusWorkspace { ws_idx } = action else {
        return;
    };
    if *ws_idx < state.session.workspaces.len() {
        switch_workspace_tracked(state, *ws_idx);
    }
}
