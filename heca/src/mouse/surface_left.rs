//! What a click or a drop MEANS in the left sidebar.
//!
//! Hit testing, click routing and drop acceptance. Every entry point here is *told* what it landed
//! on: finding a target under the pointer was the old machine's job, and the framework resolves it
//! now (F003/P097/T496).

use crate::app::interaction::InteractionSource;
use crate::app_state::AppState;
use crate::input::WmAction;
use crate::providers::workspaces::WorkspaceRow;
use heca_core::layout::PaneId;
use heca_grid_ui::drag::DropSide;

// ═══════════════════════════════════════════════════════════════════════════════
//  Click routing
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
//  Hover
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
//  Accept / Drop
// ═══════════════════════════════════════════════════════════════════════════════

/// Execute a drop during sidebar drag: swap the pane with the one it landed on, or place it.
///
/// Either way the drop only works out *what* to do and posts it as an action
/// ([`dispatch_drop`](super::dispatch_drop)) — `swap`, or `place_pane` with the place it landed. The
/// action does the move, the slide and the follow-up, exactly as it would from a key or a plugin.
pub(crate) fn accept_drop(
    state: &mut AppState,
    pane_id: PaneId,
    original_ws: usize,
    swap: bool,
    target: Option<(WorkspaceRow, DropSide)>,
) {
    // **The target arrives resolved.** It used to be hunted for under the pointer here, at the
    // moment of release — which is how the drop came to be tied to one sidebar: the hunt was keyed
    // to it. The gesture now says what it landed on (F003/P097/T496).
    let Some((row, side)) = target else {
        // Off any card: the pane stays where it is.
        return;
    };
    let action = match row {
        // A swap applies only to a pane-on-pane drop; a workspace target is a plain move.
        WorkspaceRow::Pane { pane_id: target } if swap => {
            (target != pane_id).then_some(WmAction::Swap {
                a_id: pane_id,
                b_id: target,
            })
        }
        row => place_on_row(state, pane_id, original_ws, row, side),
    };
    if let Some(action) = action {
        super::dispatch_drop(state, InteractionSource::MouseLeftSidebar, action);
    }
}

/// The `place_pane` a drop on a sidebar row means: beside a pane card (above it on its top half),
/// at the end of a column, in a new column at the end of a workspace, or — on a floating pane's
/// card — at the end of the active column where it came from. `None` when there is nowhere to go.
fn place_on_row(
    state: &AppState,
    pane_id: PaneId,
    original_ws: usize,
    row: WorkspaceRow,
    side: DropSide,
) -> Option<WmAction> {
    let (ws_idx, col_idx, pane_idx) = match row {
        WorkspaceRow::Pane { pane_id: target } => {
            if target == pane_id {
                return None;
            }
            let (ws_idx, _, _) = crate::find_pane_location(&state.session, target)?;
            let (col, row) = state
                .session
                .workspaces
                .get(ws_idx)?
                .scrolling
                .pane_indices(target)?;
            let row = if side == DropSide::Before {
                row
            } else {
                row + 1
            };
            (ws_idx, col, Some(row))
        }
        // The end of the workspace: a gap past the last column clamps to it.
        WorkspaceRow::Workspace { ws_idx, .. } => (ws_idx, usize::MAX, None),
        WorkspaceRow::Column { ws_idx, col_id, .. } => {
            let ws = state.session.workspaces.get(ws_idx)?;
            let col = ws.scrolling.columns.iter().position(|c| c.id == col_id)?;
            (ws_idx, col, Some(usize::MAX))
        }
        WorkspaceRow::FloatingPane { .. } => {
            let ws = state.session.workspaces.get(original_ws)?;
            (
                original_ws,
                ws.scrolling.active_column_idx,
                Some(usize::MAX),
            )
        }
    };
    Some(WmAction::PlacePane {
        pane_id,
        ws_idx,
        col_idx,
        pane_idx,
    })
}
