use super::*;
use crate::layout::testing::Seen;

/// **A column's box is exactly the panes it holds**, and the two views agree because they are
/// one walk (F003/P082/T474).
#[test]
fn a_column_spans_the_panes_inside_it_and_agrees_with_them() {
    let mut space = space_with_columns(2);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(99), "second"), false);

    let cols = space.r().columns_with_positions();
    assert_eq!(cols.len(), 2, "two columns, the first holding two panes");
    assert_eq!(cols[0].panes.len(), 2);

    for col in &cols {
        for pane in &col.panes {
            let slot = pane.slot;
            assert!(
                slot.loc.x >= col.rect.loc.x - 0.5
                    && slot.loc.x + slot.size.w <= col.rect.loc.x + col.rect.size.w + 0.5,
                "pane {:?} at {slot:?} must sit inside its column {:?}",
                pane.id,
                col.rect,
            );
            assert!(
                slot.loc.y >= col.rect.loc.y - 0.5
                    && slot.loc.y + slot.size.h <= col.rect.loc.y + col.rect.size.h + 0.5,
            );
        }
    }

    let flat: Vec<_> = cols
        .iter()
        .flat_map(|c| c.panes.iter().map(|p| (p.id, p.rect)))
        .collect();
    assert_eq!(
        flat,
        space.r().panes_with_positions(),
        "a pane must not be in two places depending on who asked",
    );
}

/// **A pane dragged out of its column does not stretch the column it is leaving.**
///
/// Flow and transform are separate answers: the column's box is the slots its panes occupy, and
/// a pane's own displacement moves where it is *drawn* without moving where it belongs.
#[test]
fn a_displaced_pane_moves_where_it_is_drawn_and_not_where_it_belongs() {
    let mut space = space_with_columns(1);
    let before = space.r().columns_with_positions()[0].rect;

    space.columns[0].panes[0].interactive_move_offset = Point::new(400.0, 90.0);
    let after = &space.r().columns_with_positions()[0];
    let pane = after.panes[0];

    assert_eq!(
        after.rect, before,
        "the column keeps the box its slots occupy"
    );
    assert_eq!(pane.displacement, Point::new(400.0, 90.0));
    assert_eq!(
        pane.rect.loc,
        pane.slot.loc + pane.displacement,
        "what is drawn is the slot plus the pane's own transform",
    );
    assert_eq!(
        pane.slot.loc, before.loc,
        "and the slot itself has not moved"
    );
}

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
fn a_pane_is_extracted_into_a_new_column_beside_its_own() {
    let mut space = space_with_columns(2);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(99), "second"), false);
    assert_eq!(space.columns[0].panes.len(), 2);

    assert!(space.m().extract_pane_to_new_column(PaneId(99), ColumnId(500)));

    assert_eq!(space.columns.len(), 3, "a column was created");
    assert_eq!(space.columns[0].panes.len(), 1, "it left the one it was in");
    assert_eq!(
        space.columns[1]
            .panes
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![PaneId(99)],
        "and landed immediately to the right, alone",
    );
    assert_eq!(space.columns[1].id, ColumnId(500));
}

/// A pane already alone in its column stays put and says so: it would leave a column of one and
/// land in a column of one, so nothing on screen would change.
#[test]
fn a_pane_alone_in_its_column_is_left_where_it_is() {
    let mut space = space_with_columns(2);
    let before = space.columns.len();

    assert!(!space.m().extract_pane_to_new_column(PaneId(1), ColumnId(500)));
    assert_eq!(space.columns.len(), before);
}

fn test_scrolling_space() -> Seen {
    Seen::new(
        Rectangle::from_size(Size::new(1000.0, 800.0)),
        1.0,
        LayoutOptions::default(),
    )
}

fn test_column(id: u64, width: ColumnWidth) -> Column {
    Column::new(
        ColumnId(id),
        Pane::new(PaneId(id), format!("Pane {id}")),
        width,
    )
}

/// A space with columns whose ids are `1..=n`, in order.
fn space_with_columns(n: u64) -> Seen {
    let mut space = test_scrolling_space();
    // activate=true appends in order (activate=false inserts at active+1).
    for id in 1..=n {
        space.m().add_column(None, test_column(id, ColumnWidth::Proportion(0.5)), true);
    }
    space
}

