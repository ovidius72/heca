//! The swap and move actions, run the way the registry runs them, and the facts they report.

use heca_core::layout::PaneId;

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

/// A pane moved by id in another workspace is moved there, from where it is.
#[test]
fn a_pane_in_another_workspace_is_moved_where_it_is() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::MovePaneRight { pane_id: Some(PaneId(11)) });
    assert_eq!(changes, [Change::PaneMoved { pane: PaneId(11), workspace: 1, column: 0 }]);
    assert_eq!(w.l().active_workspace_idx(), 0);
    assert!(run(&mut w, WmAction::MovePaneLeft { pane_id: Some(PaneId(99)) }).is_empty());
}

/// Moving a pane to a column of its own workspace.
#[test]
fn a_pane_moves_to_a_column_of_its_own_workspace() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::Move { pane_id: PaneId(1), target_col: 1 });
    assert_eq!(changes, [Change::PaneMoved { pane: PaneId(1), workspace: 0, column: 0 }]);
}

/// A pane alone in its column cannot be taken out of it: the server refuses, and says why.
#[test]
fn taking_a_lone_pane_out_of_its_column_is_refused() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run_for(&mut w, Some(PaneId(1)), WmAction::MovePaneToNewColumn);
    assert_eq!(changes, [Change::Refused(Refusal::OnlyPaneInColumn)]);
    assert!(run_for(&mut w, None, WmAction::MovePaneToNewColumn).is_empty());
}

/// A pane with a neighbour goes into a column of its own.
#[test]
fn a_pane_with_a_neighbour_goes_into_a_new_column() {
    let mut w = two_workspaces_of_two_columns();
    w.ws().scroll_mut().add_pane_to_column(0, None, heca_core::layout::Pane::new(PaneId(50), "x"), false);
    let changes = run_for(&mut w, Some(PaneId(50)), WmAction::MovePaneToNewColumn);
    assert_eq!(changes, [Change::PaneMoved { pane: PaneId(50), workspace: 0, column: 1 }]);
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
