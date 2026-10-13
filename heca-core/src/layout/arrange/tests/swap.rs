//! Swapping panes and columns, wherever they are.

use super::{columns, session};
use crate::layout::PaneId;
use crate::layout::scrolling::{ColumnEffect, PaneEffect, SpaceEffect};

/// A place now holding something else: what was there went, and something arrived.
fn replaced(col: usize, row: usize) -> Vec<SpaceEffect> {
    vec![
        SpaceEffect::Pane(PaneEffect::Removed { col, row }),
        SpaceEffect::Pane(PaneEffect::Inserted { col, row }),
    ]
}

#[test]
fn two_panes_of_one_column_trade_places() {
    let mut s = session(&[&[&[1, 2, 3]]]);
    let arranged = s.swap_panes(PaneId(1), PaneId(3)).expect("both are there");
    assert_eq!(
        arranged.effects,
        [(
            0,
            vec![SpaceEffect::Pane(PaneEffect::Swapped {
                col: 0,
                a: 0,
                b: 2
            })]
        )]
    );
    assert_eq!(columns(&s, 0), [[3, 2, 1]]);
}

#[test]
fn two_panes_of_two_columns_trade_places() {
    let mut s = session(&[&[&[1, 2], &[3]]]);
    let arranged = s.swap_panes(PaneId(2), PaneId(3)).expect("both are there");
    assert_eq!(
        arranged.effects,
        [(0, [replaced(0, 1), replaced(1, 0)].concat())]
    );
    assert_eq!(columns(&s, 0), [vec![1, 3], vec![2]]);
}

#[test]
fn two_panes_of_two_workspaces_trade_places() {
    let mut s = session(&[&[&[1], &[2]], &[&[3]]]);
    let arranged = s.swap_panes(PaneId(1), PaneId(3)).expect("both are there");
    assert_eq!(arranged.effects, [(0, replaced(0, 0)), (1, replaced(0, 0))]);
    assert_eq!(columns(&s, 0), [vec![3], vec![2]]);
    assert_eq!(columns(&s, 1), [vec![1]]);
}

#[test]
fn swapping_a_pane_with_itself_or_a_stranger_changes_nothing() {
    let mut s = session(&[&[&[1], &[2]]]);
    assert_eq!(s.swap_panes(PaneId(1), PaneId(1)), None);
    assert_eq!(s.swap_panes(PaneId(1), PaneId(99)), None);
    assert_eq!(columns(&s, 0), [vec![1], vec![2]]);
}

#[test]
fn columns_trade_places_within_and_between_workspaces() {
    let mut s = session(&[&[&[1], &[2]], &[&[3]]]);
    let within = s.swap_columns(0, 0, 0, 1).expect("both are there");
    assert_eq!(
        within.effects,
        [(
            0,
            vec![SpaceEffect::Column(ColumnEffect::Swapped { a: 0, b: 1 })]
        )]
    );
    assert_eq!(columns(&s, 0), [vec![2], vec![1]]);
    let between = s.swap_columns(0, 0, 1, 0).expect("both are there");
    assert_eq!(
        between.effects.len(),
        2,
        "each workspace shows its own column arriving"
    );
    assert_eq!(columns(&s, 0), [vec![3], vec![1]]);
    assert_eq!(columns(&s, 1), [vec![2]]);
    assert_eq!(s.swap_columns(0, 0, 0, 0), None, "itself");
    assert_eq!(s.swap_columns(0, 5, 1, 0), None, "out of range");
}
