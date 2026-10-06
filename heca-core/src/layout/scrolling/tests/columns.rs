//! Adding, reordering, swapping, zooming and resizing columns.

use super::*;


/// **A pane leaves its column and gets one of its own, immediately to the right**
/// (F003/P082/T474 part B; Antonio, 2026-09-10 — right of the current one, not the end of the
/// strip, so you keep your place).
/// **A new column's width is the layout's to say.** It used to be chosen by each caller: most
/// said 50%, but taking a pane into an empty workspace said 85% and two fallbacks did too, so
/// the same act gave a different column depending on how it was reached (F003/P082/T509).
#[test]
fn a_new_column_takes_the_layouts_default_width() {
    let options = LayoutOptions {
        default_column_width: ColumnWidth::Proportion(0.7),
        ..Default::default()
    };
    let area = Rectangle::from_size(Size::new(1000.0, 800.0));
    let space = Seen::new(area, 1.0, options);
    let column = space.new_column(ColumnId(1), Pane::new(PaneId(1), "p"));
    assert_eq!(column.width, ColumnWidth::Proportion(0.7));
}


#[test]
fn reorder_column_moves_and_activates() {
    let mut space = space_with_columns(4); // [1,2,3,4]
    assert!(space.m().reorder_column(0, 2));
    assert_eq!(column_ids(&space), vec![2, 3, 1, 4]);
    assert_eq!(
        space.view.active_column, 2,
        "the moved column becomes active"
    );
}


#[test]
fn reorder_column_clamps_and_no_ops() {
    let mut space = space_with_columns(3); // [1,2,3]
    assert!(space.m().reorder_column(0, 99), "dst clamps to the last index");
    assert_eq!(column_ids(&space), vec![2, 3, 1]);
    assert!(!space.m().reorder_column(1, 1), "same index is a no-op");
    assert!(
        !space.m().reorder_column(9, 0),
        "out-of-range source is a no-op"
    );
}


#[test]
fn swap_columns_exchanges_positions() {
    let mut space = space_with_columns(4); // [1,2,3,4]
    assert!(space.m().swap_columns(0, 3));
    assert_eq!(column_ids(&space), vec![4, 2, 3, 1]);
    assert!(!space.m().swap_columns(1, 1), "self-swap is a no-op");
    assert!(!space.m().swap_columns(0, 9), "out-of-range is a no-op");
}


#[test]
fn toggle_active_column_zoom_restores_previous_width() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);

    assert_eq!(space.columns[0].width, ColumnWidth::Proportion(0.5));
    assert!(!space.columns[0].is_zoomed());

    assert!(space.m().toggle_active_column_zoom());
    assert!(space.columns[0].is_zoomed());
    assert_eq!(
        space.columns[0].zoom_restore_width,
        Some(ColumnWidth::Proportion(0.5))
    );
    assert_eq!(space.r().column_width(0), 984.0);

    assert!(space.m().toggle_active_column_zoom());
    assert_eq!(space.columns[0].width, ColumnWidth::Proportion(0.5));
    assert!(!space.columns[0].is_zoomed());
}


#[test]
fn resize_column_persists_through_recompute_and_add() {
    let mut space = space_with_columns(2); // [1,2] each Proportion(0.5)
    space.m().resize_column(0, 0.2);
    assert_eq!(space.columns[0].width, ColumnWidth::Proportion(0.7));
    let w0 = space.r().column_width(0);
    // A later layout mutation recomputes the width cache from the canonical
    // `col.width` — the resize must NOT be recomputed away (the niri landmine).
    assert_eq!(space.columns[0].width, ColumnWidth::Proportion(0.7));
    assert_eq!(
        space.r().column_width(0), w0,
        "recompute preserves the manual resize"
    );
    // Adding a column must not reflow column 0 (independent proportions).
    space.m().add_column(None, test_column(3, ColumnWidth::Proportion(0.5)), true);
    assert_eq!(
        space.columns[0].width,
        ColumnWidth::Proportion(0.7),
        "resize survives add"
    );
}


