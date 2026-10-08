//! Adding, reordering, swapping, zooming and resizing columns.

use super::*;


/// **A pane leaves its column and gets one of its own, immediately to the right**
/// — right of the current one, not the end of the strip, so you keep your place.
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
/// as resizing the wrong column.
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
    assert_eq!(space.zoom_column(1), Some(ColumnEffect::Zoomed { idx: 1 }));
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

/// What a window shows after a change, to compare two ways of getting there.
fn shown(space: &Seen) -> (usize, f64, Vec<f64>) {
    let r = space.r();
    let xs = r.columns_with_positions().iter().map(|c| c.rect.loc.x).collect();
    (r.active_column_idx(), r.view_pos(), xs)
}

/// **`react` alone does what the combined operation does**: the content changes with no view, the
/// window reacts to the effect, and the window ends up where the one-call operation puts it.
#[test]
fn reacting_to_an_effect_alone_equals_the_combined_operation() {
    type Op = fn(&mut Seen) -> bool;
    type Content = fn(&mut ScrollingSpace) -> Option<ColumnEffect>;
    let cases: [(&str, Op, Content); 5] = [
        ("move", |s| s.m().reorder_column(0, 2), |c| c.move_column(0, 2)),
        ("swap", |s| s.m().swap_columns(0, 2), |c| c.swap_columns(0, 2)),
        ("zoom", |s| s.m().toggle_column_zoom(1), |c| c.zoom_column(1)),
        ("resize", |s| { s.m().resize_column(1, 0.2); true }, |c| c.set_column_width(1, ColumnWidth::Proportion(0.7))),
        ("remove", |s| s.m().remove_column(1).is_some(), |c| c.take_column(1).map(|(_, e)| e)),
    ];
    for (name, combined, content) in cases {
        let mut whole = space_with_columns(3);
        let mut apart = space_with_columns(3);
        assert!(combined(&mut whole), "{name}");
        let before = apart.r().positions();
        let effect = content(&mut apart.space).expect("an effect");
        apart.view.react(&apart.space, effect, &before);
        assert_eq!(shown(&whole), shown(&apart), "{name}: the window ends up in the same place");
    }
}

/// **Another window reacts for itself**: two windows on one content, each with its own active
/// column; removing a column shifts each one's own, and neither is told what the other does.
#[test]
fn a_second_window_reacts_to_the_same_effect_on_its_own() {
    let mut a = space_with_columns(3);
    let mut b = space_with_columns(3);
    a.m().activate_column(0);
    b.m().activate_column(2);
    let before_a = a.r().positions();
    let before_b = b.r().positions();
    let (_, effect) = a.space.take_column(0).expect("a column");
    b.space = a.space.clone();
    a.view.react(&a.space, effect, &before_a);
    b.view.react(&b.space, effect, &before_b);
    assert_eq!(a.r().active_column_idx(), 0, "the window that was on the removed column falls back");
    assert_eq!(b.r().active_column_idx(), 1, "the window further along keeps its own column");
}

/// Three half-width columns overflow the 1000px window, with column 1 active and scrolled.
fn scrolled() -> Seen {
    let mut space = space_with_columns(3);
    space.m().activate_column(1);
    space
}

/// The screen x of the left edge of column `idx`, at rest.
fn left_edge(space: &Seen, idx: usize) -> f64 {
    space.r().column_x(idx) - space.r().view_pos()
}

/// A column put in before the active one pushes the active column's index along.
#[test]
fn a_column_inserted_before_the_active_one_shifts_its_index() {
    let mut space = scrolled();
    space.m().add_column(Some(0), test_column(9, ColumnWidth::Proportion(0.5)), false);
    assert_eq!(space.r().active_column_idx(), 2);
}

/// A moved column stays active and the window stays over the same part of the strip.
#[test]
fn a_moved_column_stays_active_and_the_window_stays_over_the_same_strip() {
    let mut space = scrolled();
    let was = space.r().view_pos();
    space.m().reorder_column(1, 2);
    assert_eq!(space.r().active_column_idx(), 2);
    assert!((space.r().view_pos() - was).abs() < 0.5, "{} vs {}", space.r().view_pos(), was);
}

/// Dragging a column's edge keeps its left edge where it was on screen.
#[test]
fn resizing_a_column_keeps_its_left_edge_where_it_was_on_screen() {
    let mut space = scrolled();
    let was = left_edge(&space, 0);
    space.m().resize_column(0, 0.2);
    assert!((left_edge(&space, 0) - was).abs() < 0.5);
}

/// A zoomed column ends up inside the window.
#[test]
fn a_zoomed_column_is_brought_into_the_window() {
    let mut space = space_with_columns(4);
    assert!(space.m().toggle_active_column_zoom());
    let width = space.r().column_width(3);
    let left = -space.view.offset.target();
    assert!(left >= -0.5 && left + width <= 1000.0 + 0.5, "left {left}, width {width}");
}

/// Where the active column ends up on screen once the view has settled: its left edge, and its
/// right edge.
fn settled_edges(space: &Seen) -> (f64, f64) {
    let r = space.r();
    let left = -space.view.offset.target();
    (left, left + r.column_width(r.active_column_idx()))
}

/// **A column you swap stays on screen**, at the edges of what is visible: the rightmost visible
/// column moved left, and the leftmost moved right.
#[test]
fn a_swapped_column_is_kept_on_screen_at_the_edges() {
    let vw = 1000.0;
    for (start, step_left) in [(3usize, true), (0usize, false)] {
        let mut space = space_with_columns(4);
        space.m().activate_column(start);
        for _ in 0..3 {
            let moved = match step_left {
                true => space.m().move_column_left(),
                false => space.m().move_column_right(),
            };
            assert!(moved);
            let (left, right) = settled_edges(&space);
            assert!(
                left >= -0.5 && right <= vw + 0.5,
                "moving {}: the column is at {left}..{right} of {vw}",
                if step_left { "left" } else { "right" }
            );
        }
    }
}