fn column_ids(space: &ScrollingSpace) -> Vec<u64> {
    space.columns.iter().map(|c| c.id.0).collect()
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
fn resize_pane_height_sets_preferred_and_no_ops_single_pane() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
    // Single-pane column → no-op (the lone pane fills the column).
    space.m().resize_pane_height(0, 0, 30.0);
    assert_eq!(space.columns[0].panes[0].preferred_height, None);
    // Stack a second pane, then the resize takes effect.
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(2), "p2".to_string()), true);
    space.m().resize_pane_height(0, 0, 30.0);
    assert!(space.columns[0].panes[0].preferred_height.is_some());
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

#[test]
fn scroll_view_pans_and_clamps_to_content_bounds() {
    // Three ~half-viewport columns overflow the 1000px viewport, so the view
    // can pan — but only within the content (never scrolls the layout away).
    let mut space = space_with_columns(3);
    let vw = space.view.area.size.w;

    // Pan hard left, then again → second is a no-op (already at the left bound).
    space.m().scroll_view(-vw * 10.0);
    let left_bound = space.r().view_pos();
    space.m().scroll_view(-vw * 10.0);
    assert!(
        (space.r().view_pos() - left_bound).abs() < 1.0,
        "clamped at the left content bound"
    );

    // Pan hard right, then again → clamped at the right bound.
    space.m().scroll_view(vw * 10.0);
    let right_bound = space.r().view_pos();
    space.m().scroll_view(vw * 10.0);
    assert!(
        (space.r().view_pos() - right_bound).abs() < 1.0,
        "clamped at the right content bound"
    );
    assert!(
        right_bound > left_bound,
        "the right bound is further right than the left bound"
    );
}

#[test]
fn scroll_view_is_noop_when_all_columns_fit() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
    let before = space.r().view_pos();
    space.m().scroll_view(500.0);
    assert!(
        (space.r().view_pos() - before).abs() < f64::EPSILON,
        "a single column that fits the viewport does not scroll"
    );
}

#[test]
fn refocusing_active_column_refits_a_scrolled_view() {
    // After panning the view away, re-activating the already-active column must
    // scroll it back into view (#3: focusing a stranded column reveals it).
    let mut space = space_with_columns(3);
    let active = space.view.active_column;
    let fitted = space.r().view_pos();
    // Pan far away so the active column is off-screen.
    let pan = -space.view.area.size.w * 10.0;
    space.m().scroll_view(pan);
    assert!(
        (space.r().view_pos() - fitted).abs() > 1.0,
        "precondition: the view has moved away from the active column"
    );
    // Re-activating the same column re-fits it.
    space.m().activate_column(active);
    assert!(
        space.view.offset.is_static(),
        "ensure-visible snaps the view statically"
    );
}

/// **Splitting a column a third time gives the new pane a real pane's worth of room.**
///
/// Drag a divider and both panes carry the height you dragged them to, which between them is the
/// whole column. Split again and there is nothing left — so the newcomer used to be assigned a
/// single pixel, and the pass that scales everything to fit took barely one per cent off the
/// other two. The pane existed, in the column, and could not be seen: what that looks like is
/// "the third pane went off the screen" (Antonio, driving, 2026-09-03/04).
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

/// **Changing focus does not make the view jump — it animates.** Moving the active column changes
/// the origin `view_pos` is measured from, so the offset has to shift by exactly that much or the
/// strip would snap on the first frame and only then slide.
///
/// The columns are wider than the window, so the target is far away and the move really animates
/// (a target within a pixel is applied at once, which hides the shift).
#[test]
fn changing_the_active_column_leaves_the_view_where_it_was_until_it_animates() {
    let mut space = test_scrolling_space();
    for id in 1..=3 {
        space
            .m()
            .add_column(None, test_column(id, ColumnWidth::Fixed(800.0)), true);
    }
    space.m().activate_column(0);
    let before = space.r().view_pos();

    space.m().activate_column(2);

    assert!(
        !space.view.offset.is_static(),
        "precondition: the far column is reached by an animation",
    );
    assert!(
        (space.r().view_pos() - before).abs() < 1.0,
        "the view started from {before}, and first drew at {}",
        space.r().view_pos(),
    );
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
