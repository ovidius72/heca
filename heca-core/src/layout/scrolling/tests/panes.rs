//! Pane heights, dragging a divider, and moving a pane between columns.

use super::*;


#[test]
fn resize_pane_height_sets_preferred_and_no_ops_single_pane() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
    // Single-pane column → no-op (the lone pane fills the column).
    space.m().resize_pane_height(0, 0, 30.0);
    assert_eq!(space.columns[0].panes[0].height_share, None);
    // Stack a second pane, then the resize takes effect.
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), true);
    space.m().resize_pane_height(0, 0, 30.0);
    assert!(space.columns[0].panes[0].height_share.is_some());
    // Out-of-range column / pane index → no panic.
    space.m().resize_pane_height(9, 0, 30.0);
    space.m().resize_pane_height(0, 9, 30.0);
}


/// **A boundary moves space between its own two panes, and nothing else** (F004/P084/T413).
///
/// It used to pin one pane and let `Column::pane_heights` redistribute the remainder over every
/// pane still auto-sized. With two panes the only auto pane *was* the neighbour, so it looked
/// right; with three, dragging the TOP boundary took space from the BOTTOM pane too, which
/// collapsed to the floor and read as having disappeared.
#[test]
fn a_boundary_drag_leaves_the_pane_beyond_it_untouched() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), false);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(3), "p3".to_string()), false);

    let before: Vec<f64> = space.r().pane_heights(0);
    assert_eq!(before.len(), 3, "three stacked panes");
    let third = before[2];

    // Drag the boundary between pane 0 and pane 1 downwards.
    space.m().resize_pane_height(0, 0, 60.0);
    let after: Vec<f64> = space.r().pane_heights(0);

    assert!(
        (after[0] - (before[0] + 60.0)).abs() < 0.5,
        "the pane above grew by the drag"
    );
    assert!(
        (after[1] - (before[1] - 60.0)).abs() < 0.5,
        "…and its neighbour gave exactly that"
    );
    assert!(
        (after[2] - third).abs() < 0.5,
        "the third pane is not on this boundary and must not move: {third} -> {}",
        after[2],
    );
    // The column stays exactly full, so nothing is pushed past its bottom edge.
    let sum_before: f64 = before.iter().sum();
    let sum_after: f64 = after.iter().sum();
    assert!(
        (sum_after - sum_before).abs() < 0.5,
        "the column is still exactly full"
    );
}


/// The far side stops at its floor rather than the drag reaching past it for more space — which
/// is what let one boundary eat a pane two positions away.
#[test]
fn a_boundary_drag_stops_when_its_neighbour_hits_the_floor() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), false);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(3), "p3".to_string()), false);
    let third = space.r().pane_heights(0)[2];

    // Far more than the neighbour can give, repeatedly.
    for _ in 0..20 {
        space.m().resize_pane_height(0, 0, 500.0);
    }
    let after: Vec<f64> = space.r().pane_heights(0);

    assert!(
        after[1] >= crate::layout::column::MIN_PANE_HEIGHT - 0.5,
        "the neighbour never goes below the floor: {}",
        after[1],
    );
    assert!(
        (after[2] - third).abs() < 0.5,
        "and the pane beyond the boundary is still untouched: {third} -> {}",
        after[2],
    );
}


/// **The divider goes the way the key says, whichever pane is active** (F004/P084/T414).
///
/// `j` is directional; "grow the active pane" is not. They disagree for the last pane, which
/// has no boundary beneath it and so grows *upwards* — which is why `prefix+r` felt inverted on
/// the top and middle panes and correct on the bottom one. Naming a boundary instead of a size
/// makes one statement of it: a positive amount moves that boundary **down**, always.
#[test]
fn the_keyboard_moves_a_divider_the_same_way_from_every_pane() {
    // Each seat in a three-pane column, and the boundary each one owns.
    for (active, boundary) in [(0usize, 0usize), (1, 1), (2, 1)] {
        let mut space = test_scrolling_space();
        space.m().add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
        space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), false);
        space.m().add_pane_to_column(0, None, Pane::new(PaneId(3), "p3".to_string()), false);

        let before: Vec<f64> = space.r().pane_heights(0);
        let (h, gaps) = (space.view.area.size.h, space.options.gaps);
        let col = &mut space.columns[0];
        col.active_pane_idx = active;
        col.move_active_pane_boundary(40.0, h, gaps);
        let after: Vec<f64> = space.r().pane_heights(0);

        // A boundary moving DOWN grows the pane above it and shrinks the pane below it — the
        // same two panes, by the same amount, from whichever seat the key was pressed.
        assert!(
            (after[boundary] - (before[boundary] + 40.0)).abs() < 0.5,
            "active {active}: the pane above the boundary grew, {} -> {}",
            before[boundary],
            after[boundary],
        );
        assert!(
            (after[boundary + 1] - (before[boundary + 1] - 40.0)).abs() < 0.5,
            "active {active}: the pane below it gave exactly that, {} -> {}",
            before[boundary + 1],
            after[boundary + 1],
        );
        // The third pane is not on this boundary (T413's rule still holds).
        let untouched = if boundary == 0 { 2 } else { 0 };
        assert!(
            (after[untouched] - before[untouched]).abs() < 0.5,
            "active {active}: pane {untouched} is not on this boundary and must not move",
        );
    }
}


