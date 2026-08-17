//! **Can a click on a chrome row be answered by handlers instead of two geometric hit-tests?**
//!
//! heca answers one question on every mouse press (`heca/src/mouse.rs`, `aim_keyboard_at_click`):
//! *which container should take the keyboard, and which row should its cursor move to.* It answers
//! it with two hit-tests over the retained tree — `nav::scope_at` (which needs `Base::scope_key`)
//! and `nav::key_at` — even though the chrome tree already receives the click as an event and
//! events already bubble.
//!
//! This file is the experiment, not an opinion. It builds the real shape — a `FocusScope` wrapper
//! around rows, the way `chrome/mod.rs:615` does — and records what actually happens.
//!
//! Four questions, one test each:
//!
//! 1. does a click on a row reach the row's own handler?
//! 2. does it then reach the enclosing wrapper's?
//! 3. in which order?
//! 4. does a click on the wrapper's padding reach the wrapper **only**, leaving the cursor alone —
//!    the rule `mouse.rs` states and gets today from `key_at` returning `None`?
//!
//! And the fifth, because a right-click aims the keyboard exactly as a left-click does:
//! does `on_right_click` behave the same way?

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Flex, FocusScope, Item};
use heca_grid_ui::{Event, LayoutEngine, PointerButton, Size};
use heca_core::layout::Point;
use std::cell::RefCell;
use std::rc::Rc;

type Log = Rc<RefCell<Vec<&'static str>>>;

/// The shape `chrome/mod.rs` builds: a `FocusScope` wrapper around a dock body of rows. The
/// wrapper answers "focus me", each row answers "move the cursor to me".
fn dock(log: &Log) -> Box<dyn Component> {
    let body = Flex::column()
        .width(Length::Px(200.0))
        .height(Length::Px(300.0))
        // Padding is the case that matters: a click here is inside the container and on no row.
        .padding(40.0)
        .child({
            let log = log.clone();
            Item::new("zsh")
                .key("pane:7")
                .height(Length::Px(30.0))
                .on_click(move |_| log.borrow_mut().push("row"))
        });

    let log = log.clone();
    Box::new(
        FocusScope::new(body).on_click(move |_| log.borrow_mut().push("container")),
    )
}

/// Click at `at`, through the same entry point the app uses.
fn click(host: &mut dyn Component, at: Point, button: PointerButton) {
    LayoutEngine::new().compute(host, Size::new(400.0, 400.0));
    let _ = heca_grid_ui::dispatch(host, &Event::pointer_moved(at));
    let _ = heca_grid_ui::dispatch(host, &Event::pointer_pressed(at, button));
    let _ = heca_grid_ui::dispatch(host, &Event::pointer_released(at, button));
}

/// The row sits inside the body's 40px padding, so a point just inside it is on the row.
const ON_ROW: Point = Point::new(100.0, 55.0);
/// Well inside the wrapper, above the row — the container's own padding.
const ON_PADDING: Point = Point::new(100.0, 10.0);

#[test]
fn a_click_on_a_row_reaches_the_row_and_then_the_container() {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let mut host = dock(&log);

    click(host.as_mut(), ON_ROW, PointerButton::Left);

    assert_eq!(
        *log.borrow(),
        vec!["row", "container"],
        "the row answers first, then the click bubbles to the container that holds it"
    );
}

#[test]
fn a_click_on_the_containers_padding_reaches_the_container_only() {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let mut host = dock(&log);

    click(host.as_mut(), ON_PADDING, PointerButton::Left);

    assert_eq!(
        *log.borrow(),
        vec!["container"],
        "clicking a container's padding is not a request to move its cursor (heca/src/mouse.rs)"
    );
}

#[test]
fn a_right_click_behaves_exactly_as_a_left_click_does() {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let body = Flex::column()
        .width(Length::Px(200.0))
        .height(Length::Px(300.0))
        .padding(40.0)
        .child({
            let log = log.clone();
            Item::new("zsh")
                .key("pane:7")
                .height(Length::Px(30.0))
                .on_right_click(move |_| log.borrow_mut().push("row"))
        });
    let mut host: Box<dyn Component> = Box::new(FocusScope::new(body).on_right_click({
        let log = log.clone();
        move |_| log.borrow_mut().push("container")
    }));

    click(host.as_mut(), ON_ROW, PointerButton::Right);

    assert_eq!(
        *log.borrow(),
        vec!["row", "container"],
        "a right-click aims the keyboard exactly as a left-click does"
    );
}
