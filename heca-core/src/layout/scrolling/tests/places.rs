//! The places a pane can be put, and which pane focus lands on in the column beside.

use super::*;

/// A two-column space whose first column holds two panes: `[a][c]` over `[b]`.
fn stacked() -> Seen {
    let mut space = space_with_columns(2);
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(99), "b"), false);
    space
}

fn overlaps(a: Rectangle, b: Rectangle) -> bool {
    a.loc.x < b.loc.x + b.size.w
        && b.loc.x < a.loc.x + a.size.w
        && a.loc.y < b.loc.y + b.size.h
        && b.loc.y < a.loc.y + a.size.h
}

/// **A gap place is space, not an overlay**: it overlaps no pane and no column.
#[test]
fn an_open_gap_overlaps_no_pane_and_no_column() {
    let space = stacked();
    let cols = space.r().columns_with_positions();
    for place in space.r().places().into_iter().filter(|p| matches!(p.kind, PlaceKind::Gap(_))) {
        for col in &cols {
            assert!(!overlaps(place.rect, col.rect), "{place:?} over column {:?}", col.rect);
        }
    }
}

/// A gap place at each gap and both ends; a border at the top, between and bottom of every
/// column (the panes plus one), each of no thickness.
#[test]
fn places_exist_for_each_gap_and_for_every_border_of_every_column() {
    let space = stacked();
    let places = space.r().places();
    let kinds: Vec<_> = places.iter().map(|p| p.kind).collect();
    for gap in 0..=2 {
        assert!(kinds.contains(&PlaceKind::Gap(gap)));
    }
    for (col, rows) in [(0, 3), (1, 2)] {
        for row in 0..rows {
            assert!(kinds.contains(&PlaceKind::Row { col, row }), "col {col} row {row}");
        }
    }
    assert!(places
        .iter()
        .filter(|p| matches!(p.kind, PlaceKind::Row { .. }))
        .all(|p| p.rect.size.h == 0.0));
}

/// **The border between two panes is on their shared edge, whatever the gap.**
#[test]
fn a_border_sits_between_its_panes() {
    let space = stacked();
    let opened = space.r().columns_with_positions();
    let between = space
        .r()
        .places()
        .into_iter()
        .find(|p| p.kind == PlaceKind::Row { col: 0, row: 1 })
        .expect("a border between the two panes");
    let (above, below) = (opened[0].panes[0].slot, opened[0].panes[1].slot);
    assert!(between.rect.loc.y >= above.loc.y + above.size.h && between.rect.loc.y <= below.loc.y);
}

/// Reading the places moves nothing, and the place before each column is a standing line in the
/// middle of the gap, with no width.
#[test]
fn reading_the_places_moves_no_column() {
    let space = stacked();
    let closed = space.r().columns_with_positions();
    let gap = space.r().places().into_iter().find(|p| p.kind == PlaceKind::Gap(1)).map(|p| p.rect);
    let opened = space.r().columns_with_positions();
    assert_eq!(opened[0].rect, closed[0].rect);
    assert_eq!(opened[1].rect, closed[1].rect);
    let gaps = space.options.gaps;
    assert_eq!(gap.map(|r| r.size.w), Some(0.0));
    assert_eq!(gap.map(|r| r.loc.x), Some(closed[1].rect.loc.x - gaps / 2.0));
}

/// A 2 × 2 strip, `[a][c]` over `[b][d]`: columns 1 and 2, panes (1, 11) and (2, 12).
fn two_by_two(focus: ColumnFocus) -> Seen {
    let mut space = space_with_columns(2);
    space.options.column_focus = focus;
    space.m().add_pane_to_column(0, None, Pane::new(PaneId(11), "b"), false);
    space.m().add_pane_to_column(1, None, Pane::new(PaneId(12), "d"), false);
    space
}

fn focused_pane(space: &Seen) -> Option<PaneId> {
    space.r().active_pane().map(|p| p.id)
}

/// **From c, left lands on the pane level with c under `row` and on the last-used pane under
/// `last`** — with b remembered as the last used in the left column.
#[test]
fn column_focus_row_lands_level_and_last_lands_on_the_remembered_pane() {
    for (focus, want) in [(ColumnFocus::Last, PaneId(11)), (ColumnFocus::Row, PaneId(1))] {
        let mut space = two_by_two(focus);
        space.columns[0].active_pane_idx = 1; // b was the last used
        space.view.active_column = 1;
        space.columns[1].active_pane_idx = 0; // we are on c
        assert!(space.m().focus_left());
        assert_eq!(focused_pane(&space), Some(want), "{focus:?}");
    }
}

