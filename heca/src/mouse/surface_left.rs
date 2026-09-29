//! What a click or a drop MEANS in the left sidebar.
//!
//! Hit testing, click routing and drop acceptance. Every entry point here is *told* what it landed
//! on: finding a target under the pointer was the old machine's job, and the framework resolves it
//! now (F003/P097/T496).

use crate::app::interaction::InteractionSource;
use crate::app_state::{AppState, InteractiveMovePhase};
use crate::chrome::ChromeDragItem;
use crate::input::WmAction;
use crate::providers::workspaces::WorkspaceRow;
use heca_core::layout::{PaneId, Workspace};
use heca_grid_ui::drag::DropSide;

// ═══════════════════════════════════════════════════════════════════════════════
//  Geometry
// ═══════════════════════════════════════════════════════════════════════════════

/// Helper: compute sidebar geometry (width, top, bottom).
fn sidebar_bounds(state: &AppState) -> (f32, f32, f32, f32) {
    let chrome = crate::chrome::ChromeConfig::of(state);
    let win_h = chrome.window().h as f32;
    // `left_sidebar_width` is 0 when Hidden (no icon rail), so the bounds collapse
    // to nothing and no click lands in the region — see `docs/sidebar-provider-modes.md`.
    let total_w = chrome.left_sidebar_width;
    let gap = chrome.sidebar_gap.max(0.0);
    let sx = gap.min(total_w * 0.5);
    let sw = (total_w - sx * 2.0).max(0.0);
    let sidebar_top = chrome.tab_bar_height + gap;
    let sidebar_bottom = (win_h - chrome.status_bar_height - gap).max(sidebar_top);
    (sx, sw, sidebar_top, sidebar_bottom)
}

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
    state.mouse.interactive_move = None;
    state.mouse.insert_hint = None;
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

/// A copy of workspace `ws_idx` with `pane_id` already taken out — what `place_pane` counts its
/// indices on, since it takes the pane out before placing it. Placing next to a card in the same
/// column the pane came from would otherwise land one row off.
fn without(state: &AppState, ws_idx: usize, pane_id: PaneId) -> Option<Workspace> {
    let mut ws = state.session.workspaces.get(ws_idx)?.clone();
    ws.take_pane(pane_id);
    Some(ws)
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
            let (col, row) = without(state, ws_idx, pane_id)?
                .scrolling
                .pane_indices(target)?;
            let row = if side == DropSide::Before {
                row
            } else {
                row + 1
            };
            (ws_idx, col, Some(row))
        }
        WorkspaceRow::Workspace { ws_idx, .. } => (
            ws_idx,
            without(state, ws_idx, pane_id)?.scrolling.columns.len(),
            None,
        ),
        WorkspaceRow::Column { ws_idx, col_id, .. } => {
            let ws = without(state, ws_idx, pane_id)?;
            let col = ws.scrolling.columns.iter().position(|c| c.id == col_id)?;
            (ws_idx, col, Some(usize::MAX))
        }
        WorkspaceRow::FloatingPane { .. } => {
            let ws = without(state, original_ws, pane_id)?;
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

// ═══════════════════════════════════════════════════════════════════════════════
//  Interactive-move drop (content-area drag → sidebar target)
// ═══════════════════════════════════════════════════════════════════════════════

/// Handle a drop during interactive move where the cursor is over the sidebar.
///
/// Returns true if the drop was handled (sidebar target found).
/// Ported from the former `sidebar_drop::handle_drop()`.
pub(crate) fn handle_interactive_move_drop(state: &mut AppState, pos: (f32, f32)) -> bool {
    let (sx, sw, sidebar_top, sidebar_bottom) = sidebar_bounds(state);
    if pos.0 < sx || pos.0 >= sx + sw || pos.1 < sidebar_top || pos.1 >= sidebar_bottom {
        return false;
    }

    // Resolved against the **retained** tree's real laid-out bounds, like every other row
    // resolution — not by arithmetic on a fixed row height, which stopped describing this sidebar
    // the moment its rows became widgets of varying height (F003/P085/T356).
    let item = match crate::chrome::sidebar_item_at(state, pos) {
        Some(item) => item,
        None => return false,
    };

    // Source pane is in the layout. Get its ID from the drag state.
    let source_id = match state.mouse.interactive_move {
        Some(InteractiveMovePhase::Moving { pane_id, .. }) => pane_id,
        _ => return false,
    };

    let original_ws = state.session.active_workspace_idx;

    // Reset drag offset so layout positions are correct for removal.
    crate::mouse::interactive::reset_interactive_move_offset(state);

    // The same one answer the sidebar's own drops and the framework's outline use.
    let shift_held = heca_grid_ui::drag::DropAction::held().is_swap();

    match item {
        ChromeDragItem::Pane(target_pid) if shift_held => {
            // Swap: the same action a click or a key would run, through the same door.
            // The source pane is still in the layout (interactive move keeps it there).
            super::dispatch_drop(
                state,
                InteractionSource::MouseLeftSidebar,
                WmAction::Swap {
                    a_id: source_id,
                    b_id: target_pid,
                },
            );
        }
        _ => {
            // Move: the drop says where; `place_pane` does it.
            let row = match item {
                ChromeDragItem::Pane(target) => Some(WorkspaceRow::Pane { pane_id: target }),
                ChromeDragItem::Workspace { ws } => {
                    state
                        .session
                        .workspaces
                        .get(ws)
                        .map(|w| WorkspaceRow::Workspace {
                            ws_idx: ws,
                            ws_id: w.id,
                        })
                }
                ChromeDragItem::Column { ws, col } => state
                    .session
                    .workspaces
                    .get(ws)
                    .and_then(|w| w.scrolling.columns.get(col))
                    .map(|c| WorkspaceRow::Column {
                        ws_idx: ws,
                        col_idx: col,
                        col_id: c.id,
                    }),
            };
            if let Some(action) = row
                .and_then(|row| place_on_row(state, source_id, original_ws, row, DropSide::After))
            {
                super::dispatch_drop(state, InteractionSource::MouseLeftSidebar, action);
            }
        }
    }

    state.mouse.interactive_move = None;
    state.mouse.insert_hint = None;
    true
}
