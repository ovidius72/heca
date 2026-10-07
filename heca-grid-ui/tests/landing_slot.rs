//! A place something can land: hidden until a drag it accepts begins, outlined with its letter
//! while one is in flight, and dropped onto like any target.

mod common;

use common::{paint_via_child, press_at, release_at};
use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::LandingSlot;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, Point, Size, Theme};

fn move_to(root: &mut dyn Component, pos: Point) {
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_moved(pos));
}

/// A source of `kind` on the left and a placeholder slot that appears for `"pane"` on the right.
fn window(kind: &str) -> Flex {
    let mut root = Flex::row()
        .width(300.0)
        .height(50.0)
        .child(
            Flex::row()
                .at_rect(0.0, 0.0, 100.0, 50.0)
                .key("src")
                .draggable_as(kind),
        )
        .child(
            LandingSlot::new()
                .label("b")
                .while_dragging("pane")
                .key("slot:1")
                .at_rect(200.0, 0.0, 50.0, 50.0),
        );
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 50.0));
    root
}

fn drawn_letters(root: &Flex) -> Vec<String> {
    paint_via_child(root, &Theme::default())
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) if !t.text.is_empty() => Some(t.text.clone()),
            _ => None,
        })
        .collect()
}

fn pick_up(root: &mut Flex) {
    press_at(root, Point::new(10.0, 10.0), PointerButton::Left);
    move_to(root, Point::new(60.0, 40.0));
}

/// **The slot is not there until a drag of the kind it takes begins, and it goes when that ends** —
/// nothing hands it a flag.
#[test]
fn a_slot_appears_while_a_pane_is_carried_and_goes_when_it_ends() {
    let mut root = window("pane");
    assert!(drawn_letters(&root).is_empty(), "hidden before the drag");
    pick_up(&mut root);
    assert_eq!(drawn_letters(&root), ["b"], "its letter, while a pane is carried");
    release_at(&mut root, Point::new(60.0, 40.0), PointerButton::Left);
    assert!(drawn_letters(&root).is_empty(), "gone when it ends");
}

/// A drag of something else never shows it.
#[test]
fn a_slot_stays_hidden_for_a_drag_of_another_kind() {
    let mut root = window("column");
    pick_up(&mut root);
    assert!(drawn_letters(&root).is_empty());
}

/// Dropping onto the slot says what was dropped on it.
#[test]
fn a_drop_on_the_slot_names_it() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let sink = dropped.clone();
    heca_grid_ui::drag::install_drop_sink(move |d| {
        sink.borrow_mut().push((d.source, d.target));
    });
    let mut root = window("pane");
    pick_up(&mut root);
    let on_slot = Point::new(220.0, 20.0);
    move_to(&mut root, on_slot);
    release_at(&mut root, on_slot, PointerButton::Left);
    assert_eq!(*dropped.borrow(), [("src".to_string(), "slot:1".to_string())]);
}
