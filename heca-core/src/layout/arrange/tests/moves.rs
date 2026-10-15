//! Moving panes and columns, within a workspace and between workspaces.

use super::{HEIGHT, columns, float, session};
use crate::layout::scrolling::{ColumnEffect, PaneEffect, SpaceEffect};
use crate::layout::{Arranged, FocusDomain, Moved, PaneId};

/// The pane leaves its workspace, which it emptied: the workspace goes, what is left is numbered
/// as it is now, and only the workspace it landed in has anything to show.
#[test]
fn a_pane_leaving_its_last_place_removes_the_workspace_it_emptied() {
    let mut s = session(&[&[&[1]], &[&[2]], &[&[3]]]);
    let moved = s.move_pane_to_workspace(PaneId(1), 2, 0, false, HEIGHT);
    let landed = SpaceEffect::Column(ColumnEffect::Inserted { idx: 0 });
    assert_eq!(
        moved,
        Some(Moved {
            workspace: 1,
            column: 0,
            arranged: Arranged {
                removed_workspace: Some(0),
                effects: vec![(1, vec![landed])]
            },
        })
    );
    assert_eq!(s.workspaces.len(), 2);
    assert_eq!(columns(&s, 1), [vec![1], vec![3]]);
}

/// Each workspace says what happened to it: the one it left lost a pane, the one it joined got
/// one at the bottom of the column.
#[test]
fn a_pane_stacks_into_a_column_of_another_workspace() {
    let mut s = session(&[&[&[1, 9]], &[&[2]]]);
    let moved = s
        .move_pane_to_workspace(PaneId(1), 1, 0, true, HEIGHT)
        .expect("it moves");
    assert_eq!((moved.workspace, moved.column), (1, 0));
    assert_eq!(
        moved.arranged.effects,
        [
            (
                0,
                vec![SpaceEffect::Pane(PaneEffect::Removed { col: 0, row: 0 })]
            ),
            (
                1,
                vec![SpaceEffect::Pane(PaneEffect::Inserted { col: 0, row: 1 })]
            ),
        ]
    );
    assert_eq!(columns(&s, 1), [vec![2, 1]]);
    assert_eq!(
        s.move_pane_to_workspace(PaneId(1), 1, 0, true, HEIGHT),
        None,
        "already there"
    );
}

#[test]
fn a_column_moves_to_the_end_of_another_workspace() {
    let mut s = session(&[&[&[1], &[2]], &[&[3]]]);
    let moved = s
        .move_column(0, 0, 1, crate::layout::END)
        .expect("it moves");
    assert_eq!(
        (
            moved.workspace,
            moved.column,
            moved.arranged.removed_workspace
        ),
        (1, 1, None)
    );
    assert_eq!(columns(&s, 1), [vec![3], vec![1]]);
}

#[test]
fn the_last_column_leaving_removes_its_workspace_and_renumbers_the_target() {
    let mut s = session(&[&[&[1]], &[&[2]]]);
    let moved = s
        .move_column(0, 0, 1, crate::layout::END)
        .expect("it moves");
    assert_eq!(
        (
            moved.workspace,
            moved.column,
            moved.arranged.removed_workspace
        ),
        (0, 1, Some(0))
    );
    assert_eq!(columns(&s, 0), [vec![2], vec![1]]);
}

#[test]
fn a_column_is_reordered_within_its_workspace() {
    let mut s = session(&[&[&[1], &[2], &[3]]]);
    let moved = s.move_column(0, 0, 0, 2).expect("it moves");
    assert_eq!(moved.column, 2);
    assert_eq!(
        moved.arranged.effects,
        [(
            0,
            vec![SpaceEffect::Column(ColumnEffect::Moved { from: 0, to: 2 })]
        )]
    );
    assert_eq!(columns(&s, 0), [vec![2], vec![3], vec![1]]);
    assert_eq!(s.move_column(0, 1, 0, 1), None, "to where it is");
    assert_eq!(s.move_column(0, 7, 0, 1), None, "no such column");
}

#[test]
fn the_last_workspace_is_not_removed_when_empty() {
    let mut s = session(&[&[]]);
    assert!(!s.remove_workspace_if_empty(0));
    assert_eq!(s.workspaces.len(), 1);
}

#[test]
fn a_pane_is_taken_to_the_bottom_of_a_column() {
    let mut s = session(&[&[&[1], &[2, 3]]]);
    // Column 1 is where it is asked to go; pane 1 leaving empties column 0, so it lands in 0.
    let moved = s.take_pane_into(PaneId(1), 0, 1, HEIGHT).expect("it moves");
    assert_eq!((moved.workspace, moved.column), (0, 0));
    assert_eq!(columns(&s, 0), [vec![2, 3, 1]]);
    assert_eq!(
        s.take_pane_into(PaneId(1), 0, 0, HEIGHT),
        None,
        "already the tail"
    );
    assert_eq!(
        s.take_pane_into(PaneId(1), 0, 5, HEIGHT),
        None,
        "past the last column is the last"
    );
    assert_eq!(s.take_pane_into(PaneId(99), 0, 0, HEIGHT), None);
}

