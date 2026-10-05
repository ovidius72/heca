//! The naming actions, run the way the registry runs them, and the facts they report.

use heca_core::layout::PaneId;

use super::testing::{run, run_for, two_workspaces_of_two_columns};
use crate::input::WmAction;
use crate::server::Change;

fn pane_name(w: &heca_core::layout::testing::Windowed, ws: usize, pane: u64) -> Option<String> {
    w.session.workspaces[ws].find_pane(PaneId(pane)).and_then(|p| p.custom_name.clone())
}

/// A pane is named wherever it is, and an empty name clears it.
#[test]
fn a_pane_is_named_in_any_workspace_and_cleared_by_an_empty_name() {
    let mut w = two_workspaces_of_two_columns();
    let name = |name: &str| WmAction::RenameTarget { pane_id: PaneId(12), name: name.into() };
    assert_eq!(run(&mut w, name("logs")), [Change::NamesChanged]);
    assert_eq!(pane_name(&w, 1, 12).as_deref(), Some("logs"));
    assert_eq!(run(&mut w, name("")), [Change::NamesChanged]);
    assert_eq!(pane_name(&w, 1, 12), None);
    assert!(run(&mut w, WmAction::RenameTarget { pane_id: PaneId(99), name: "x".into() }).is_empty());
}

#[test]
fn a_workspace_and_a_column_are_named() {
    let mut w = two_workspaces_of_two_columns();
    let changes = run(&mut w, WmAction::RenameWorkspaceTo { ws_idx: 1, name: "work".into() });
    assert_eq!(changes, [Change::NamesChanged]);
    assert_eq!(w.session.workspaces[1].name.as_deref(), Some("work"));
    let changes = run(&mut w, WmAction::RenameColumnTo { ws_idx: 0, col_idx: 1, name: "side".into() });
    assert_eq!(changes, [Change::NamesChanged]);
    assert_eq!(w.session.workspaces[0].scrolling.columns[1].name.as_deref(), Some("side"));
    assert!(run(&mut w, WmAction::RenameWorkspaceTo { ws_idx: 7, name: "x".into() }).is_empty());
}

/// The resets clear the name of the asker's focused pane / shown workspace, or of the one named.
#[test]
fn the_resets_clear_names() {
    let mut w = two_workspaces_of_two_columns();
    for (pane, ws) in [(1u64, 0usize), (12, 1)] {
        run(&mut w, WmAction::RenameTarget { pane_id: PaneId(pane), name: "n".into() });
        let _ = ws;
    }
    assert!(run(&mut w, WmAction::ResetPaneName).is_empty(), "no focused pane");
    assert_eq!(run_for(&mut w, Some(PaneId(1)), WmAction::ResetPaneName), [Change::NamesChanged]);
    assert_eq!(pane_name(&w, 0, 1), None);
    assert_eq!(run(&mut w, WmAction::ResetPaneNameById { pane_id: PaneId(12) }), [Change::NamesChanged]);
    assert_eq!(pane_name(&w, 1, 12), None);

    w.session.workspaces[0].name = Some("a".into());
    w.session.workspaces[1].name = Some("b".into());
    assert_eq!(run(&mut w, WmAction::ResetWorkspaceName), [Change::NamesChanged]);
    assert_eq!(w.session.workspaces[0].name, None, "the shown workspace");
    assert_eq!(run(&mut w, WmAction::ResetWorkspaceNameByIdx { ws_idx: 1 }), [Change::NamesChanged]);
    assert_eq!(w.session.workspaces[1].name, None);
}
