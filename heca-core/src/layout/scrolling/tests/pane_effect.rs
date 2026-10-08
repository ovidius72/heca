//! The pane operations on the content alone — no window, no view — and what each says it did.

use super::*;
use crate::layout::column::MIN_PANE_HEIGHT;

/// A bare space, columns `1..=n` each holding the pane of the same id, and nothing looking at it.
fn bare(n: u64) -> ScrollingSpace {
    let mut space = ScrollingSpace::new(LayoutOptions::default());
    for id in 1..=n {
        space.insert_column(space.columns.len(), test_column(id, ColumnWidth::Proportion(0.5)));
    }
    space
}

fn pane_ids(space: &ScrollingSpace, col: usize) -> Vec<u64> {
    space.columns[col].panes.iter().map(|p| p.id.0).collect()
}

const HEIGHT: f64 = 800.0;

#[test]
fn a_pane_is_inserted_at_its_row_or_at_the_bottom() {
    let mut space = bare(1);
    let effect = space.insert_pane(0, None, Pane::new(PaneId(7), ""), HEIGHT);
    assert_eq!(effect, Some(PaneEffect::Inserted { col: 0, row: 1 }));

    let effect = space.insert_pane(0, Some(0), Pane::new(PaneId(8), ""), HEIGHT);
    assert_eq!(effect, Some(PaneEffect::Inserted { col: 0, row: 0 }));

    let effect = space.insert_pane(0, Some(99), Pane::new(PaneId(9), ""), HEIGHT);
    assert_eq!(effect, Some(PaneEffect::Inserted { col: 0, row: 3 }), "a row past the end clamps");
    assert_eq!(pane_ids(&space, 0), vec![8, 1, 7, 9]);

    assert_eq!(space.insert_pane(5, None, Pane::new(PaneId(10), ""), HEIGHT), None);
}

/// A pane arriving in a column whose panes were dragged to fill it still gets on screen: the
/// dragged heights give way, and every pane keeps at least the floor.
#[test]
fn an_inserted_pane_is_given_room_in_a_column_that_was_full() {
    let mut space = bare(1);
    space.insert_pane(0, None, Pane::new(PaneId(2), ""), HEIGHT);
    let gaps = space.options.gaps;
    space.columns[0].set_pane_height(0, HEIGHT, HEIGHT, gaps);

    space.insert_pane(0, None, Pane::new(PaneId(3), ""), HEIGHT);

    let heights = space.columns[0].pane_heights(HEIGHT, gaps);
    let room = HEIGHT - gaps * (heights.len() as f64 + 1.0);
    assert!((heights.iter().sum::<f64>() - room).abs() < 0.5, "heights {heights:?} vs room {room}");
    assert!(heights.iter().all(|h| *h >= MIN_PANE_HEIGHT - 0.5), "heights {heights:?}");
}

#[test]
fn taking_a_pane_says_whether_its_column_went_with_it() {
    let mut space = bare(2);
    space.insert_pane(0, None, Pane::new(PaneId(7), ""), HEIGHT);

    let (pane, effect) = space.take_pane_from(0, 1).expect("row 1 is there");
    assert_eq!(pane.id, PaneId(7));
    assert_eq!(effect, SpaceEffect::Pane(PaneEffect::Removed { col: 0, row: 1 }));

    let (pane, effect) = space.take_pane_from(0, 0).expect("the last pane is there");
    assert_eq!(pane.id, PaneId(1));
    assert_eq!(effect, SpaceEffect::Column(ColumnEffect::Removed { idx: 0 }));
    assert_eq!(column_ids(&space), vec![2]);
}

#[test]
fn taking_a_pane_that_is_not_there_changes_nothing() {
    let mut space = bare(2);
    assert!(space.take_pane_from(0, 1).is_none(), "a column of one has no row 1");
    assert!(space.take_pane_from(9, 0).is_none());
    assert_eq!(column_ids(&space), vec![1, 2]);
}

