//! Tests for pane heights.

use super::*;

fn column_of(n: usize) -> Column {
    let mut col = Column::new(
        ColumnId(1),
        Pane::new(PaneId(1), "p1"),
        ColumnWidth::Proportion(1.0),
    );
    for i in 1..n {
        col.add_pane_at(i, Pane::new(PaneId(i as u64 + 1), format!("p{}", i + 1)));
    }
    col
}

/// **The panes exactly fill the column, always.**
///
/// A pane that has been resized carries a fixed height, and the floor the others were given was
/// worked out against the *whole* column rather than the room those fixed panes had left — so
/// three panes in a resized column summed to more than the column and the last one was pushed
/// off the bottom of the screen. It happened only in a column that had been resized, which is
/// exactly why a freshly created one looked fine.
#[test]
fn panes_never_sum_to_more_than_the_column() {
    let working = 800.0;
    let gaps = 4.0;
    for fixed in [None, Some(600.0), Some(700.0), Some(60.0)] {
        let mut col = column_of(3);
        if let Some(h) = fixed {
            col.set_pane_height(0, h, working, gaps);
        }
        let heights = col.pane_heights(working, gaps);
        let total: f64 = heights.iter().sum();
        let available = working - gaps * (col.panes.len() as f64 + 1.0);
        assert!(
            total <= available + 0.5,
            "three panes summed to {total} in {available} of room (first pane fixed at {fixed:?})"
        );
    }
}

/// …and they fill it, rather than leaving a gap at the bottom.
#[test]
fn panes_fill_the_column_they_are_given() {
    let working = 800.0;
    let gaps = 4.0;
    let mut col = column_of(3);
    col.set_pane_height(0, 200.0, working, gaps);
    let heights = col.pane_heights(working, gaps);
    let total: f64 = heights.iter().sum();
    let available = working - gaps * 4.0;
    assert!((total - available).abs() < 0.5, "{total} of {available}");
}

/// **A boundary moves space between its own two panes and nothing else** — right up to the
/// limit.
///
/// Dragging the middle pane's lower boundary moved the bottom edge, and then at a certain point
/// started moving the *top* one too: the untouched panes were being forced above the room left
/// to them, the heights summed past the column, and the proportional scale that keeps the column
/// full then shrank every pane — including the two the drag had just pinned.
/// **A column is never narrower than its floor, whatever the window did** (F003/P082/T478).
///
/// `MIN_COLUMN_WIDTH` used to be enforced only by the resize handlers, which stop *you* dragging
/// a column to nothing and stopped nothing else. Two sidebars in a narrow window squeezed the
/// working area to zero, a proportion of zero resolved to zero, and every pane in the column
/// laid out with no width at all — still lettered by `prefix+/`, its keycap drawn beside a pane
/// with no inside.
#[test]
fn a_column_keeps_its_floor_when_the_working_area_collapses() {
    let floor = crate::layout::scrolling::MIN_COLUMN_WIDTH;
    let col = column_of(1);
    assert!(
        col.resolve_width(0.0, 8.0) >= floor,
        "a column in a working area squeezed to nothing still has a width (got {})",
        col.resolve_width(0.0, 8.0),
    );
    assert!(
        col.resolve_width(150.0, 8.0) >= floor,
        "and so does one whose proportion resolves below the floor (got {})",
        col.resolve_width(150.0, 8.0),
    );
}

/// A column with room resolves to what its proportion actually asks for — the floor is a floor,
/// not a width.
#[test]
fn a_column_with_room_is_sized_by_its_proportion_not_the_floor() {
    let col = column_of(1);
    let w = col.resolve_width(1200.0, 8.0);
    assert!(
        w > crate::layout::scrolling::MIN_COLUMN_WIDTH,
        "a wide working area gives a wide column (got {w})",
    );
}

#[test]
fn a_resize_leaves_every_other_pane_where_it_was() {
    let working = 400.0;
    let gaps = 4.0;
    let mut col = column_of(3);
    // Two panes pinned by earlier drags, leaving the third less than its floor. That is the
    // case: the third was then forced up to the column-wide floor, the heights summed past the
    // column, and the scale that keeps the column full pulled the two pinned panes off the
    // sizes the user had just set.
    col.set_pane_height(0, 250.0, working, gaps);
    col.set_pane_height(1, MIN_PANE_HEIGHT, working, gaps);
    let heights = col.pane_heights(working, gaps);

    assert!(
        (heights[0] - 250.0).abs() < 0.5,
        "a pinned pane keeps the height it was given (got {})",
        heights[0]
    );
    assert!(
        (heights[1] - MIN_PANE_HEIGHT).abs() < 0.5,
        "and so does the one on the other side of that boundary (got {})",
        heights[1]
    );
}

/// **A column too small for every pane's floor degrades proportionally** rather than pushing
/// the last pane out — which is what the floor's own note asks for.
#[test]
fn a_short_column_shrinks_every_pane_rather_than_losing_one() {
    let col = column_of(5);
    let heights = col.pane_heights(200.0, 2.0);
    let available = 200.0 - 2.0 * 6.0;
    let total: f64 = heights.iter().sum();
    assert!(total <= available + 0.5, "{total} of {available}");
    assert!(
        heights.iter().all(|h| *h > 0.0),
        "no pane collapses to nothing"
    );
}

/// **A dragged height is a proportion**: the same column at two window heights keeps the split
/// the user dragged to, which a pixel height cannot.
#[test]
fn a_dragged_split_keeps_its_proportion_when_the_window_changes() {
    let gaps = 4.0;
    let mut col = column_of(2);
    col.set_pane_height(0, 0.6 * (800.0 - gaps * 3.0), 800.0, gaps);
    for working in [800.0, 400.0] {
        let heights = col.pane_heights(working, gaps);
        let ratio = heights[0] / heights.iter().sum::<f64>();
        assert!((ratio - 0.6).abs() < 0.01, "at {working}px the split is {ratio}");
    }
}

/// Setting a height with no room to measure against changes nothing.
#[test]
fn a_height_set_in_no_room_changes_nothing() {
    let mut col = column_of(2);
    col.set_pane_height(0, 100.0, 0.0, 4.0);
    assert_eq!(col.panes[0].height_share, None);
}

/// A pane that arrives does not change the height another was dragged to (when they all fit): the
/// share is re-expressed against the room the column has with the new pane in it.
#[test]
fn a_new_pane_leaves_a_dragged_height_in_pixels_where_it_was() {
    let (working, gaps) = (800.0, 4.0);
    let mut col = column_of(2);
    col.set_pane_height(0, 300.0, working, gaps);
    col.make_room_for_one_more(working, gaps);
    col.add_pane_at(2, Pane::new(PaneId(9), "new"));
    let heights = col.pane_heights(working, gaps);
    assert!((heights[0] - 300.0).abs() < 0.5, "{}", heights[0]);
}
