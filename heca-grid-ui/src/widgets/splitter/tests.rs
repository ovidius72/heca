use std::cell::RefCell;
use std::rc::Rc;

use super::Splitter;
use crate::builders::{ComponentExt, LayoutExt, Parent};
use crate::component::dispatch;
use crate::cursor::{Cursor, cursor_at};
use crate::event::{Event, PointerButton};
use crate::style::Spacing;
use crate::widgets::Flex;
use crate::{LayoutEngine, Point, Size};

fn recording() -> (Splitter, Rc<RefCell<Vec<f32>>>) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    (
        Splitter::vertical().on_resize(move |d| sink.borrow_mut().push(d)),
        seen,
    )
}

/// A 300 x 100 row: a splitter 8 wide at x 100, with a 100 x 100 float laid over x 0..100.
fn window(splitter: Splitter, float_over: bool) -> Flex {
    let mut root = Flex::row()
        .width(300.0)
        .height(100.0)
        .child(splitter.at_rect(100.0, 0.0, 8.0, 100.0));
    if float_over {
        root = root.child(Flex::row().at_rect(90.0, 0.0, 20.0, 100.0).on_click(|_| {}));
    }
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));
    root
}

fn press(root: &mut Flex, x: f64) {
    dispatch(
        root,
        &Event::pointer_pressed(Point::new(x, 50.0), PointerButton::Left),
    );
}
fn move_to(root: &mut Flex, x: f64) {
    dispatch(root, &Event::pointer_moved(Point::new(x, 50.0)));
}
fn release(root: &mut Flex, x: f64) {
    dispatch(
        root,
        &Event::pointer_released(Point::new(x, 50.0), PointerButton::Left),
    );
}

/// **A drag tells the owner how far the edge moved, move by move, and ends with the button** —
/// wherever the pointer has gone since the press.
#[test]
fn dragging_a_splitter_reports_each_distance_until_the_button_is_released() {
    let (splitter, seen) = recording();
    let mut root = window(splitter, false);
    press(&mut root, 104.0);
    move_to(&mut root, 110.0);
    move_to(&mut root, 150.0); // far outside the zone: still the same drag
    release(&mut root, 150.0);
    move_to(&mut root, 200.0); // after the release: nothing
    assert_eq!(*seen.borrow(), vec![6.0, 40.0]);
}

/// **The grab zone reaches past the box by a step of the spacing scale, and no further** — and a
/// wider step reaches further. It is font-relative, so it is the same at any zoom.
#[test]
fn the_grab_zone_is_the_box_and_a_spacing_step_across_the_edge() {
    let grabbed = |splitter: Splitter, at: f64| {
        let (splitter, seen) = {
            let seen = Rc::new(RefCell::new(Vec::new()));
            let sink = seen.clone();
            (splitter.on_resize(move |d| sink.borrow_mut().push(d)), seen)
        };
        let mut root = window(splitter, false);
        press(&mut root, at);
        move_to(&mut root, at + 4.0);
        release(&mut root, at + 4.0);
        !seen.borrow().is_empty()
    };
    assert!(
        grabbed(Splitter::vertical(), 98.0),
        "just outside the box, in the margin"
    );
    assert!(!grabbed(Splitter::vertical(), 90.0), "well outside");
    assert!(
        grabbed(Splitter::vertical().grab(Spacing::Lg), 90.0),
        "a wider step reaches it"
    );
}

/// **A widget laid over the edge covers it**: a press there is the widget's, not the splitter's,
/// and the cursor is the widget's — the defect a hit test kept by hand could not avoid.
#[test]
fn a_widget_over_the_edge_is_not_grabbed_through() {
    let (splitter, seen) = recording();
    let mut root = window(splitter, true);
    press(&mut root, 100.0);
    move_to(&mut root, 120.0);
    release(&mut root, 120.0);
    assert!(seen.borrow().is_empty(), "the float took the press");
    assert_eq!(cursor_at(&root, Point::new(100.0, 50.0)), Cursor::Default);
}

/// The edge shows the resize cursor for the way it moves.
#[test]
fn the_cursor_over_an_edge_is_the_resize_cursor_for_its_axis() {
    let (splitter, _) = recording();
    let root = window(splitter, false);
    assert_eq!(
        cursor_at(&root, Point::new(104.0, 50.0)),
        Cursor::ResizeHorizontal
    );
    let mut stacked = Flex::column()
        .width(100.0)
        .height(300.0)
        .child(Splitter::horizontal().at_rect(0.0, 100.0, 100.0, 8.0));
    LayoutEngine::new().compute(&mut stacked, Size::new(100.0, 300.0));
    assert_eq!(
        cursor_at(&stacked, Point::new(50.0, 104.0)),
        Cursor::ResizeVertical
    );
}
