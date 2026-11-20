//! Opening under a stationary pointer must preserve keyboard selection.
//!
//! The rows note what the pointer did and the palette acts on it at its next tick, so each step
//! here dispatches the event and then lets a frame pass.

use super::tests::{row_bounds, shown};
use super::*;
use heca_core::layout::{Point, Size};
use std::rc::Rc;

#[test]
fn hover_refresh_does_not_change_the_initial_or_keyboard_selected_command() {
    let chosen = Rc::new(Cell::new(None));
    let mut palette = CommandPalette::new();
    for (index, label) in ["First", "Second", "Third"].into_iter().enumerate() {
        let chosen = chosen.clone();
        palette = palette.command(Command::new(label, move || chosen.set(Some(index))).preselect(index == 1));
    }
    let (mut palette, _) = shown(palette.default_open(true), Size::new(900.0, 700.0));
    let row = row_bounds(&mut palette, 0);
    let at = Point::new(row.loc.x + row.size.w / 2.0, row.loc.y + row.size.h / 2.0);
    let frame = |palette: &mut CommandPalette, event: &Event| {
        crate::dispatch(palette, event);
        palette.tick(0.0);
    };
    assert_eq!(palette.selected, 1);
    frame(&mut palette, &Event::pointer_rechecked(at));
    assert_eq!(palette.selected, 1, "mounting under the pointer must not override preselection");
    frame(&mut palette, &Event::Widget(WidgetIntent::MenuDown));
    assert_eq!(palette.selected, 2);
    frame(&mut palette, &Event::pointer_rechecked(at));
    assert_eq!(palette.selected, 2, "a hover refresh must not override keyboard navigation");
    frame(&mut palette, &Event::pointer_moved(at));
    assert_eq!(palette.selected, 0, "real pointer motion still selects a row");
    frame(&mut palette, &Event::pointer_pressed(at, crate::PointerButton::Left));
    assert_eq!(chosen.get(), Some(0));
}
