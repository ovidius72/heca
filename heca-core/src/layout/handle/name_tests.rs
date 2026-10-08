//! Naming and clearing names.

use crate::layout::testing::Windowed;
use crate::layout::PaneId;

#[test]
fn a_pane_workspace_and_column_are_named_and_cleared_by_an_empty_name() {
    let mut w = Windowed::with_shape(&[&[&[1], &[2]], &[&[3]]]);
    assert!(w.m().rename_pane(PaneId(3), "logs"));
    assert_eq!(w.session.workspaces[1].find_pane(PaneId(3)).and_then(|p| p.custom_name.clone()).as_deref(), Some("logs"));
    assert!(w.m().rename_pane(PaneId(3), ""));
    assert_eq!(w.session.workspaces[1].find_pane(PaneId(3)).and_then(|p| p.custom_name.clone()), None);

    assert!(w.m().rename_workspace(1, "work"));
    assert_eq!(w.session.workspaces[1].name.as_deref(), Some("work"));
    assert!(w.m().rename_workspace(1, ""));
    assert_eq!(w.session.workspaces[1].name, None);

    assert!(w.m().rename_column(0, 1, "side"));
    assert_eq!(w.session.workspaces[0].scrolling.columns[1].name.as_deref(), Some("side"));
    assert!(w.m().rename_column(0, 1, ""));
    assert_eq!(w.session.workspaces[0].scrolling.columns[1].name, None);
}

#[test]
fn naming_what_is_not_there_says_so() {
    let mut w = Windowed::with_shape(&[&[&[1]]]);
    assert!(!w.m().rename_pane(PaneId(9), "x"));
    assert!(!w.m().rename_workspace(4, "x"));
    assert!(!w.m().rename_column(0, 4, "x"));
    assert!(!w.m().rename_column(4, 0, "x"));
}