/// The size verbs keep meaning size. `pane_height_increase` says "increase", so it grows the
/// active pane whichever edge has to move — the opposite reading to the directional one above,
/// and both are correct for the words they are spelled with.
#[test]
fn the_size_verb_still_grows_the_active_pane_from_every_seat() {
    for active in [0usize, 1, 2] {
        let mut space = test_scrolling_space();
        space.m().add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
        space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), false);
        space.m().add_pane_to_column(0, None, Pane::new(PaneId(3), "p3".to_string()), false);

        let before: Vec<f64> = space.r().pane_heights(0);
        let (h, gaps) = (space.view.area.size.h, space.options.gaps);
        let col = &mut space.columns[0];
        col.active_pane_idx = active;
        col.resize_active_pane_height(40.0, h, gaps);

        let after = space.r().pane_heights(0);
        assert!(
            (after[active] - (before[active] + 40.0)).abs() < 0.5,
            "active {active}: the active pane grew, {} -> {}",
            before[active],
            after[active],
        );
    }
}


/// **Splitting a column a third time gives the new pane a real pane's worth of room.**
///
/// Drag a divider and both panes carry the height you dragged them to, which between them is the
/// whole column. Split again and there is nothing left — so the newcomer used to be assigned a
/// single pixel, and the pass that scales everything to fit took barely one per cent off the
/// other two. The pane existed, in the column, and could not be seen: what that looks like is
/// "the third pane went off the screen".
///
/// ⚠️ **Asserting that the heights sum to the column proves nothing here** — they always did,
/// which is why an earlier look at this concluded the arithmetic was correct and stopped. The
/// assertion that catches it is that every pane is big enough to be a pane.
#[test]
fn a_pane_added_to_a_column_that_was_dragged_full_still_gets_room() {
    let opts = LayoutOptions {
        gaps: 8.0,
        ..Default::default()
    };
    let area = Rectangle::from_size(Size::new(1000.0, 800.0));
    let mut space = Seen::new(area, 1.0, opts);
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), String::from("p2")), true);
    // Drag the divider well off centre, so the two of them fill the column between them.
    space.m().resize_pane_height(0, 0, 120.0);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(3), String::from("p3")), true);

    let panes = space.r().panes_with_positions();
    assert_eq!(panes.len(), 3, "three panes in the column");
    for (id, rect) in &panes {
        assert!(
            rect.size.h >= crate::layout::column::MIN_PANE_HEIGHT - 0.5,
            "pane {id:?} came out {}px tall — a pane nobody can see \
             ({:?})",
            rect.size.h,
            panes.iter().map(|(_, r)| r.size.h).collect::<Vec<_>>()
        );
    }
    let last = panes.last().expect("three panes").1;
    assert!(
        last.loc.y + last.size.h <= area.size.h,
        "and the column still ends inside its own area"
    );
}


