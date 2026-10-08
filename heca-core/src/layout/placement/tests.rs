use crate::layout::testing::Windowed;
use crate::layout::types::Size;
use crate::layout::{ColumnId, FocusDomain, Pane, PaneId};

/// One workspace, three panes, each in its own column: ids 1, 2, 3.
fn session() -> Windowed {
    let mut s = Windowed::new(Size::new(1000.0, 800.0), 1.0);
    for i in 1..=3 {
        s.m().add_pane(Pane::new(PaneId(i), ""), None, true);
    }
    s
}

#[test]
fn a_pane_is_taken_from_its_column_or_from_the_floating_layer() {
    let mut s = session();
    let mut ws = s.ws();
    assert_eq!(ws.take_pane(PaneId(2)).map(|p| p.id), Some(PaneId(2)));
    assert!(ws.scrolling.pane_indices(PaneId(2)).is_none());

    let rect = ws.reader().default_float_rect();
    ws.add_floating_pane(Pane::new(PaneId(9), ""), rect, None);
    assert_eq!(ws.take_pane(PaneId(9)).map(|p| p.id), Some(PaneId(9)));
    assert!(ws.floating_panes.is_empty());
    assert_eq!(ws.focus_domain, FocusDomain::Tiled);
    assert!(ws.take_pane(PaneId(42)).is_none());
}

#[test]
fn a_pane_is_placed_at_a_row_of_a_column() {
    let mut s = session();
    let mut ws = s.ws();
    let pane = ws.take_pane(PaneId(3)).unwrap();
    ws.place_pane(pane, 0, Some(0), ColumnId(99));
    assert_eq!(ws.scrolling.pane_indices(PaneId(3)), Some((0, 0)));
    assert_eq!(ws.active_pane().map(|p| p.id), Some(PaneId(3)));
}

#[test]
fn with_no_row_the_pane_gets_a_column_of_its_own() {
    let mut s = session();
    let mut ws = s.ws();
    let pane = ws.take_pane(PaneId(3)).unwrap();
    ws.place_pane(pane, 0, None, ColumnId(99));
    assert_eq!(ws.scrolling.columns[0].id, ColumnId(99));
    assert_eq!(ws.scrolling.pane_indices(PaneId(3)), Some((0, 0)));
}


/// Columns by the panes they hold, for the tests that move a pane among them.
fn strip(columns: &[&[u64]]) -> Windowed {
    Windowed::with_shape(&[columns])
}

/// The columns by the panes they hold, as a plain list.
fn shape(s: &Windowed) -> Vec<Vec<u64>> {
    s.session.workspaces[0]
        .scrolling
        .columns
        .iter()
        .map(|c| c.panes.iter().map(|p| p.id.0).collect())
        .collect()
}

fn moved(s: &mut Windowed, pane: u64, col: usize, row: Option<usize>) -> Vec<Vec<u64>> {
    let mut ws = s.ws();
    assert!(ws.move_pane(PaneId(pane), col, row, ColumnId(99)));
    shape(s)
}

/// A gap is the one the caller saw: a pane that shares its column leaves nothing behind.
#[test]
fn a_new_column_lands_at_the_gap_that_was_drawn() {
    let mut s = strip(&[&[1, 2], &[3], &[4]]);
    assert_eq!(moved(&mut s, 1, 3, None), [vec![2], vec![3], vec![4], vec![1]]);
    let mut s = strip(&[&[1, 2], &[3], &[4]]);
    assert_eq!(moved(&mut s, 1, 1, None), [vec![2], vec![1], vec![3], vec![4]]);
    let mut s = strip(&[&[1, 2], &[3], &[4]]);
    assert_eq!(moved(&mut s, 2, 0, None), [vec![2], vec![1], vec![3], vec![4]]);
}

/// A pane alone in its column takes the column with it, so a gap to its right is one nearer.
#[test]
fn a_gap_right_of_the_column_the_pane_empties_is_one_nearer() {
    let mut s = strip(&[&[1], &[3], &[4]]);
    assert_eq!(moved(&mut s, 1, 3, None), [vec![3], vec![4], vec![1]]);
    let mut s = strip(&[&[1], &[3], &[4]]);
    assert_eq!(moved(&mut s, 1, 2, None), [vec![3], vec![1], vec![4]]);
}

/// The two gaps beside its own lone column, and that column itself, put it back where it is.
#[test]
fn a_place_that_is_where_the_pane_already_is_changes_nothing() {
    for gap in [1, 2] {
        let mut s = strip(&[&[3], &[1], &[4]]);
        assert_eq!(moved(&mut s, 1, gap, None), [vec![3], vec![1], vec![4]]);
    }
    let mut s = strip(&[&[3], &[1], &[4]]);
    assert_eq!(moved(&mut s, 1, 1, Some(0)), [vec![3], vec![1], vec![4]]);
}

/// A row is the row it was seen at, whichever side of it the pane came from.
#[test]
fn a_row_in_the_panes_own_column_is_the_row_that_was_seen() {
    // Pane 1 is above row 2 of its column: taking it out lifts every row below it by one, so
    // "before pane 3" is still before pane 3.
    let mut s = strip(&[&[1, 2, 3]]);
    assert_eq!(moved(&mut s, 1, 0, Some(2)), [vec![2, 1, 3]]);
    let mut s = strip(&[&[1, 2, 3]]);
    assert_eq!(moved(&mut s, 3, 0, Some(0)), [vec![3, 1, 2]]);
}

/// A column to its left that the pane leaves empty shifts the target column by one.
#[test]
fn a_target_column_right_of_the_one_the_pane_empties_is_one_nearer() {
    let mut s = strip(&[&[1], &[3], &[4]]);
    assert_eq!(moved(&mut s, 1, 2, Some(1)), [vec![3], vec![4, 1]]);
}

#[test]
fn the_gaps_that_change_the_strip_leave_out_the_two_beside_a_lone_pane() {
    let s = strip(&[&[3], &[1], &[4]]);
    let ws = &s.session.workspaces[0];
    assert_eq!(ws.new_column_gaps(PaneId(1)), [0, 3]);
    let s = strip(&[&[1, 2], &[3]]);
    assert_eq!(s.session.workspaces[0].new_column_gaps(PaneId(1)), [0, 1, 2]);
}

#[test]
fn a_place_past_the_end_clamps_to_it() {
    let mut s = session();
    let mut ws = s.ws();
    let pane = ws.take_pane(PaneId(1)).unwrap();
    ws.place_pane(pane, 50, Some(50), ColumnId(99));
    let last = ws.scrolling.columns.len() - 1;
    assert_eq!(ws.scrolling.pane_indices(PaneId(1)), Some((last, 0)));
}

#[test]
fn the_rows_that_change_the_strip_leave_out_the_two_beside_the_pane() {
    let s = strip(&[&[1, 2], &[3]]);
    let ws = &s.session.workspaces[0];
    // Pane 1 is row 0 of column 0: rows 0 and 1 of that column put it back where it is.
    assert_eq!(ws.new_row_places(PaneId(1)), [(0, 2), (1, 0), (1, 1)]);
}
