use super::*;
use crate::Size;
use crate::builders::{ComponentExt, LayoutExt, Parent};
use crate::layout::LayoutEngine;
use crate::widgets::Flex;

fn laid_out(mut root: Flex) -> Flex {
    LayoutEngine::new().compute(&mut root, Size::new(400.0, 300.0));
    root
}

/// **The nearest widget that declared a cursor decides, and a widget on top answers for itself.**
/// A cursor declared on a container reaches what it holds; a child that declared its own beats it;
/// a widget laid over both is what the pointer is over, so its answer is the one given.
#[test]
fn the_nearest_declared_cursor_over_the_pointer_wins() {
    let root = laid_out(
        Flex::column()
            .width(400.0)
            .height(300.0)
            .cursor(Cursor::Text)
            .child(Flex::row().width(100.0).height(100.0))
            .child(
                Flex::row()
                    .width(100.0)
                    .height(100.0)
                    .cursor(Cursor::Pointer),
            )
            .child(
                Flex::row()
                    .at_rect(0.0, 0.0, 50.0, 50.0)
                    .cursor(Cursor::ResizeHorizontal),
            ),
    );
    assert_eq!(
        cursor_at(&root, Point::new(80.0, 80.0)),
        Cursor::Text,
        "the container's"
    );
    assert_eq!(
        cursor_at(&root, Point::new(80.0, 180.0)),
        Cursor::Pointer,
        "the child's own"
    );
    assert_eq!(
        cursor_at(&root, Point::new(10.0, 10.0)),
        Cursor::ResizeHorizontal,
        "what lies over them is what the pointer is over"
    );
}

/// With nothing declared, the pointer is an arrow, and over a widget that can be picked up it is a
/// hand.
#[test]
fn undeclared_is_an_arrow_and_a_draggable_is_a_hand() {
    let root = laid_out(
        Flex::column()
            .width(400.0)
            .height(300.0)
            .child(
                Flex::row()
                    .key("card")
                    .width(100.0)
                    .height(100.0)
                    .draggable(),
            )
            .child(Flex::row().width(100.0).height(100.0)),
    );
    assert_eq!(cursor_at(&root, Point::new(50.0, 50.0)), Cursor::Grab);
    assert_eq!(cursor_at(&root, Point::new(50.0, 150.0)), Cursor::Default);
}
