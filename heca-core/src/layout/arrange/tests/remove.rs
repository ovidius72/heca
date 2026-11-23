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

/// **A workspace moves with everything in it**, to the place asked, counted as the list is after
/// the move; past the end is last; nowhere to go is nothing.
#[test]
fn a_workspace_moves_to_the_place_asked_with_everything_in_it() {
    let mut s = session(&[&[&[1]], &[&[2]], &[&[3, 4]]]);
    let order = |s: &crate::layout::Session| -> Vec<Vec<u64>> {
        (0..s.workspaces.len())
            .map(|ws| super::columns(s, ws).concat())
            .collect()
    };
    assert!(s.move_workspace(2, 0));
    assert_eq!(order(&s), [vec![3, 4], vec![1], vec![2]]);
    assert!(s.move_workspace(0, 99), "past the end is last");
    assert_eq!(order(&s), [vec![1], vec![2], vec![3, 4]]);
    assert!(!s.move_workspace(1, 1), "already there");
    assert!(!s.move_workspace(7, 0), "no such workspace");
    assert_eq!(order(&s), [vec![1], vec![2], vec![3, 4]]);
}

/// **A workspace keeps its own number** — given when it is made, the next one each time, never
/// reused after a removal, and the same wherever it moves. It is the `N` of "Workspace N" until
/// the workspace is named.
#[test]
fn a_workspace_keeps_its_own_number() {
    let mut s = crate::layout::Session::new(
        crate::layout::SessionId(1),
        crate::layout::LayoutOptions::default(),
    );
    s.add_workspace();
    s.add_workspace();
    let numbers = |s: &crate::layout::Session| -> Vec<u32> {
        s.workspaces.iter().map(|ws| ws.number).collect()
    };
    assert_eq!(numbers(&s), [1, 2, 3]);
    assert!(s.move_workspace(2, 0));
    assert_eq!(numbers(&s), [3, 1, 2], "moved, each keeps its own");
    assert_eq!(s.workspaces[0].label(), "Workspace 3");
    assert!(s.remove_workspace(0));
    s.add_workspace();
    assert_eq!(
        numbers(&s),
        [1, 2, 4],
        "the next number, never one that was used"
    );
    s.workspaces[1].name = Some("api".into());
    assert_eq!(s.workspaces[1].label(), "api", "a given name wins");
}

/// Content from before numbers existed arrives with none, and is numbered in list order after any
/// that has one — so every workspace still has a name.
#[test]
fn content_without_numbers_is_numbered_on_rebuild() {
    let s = session(&[&[&[1]], &[&[2]]]);
    let mut content = s.content();
    for ws in &mut content.workspaces {
        ws.number = 0;
    }
    let rebuilt = content.rebuild(s.id, s.options.clone());
    let numbers: Vec<u32> = rebuilt.workspaces.iter().map(|ws| ws.number).collect();
    assert_eq!(numbers, [1, 2]);
}

/// **The window keeps showing the workspace it showed** when that workspace moves — the shown one
/// is held by identity, so it is found at its new position with its own active pane.
#[test]
fn the_shown_workspace_stays_shown_when_it_moves() {
    let mut w = crate::layout::testing::Windowed::with_shape(&[&[&[1]], &[&[2]], &[&[3]]]);
    w.show(0);
    let shown = w
        .l()
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id);
    assert!(w.session.move_workspace(0, 1));
    assert_eq!(
        w.l().active_workspace_idx(),
        1,
        "it is now second, and still the one shown"
    );
    assert_eq!(
        w.l()
            .active_workspace()
            .and_then(|ws| ws.active_pane())
            .map(|p| p.id),
        shown
    );
}
