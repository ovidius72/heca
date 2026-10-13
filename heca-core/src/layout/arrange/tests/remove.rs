//! Removing a pane, a column or a workspace, and what each says went.

use super::{float, session};
use crate::layout::scrolling::{ColumnEffect, PaneEffect, SpaceEffect};
use crate::layout::{Arranged, PaneId, Removed};

#[test]
fn a_pane_is_removed_from_whichever_workspace_holds_it() {
    let mut s = session(&[&[&[1, 2]], &[&[3]]]);
    let removed = s.remove_pane(PaneId(3));
    let arranged = Arranged {
        removed_workspace: Some(1),
        effects: Vec::new(),
    };
    assert_eq!(
        removed,
        Some(Removed {
            panes: vec![PaneId(3)],
            arranged
        })
    );
    assert_eq!(s.workspaces.len(), 1, "the workspace it emptied went");
    assert_eq!(s.remove_pane(PaneId(3)), None);
    let removed = s.remove_pane(PaneId(2)).expect("it is there");
    assert_eq!(
        removed.arranged.effects,
        [(
            0,
            vec![SpaceEffect::Pane(PaneEffect::Removed { col: 0, row: 1 })]
        )]
    );
}

#[test]
fn the_last_pane_of_the_last_workspace_leaves_it_empty() {
    let mut s = session(&[&[&[1]]]);
    let removed = s.remove_pane(PaneId(1)).expect("it is there");
    assert_eq!(removed.arranged.removed_workspace, None);
    assert_eq!(
        removed.arranged.effects,
        [(
            0,
            vec![SpaceEffect::Column(ColumnEffect::Removed { idx: 0 })]
        )]
    );
    assert_eq!(s.workspaces.len(), 1);
}

#[test]
fn a_floating_pane_is_removed_and_a_workspace_with_tiles_stays() {
    let mut s = session(&[&[&[1]], &[&[2]]]);
    float(&mut s, 0, 9);
    let removed = s.remove_pane(PaneId(9));
    assert_eq!(
        removed,
        Some(Removed {
            panes: vec![PaneId(9)],
            arranged: Arranged::default()
        })
    );
    assert!(s.workspaces[0].floating_panes.is_empty());
}

#[test]
fn a_column_is_removed_with_its_panes_and_its_workspace_stays() {
    let mut s = session(&[&[&[1, 2], &[3]]]);
    let removed = s.remove_column(0, 0).expect("it is there");
    assert_eq!(removed.panes, [PaneId(1), PaneId(2)]);
    assert_eq!(
        removed.arranged.effects,
        [(
            0,
            vec![SpaceEffect::Column(ColumnEffect::Removed { idx: 0 })]
        )]
    );
    let removed = s.remove_column(0, 9);
    assert_eq!(
        removed.map(|r| r.panes),
        Some(vec![PaneId(3)]),
        "past the last means the last"
    );
    assert_eq!(s.workspaces.len(), 1);
    assert_eq!(s.remove_column(0, 0), None, "no column left");
    assert_eq!(s.remove_column(5, 0), None);
}

#[test]
fn a_workspace_is_removed_with_everything_in_it() {
    let mut s = session(&[&[&[1]], &[&[2, 3], &[4]]]);
    float(&mut s, 1, 9);
    let removed = s.remove_workspace_with_panes(1).expect("it is there");
    assert_eq!(removed.panes, [PaneId(2), PaneId(3), PaneId(4), PaneId(9)]);
    assert_eq!(removed.arranged.removed_workspace, Some(1));
    assert_eq!(s.remove_workspace_with_panes(1), None);
    assert!(
        s.remove_workspace_with_panes(0).is_some(),
        "the last one may go"
    );
    assert!(s.workspaces.is_empty());
}

#[test]
fn a_workspace_with_only_a_floating_pane_left_stays() {
    let mut s = session(&[&[&[1]], &[&[2]]]);
    float(&mut s, 1, 9);
    let removed = s.remove_pane(PaneId(2)).expect("it is there");
    assert_eq!(removed.arranged.removed_workspace, None);
    assert_eq!(s.workspaces.len(), 2);
}