/// The source column empties and goes, which moves the target one place back: the pane still
/// lands in the column it was sent to.
#[test]
fn a_pane_moved_out_of_a_column_of_one_lands_in_the_column_it_was_sent_to() {
    let mut space = bare(3);
    let effects = space.move_pane_between(0, 0, 2, HEIGHT).expect("it moves");
    assert_eq!(
        effects,
        vec![
            SpaceEffect::Column(ColumnEffect::Removed { idx: 0 }),
            SpaceEffect::Pane(PaneEffect::Inserted { col: 1, row: 1 }),
        ]
    );
    assert_eq!(column_ids(&space), vec![2, 3]);
    assert_eq!(pane_ids(&space, 1), vec![3, 1]);
}

#[test]
fn a_pane_is_not_moved_into_its_own_column_or_past_the_last() {
    let mut space = bare(2);
    assert!(space.move_pane_between(0, 0, 0, HEIGHT).is_none());
    assert!(space.move_pane_between(0, 0, 2, HEIGHT).is_none());
    assert_eq!(column_ids(&space), vec![1, 2]);
}

#[test]
fn an_extracted_pane_gets_a_column_of_its_own_at_the_default_width() {
    let mut space = bare(2);
    space.insert_pane(0, None, Pane::new(PaneId(7), ""), HEIGHT);
    space.options.default_column_width = ColumnWidth::Proportion(0.3);

    let effects = space.extract_pane(0, 1, ColumnId(50), 1).expect("it moves");
    assert_eq!(
        effects,
        vec![
            SpaceEffect::Pane(PaneEffect::Removed { col: 0, row: 1 }),
            SpaceEffect::Column(ColumnEffect::Inserted { idx: 1 }),
        ]
    );
    assert_eq!(column_ids(&space), vec![1, 50, 2]);
    assert_eq!(pane_ids(&space, 1), vec![7]);
    assert_eq!(space.columns[1].width, ColumnWidth::Proportion(0.3));
}

/// Taking the last pane of a column out to the right of it: the column it left is gone, so the
/// new one takes its place.
#[test]
fn a_pane_extracted_from_a_column_of_one_takes_its_place() {
    let mut space = bare(3);
    let effects = space.extract_pane(1, 0, ColumnId(50), 2).expect("it moves");
    assert_eq!(
        effects,
        vec![
            SpaceEffect::Column(ColumnEffect::Removed { idx: 1 }),
            SpaceEffect::Column(ColumnEffect::Inserted { idx: 1 }),
        ]
    );
    assert_eq!(column_ids(&space), vec![1, 50, 3]);
}

#[test]
fn swapping_panes_carries_the_active_one_with_it() {
    let mut space = bare(1);
    space.insert_pane(0, None, Pane::new(PaneId(7), ""), HEIGHT);
    space.insert_pane(0, None, Pane::new(PaneId(8), ""), HEIGHT);
    space.columns[0].active_pane_idx = 0;

    let effect = space.swap_panes_in_column(0, 0, 2);
    assert_eq!(effect, Some(PaneEffect::Swapped { col: 0, a: 0, b: 2 }));
    assert_eq!(pane_ids(&space, 0), vec![8, 7, 1]);
    assert_eq!(space.columns[0].active_pane_idx, 2, "the active pane went with the swap");

    assert!(space.swap_panes_in_column(0, 1, 1).is_none());
    assert!(space.swap_panes_in_column(0, 0, 3).is_none());
}

/// A window shows a swap by sliding each pane from the slot it had: at the start of the slide
/// both are drawn exactly where they were.
#[test]
fn a_swapped_pane_slides_from_where_it_was() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), ""), true);
    let drawn = |space: &Seen| -> Vec<(PaneId, Rectangle)> { space.r().panes_with_positions() };
    let was = drawn(&space);
    let before = space.r().positions();

    let effect = space.space.swap_panes_in_column(0, 0, 1).expect("both rows are there");
    space.m().show(&[SpaceEffect::Pane(effect)], &before);

    let now = drawn(&space);
    for (id, rect) in was {
        let still = now.iter().find(|(p, _)| *p == id).map(|(_, r)| r.loc);
        assert_eq!(still, Some(rect.loc), "pane {id:?} starts where it was");
    }
}
