//! The swap and move actions, run the way the registry runs them, and the facts they report.

use heca_core::layout::testing::Windowed;
use heca_core::layout::{Pane, PaneId};

use super::testing::{run, run_for, two_workspaces_of_two_columns};
use crate::input::WmAction;
use crate::server::{Change, Refusal};

/// A swap says the layout changed; swapping a pane with itself says nothing.
#[test]
fn a_swap_of_two_panes_reports_a_layout_change() {
    let mut w = two_workspaces_of_two_columns();
    assert_eq!(
        run(&mut w, WmAction::Swap { a_id: PaneId(1), b_id: PaneId(2) }),
        [Change::LayoutChanged]
    );
    assert_eq!(w.session.workspaces[0].scrolling.columns[0].panes[0].id, PaneId(2));
    assert!(run(&mut w, WmAction::Swap { a_id: PaneId(1), b_id: PaneId(1) }).is_empty());
}

/// Swapping columns across workspaces trades them and says so.
#[test]
fn a_swap_of_columns_across_workspaces_reports_a_layout_change() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::SwapColumns { a_ws: 0, a_col: 0, b_ws: 1, b_col: 0 });
    assert_eq!(changes, [Change::LayoutChanged]);
    assert_eq!(w.session.workspaces[0].scrolling.columns[0].panes[0].id, PaneId(11));
}

/// A pane moved to another workspace is reported where it landed, and the server does not show it.
#[test]
fn a_pane_moved_to_another_workspace_says_where_it_landed() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::MovePaneToWorkspace { pane_id: PaneId(1), ws_idx: 1 });
    assert_eq!(changes, [Change::PaneMoved { pane: PaneId(1), workspace: 1, column: 0 }]);
    assert_eq!(w.l().active_workspace_idx(), 0, "the server never switches the shown workspace");
}

/// A move that empties a workspace says it was removed, before it says where the thing landed.
#[test]
fn a_move_that_empties_a_workspace_reports_its_removal_first() {
    let mut w = two_workspaces_of_two_columns();
    run(&mut w, WmAction::MoveColumnToWorkspace { col_idx: 0, ws_idx: 1, focus: true });
    let changes = run(&mut w, WmAction::MoveColumnToWorkspace { col_idx: 0, ws_idx: 1, focus: true });
    assert_eq!(
        changes,
        [Change::WorkspaceRemoved { index: 0 }, Change::ColumnMoved { workspace: 0, column: 3 }]
    );
    assert_eq!(w.session.workspaces.len(), 1);
}

/// A column moved up or down goes to the neighbouring workspace; at the edge nothing happens.
#[test]
fn a_column_moves_to_the_neighbouring_workspace() {
    let mut w = two_workspaces_of_two_columns();
    assert!(run(&mut w, WmAction::MoveColumnUp).is_empty(), "no workspace above the first");
    let changes = run(&mut w, WmAction::MoveColumnDown);
    assert_eq!(changes, [Change::ColumnMoved { workspace: 1, column: 2 }]);
}

/// A column moved to a place among the columns of its own workspace.
#[test]
fn a_column_is_moved_to_an_index() {
    let mut w = two_workspaces_of_two_columns();
    let action = WmAction::MoveColumn { src_ws: 0, src_col: 0, dst_ws: 0, dst_idx: 1, focus: false };
    assert_eq!(run(&mut w, action), [Change::ColumnMoved { workspace: 0, column: 1 }]);
}

/// The columns of workspace `ws`, as pane ids.
fn columns(w: &Windowed, ws: usize) -> Vec<Vec<u64>> {
    w.session.workspaces[ws]
        .scrolling
        .columns
        .iter()
        .map(|c| c.panes.iter().map(|p| p.id.0).collect())
        .collect()
}

/// What a pane that moved within a workspace says: what changed there, then where it landed.
fn moved(changes: &[Change]) -> Option<Change> {
    match changes {
        [Change::Arranged { effects, .. }, landed @ Change::PaneMoved { .. }] if !effects.is_empty() => {
            Some(landed.clone())
        }
        _ => None,
    }
}

