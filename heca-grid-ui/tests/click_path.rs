//! **One click path, held by tests**: wiring `.on_click(..)` is what makes a control a control,
//! and a container that only listens stays a container.

mod common;

use common::click_at;
use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{BadgeButton, Choice, IconButton, Item};
use heca_grid_ui::{EventKind, LayoutEngine, Point, Size};
use std::cell::RefCell;
use std::rc::Rc;

/// The kinds of event a handler was given, in order.
type Seen = Rc<RefCell<Vec<EventKind>>>;

fn seen() -> Seen {
    Rc::new(RefCell::new(Vec::new()))
}

/// A click handler that writes down what it was handed.
fn recorder(seen: &Seen) -> impl FnMut(&mut EventCx<'_>) + 'static {
    let seen = seen.clone();
    move |cx| seen.borrow_mut().push(cx.event().kind())
}

/// **Wiring a click is what makes each of them a control**: a keyboard stop, lettered by the
/// picker, and one click target — and the same widget with nothing wired is none of those. Each
/// widget's role says it once; no builder sets the flags by hand.
#[test]
fn wiring_a_click_makes_every_control_focusable_and_one_click_target() {
    let log = seen();
    let wired: Vec<(&str, Box<dyn Component>)> = vec![
        (
            "Button",
            Box::new(Button::new("Go").on_click(recorder(&log))),
        ),
        (
            "IconButton",
            Box::new(IconButton::new(Icon::new(Glyph::Plus)).on_click(recorder(&log))),
        ),
        (
            "BadgeButton",
            Box::new(BadgeButton::new("Go").on_click(recorder(&log))),
        ),
        (
            "Row",
            Box::new(Row::new().child(Label::new("Go")).on_click(recorder(&log))),
        ),
        ("Item", Box::new(Item::new("Go").on_click(recorder(&log)))),
        (
            "Choice",
            Box::new(Choice::labeled("go", "Go").on_click(recorder(&log))),
        ),
    ];
    for (name, control) in &wired {
        assert!(control.focusable(), "{name} is a keyboard stop once wired");
        assert!(
            control.base().activatable,
            "{name} wears a pick letter once wired"
        );
        assert!(
            control.base().one_click_target,
            "{name} takes the press for its whole box"
        );
    }
    let plain: Vec<(&str, Box<dyn Component>)> = vec![
        ("Row", Box::new(Row::new())),
        ("Item", Box::new(Item::new("x"))),
        (
            "IconButton",
            Box::new(IconButton::new(Icon::new(Glyph::Plus))),
        ),
    ];
    for (name, control) in &plain {
        assert!(
            !control.focusable(),
            "a {name} with nothing wired is nothing to focus"
        );
        assert!(!control.base().one_click_target, "and claims no press");
    }
}

/// **A container that only listens is not a control.** A pane shell and a dock both put an
/// `on_click` on a big area full of other controls; wiring it must not make the area focusable or
/// make it claim the press, or the button inside would never be pressed.
#[test]
fn a_container_with_a_click_handler_still_lets_its_child_button_take_the_press() {
    let outer = seen();
    let inner = seen();
    let mut shell = Flex::column()
        .padding(Spacing::Lg)
        .child(Button::new("Close").key("close").on_click(recorder(&inner)))
        .on_click(recorder(&outer));
    assert!(!shell.focusable(), "a listener is not a keyboard stop");
    assert!(
        !shell.base().one_click_target,
        "and does not claim the press"
    );
    LayoutEngine::new().compute(&mut shell, Size::new(400.0, 200.0));

    let b = heca_grid_ui::node_with_key(&shell, "close")
        .expect("the button is there")
        .base()
        .bounds;
    let centre = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    click_at(&mut shell, centre, PointerButton::Left);
    assert_eq!(
        *inner.borrow(),
        vec![EventKind::Click],
        "the button was pressed"
    );
    assert_eq!(
        *outer.borrow(),
        vec![EventKind::Click],
        "and the click bubbled to the shell after it, as a DOM listener on a container hears it",
    );

    outer.borrow_mut().clear();
    inner.borrow_mut().clear();
    let shell_box = shell.base().bounds;
    click_at(
        &mut shell,
        Point::new(shell_box.loc.x + 1.0, shell_box.loc.y + 1.0),
        PointerButton::Left,
    );
    assert!(
        inner.borrow().is_empty(),
        "the button is not pressed from the padding"
    );
    assert_eq!(
        *outer.borrow(),
        vec![EventKind::Click],
        "the shell's own padding reaches it"
    );
}
