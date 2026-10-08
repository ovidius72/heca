//! Putting a pane at an exact place — what a drop means, and what a key, a plugin or the RPC names
//! to do the same (`place_pane`).

use crate::app_state::AppState;
use crate::input::WmAction;
use heca_core::layout::ColumnId;

/// Take the pane out of wherever it is and put it where the action says
/// ([`Workspace::move_pane`] within one workspace, [`Workspace::take_pane`] and
/// [`Workspace::place_pane`] between two), sliding it from where it was when it
/// stays on screen. Nothing happens when the destination workspace does not exist — checked before
/// the pane is taken, so a bad index never loses it.
///
/// [`Workspace::move_pane`]: heca_core::layout::Workspace::move_pane
/// [`Workspace::take_pane`]: heca_core::layout::Workspace::take_pane
/// [`Workspace::place_pane`]: heca_core::layout::Workspace::place_pane
pub fn handle_place_pane(state: &mut AppState, action: &WmAction) {
    let WmAction::PlacePane {
        pane_id,
        ws_idx,
        col_idx,
        pane_idx,
    } = *action
    else {
        return;
    };
    if ws_idx >= state.session.workspaces.len() {
        return;
    }
    let Some((src_ws, _, _)) = crate::find_pane_location(&state.session, pane_id)
        .or_else(|| floating_location(state, pane_id))
    else {
        return;
    };
    let before = pane_rect(state, src_ws, pane_id);
    let new_column_id = ColumnId(state.session.next_id());
    if src_ws == ws_idx {
        // Within one workspace the place is counted as it is now, with the pane still in it; the
        // workspace works out what taking the pane out does to the numbers.
        if !state.session.workspaces[ws_idx].move_pane(pane_id, col_idx, pane_idx, new_column_id) {
            return;
        }
    } else {
        let Some(pane) = state.session.workspaces[src_ws].take_pane(pane_id) else {
            return;
        };
        state.session.workspaces[ws_idx].place_pane(pane, col_idx, pane_idx, new_column_id);
    }
    if let (Some(from), true) = (before, src_ws == ws_idx)
        && let Some(to) = pane_rect(state, ws_idx, pane_id)
    {
        slide_from(state, ws_idx, pane_id, from, to);
    }
}

/// The workspace holding `pane_id` among its floating panes.
fn floating_location(
    state: &AppState,
    pane_id: heca_core::layout::PaneId,
) -> Option<(usize, usize, usize)> {
    state
        .session
        .workspaces
        .iter()
        .position(|ws| ws.floating_panes.iter().any(|f| f.pane.id == pane_id))
        .map(|ws| (ws, 0, 0))
}

/// Where a tiled pane is drawn in its workspace.
fn pane_rect(
    state: &AppState,
    ws_idx: usize,
    pane_id: heca_core::layout::PaneId,
) -> Option<heca_core::layout::types::Rectangle> {
    state.session.workspaces[ws_idx]
        .scrolling
        .panes_with_positions()
        .into_iter()
        .find(|(id, _)| *id == pane_id)
        .map(|(_, rect)| rect)
}

/// Animate the placed pane from where it was, so a drop reads as a move and not a jump.
fn slide_from(
    state: &mut AppState,
    ws_idx: usize,
    pane_id: heca_core::layout::PaneId,
    from: heca_core::layout::types::Rectangle,
    to: heca_core::layout::types::Rectangle,
) {
    let ws = &mut state.session.workspaces[ws_idx];
    if let Some((col, row)) = ws.scrolling.pane_indices(pane_id) {
        ws.scrolling.columns[col].panes[row].animate_move_from(
            heca_core::layout::types::Point::new(from.loc.x - to.loc.x, from.loc.y - to.loc.y),
            heca_core::layout::animation::AnimationConfig::default(),
        );
    }
}
