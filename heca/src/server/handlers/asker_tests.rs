//! **The server acts where its asker is, not where the window's own view is.** Two windows share
//! a session and each shows its own workspace; a request carries its asker's place, and nothing
//! here reads the view's.

use heca_core::layout::{ColumnEffect, ColumnWidth, PaneId, Size};

use super::testing::{run_as, two_workspaces_of_two_columns};
use crate::input::WmAction;
use crate::server::{Asker, Change};

/// The size the test window lays a workspace out in.
const WINDOW: Size = Size { w: 1000.0, h: 800.0 };

/// An asker in workspace 1, column 0 — while the window the session is held through shows 0.
fn in_workspace_1() -> Asker {
    Asker { focused_pane: Some(PaneId(11)), workspace: 1, column: 0, area: WINDOW }
}

fn width(w: &heca_core::layout::testing::Windowed, ws: usize, col: usize) -> ColumnWidth {
    w.session.workspaces[ws].scrolling.columns[col].width
}

/// A resize by key changes the asker's workspace and leaves the one the view shows alone.
#[test]
fn a_resize_changes_the_workspace_the_asker_is_in() {
    let mut w = two_workspaces_of_two_columns();
    let (shown_before, asked_before) = (width(&w, 0, 0), width(&w, 1, 0));
    let changes = run_as(&mut w, in_workspace_1(), WmAction::ResizeIncrease);
    assert_eq!(changes, [Change::ColumnsChanged { workspace: 1, effect: ColumnEffect::Resized { idx: 0 } }]);
    assert_ne!(width(&w, 1, 0), asked_before, "the asker's workspace changed");
    assert_eq!(width(&w, 0, 0), shown_before, "the view's workspace did not");
}

/// A pane floats in the asker's workspace.
#[test]
fn a_float_acts_in_the_askers_workspace() {
    let mut w = two_workspaces_of_two_columns();
    run_as(&mut w, in_workspace_1(), WmAction::Float);
    assert_eq!(w.session.workspaces[1].floating_panes.len(), 1);
    assert!(w.session.workspaces[0].floating_panes.is_empty());
}

/// Moving the asker's column goes from the asker's workspace and column.
#[test]
fn a_column_moves_from_the_askers_place() {
    let mut w = two_workspaces_of_two_columns();
    let up = run_as(&mut w, in_workspace_1(), WmAction::MoveColumnUp);
    assert_eq!(up, [Change::ColumnMoved { workspace: 0, column: 2 }]);
    assert!(run_as(&mut w, in_workspace_1(), WmAction::MoveColumnDown).is_empty(), "nothing below the last workspace");
    let in_0 = Asker { focused_pane: None, workspace: 0, column: 1, area: WINDOW };
    let down = run_as(&mut w, in_0, WmAction::MoveColumnDown);
    assert_eq!(down, [Change::ColumnMoved { workspace: 1, column: 1 }]);
    assert_eq!(w.session.workspaces[0].scrolling.columns.len(), 2);
}

/// A taken pane lands in the asker's workspace.
#[test]
fn a_taken_pane_lands_in_the_askers_workspace() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run_as(&mut w, in_workspace_1(), WmAction::TakePane { pane_id: PaneId(1), focus_after: true });
    assert_eq!(changes, [Change::PaneMoved { pane: PaneId(1), workspace: 1, column: 1 }]);
}

/// Resetting the workspace name clears the asker's workspace.
#[test]
fn a_reset_clears_the_name_of_the_askers_workspace() {
    let mut w = two_workspaces_of_two_columns();
    w.session.workspaces[0].name = Some("a".into());
    w.session.workspaces[1].name = Some("b".into());
    run_as(&mut w, in_workspace_1(), WmAction::ResetWorkspaceName);
    assert_eq!(w.session.workspaces[1].name, None);
    assert_eq!(w.session.workspaces[0].name.as_deref(), Some("a"));
}

/// With no pane named, the asker's workspace's active pane moves.
#[test]
fn a_move_with_no_pane_named_moves_in_the_askers_workspace() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run_as(&mut w, in_workspace_1(), WmAction::MovePaneLeft { pane_id: None });
    assert_eq!(changes, [Change::PaneMoved { pane: PaneId(12), workspace: 1, column: 0 }]);
}

/// A column moved to a workspace goes from the asker's workspace.
#[test]
fn a_column_moved_to_a_workspace_goes_from_the_askers_workspace() {
    let mut w = two_workspaces_of_two_columns();
    let action = WmAction::MoveColumnToWorkspace { col_idx: 0, ws_idx: 0, focus: true };
    let changes = run_as(&mut w, in_workspace_1(), action);
    assert_eq!(changes, [Change::ColumnMoved { workspace: 0, column: 2 }]);
    assert_eq!(w.session.workspaces[1].scrolling.columns.len(), 1);
}

/// A window's place becomes the asker's data in one function: the workspace it shows, and the
/// column that is active in it.
#[test]
fn an_asker_is_where_the_window_is() {
    let mut w = two_workspaces_of_two_columns();
    w.show(1);
    let asker = Asker::seen_through(w.l(), Some(PaneId(12)));
    let area = w.l().active_workspace().map(|ws| ws.scroll().area().size).expect("a workspace");
    assert_eq!(asker, Asker { focused_pane: Some(PaneId(12)), workspace: 1, column: 1, area });
    assert_eq!(area, WINDOW, "the area of the window the test fixture makes");
}

/// Moving the active column left goes from the asker's column, whichever the view has active.
#[test]
fn swap_left_moves_the_askers_column() {
    let mut w = two_workspaces_of_two_columns();
    let asker = Asker { focused_pane: None, workspace: 0, column: 1, area: WINDOW };
    assert_eq!(
        run_as(&mut w, asker, WmAction::SwapLeft),
        [Change::ColumnsChanged { workspace: 0, effect: ColumnEffect::Moved { from: 1, to: 0 } }]
    );
    assert_eq!(w.session.workspaces[0].scrolling.columns[0].panes[0].id, PaneId(2));
    let first = Asker { column: 0, ..asker };
    assert!(run_as(&mut w, first, WmAction::SwapLeft).is_empty(), "nothing left of the first");
    assert_eq!(
        run_as(&mut w, first, WmAction::SwapRight),
        [Change::ColumnsChanged { workspace: 0, effect: ColumnEffect::Moved { from: 0, to: 1 } }]
    );
}

/// With three columns the step is one place, from the asker's column, not to an end.
#[test]
fn swap_left_steps_one_place_and_resize_acts_on_the_askers_column() {
    let mut w = heca_core::layout::testing::Windowed::with_shape(&[&[&[1], &[2], &[3]]]);
    let asker = Asker { focused_pane: None, workspace: 0, column: 2, area: WINDOW };
    assert_eq!(
        run_as(&mut w, asker, WmAction::SwapLeft),
        [Change::ColumnsChanged { workspace: 0, effect: ColumnEffect::Moved { from: 2, to: 1 } }]
    );
    assert_eq!(
        run_as(&mut w, asker, WmAction::ResizeIncrease),
        [Change::ColumnsChanged { workspace: 0, effect: ColumnEffect::Resized { idx: 2 } }]
    );
}
