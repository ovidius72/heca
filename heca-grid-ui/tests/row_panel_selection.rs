//! A row selected the way an overlay panel's list does it: accent fill and edge at the panel-row
//! strengths, a short bar on the left, and the one an enclosing scroll region follows.

mod common;

use heca_grid_ui::widgets::{Label, Row};
use heca_grid_ui::{Component, LayoutEngine, Parent, Size, Theme};

fn painted(row: &mut Row) -> Vec<heca_grid_ui::scene::RectCmd> {
    LayoutEngine::new().compute(row, Size::new(300.0, 100.0));
    common::rects(&common::paint_via_child(row, &Theme::default()))
}

#[test]
fn the_panel_selected_row_draws_the_panel_row_fill_edge_and_bar() {
    let theme = Theme::default();
    let ia = theme.colors.interaction;
    let accent = theme.colors.accent;
    let mut row = Row::new()
        .panel_selection(true)
        .active(true)
        .child(Label::new("x"));
    let rects = painted(&mut row);
    let fill = rects
        .iter()
        .find(|r| r.fill == accent.with_alpha(ia.panel_row_fill))
        .expect("accent fill");
    assert_eq!(
        fill.border.map(|b| b.color),
        Some(accent.with_alpha(ia.panel_row_border))
    );
    assert_eq!(fill.rect, row.base().bounds, "over the row's whole box");
    let bar = rects
        .iter()
        .find(|r| r.fill == accent && r.rect.size.w < 4.0)
        .expect("the left bar");
    assert_eq!(bar.rect.loc.x, row.base().bounds.loc.x);
    assert!(bar.rect.size.h < row.base().bounds.size.h);
}

#[test]
fn an_unselected_panel_row_draws_no_selection_and_only_the_selected_one_asks_to_be_seen() {
    let mut idle = Row::new().panel_selection(true).child(Label::new("x"));
    let rects = painted(&mut idle);
    assert!(
        rects.iter().all(|r| r.border.is_none()),
        "nothing selected, nothing drawn"
    );
    assert!(!idle.wants_visible());
    let lit = Row::new()
        .panel_selection(true)
        .active(true)
        .child(Label::new("x"));
    assert!(
        lit.wants_visible(),
        "the selection is the cursor a scroll region follows"
    );
    let plain = Row::new().active(true).child(Label::new("x"));
    assert!(
        !plain.wants_visible(),
        "an ordinary active row is not a cursor"
    );
}