#[test]
fn resize_column_clamps_and_ignores_out_of_range() {
    let mut space = space_with_columns(2);
    space.m().resize_column(0, 10.0); // huge delta clamps to the full-width cap (1.0 = viewport)
    assert_eq!(space.columns[0].width, ColumnWidth::Proportion(1.0));
    // A huge negative delta clamps to the min-width proportion (small, non-zero).
    space.m().resize_column(1, -10.0);
    match space.columns[1].width {
        ColumnWidth::Proportion(p) => {
            assert!(
                p > 0.0 && p < 0.5,
                "clamped to a small but non-zero min, got {p}"
            );
        }
        other => panic!("expected a proportion, got {other:?}"),
    }
    space.m().resize_column(99, 0.1); // out of range → no-op, no panic
    assert_eq!(space.columns.len(), 2);
}


#[test]
fn a_column_is_zoomed_by_index_without_becoming_active() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.4)), true);
    space.m().add_column(None, test_column(2, ColumnWidth::Proportion(0.6)), false);

    assert!(space.m().toggle_column_zoom(1));
    assert!(space.columns[1].is_zoomed());
    assert!(!space.columns[0].is_zoomed());
    assert_eq!(space.view.active_column, 0, "zooming does not move the active column");

    assert!(!space.m().toggle_column_zoom(9), "no such column");
}


#[test]
fn columns_can_be_zoomed_independently() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.4)), true);
    space.m().add_column(None, test_column(2, ColumnWidth::Proportion(0.6)), false);

    assert!(space.m().toggle_active_column_zoom());
    assert!(space.columns[0].is_zoomed());
    assert_eq!(
        space.columns[0].zoom_restore_width,
        Some(ColumnWidth::Proportion(0.4))
    );

    space.m().activate_column(1);
    assert!(space.m().toggle_active_column_zoom());

    assert!(space.columns[0].is_zoomed());
    assert_eq!(
        space.columns[0].zoom_restore_width,
        Some(ColumnWidth::Proportion(0.4))
    );
    assert!(space.columns[1].is_zoomed());
    assert_eq!(
        space.columns[1].zoom_restore_width,
        Some(ColumnWidth::Proportion(0.6))
    );

    assert!(space.m().toggle_active_column_zoom());
    assert!(space.columns[0].is_zoomed());
    assert!(!space.columns[1].is_zoomed());
    assert_eq!(space.columns[1].width, ColumnWidth::Proportion(0.6));
}


/// **Both of a column's edges can be moved, and moving one is a transfer, not a shove.**
///
/// A column's left edge is the right edge of the column before it, so moving it takes width
/// from one and gives it to the other — the active column's *far* edge does not budge. Resizing
/// only the neighbour would widen it and push the active column sideways unchanged, which reads
/// as resizing the wrong column (Antonio, driving, 2026-09-04).
#[test]
fn moving_a_columns_left_edge_trades_width_with_the_column_before_it() {
    let mut space = space_with_columns(3);
    space.m().activate_column(1);
    let widths = |s: &Seen| -> Vec<f64> { (0..s.columns.len()).map(|i| s.r().column_width(i)).collect() };

    let before = widths(&space);
    space.m().resize_active_column(0.1);
    let after_right = widths(&space);
    assert!(
        after_right[1] > before[1],
        "its own right edge widens it ({before:?} → {after_right:?})"
    );
    assert!(
        (after_right[0] - before[0]).abs() < 0.5,
        "and leaves the column beside it alone"
    );

    space.m().move_active_column_left_boundary(0.1);
    let after_left = widths(&space);
    assert!(
        after_left[0] > after_right[0],
        "the neighbour gains ({after_right:?} → {after_left:?})"
    );
    assert!(
        after_left[1] < after_right[1],
        "…and the ACTIVE column gives it up — a transfer, not a shove"
    );
    let gained = after_left[0] - after_right[0];
    let given = after_right[1] - after_left[1];
    assert!(
        (gained - given).abs() < 0.5,
        "equal and opposite, so the active column's far edge stays put ({gained} vs {given})"
    );

    // The first column has nothing to its left, so there is no boundary to move.
    space.m().activate_column(0);
    let pinned = widths(&space);
    space.m().move_active_column_left_boundary(0.1);
    assert_eq!(
        widths(&space),
        pinned,
        "no edge to the left of the first column"
    );
}