/// Under `row` the way back is level too, and a stack of different heights picks the pane with
/// the larger overlap.
#[test]
fn column_focus_row_goes_back_level_and_follows_the_larger_overlap() {
    let mut space = two_by_two(ColumnFocus::Row);
    space.view.active_column = 0;
    space.columns[0].active_pane_idx = 1; // on b
    assert!(space.m().focus_right());
    assert_eq!(focused_pane(&space), Some(PaneId(12)), "b is level with d");

    // The left column's second pane takes most of the height; from the right column's first pane
    // the larger overlap is with the left column's tall pane.
    let mut space = two_by_two(ColumnFocus::Row);
    let (h, gaps) = (space.r().area().size.h, space.options.gaps);
    space.columns[0].set_pane_height(0, 100.0, h, gaps);
    space.view.active_column = 1;
    space.columns[1].active_pane_idx = 0;
    assert!(space.m().focus_left());
    assert_eq!(focused_pane(&space), Some(PaneId(11)), "most of its span is level with b");
}

fn place(space: &Seen, kind: PlaceKind) -> Place {
    space.r().places().into_iter().find(|p| p.kind == kind).expect("the place is there")
}

fn holds(zone: Rectangle, x: f64, y: f64) -> bool {
    x >= zone.loc.x && x <= zone.loc.x + zone.size.w && y >= zone.loc.y && y <= zone.loc.y + zone.size.h
}

/// **The empty space beside the strip is the place at that end**: a drop right of the last column
/// makes a new column there, however far right it lands.
#[test]
fn the_space_after_the_last_column_belongs_to_the_last_gap() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.3)), true);
    let end = place(&space, PlaceKind::Gap(1));
    let area = space.r().area();
    let (right, middle) = (area.loc.x + area.size.w - 1.0, area.loc.y + area.size.h / 2.0);
    assert!(holds(end.zone, right, middle), "{:?} misses the far right", end.zone);
    assert!(holds(end.zone, end.rect.loc.x, middle), "and holds its own line");
    assert_eq!(end.line_share(), 0.0, "the line is at the start of the space it takes");
}

/// The top border reaches up to the area's edge and a quarter into the first pane; the bottom one
/// down to the edge and a quarter into the last. A border between two panes stays a line.
#[test]
fn the_top_and_bottom_borders_reach_the_area_edge_and_into_the_pane() {
    let space = stacked();
    let area = space.r().area();
    let first = space.r().columns_with_positions()[0].panes[0].slot;
    let top = place(&space, PlaceKind::Row { col: 0, row: 0 });
    let x = first.loc.x + first.size.w / 2.0;
    assert!(holds(top.zone, x, area.loc.y), "reaches the top of the area");
    assert!(holds(top.zone, x, first.loc.y + first.size.h * space.options.drop_edge_reach - 1.0));
    assert!(!holds(top.zone, x, first.loc.y + first.size.h / 2.0), "not half the pane");
    let bottom = place(&space, PlaceKind::Row { col: 0, row: 2 });
    assert!(holds(bottom.zone, x, area.loc.y + area.size.h), "reaches the bottom of the area");
    let between = place(&space, PlaceKind::Row { col: 0, row: 1 });
    assert_eq!(between.zone, between.rect);
}

/// **A place moves whole**: its line and its drop area together, so placing it in the window
/// cannot leave the area behind.
#[test]
fn a_place_moves_its_line_and_its_drop_area_together() {
    let space = stacked();
    let end = place(&space, PlaceKind::Gap(2));
    let moved = end.moved_by(Point::new(466.0, 90.0));
    assert_eq!(moved.rect.loc, Point::new(end.rect.loc.x + 466.0, end.rect.loc.y + 90.0));
    assert_eq!(moved.zone.loc, Point::new(end.zone.loc.x + 466.0, end.zone.loc.y + 90.0));
    assert_eq!((moved.rect.size, moved.zone.size), (end.rect.size, end.zone.size));
}
