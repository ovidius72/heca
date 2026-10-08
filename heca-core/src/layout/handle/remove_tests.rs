//! Removing a pane, a column or a workspace, and what each says went.

use crate::layout::testing::Windowed;
use crate::layout::{Pane, PaneId, Removed};

fn window(shape: &[&[&[u64]]]) -> Windowed {
    Windowed::with_shape(shape)
}

#[test]
fn a_pane_is_removed_from_whichever_workspace_holds_it() {
    let mut w = window(&[&[&[1, 2]], &[&[3]]]);
    let removed = w.m().remove_pane_anywhere(PaneId(3));
    assert_eq!(removed, Some(Removed { panes: vec![PaneId(3)], removed_workspace: Some(1) }));
    assert_eq!(w.session.workspaces.len(), 1, "the workspace it emptied went");
    assert_eq!(w.m().remove_pane_anywhere(PaneId(3)), None);
}

#[test]
fn the_last_pane_of_the_last_workspace_leaves_it_empty() {
    let mut w = window(&[&[&[1]]]);
    let removed = w.m().remove_pane_anywhere(PaneId(1));
    assert_eq!(removed, Some(Removed { panes: vec![PaneId(1)], removed_workspace: None }));
    assert_eq!(w.session.workspaces.len(), 1);
}

#[test]
fn a_floating_pane_is_removed_and_a_workspace_with_tiles_stays() {
    let mut w = window(&[&[&[1]], &[&[2]]]);
    let rect = w.ws().reader().default_float_rect();
    w.ws().add_floating_pane(Pane::new(PaneId(9), "f"), rect, None);
    let removed = w.m().remove_pane_anywhere(PaneId(9));
    assert_eq!(removed, Some(Removed { panes: vec![PaneId(9)], removed_workspace: None }));
    assert!(w.session.workspaces[0].floating_panes.is_empty());
}

#[test]
fn a_column_is_removed_with_its_panes_and_its_workspace_stays() {
    let mut w = window(&[&[&[1, 2], &[3]]]);
    let removed = w.m().remove_column_with_panes(0, 0);
    assert_eq!(removed, Some(Removed { panes: vec![PaneId(1), PaneId(2)], removed_workspace: None }));
    let removed = w.m().remove_column_with_panes(0, 9);
    assert_eq!(removed.map(|r| r.panes), Some(vec![PaneId(3)]), "past the last means the last");
    assert_eq!(w.session.workspaces.len(), 1);
    assert_eq!(w.m().remove_column_with_panes(0, 0), None, "no column left");
    assert_eq!(w.m().remove_column_with_panes(5, 0), None);
}

#[test]
fn a_workspace_is_removed_with_everything_in_it() {
    let mut w = window(&[&[&[1]], &[&[2, 3], &[4]]]);
    let rect = w.ws().reader().default_float_rect();
    w.show(1);
    w.ws().add_floating_pane(Pane::new(PaneId(9), "f"), rect, None);
    w.show(0);
    let removed = w.m().remove_workspace_with_panes(1);
    assert_eq!(
        removed,
        Some(Removed {
            panes: vec![PaneId(2), PaneId(3), PaneId(4), PaneId(9)],
            removed_workspace: Some(1)
        })
    );
    assert_eq!(w.m().remove_workspace_with_panes(1), None);
    assert!(w.m().remove_workspace_with_panes(0).is_some(), "the last one may go");
    assert!(w.session.workspaces.is_empty());
}

#[test]
fn a_workspace_with_only_a_floating_pane_left_stays() {
    let mut w = window(&[&[&[1]], &[&[2]]]);
    let rect = w.ws().reader().default_float_rect();
    w.show(1);
    w.ws().add_floating_pane(Pane::new(PaneId(9), "f"), rect, None);
    w.show(0);
    let removed = w.m().remove_pane_anywhere(PaneId(2));
    assert_eq!(removed, Some(Removed { panes: vec![PaneId(2)], removed_workspace: None }));
    assert_eq!(w.session.workspaces.len(), 2);
}
