//! How a window shows a change to the panes: on its own, from its own before-picture, following
//! only what it chooses to.

use super::*;

/// Columns 1 to 4, the first also holding pane 9, and a second window looking at column 3.
fn two_windows() -> (Seen, ScrollView) {
    let mut first = space_with_columns(4);
    first.m().add_pane_to_column(0, None, Pane::new(PaneId(9), ""), false);
    first.m().activate_column(0);
    let mut second = ScrollView::new(Rectangle::from_size(Size::new(1000.0, 800.0)), 1.0);
    first.space.through_mut(&mut second).activate_column(2);
    (first, second)
}

fn active_column_id(space: &ScrollingSpace, view: &ScrollView) -> Option<ColumnId> {
    space.through(view).active_column().map(|c| c.id)
}

/// A pane moved by one window: that window follows it, the other stays on the column it was
/// looking at.
#[test]
fn a_window_that_did_not_move_a_pane_stays_on_its_column() {
    let (mut first, mut second) = two_windows();
    let seen_by_second = first.space.through(&second).positions();

    assert!(first.m().move_active_pane_right(ColumnId(50)));
    let effects = [
        SpaceEffect::Pane(PaneEffect::Removed { col: 0, row: 0 }),
        SpaceEffect::Pane(PaneEffect::Inserted { col: 1, row: 1 }),
    ];
    second.react_to(&first.space, &effects, &seen_by_second);

    assert_eq!(active_column_id(&first.space, &first.view), Some(ColumnId(2)));
    assert_eq!(active_column_id(&first.space, &second), Some(ColumnId(3)));
}

/// The column a move empties goes, which renumbers the ones after it: a window looking at one of
/// those still looks at the same column.
#[test]
fn a_window_keeps_its_column_when_a_move_removes_one_before_it() {
    let (mut space, mut second) = two_windows();
    space.space.columns[0].panes.truncate(1);
    let before = space.space.through(&second).positions();

    let effects = space.space.move_pane_between(0, 0, 1, 800.0).expect("it moves");
    second.react_to(&space.space, &effects, &before);

    assert_eq!(active_column_id(&space.space, &second), Some(ColumnId(3)));
}

/// A pane taken into a new column before the one a window looks at renumbers it: the window
/// still looks at the same column.
#[test]
fn a_window_keeps_its_column_when_a_move_adds_one_before_it() {
    let (mut space, mut second) = two_windows();
    let before = space.space.through(&second).positions();

    let effects = space.space.extract_pane(0, 1, ColumnId(50), 1).expect("it moves");
    second.react_to(&space.space, &effects, &before);

    assert_eq!(active_column_id(&space.space, &second), Some(ColumnId(3)));
}

/// Each window slides the moved pane from where **it** drew it.
#[test]
fn a_moved_pane_starts_where_each_window_drew_it() {
    let (mut space, mut second) = two_windows();
    let drawn = |space: &ScrollingSpace, view: &ScrollView| {
        space.through(view).panes_with_positions().into_iter().find(|(id, _)| *id == PaneId(9))
    };
    let was = drawn(&space.space, &second);
    let before = space.space.through(&second).positions();

    let effects = space.space.move_pane_between(0, 1, 1, 800.0).expect("it moves");
    second.react_to(&space.space, &effects, &before);

    assert_eq!(drawn(&space.space, &second).map(|(_, r)| r.loc), was.map(|(_, r)| r.loc));
}

/// Following is the window's choice, made after it has shown the change.
#[test]
fn a_window_follows_the_column_it_chooses() {
    let (space, mut second) = two_windows();
    second.follow(&space.space, 0);
    assert_eq!(active_column_id(&space.space, &second), Some(ColumnId(1)));

    second.follow(&space.space, 9);
    assert_eq!(active_column_id(&space.space, &second), Some(ColumnId(1)), "no column 9");
}

/// A space laid out beside a sidebar: its area does not start at the window's left edge.
fn beside_a_sidebar() -> Seen {
    let area = Rectangle::new(Point::new(466.0, 90.0), Size::new(1060.0, 1150.0));
    Seen::new(area, 1.0, LayoutOptions::default())
}

/// Where a lone column's view rests: at the left edge, as when it was the first column made.
fn resting_view_of_one_column() -> f64 {
    let mut fresh = beside_a_sidebar();
    fresh.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
    fresh.r().target_view_pos()
}

/// Moving the last pane of a column into the other one leaves a single column, which rests where
/// a single column always does, whichever way the pane went.
#[test]
fn the_column_left_after_a_move_rests_at_the_left_edge() {
    let rest = resting_view_of_one_column();
    for left in [true, false] {
        let mut space = beside_a_sidebar();
        for id in 1..=2 {
            space.m().add_column(None, test_column(id, ColumnWidth::Proportion(0.5)), true);
        }
        // Still easing from the focus change when the key is pressed, as in the app.
        space.m().activate_column(if left { 1 } else { 0 });
        let moved = match left {
            true => space.m().move_active_pane_left(ColumnId(50)),
            false => space.m().move_active_pane_right(ColumnId(50)),
        };
        assert!(moved);
        assert_eq!(space.columns.len(), 1);
        let target = space.r().target_view_pos();
        assert!((target - rest).abs() < 0.5, "moved left: {left}; view rests at {target}, not {rest}");
    }
}

/// Two columns that both fit, the right one active with two panes: moving its pane left follows
/// it there without scrolling, so nothing that was on screen leaves it.
#[test]
fn following_a_pane_into_a_column_on_screen_does_not_scroll() {
    let mut space = beside_a_sidebar();
    for id in 1..=2 {
        space.m().add_column(None, test_column(id, ColumnWidth::Proportion(0.5)), true);
    }
    space.m().add_pane_to_column(1, None, Pane::new(PaneId(9), ""), true);
    space.view.offset = ViewOffset::Static(space.view.offset.target());
    let rest = space.r().target_view_pos();

    assert!(space.m().move_active_pane_left(ColumnId(50)));

    assert_eq!(space.r().active_column_idx(), 0);
    let target = space.r().target_view_pos();
    assert!((target - rest).abs() < 0.5, "the view moved from {rest} to {target}");
}

/// Two columns that both fit, the right one active: closing it leaves the left one where it was.
#[test]
fn closing_the_active_column_leaves_the_others_where_they_were() {
    let mut space = beside_a_sidebar();
    for id in 1..=2 {
        space.m().add_column(None, test_column(id, ColumnWidth::Proportion(0.5)), true);
    }
    space.view.offset = ViewOffset::Static(space.view.offset.target());
    let rest = space.r().target_view_pos();

    assert!(space.m().remove_column(1).is_some());

    let target = space.r().target_view_pos();
    assert!((target - rest).abs() < 0.5, "the view moved from {rest} to {target}");
}