/// A pane moved by id in another workspace is moved there, from where it is.
#[test]
fn a_pane_in_another_workspace_is_moved_where_it_is() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::MovePaneRight { pane_id: Some(PaneId(11)) });
    assert_eq!(moved(&changes), Some(Change::PaneMoved { pane: PaneId(11), workspace: 1, column: 0 }));
    assert_eq!(columns(&w, 1), [vec![12, 11]]);
    assert_eq!(w.l().active_workspace_idx(), 0, "the server never switches the shown workspace");
    assert!(run(&mut w, WmAction::MovePaneLeft { pane_id: Some(PaneId(99)) }).is_empty());
}

/// A pane moved left at the first column, with a neighbour, gets a column of its own there.
#[test]
fn a_pane_moved_past_the_edge_gets_a_column_of_its_own() {
    let mut w = two_workspaces_of_two_columns();
    w.ws().scroll_mut().add_pane_to_column(0, None, Pane::new(PaneId(50), "x"), false);
    let changes = run(&mut w, WmAction::MovePaneLeft { pane_id: Some(PaneId(50)) });
    assert_eq!(moved(&changes), Some(Change::PaneMoved { pane: PaneId(50), workspace: 0, column: 0 }));
    assert_eq!(columns(&w, 0), [vec![50], vec![1], vec![2]]);
}

/// Moving a pane into another column of its workspace: the column it emptied goes.
#[test]
fn a_pane_moves_to_a_column_of_its_own_workspace() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::Move { pane_id: PaneId(1), target_col: 1 });
    assert_eq!(moved(&changes), Some(Change::PaneMoved { pane: PaneId(1), workspace: 0, column: 0 }));
    assert_eq!(columns(&w, 0), [vec![2, 1]]);
}

/// A column one past the last is a new column at the end; the column it is in already is nothing.
#[test]
fn a_pane_moves_to_a_new_last_column() {
    let mut w = Windowed::with_shape(&[&[&[1, 2], &[3]]]);
    let changes = run(&mut w, WmAction::Move { pane_id: PaneId(1), target_col: 2 });
    assert_eq!(moved(&changes), Some(Change::PaneMoved { pane: PaneId(1), workspace: 0, column: 2 }));
    assert_eq!(columns(&w, 0), [vec![2], vec![3], vec![1]]);
    assert!(run(&mut w, WmAction::Move { pane_id: PaneId(1), target_col: 2 }).is_empty());
    assert!(run(&mut w, WmAction::Move { pane_id: PaneId(1), target_col: 9 }).is_empty());
}

/// A pane alone in its column cannot be taken out of it: the server refuses, and says why.
#[test]
fn taking_a_lone_pane_out_of_its_column_is_refused() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run_for(&mut w, Some(PaneId(1)), WmAction::MovePaneToNewColumn);
    assert_eq!(changes, [Change::Refused(Refusal::OnlyPaneInColumn)]);
    assert_eq!(columns(&w, 0), [vec![1], vec![2]]);
    assert!(run_for(&mut w, None, WmAction::MovePaneToNewColumn).is_empty());
}

/// A pane with a neighbour goes into a column of its own, right of the one it left.
#[test]
fn a_pane_with_a_neighbour_goes_into_a_new_column() {
    let mut w = two_workspaces_of_two_columns();
    w.ws().scroll_mut().add_pane_to_column(0, None, Pane::new(PaneId(50), "x"), false);
    let changes = run_for(&mut w, Some(PaneId(50)), WmAction::MovePaneToNewColumn);
    assert_eq!(moved(&changes), Some(Change::PaneMoved { pane: PaneId(50), workspace: 0, column: 1 }));
    assert_eq!(columns(&w, 0), [vec![1], vec![50], vec![2]]);
}

/// Swapping the active pane down and up moves it in its column, and says what changed there; at
/// the edge nothing does.
#[test]
fn the_active_pane_swaps_within_its_column() {
    let mut w = Windowed::with_shape(&[&[&[1, 2, 3]]]);
    w.session.workspaces[0].scrolling.columns[0].active_pane_idx = 0;
    assert!(run(&mut w, WmAction::SwapUp).is_empty(), "already at the top");
    let changes = run(&mut w, WmAction::SwapDown);
    assert!(matches!(changes[..], [Change::Arranged { workspace: 0, .. }]), "{changes:?}");
    assert_eq!(columns(&w, 0), [vec![2, 1, 3]]);
    assert_eq!(w.session.workspaces[0].scrolling.columns[0].active_pane_idx, 1, "it travelled");
}