/// The column the pane leaves goes, which brings the asked column one place nearer: the pane
/// still lands in the column that was asked for.
#[test]
fn a_pane_taken_from_a_column_before_the_target_lands_in_the_target() {
    let mut s = session(&[&[&[1], &[2], &[3], &[4]]]);
    let moved = s.take_pane_into(PaneId(1), 0, 2, HEIGHT).expect("it moves");
    assert_eq!(moved.column, 1);
    assert_eq!(columns(&s, 0), [vec![2], vec![3, 1], vec![4]]);
}

#[test]
fn a_floating_pane_is_taken_into_the_tiling_and_leaves_the_others_unfocused() {
    let mut s = session(&[&[&[1]]]);
    float(&mut s, 0, 8);
    float(&mut s, 0, 9);
    let moved = s.take_pane_into(PaneId(9), 0, 0, HEIGHT).expect("it moves");
    assert_eq!(
        moved.arranged.effects,
        [(
            0,
            vec![SpaceEffect::Pane(PaneEffect::Inserted { col: 0, row: 1 })]
        )]
    );
    assert_eq!(columns(&s, 0), [[1, 9]]);
    let ws = &s.workspaces[0];
    assert_eq!(ws.floating_panes.len(), 1);
    assert!(!ws.floating_panes[0].is_active);
    assert_eq!(ws.focus_domain, FocusDomain::Tiled);
}

#[test]
fn taking_the_only_pane_of_another_workspace_removes_it_and_renumbers() {
    let mut s = session(&[&[&[1]], &[&[2]]]);
    let moved = s.take_pane_into(PaneId(1), 1, 0, HEIGHT).expect("it moves");
    assert_eq!(
        (
            moved.workspace,
            moved.column,
            moved.arranged.removed_workspace
        ),
        (0, 0, Some(0))
    );
    assert_eq!(columns(&s, 0), [[2, 1]]);
}

#[test]
fn a_pane_taken_into_an_empty_workspace_gets_a_column() {
    let mut s = session(&[&[&[1], &[2]], &[]]);
    let moved = s.take_pane_into(PaneId(1), 1, 0, HEIGHT).expect("it moves");
    assert_eq!((moved.workspace, moved.column), (1, 0));
    assert_eq!(columns(&s, 1), [[1]]);
}

/// Within its own workspace a place is counted as the strip is now, with the pane still in it:
/// row 1 of column 1 is between 2 and 3, even though taking pane 1 out first renumbers that column.
#[test]
fn a_pane_is_placed_at_a_row_or_in_a_new_column() {
    let mut s = session(&[&[&[1], &[2, 3]]]);
    let still = s
        .place_pane(PaneId(1), 0, 0, Some(1), HEIGHT)
        .expect("it is there");
    assert_eq!(
        (still.column, still.arranged.effects.len()),
        (0, 0),
        "its own lone column: nothing moves"
    );
    let moved = s
        .place_pane(PaneId(1), 0, 1, Some(1), HEIGHT)
        .expect("it moves");
    assert_eq!((moved.workspace, moved.column), (0, 0));
    assert_eq!(columns(&s, 0), [[2, 1, 3]]);
    let moved = s
        .place_pane(PaneId(3), 0, 0, None, HEIGHT)
        .expect("it moves");
    assert_eq!(moved.column, 0);
    assert_eq!(columns(&s, 0), [vec![3], vec![2, 1]]);
}

#[test]
fn a_pane_is_placed_in_another_workspace() {
    let mut s = session(&[&[&[1], &[2]], &[&[3]]]);
    let moved = s
        .place_pane(PaneId(1), 1, 0, Some(0), HEIGHT)
        .expect("it moves");
    assert_eq!((moved.workspace, moved.column), (1, 0));
    assert_eq!(columns(&s, 0), [[2]]);
    assert_eq!(columns(&s, 1), [[1, 3]]);
}

#[test]
fn a_pane_placed_in_a_workspace_that_is_not_there_is_not_lost() {
    let mut s = session(&[&[&[1], &[2]]]);
    assert_eq!(s.place_pane(PaneId(1), 5, 0, None, HEIGHT), None);
    assert_eq!(columns(&s, 0), [vec![1], vec![2]]);
}

#[test]
fn a_pane_is_made_in_a_column_capped_to_the_last_one() {
    let mut s = session(&[&[&[1], &[2]]]);
    let added = s.add_pane_to_column(0, 9, HEIGHT).expect("workspace 0");
    assert_eq!((added.workspace, added.column), (0, 1));
    assert_eq!(columns(&s, 0), [vec![1], vec![2, added.pane.0]]);
    assert!(s.add_pane_to_column(7, 0, HEIGHT).is_none());
}

#[test]
fn a_pane_is_made_in_an_empty_workspace_and_in_a_new_column() {
    let mut s = session(&[&[&[1]], &[]]);
    let first = s.add_pane_to_column(1, 3, HEIGHT).expect("workspace 1");
    assert_eq!(columns(&s, 1), [vec![first.pane.0]]);
    let second = s.add_column(1, 1).expect("workspace 1");
    assert_eq!(columns(&s, 1), [vec![first.pane.0], vec![second.pane.0]]);
    assert_eq!(second.column, 1);
    assert_eq!(
        second.arranged.effects,
        [(
            1,
            vec![SpaceEffect::Column(ColumnEffect::Inserted { idx: 1 })]
        )]
    );
    assert_ne!(first.pane, second.pane);
}
