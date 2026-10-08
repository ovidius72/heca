//! Moving a pane: to another column, another workspace, or a column of its own.

use heca_core::layout::{ColumnId, Direction, Moved, PaneId, SpaceEffect};

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

/// What a pane that moved within workspace `workspace` says: what changed there, then where it
/// landed.
fn moved_within(workspace: usize, pane: PaneId, effects: Vec<SpaceEffect>) -> Vec<Change> {
    let landed = SpaceEffect::landing(&effects);
    let mut changes = vec![Change::Arranged { workspace, effects }];
    if let Some((column, _)) = landed {
        changes.push(Change::PaneMoved { pane, workspace, column });
    }
    changes
}

/// Where a pane is — workspace, column, row — and which: `pane` when it is named, otherwise the
/// active pane of the asker's column.
fn spot(cx: &ServerCx<'_>, pane: Option<PaneId>) -> Option<(PaneId, usize, usize, usize)> {
    let session = cx.layout.session();
    match pane {
        Some(id) => session.pane_location(id).map(|(ws, col, row)| (id, ws, col, row)),
        None => {
            let (ws, col) = (cx.asker.workspace, cx.asker.column);
            let column = session.workspaces.get(ws)?.scrolling.columns.get(col)?;
            let row = column.active_pane_idx;
            column.panes.get(row).map(|p| (p.id, ws, col, row))
        }
    }
}

/// Move `pane` — the asker's active one when none is named — one column along `dir`.
fn move_along(cx: &mut ServerCx<'_>, pane: Option<PaneId>, dir: Direction) -> Vec<Change> {
    let Some((moving, workspace, col, row)) = spot(cx, pane) else {
        return Vec::new();
    };
    let new_column = ColumnId(cx.layout.next_id());
    let height = cx.asker.area.h;
    let moved = cx.arrange(workspace, |space, _| {
        space.move_pane_along(col, row, dir, new_column, height)
    });
    match moved {
        Some(effects) => moved_within(workspace, moving, effects),
        // Nothing moved, but a pane that was named is still the one the window asked about.
        None => vec![Change::LayoutChanged],
    }
}

/// Move `pane` into column `dst_col` of its own workspace; `dst_col` equal to the number of
/// columns makes a new one at the end.
fn move_within(cx: &mut ServerCx<'_>, pane: PaneId, dst_col: usize) -> Vec<Change> {
    let Some((_, workspace, col, row)) = spot(cx, Some(pane)) else {
        return Vec::new();
    };
    let new_column = ColumnId(cx.layout.next_id());
    let height = cx.asker.area.h;
    let moved = cx.arrange(workspace, |space, _| {
        let count = space.columns.len();
        match dst_col {
            to if to == col || to > count => None,
            to if to == count => space.extract_pane(col, row, new_column, to),
            to => space.move_pane_between(col, row, to, height),
        }
    });
    moved.map_or_else(Vec::new, |effects| moved_within(workspace, pane, effects))
}

pub(super) fn move_pane_left(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MovePaneLeft { pane_id } = action else {
        return Vec::new();
    };
    move_along(cx, *pane_id, Direction::Left)
}

pub(super) fn move_pane_right(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::MovePaneRight { pane_id } = action else {
        return Vec::new();
    };
    move_along(cx, *pane_id, Direction::Right)
}

/// Move a pane into a column of its own workspace.
pub(super) fn move_to_column_here(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::Move { pane_id, target_col } = action else {
        return Vec::new();
    };
    move_within(cx, *pane_id, *target_col)
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
    match workspace == *ws_idx {
        true => move_within(cx, *pane_id, *col_idx),
        false => landed(*pane_id, cx.layout.move_pane_to_workspace(*pane_id, *ws_idx, *col_idx, true)),
    }
}

/// Take the asker's focused pane out of its column into a new one, right of it.
pub(super) fn move_to_new_column(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let Some((pane, workspace, col, row)) = cx.asker.focused_pane.and_then(|p| spot(cx, Some(p))) else {
        return Vec::new();
    };
    let new_column = ColumnId(cx.layout.next_id());
    let moved = cx.arrange(workspace, |space, _| {
        let alone = space.columns.get(col).is_none_or(|c| c.panes.len() <= 1);
        match alone {
            true => None,
            false => space.extract_pane(col, row, new_column, col + 1),
        }
    });
    match moved {
        Some(effects) => moved_within(workspace, pane, effects),
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