/// **Both of a pane's edges can be moved.** The counterpart of the column test above: plain
/// `j`/`k` move the boundary below the active pane, `edge = "top"` the one above it.
#[test]
fn a_pane_can_be_resized_from_either_of_its_edges() {
    let opts = LayoutOptions {
        gaps: 8.0,
        ..Default::default()
    };
    let area = Rectangle::from_size(Size::new(1000.0, 800.0));
    let mut space = Seen::new(area, 1.0, opts);
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), String::from("p2")), true);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(3), String::from("p3")), true);
    let heights = |s: &Seen| -> Vec<f64> {
        s.r()
            .panes_with_positions()
            .iter()
            .map(|(_, r)| r.size.h)
            .collect()
    };

    // The middle pane: moving its BOTTOM edge down trades with the pane below.
    let col = space.columns[0].active_pane_idx;
    assert_eq!(col, 2, "the last added pane is active");
    space.columns[0].activate_pane(1);

    let before = heights(&space);
    let h = space.view.area.size.h;
    let gaps = space.options.gaps;
    space.columns[0].move_active_pane_boundary(40.0, h, gaps);
    let after_bottom = heights(&space);
    assert!(
        (after_bottom[0] - before[0]).abs() < 0.5,
        "the pane ABOVE is untouched by the bottom edge ({before:?} → {after_bottom:?})"
    );

    space.columns[0].move_active_pane_top_boundary(40.0, h, gaps);
    let after_top = heights(&space);
    assert!(
        (after_top[2] - after_bottom[2]).abs() < 0.5,
        "and the pane BELOW is untouched by the top edge ({after_bottom:?} → {after_top:?})"
    );
    assert!(
        after_top[0] > after_bottom[0],
        "moving the top edge down grows the pane above it"
    );

    // The first pane has nothing above it.
    space.columns[0].activate_pane(0);
    let pinned = heights(&space);
    space.columns[0].move_active_pane_top_boundary(40.0, h, gaps);
    assert_eq!(heights(&space), pinned, "no edge above the first pane");
}


/// A pane's top is the gaps and the panes above it — worked out from the column, never stored.
#[test]
fn a_pane_starts_below_the_panes_above_it() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), false);
    let gaps = space.options.gaps;
    let heights = space.r().pane_heights(0);
    assert_eq!(space.r().pane_y_in_column(0, 0), gaps);
    assert_eq!(space.r().pane_y_in_column(0, 1), gaps + heights[0] + gaps);
}

/// The bottom edge of the lowest pane of column 0, and of the area it is laid out in.
fn bottoms(space: &Seen) -> (f64, f64) {
    let r = space.r();
    let columns = r.columns_with_positions();
    let lowest = columns[0].panes.iter().map(|p| p.slot.loc.y + p.slot.size.h).fold(0.0, f64::max);
    (lowest, r.area().loc.y + r.area().size.h)
}

/// Where each of `panes` is drawn, slide included, rounded to a hundredth of a pixel.
fn drawn_at(space: &Seen, panes: &[PaneId]) -> Vec<(i64, i64)> {
    let drawn = space.r().panes_with_positions();
    let at = |id: &PaneId| drawn.iter().find(|(p, _)| p == id).map(|(_, r)| r.loc);
    let round = |v: f64| (v * 100.0).round() as i64;
    panes.iter().filter_map(at).map(|loc| (round(loc.x), round(loc.y))).collect()
}

/// **Every pane stays on screen** after heights are dragged and panes are swapped or moved: the
/// column's heights always fill exactly the room it has.
#[test]
fn panes_stay_on_screen_after_a_resize_and_a_swap() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(1.0)), true);
    for id in 2..=4 {
        space.m().add_pane_to_column(0, None, Pane::new(PaneId(id), format!("p{id}")), true);
    }
    let (h, gaps) = (space.r().area().size.h, space.options.gaps);
    let room = h - gaps * 5.0;
    for step in 0..12 {
        space.m().resize_pane_height(0, step % 3, if step % 2 == 0 { 70.0 } else { -45.0 });
        let total: f64 = space.r().pane_heights(0).iter().sum();
        assert!((total - room).abs() < 0.5, "step {step}: heights {total} vs room {room}");
        let (lowest, bottom) = bottoms(&space);
        assert!(lowest <= bottom - gaps + 0.5, "step {step}: lowest pane ends at {lowest} of {bottom}");
        let before = space.r().positions();
        let n = space.columns[0].panes.len();
        let (a, b) = (step % n, (step + 1) % n);
        let swapped = [space.columns[0].panes[a].id, space.columns[0].panes[b].id];
        let was = drawn_at(&space, &swapped);
        let effect = space.space.swap_panes_in_column(0, a, b).expect("rows a and b are there");
        space.m().show(&[SpaceEffect::Pane(effect)], &before);
        let total: f64 = space.r().pane_heights(0).iter().sum();
        assert!((total - room).abs() < 0.5, "after a swap at step {step}: {total} vs {room}");
        // Each swapped pane starts its slide where it was drawn, even mid-way through an earlier
        // slide, so a swap never throws a pane off screen.
        assert_eq!(drawn_at(&space, &swapped), was, "step {step}: a swapped pane jumped");
    }
}
