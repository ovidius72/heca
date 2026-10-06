//! A container with nothing to answer for lets the pointer through its own box.

use crate::builders::{ComponentExt, LayoutExt, Parent};
use crate::widgets::Flex;
use crate::{Color, Component, LayoutEngine, Point, Size, hit_test};

/// A 400 x 100 row: a target on the left, and over the whole row a wrapper holding a small control
/// on the right. `wrapper` is applied to the covering wrapper.
fn row(wrapper: impl FnOnce(Flex) -> Flex) -> Flex {
    let mut root = Flex::row()
        .width(400.0)
        .height(100.0)
        .child(
            Flex::row()
                .at_rect(0.0, 0.0, 100.0, 100.0)
                .key("target")
                .on_click(|_| {}),
        )
        .child(wrapper(
            Flex::row().at_rect(0.0, 0.0, 400.0, 100.0).child(
                Flex::row()
                    .at_rect(300.0, 0.0, 50.0, 50.0)
                    .key("control")
                    .on_click(|_| {}),
            ),
        ));
    LayoutEngine::new().compute(&mut root, Size::new(400.0, 100.0));
    root
}

/// **A bare wrapper is not what the pointer lands on where it holds nothing** — the thing behind
/// it is — while what it holds still answers. Nobody sets a flag for this.
#[test]
fn a_bare_wrapper_leaves_the_pointer_to_what_is_behind_it() {
    let open = row(|w| w);
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
}

/// A wrapper that paints, or answers for itself, covers what is behind it.
#[test]
fn a_wrapper_that_paints_or_answers_covers_what_is_behind_it() {
    let painted = row(|mut w| {
        w.base_mut().style.visual.fill = Some(Color::rgb(10, 10, 10));
        w
    });
    assert_eq!(hit_test(&painted, Point::new(50.0, 50.0)), Some(vec![1]));

    let answering = row(|w| w.on_click(|_| {}));
    assert_eq!(hit_test(&answering, Point::new(50.0, 50.0)), Some(vec![1]));
}
