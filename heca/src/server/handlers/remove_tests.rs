//! The close and delete actions, run the way the registry runs them, and the facts they report.

use heca_core::layout::PaneId;

use super::testing::{run, run_for, two_workspaces_of_two_columns};
use crate::input::WmAction;
use crate::server::Change;

/// **A by-id close finds the pane wherever it is**, not only in the workspace the asker is in: the
/// sidebar, a menu, the exposé and RPC all name panes the keyboard is not on.
#[test]
fn closing_a_pane_by_id_finds_it_in_another_workspace() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::ClosePaneById { pane_id: PaneId(12) });
    assert_eq!(changes, [Change::PaneRemoved { pane: PaneId(12) }, Change::LayoutChanged]);
    assert!(w.session.workspaces[1].find_pane(PaneId(12)).is_none());
    assert!(run(&mut w, WmAction::ClosePaneById { pane_id: PaneId(12) }).is_empty());
}

/// Closing the last pane of a workspace removes the workspace, and says so after the pane.
#[test]
fn closing_a_workspaces_last_pane_reports_the_workspace_too() {
    let mut w = two_workspaces_of_two_columns();
    run(&mut w, WmAction::ClosePaneById { pane_id: PaneId(11) });
    let changes = run(&mut w, WmAction::ClosePaneById { pane_id: PaneId(12) });
    assert_eq!(changes, [Change::PaneRemoved { pane: PaneId(12) }, Change::WorkspaceRemoved { index: 1 }]);
}

/// `close` is the asker's focused pane; with none focused nothing happens.
#[test]
fn close_closes_the_askers_focused_pane() {
    let mut w = two_workspaces_of_two_columns();
    assert!(run(&mut w, WmAction::ClosePane).is_empty());
    let changes = run_for(&mut w, Some(PaneId(2)), WmAction::ClosePane);
    assert_eq!(changes, [Change::PaneRemoved { pane: PaneId(2) }, Change::LayoutChanged]);
}

/// A column goes with every pane in it, each reported.
#[test]
fn deleting_a_column_reports_each_of_its_panes() {
    let mut w = two_workspaces_of_two_columns();
    w.ws().scroll_mut().add_pane_to_column(0, None, heca_core::layout::Pane::new(PaneId(50), "x"), false);
    let changes = run(&mut w, WmAction::DeleteColumn { ws_idx: 0, col_idx: 0 });
    assert_eq!(
        changes,
        [
            Change::PaneRemoved { pane: PaneId(1) },
            Change::PaneRemoved { pane: PaneId(50) },
            Change::LayoutChanged
        ]
    );
    assert!(run(&mut w, WmAction::DeleteColumn { ws_idx: 9, col_idx: 0 }).is_empty());
}

/// A workspace goes with everything in it, and is reported gone.
#[test]
fn deleting_a_workspace_reports_its_panes_and_itself() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::DeleteWorkspace { ws_idx: 1 });
    assert_eq!(
        changes,
        [
            Change::PaneRemoved { pane: PaneId(11) },
            Change::PaneRemoved { pane: PaneId(12) },
            Change::WorkspaceRemoved { index: 1 }
        ]
    );
    assert_eq!(w.session.workspaces.len(), 1);
}
