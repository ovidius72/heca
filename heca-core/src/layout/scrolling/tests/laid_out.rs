//! Where columns and panes are laid out, and what is drawn where.

use super::*;


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

    space.m().slide_pane(PaneId(1), Point::new(400.0, 90.0), AnimationConfig::default());
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
