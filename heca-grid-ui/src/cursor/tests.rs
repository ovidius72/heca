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

/// A widget whose cursor depends on **where** the pointer is answers by point: the left half of it
/// is a link, the right half is not — and its answer beats the cursor declared on what holds it.
#[test]
fn a_widget_can_answer_the_cursor_by_point() {
    use crate::component::{Base, Component};

    struct LeftHalfIsALink {
        base: Base,
    }
    impl Component for LeftHalfIsALink {
        fn base(&self) -> &Base {
            &self.base
        }
        fn base_mut(&mut self) -> &mut Base {
            &mut self.base
        }
        fn cursor_over(&self, point: Point) -> Option<Cursor> {
            let b = self.base.bounds;
            (point.x < b.loc.x + b.size.w / 2.0).then_some(Cursor::Pointer)
        }
    }
    let mut link = LeftHalfIsALink { base: Base::new() };
    link.base.style.layout.width = crate::Length::Px(100.0);
    link.base.style.layout.height = crate::Length::Px(100.0);
    let root = laid_out(
        Flex::column()
            .width(400.0)
            .height(300.0)
            .cursor(Cursor::Text)
            .child(link),
    );
    assert_eq!(cursor_at(&root, Point::new(20.0, 50.0)), Cursor::Pointer);
    assert_eq!(
        cursor_at(&root, Point::new(80.0, 50.0)),
        Cursor::Text,
        "off the link it is the holder's"
    );
}