/// **A slide belongs to the window that shows it**: swapping columns starts one in this window's
/// view, so the columns are drawn away from where they rest; another window looking at the same
/// content draws them at rest.
#[test]
fn a_move_slides_in_the_window_that_made_it_and_not_in_another() {
    let mut space = space_with_columns(3);
    let rest = |ref_: &ScrollingRef<'_>| -> Vec<f64> {
        ref_.columns_with_positions()
            .iter()
            .map(|c| c.rect.loc.x - (ref_.column_x(c.idx) - ref_.view_pos()))
            .collect()
    };
    let settled = space.r();
    assert!(rest(&settled).iter().all(|d| d.abs() < 0.01), "nothing has moved yet");
    assert!(space.m().swap_columns(0, 2));
    assert!(rest(&space.r()).iter().any(|d| d.abs() > 0.01), "this window shows the move easing");

    let other = ScrollView::new(Rectangle::from_size(Size::new(1000.0, 800.0)), 1.0);
    let elsewhere = space.space.through(&other);
    assert!(rest(&elsewhere).iter().all(|d| d.abs() < 0.01), "another window shows none");
}

/// **The content changes with no view at all**: these run on a bare `ScrollingSpace`, which has
/// nowhere to look and nothing to animate, and each says what it did.
#[test]
fn the_content_changes_and_says_what_it_did_with_no_view() {
    let mut space = ScrollingSpace::new(LayoutOptions::default());
    let column = |id| test_column(id, ColumnWidth::Proportion(0.5));
    assert_eq!(space.insert_column(0, column(1)), ColumnEffect::Inserted { idx: 0 });
    assert_eq!(space.insert_column(9, column(2)), ColumnEffect::Inserted { idx: 1 }, "past the end is the end");
    space.insert_column(2, column(3));
    assert_eq!(space.move_column(0, 9), Some(ColumnEffect::Moved { from: 0, to: 2 }));
    assert_eq!(column_ids(&space), [2, 3, 1]);
    assert_eq!(space.move_column(1, 1), None);
    assert_eq!(space.swap_columns(0, 2), Some(ColumnEffect::Swapped { a: 0, b: 2 }));
    assert_eq!(column_ids(&space), [1, 3, 2]);
    assert_eq!(space.swap_columns(0, 0), None);
    assert_eq!(space.set_column_width(1, ColumnWidth::Fixed(300.0)), Some(ColumnEffect::Resized { idx: 1 }));
    assert_eq!(space.columns[1].width, ColumnWidth::Fixed(300.0));
    assert_eq!(space.zoom_column(1), Some(ColumnEffect::Resized { idx: 1 }));
    assert!(space.columns[1].is_zoomed());
    space.zoom_column(1);
    assert_eq!(space.columns[1].width, ColumnWidth::Fixed(300.0), "zoom restores the width");
    space.zoom_column(1);
    space.set_column_width(1, ColumnWidth::Fixed(200.0));
    assert!(!space.columns[1].is_zoomed(), "a resize clears the zoom");
    let (taken, effect) = space.take_column(0).expect("a column");
    assert_eq!((taken.id.0, effect), (1, ColumnEffect::Removed { idx: 0 }));
    assert_eq!(space.take_column(7).map(|_| ()), None);
}

/// The positions taken before a change are the columns' x by identity and the scroll.
#[test]
fn positions_are_the_columns_x_by_identity_and_the_scroll() {
    let space = space_with_columns(3);
    let before = space.r().positions();
    assert_eq!(before.columns.len(), 3);
    assert_eq!(before.columns[0].1, 0.0);
    assert!(before.columns[1].1 > 0.0);
    assert_eq!(before.view_pos, space.r().view_pos());
}
