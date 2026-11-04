//! Opening under a stationary pointer must preserve keyboard selection.

use super::*;
use crate::{LayoutEngine, Scene, Theme};
use std::rc::Rc;

#[test]
fn hover_refresh_does_not_change_the_initial_or_keyboard_selected_command() {
    let chosen = Rc::new(Cell::new(None));
    let mut palette = CommandPalette::new();
    for (index, label) in ["First", "Second", "Third"].into_iter().enumerate() {
        let chosen = chosen.clone();
        palette = palette.command(Command::new(label, move || chosen.set(Some(index))).preselect(index == 1));
    }
    let mut palette = palette.default_open(true);
    LayoutEngine::new().compute(&mut palette, Size::new(900.0, 700.0));
    let mut scene = Scene::new();
    palette.paint(&mut PaintCx::new(&mut scene, &Theme::default()).with_viewport(Size::new(900.0, 700.0)));
    let results = palette.results();
    let (panel, _, top) = palette.layout(&results);
    let (_, row) = palette.row_rects(&results, panel, top)[0];
    let at = Point::new(row.loc.x + row.size.w / 2.0, row.loc.y + row.size.h / 2.0);
    assert_eq!(palette.selected, 1);
    crate::dispatch(&mut palette, &Event::pointer_rechecked(at));
    assert_eq!(palette.selected, 1, "mounting under the pointer must not override preselection");
    crate::dispatch(&mut palette, &Event::Widget(WidgetIntent::MenuDown));
    assert_eq!(palette.selected, 2);
    crate::dispatch(&mut palette, &Event::pointer_rechecked(at));
    assert_eq!(palette.selected, 2, "a hover refresh must not override keyboard navigation");
    crate::dispatch(&mut palette, &Event::pointer_moved(at));
    assert_eq!(palette.selected, 0, "real pointer motion still selects a row");
    crate::dispatch(&mut palette, &Event::pointer_pressed(at, crate::PointerButton::Left));
    assert_eq!(chosen.get(), Some(0));
}
