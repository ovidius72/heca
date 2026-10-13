//! Naming and clearing names.

use super::session;
use crate::layout::PaneId;

#[test]
fn a_pane_workspace_and_column_are_named_and_cleared_by_an_empty_name() {
    let mut s = session(&[&[&[1], &[2]], &[&[3]]]);
    assert!(s.rename_pane(PaneId(3), "logs"));
    assert_eq!(
        s.workspaces[1]
            .find_pane(PaneId(3))
            .and_then(|p| p.custom_name.clone())
            .as_deref(),
        Some("logs")
    );
    assert!(s.rename_pane(PaneId(3), ""));
    assert_eq!(
        s.workspaces[1]
            .find_pane(PaneId(3))
            .and_then(|p| p.custom_name.clone()),
        None
    );

    assert!(s.rename_workspace(1, "work"));
    assert_eq!(s.workspaces[1].name.as_deref(), Some("work"));
    assert!(s.rename_workspace(1, ""));
    assert_eq!(s.workspaces[1].name, None);

    assert!(s.rename_column(0, 1, "side"));
    assert_eq!(
        s.workspaces[0].scrolling.columns[1].name.as_deref(),
        Some("side")
    );
    assert!(s.rename_column(0, 1, ""));
    assert_eq!(s.workspaces[0].scrolling.columns[1].name, None);
}

#[test]
fn naming_what_is_not_there_says_so() {
    let mut s = session(&[&[&[1]]]);
    assert!(!s.rename_pane(PaneId(9), "x"));
    assert!(!s.rename_workspace(4, "x"));
    assert!(!s.rename_column(0, 4, "x"));
    assert!(!s.rename_column(4, 0, "x"));
}