/// Taking a pane puts it at the bottom of the asker's active column and says where it landed.
#[test]
fn a_taken_pane_lands_in_the_active_column() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::TakePane { pane_id: PaneId(1), focus_after: true });
    assert_eq!(changes, [Change::PaneMoved { pane: PaneId(1), workspace: 0, column: 0 }]);
    assert!(run(&mut w, WmAction::TakePane { pane_id: PaneId(1), focus_after: true }).is_empty());
}

/// Placing a pane in a workspace that is not there says nothing and keeps the pane.
#[test]
fn a_placed_pane_says_where_it_landed_and_a_bad_place_keeps_it() {
    let mut w = two_workspaces_of_two_columns();
    let place = |ws_idx| WmAction::PlacePane { pane_id: PaneId(1), ws_idx, col_idx: 0, pane_idx: None };
    assert!(run(&mut w, place(9)).is_empty());
    assert_eq!(w.session.workspaces[0].scrolling.columns.len(), 2);
    assert_eq!(
        run(&mut w, place(1)),
        [Change::PaneMoved { pane: PaneId(1), workspace: 1, column: 0 }]
    );
}

/// Making a pane says it was added and where; a workspace that is not there says nothing.
#[test]
fn a_made_pane_is_reported_as_added() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::AddPaneToColumn { ws_idx: 1, col_idx: 0 });
    let [Change::PaneAdded { workspace: 1, column: 0, pane }] = changes[..] else {
        panic!("unexpected {changes:?}");
    };
    assert!(w.session.workspaces[1].find_pane(pane).is_some());
    let changes = run(&mut w, WmAction::AddColumnToWorkspace { ws_idx: 0 });
    assert!(matches!(changes[..], [Change::PaneAdded { workspace: 0, .. }]));
    assert_eq!(w.session.workspaces[0].scrolling.columns.len(), 3, "a column of its own");
    assert!(run(&mut w, WmAction::AddColumnToWorkspace { ws_idx: 9 }).is_empty());
}

/// Run `action` for the window and have the window react to what it says, as the app does.
fn run_and_react(w: &mut Windowed, focused: Option<PaneId>, action: WmAction) {
    let before = w.l().before();
    let changes = run_for(w, focused, action.clone());
    let (mut last_ws, mut panes, mut cursors) = (None, Vec::new(), Vec::new());
    let mut tracking = crate::app_state::reaction::Tracking {
        last_visited_ws: &mut last_ws,
        last_visited_pane_per_ws: &mut panes,
        expose_cursor_per_ws: &mut cursors,
    };
    for change in &changes {
        crate::app_state::reaction::window_reacts(&mut w.m(), &mut tracking, change, Some(&action), Some(&before));
    }
}

/// Two columns of two panes that both fit: the pane moved left from the second lands in the first,
/// the window follows it there, and nothing that was on screen leaves it.
#[test]
fn a_window_follows_a_pane_it_moved_without_scrolling() {
    let mut w = Windowed::with_shape(&[&[&[1, 2], &[3, 4]]]);
    w.ws().scroll_mut().activate_column(1);
    let rest = w.ws().scroll().target_view_pos();
    let drawn = |w: &mut Windowed| {
        w.ws().scroll().panes_with_positions().into_iter().find(|(id, _)| *id == PaneId(4)).map(|(_, r)| r.loc)
    };
    let was = drawn(&mut w);

    run_and_react(&mut w, Some(PaneId(4)), WmAction::MovePaneLeft { pane_id: Some(PaneId(4)) });

    assert_eq!(drawn(&mut w), was, "the moved pane slides from where it was drawn");

    assert_eq!(columns(&w, 0), [vec![1, 2, 4], vec![3]]);
    let focused = w.l().active_workspace().and_then(|ws| ws.scroll().active_pane().map(|p| p.id));
    assert_eq!(focused, Some(PaneId(4)), "the window followed the pane");
    let target = w.ws().scroll().target_view_pos();
    assert!((target - rest).abs() < 0.5, "the view moved from {rest} to {target}");
}
