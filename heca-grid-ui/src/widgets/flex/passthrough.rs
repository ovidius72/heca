//! A wrapper that lets the pointer through its own box.

use crate::builders::{ComponentExt, LayoutExt, Parent};
use crate::widgets::Flex;
use crate::{LayoutEngine, Point, Size, hit_test};

/// A 400 x 100 row: a target on the left, and over the whole row a wrapper holding a small control
/// on the right.
fn row(passthrough: bool) -> Flex {
    let mut root = Flex::row()
        .width(400.0)
        .height(100.0)
        .child(
            Flex::row()
                .at_rect(0.0, 0.0, 100.0, 100.0)
                .key("target")
                .on_click(|_| {}),
        )
        .child(
            Flex::row()
                .at_rect(0.0, 0.0, 400.0, 100.0)
                .pointer_passthrough(passthrough)
                .child(
                    Flex::row()
                        .at_rect(300.0, 0.0, 50.0, 50.0)
                        .key("control")
                        .on_click(|_| {}),
                ),
        );
    LayoutEngine::new().compute(&mut root, Size::new(400.0, 100.0));
    root
}

/// **A wrapper that passes the pointer through is not what it lands on where it holds nothing** —
/// the thing behind it is — while what it holds still answers.
#[test]
fn a_wrapper_that_passes_through_leaves_the_pointer_to_what_is_behind_it() {
    let open = row(true);
    assert_eq!(
        hit_test(&open, Point::new(50.0, 50.0)),
        Some(vec![0]),
        "the target behind it"
    );
    assert_eq!(
        hit_test(&open, Point::new(320.0, 20.0)),
        Some(vec![1, 0]),
        "what it holds still answers"
    );

    let covering = row(false);
    assert_eq!(
        hit_test(&covering, Point::new(50.0, 50.0)),
        Some(vec![1]),
        "an ordinary wrapper covers what is behind it"
    );
}
