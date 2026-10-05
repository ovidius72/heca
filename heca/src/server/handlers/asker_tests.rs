//! **The server acts where its asker is, not where the window's own view is.** Two windows share
//! a session and each shows its own workspace; a request carries its asker's place, and nothing
//! here reads the view's.

use heca_core::layout::{ColumnWidth, PaneId};

use super::testing::{run_as, two_workspaces_of_two_columns};
use crate::input::WmAction;
use crate::server::{Asker, Change};

/// An asker in workspace 1, column 0 — while the window the session is held through shows 0.
fn in_workspace_1() -> Asker {
    Asker { focused_pane: Some(PaneId(11)), workspace: 1, column: 0 }
}

fn width(w: &heca_core::layout::testing::Windowed, ws: usize, col: usize) -> ColumnWidth {
    w.session.workspaces[ws].scrolling.columns[col].width
}

/// A resize by key changes the asker's workspace and leaves the one the view shows alone.
#[test]
fn a_resize_changes_the_workspace_the_asker_is_in() {
    let mut w = two_workspaces_of_two_columns();
    let (shown_before, asked_before) = (width(&w, 0, 1), width(&w, 1, 1));
    let changes = run_as(&mut w, in_workspace_1(), WmAction::ResizeIncrease);
    assert_eq!(changes, [Change::LayoutChanged]);
    assert_ne!(width(&w, 1, 1), asked_before, "the asker's workspace changed");
    assert_eq!(width(&w, 0, 1), shown_before, "the view's workspace did not");
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
    let in_0 = Asker { focused_pane: None, workspace: 0, column: 1 };
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
    assert_eq!(asker, Asker { focused_pane: Some(PaneId(12)), workspace: 1, column: 1 });
}
