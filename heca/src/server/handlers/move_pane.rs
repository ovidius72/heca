//! Moving a pane: to another column, another workspace, or a column of its own.

use heca_core::layout::{ColumnId, Moved, PaneId};

use crate::input::WmAction;
use crate::server::{Change, Refusal, ServerCx};

/// What a pane that landed says.
fn landed(pane: PaneId, moved: Option<Moved>) -> Vec<Change> {
    match moved {
        Some(m) => Change::after_move(
            m,
            Change::PaneMoved {
                pane,
                workspace: m.workspace,
                column: m.column,
            },
        ),
        None => Vec::new(),
    }
}

/// Move the active pane of the workspace `pane` is in — the asker's active one when no pane is
/// named — one column along. `step` does it.
fn move_along(
    cx: &mut ServerCx<'_>,
    pane: Option<PaneId>,
    step: impl FnOnce(&mut heca_core::layout::WorkspaceMut<'_>, ColumnId) -> bool,
) -> Vec<Change> {
    let workspace = match pane {
        Some(id) => cx.layout.session().pane_location(id).map(|(ws, ..)| ws),
        None => Some(cx.asker.workspace),
    };
    // Allocated before the workspace is borrowed; spent only if the move makes a column.
    let new_column = ColumnId(cx.layout.next_id());
    let Some(mut ws) = workspace.and_then(|idx| cx.layout.workspace_mut(idx)) else {
        return Vec::new();
    };
    if let Some(id) = pane {
        ws.activate_pane(id);
    }
    let Some(moving) = ws.active_pane().map(|p| p.id) else {
        return Vec::new();
    };
    if !step(&mut ws, new_column) {
        return vec![Change::LayoutChanged];
    }
    let column = ws.scroll().active_column_idx();
    let workspace = workspace.unwrap_or_default();
    vec![Change::PaneMoved { pane: moving, workspace, column }]
}

pub(super) fn move_pane_left(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MovePaneLeft { pane_id } = action else {
        return Vec::new();
    };
    move_along(cx, *pane_id, |ws, id| ws.scroll_mut().move_active_pane_left(id))
}

pub(super) fn move_pane_right(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MovePaneRight { pane_id } = action else {
        return Vec::new();
    };
    move_along(cx, *pane_id, |ws, id| ws.scroll_mut().move_active_pane_right(id))
}

/// Move a pane into a column of its own workspace.
pub(super) fn move_to_column_here(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::Move { pane_id, target_col } = action else {
        return Vec::new();
    };
    landed(*pane_id, cx.layout.move_pane_to_column(*pane_id, *target_col))
}

/// Move a pane to another workspace, as a column of its own.
pub(super) fn move_to_workspace(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MovePaneToWorkspace { pane_id, ws_idx } = action else {
        return Vec::new();
    };
    let Some((_, column, _)) = cx.layout.session().pane_location(*pane_id) else {
        return Vec::new();
    };
    landed(*pane_id, cx.layout.move_pane_to_workspace(*pane_id, *ws_idx, column, false))
}

/// Move a pane to column `col_idx` of workspace `ws_idx`, stacked with that column's panes.
pub(super) fn move_to_column(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MovePaneToColumn {
        pane_id,
        ws_idx,
        col_idx,
    } = action
    else {
        return Vec::new();
    };
    let Some((workspace, ..)) = cx.layout.session().pane_location(*pane_id) else {
        return Vec::new();
    };
    let moved = match workspace == *ws_idx {
        true => cx.layout.move_pane_to_column(*pane_id, *col_idx),
        false => cx.layout.move_pane_to_workspace(*pane_id, *ws_idx, *col_idx, true),
    };
    landed(*pane_id, moved)
}

/// Take the asker's focused pane out of its column into a new one, right of it.
pub(super) fn move_to_new_column(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let Some(pane) = cx.asker.focused_pane else {
        return Vec::new();
    };
    if cx.layout.session().pane_location(pane).is_none() {
        return Vec::new();
    }
    match cx.layout.move_pane_to_new_column(pane) {
        Some(moved) => landed(pane, Some(moved)),
        None => vec![Change::Refused(Refusal::OnlyPaneInColumn)],
    }
}

/// Take a pane from wherever it is to the bottom of the asker's active column.
pub(super) fn take_pane(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::TakePane {
        pane_id,
        focus_after,
    } = action
    else {
        return Vec::new();
    };
    let into = cx.asker.workspace;
    landed(*pane_id, cx.layout.take_pane_into(*pane_id, into, *focus_after))
}

/// Put a pane at an exact place.
pub(super) fn place_pane(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::PlacePane {
        pane_id,
        ws_idx,
        col_idx,
        pane_idx,
    } = action
    else {
        return Vec::new();
    };
    landed(*pane_id, cx.layout.place_pane(*pane_id, *ws_idx, *col_idx, *pane_idx))
}
