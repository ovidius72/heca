//! Focus moves: columns, panes and workspaces, and the focus toggles.

use crate::app::focus::{find_pane_workspace, focus_pane_by_id};
use crate::app_state::AppState;
use crate::input::WmAction;
use crate::switch_workspace_tracked;

pub fn handle_focus_left(state: &mut AppState, _action: &WmAction) -> crate::actions::ActionResult {
    state.layout_mut().focus_left();
    Ok(())
}

pub fn handle_focus_right(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    state.layout_mut().focus_right();
    Ok(())
}

pub fn handle_focus_up(state: &mut AppState, _action: &WmAction) -> crate::actions::ActionResult {
    if let Some(mut ws) = state.layout_mut().active_workspace_mut() {
        ws.focus_up();
    }
    Ok(())
}

pub fn handle_focus_down(state: &mut AppState, _action: &WmAction) -> crate::actions::ActionResult {
    if let Some(mut ws) = state.layout_mut().active_workspace_mut() {
        ws.focus_down();
    }
    Ok(())
}

pub fn handle_next_pane(state: &mut AppState, _action: &WmAction) -> crate::actions::ActionResult {
    state.layout_mut().focus_right();
    Ok(())
}

pub fn handle_prev_pane(state: &mut AppState, _action: &WmAction) -> crate::actions::ActionResult {
    state.layout_mut().focus_left();
    Ok(())
}

pub fn handle_workspace_next(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    let current_ws = state.layout().active_workspace_idx();
    let next = (current_ws + 1).min(state.session.workspaces.len().saturating_sub(1));
    if next != current_ws {
        switch_workspace_tracked(state, next);
    }
    Ok(())
}

pub fn handle_workspace_prev(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    let current_ws = state.layout().active_workspace_idx();
    let prev = current_ws.saturating_sub(1);
    if prev != current_ws {
        switch_workspace_tracked(state, prev);
    }
    Ok(())
}

pub fn handle_focus_toggle_local(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    let ws_idx = state.layout().active_workspace_idx();
    if let Some(prev_pane) = state
        .last_visited_pane_per_ws
        .get(ws_idx)
        .copied()
        .flatten()
        && Some(prev_pane) != state.focused_pane
    {
        focus_pane_by_id(state, prev_pane);
    }
    Ok(())
}

pub fn handle_focus_toggle_global(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    if let Some(prev_ws) = state.last_visited_ws_idx {
        let current_ws = state.layout().active_workspace_idx();
        if prev_ws != current_ws {
            switch_workspace_tracked(state, prev_ws);
        }
    }
    Ok(())
}

/// **Back to the last pane you were in, wherever it is** — another workspace included. Run again, it
/// returns, like switching between two windows. A pane that has closed since is no answer.
pub fn handle_focus_toggle_last(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    if let Some(last) = state.last_focused
        && Some(last) != state.focused_pane
        && find_pane_workspace(&state.session, last).is_some()
    {
        focus_pane_by_id(state, last);
    }
    Ok(())
}

pub fn handle_focus_pane(state: &mut AppState, action: &WmAction) -> crate::actions::ActionResult {
    let WmAction::FocusPane { pane_id } = action else {
        return Ok(());
    };
    // Focusing a pane changes which pane is active and **nothing else**. It does not release
    // container focus.
    //
    // The release once lived here, reasoning that "the keyboard goes to the pane". That is
    // not a property of a pane getting focus — it is a property of *what the user asked for*, and
    // this handler cannot tell the two apart. `Space` (hint) focuses a pane and deliberately keeps
    // the keyboard on the dock; the rule fired anyway and `j`/`k` stopped working.
    //
    // Every way out is now explicit at the site that means it: the component's activate-and-leave
    // queues `unfocus_dock` itself (`providers/workspaces/mod.rs`), `Esc` releases, a click that
    // lands outside every container releases (`mouse.rs`), and another container taking focus
    // takes the keyboard from this one (`tree_focus::focus_dock`).
    focus_pane_by_id(state, *pane_id);
    Ok(())
}

pub fn handle_focus_workspace(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::FocusWorkspace { ws_idx } = action else {
        return Ok(());
    };
    if *ws_idx < state.session.workspaces.len() {
        switch_workspace_tracked(state, *ws_idx);
    }
    Ok(())
}
