//! **`.on_press(Intent)`** — a click that says what it does, on every widget.
//!
//! The click dispatches the intent through the host's sink, and a `prefix+/` pick of the same
//! widget carries the *same* intent as data, so a host can ask its policy about it before a letter
//! is spent.

mod common;

use common::{click_at, give_keyboard};
use heca_grid_ui::prelude::*;
use heca_grid_ui::intent::install_intent_sink;
use heca_grid_ui::{Hint, LayoutEngine, Point, Size, fire_hint, hint_intent};
use heca_view::Intent;
use std::cell::RefCell;
use std::rc::Rc;

type Sent = Rc<RefCell<Vec<Intent>>>;

/// Install a sink that records what the host would have been asked to do. The sink is
/// thread-local and each test runs on its own thread, so tests do not see each other's.
fn sink() -> Sent {
    let sent: Sent = Rc::new(RefCell::new(Vec::new()));
    let out = sent.clone();
    install_intent_sink(move |intent| out.borrow_mut().push(intent));
    sent
}

fn close() -> Intent {
    Intent::new("close_pane_by_id").arg("pane_id", 7_i64.into())
}

fn centre(root: &dyn Component) -> Point {
    let b = root.base().bounds;
    Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0)
}

fn lay(root: &mut dyn Component) {
    LayoutEngine::new().compute(root, Size::new(300.0, 100.0));
}

/// A click dispatches the intent — on a control, and on a plain container with no widget of its
/// own for it (any widget has `on_press`).
#[test]
fn a_click_dispatches_the_intent_on_any_widget() {
    for mut widget in [
        Box::new(IconButton::new(Icon::new(Glyph::Close)).on_press(close())) as Box<dyn Component>,
        Box::new(Button::new("Close").on_press(close())),
        Box::new(Row::new().child(Label::new("x")).on_press(close())),
        Box::new(Flex::row().child(Label::new("Close")).on_press(close())),
    ] {
        let sent = sink();
        lay(widget.as_mut());
        let at = centre(widget.as_ref());
        click_at(widget.as_mut(), at, PointerButton::Left);
        assert_eq!(*sent.borrow(), vec![close()]);
    }
}

/// Enter on the focused control is the same click, so the same intent — once.
#[test]
fn enter_on_a_focused_control_dispatches_the_intent_once() {
    let sent = sink();
    let mut button = Button::new("Close").on_press(close());
    lay(&mut button);
    give_keyboard(&mut button);
    heca_grid_ui::dispatch(
        &mut button,
        &Event::Key {
            key: GridKey::Enter,
            pressed: true,
        },
    );
    assert_eq!(*sent.borrow(), vec![close()]);
}

/// **The pick carries the same intent as the click** — as data, for the host's policy — and
/// firing it does what the click does.
#[test]
fn the_pick_carries_the_same_intent_and_fires_it() {
    let sent = sink();
    let mut root = Flex::row().child(IconButton::new(Icon::new(Glyph::Close)).on_press(close()));
    lay(&mut root);
    let targets = heca_grid_ui::collect_hints(&root);
    assert_eq!(targets.len(), 1);
    let path = &targets[0].0;
    assert_eq!(hint_intent(&root, path), Some(close()), "the pick names its act");
    assert!(fire_hint(&mut root, path));
    assert_eq!(*sent.borrow(), vec![close()]);
}

/// A later `on_hint` overrides what a pick does, and leaves the click alone.
#[test]
fn a_later_on_hint_overrides_the_pick_only() {
    let sent = sink();
    let peek = Intent::new("peek");
    let mut root = Flex::row().child(
        Row::new()
            .child(Label::new("x"))
            .on_press(close())
            .on_hint(Hint::of(peek.clone(), {
                let peek = peek.clone();
                move || heca_grid_ui::event::EventCx::new(&Event::Focus).dispatch(peek.clone())
            })),
    );
    lay(&mut root);
    let targets = heca_grid_ui::collect_hints(&root);
    let path = &targets[0].0;
    assert_eq!(hint_intent(&root, path), Some(peek.clone()), "the pick is the override");

    let at = centre(root.base().children[0].as_ref());
    click_at(&mut root, at, PointerButton::Left);
    assert_eq!(*sent.borrow(), vec![close()], "the click still does what on_press said");

    sent.borrow_mut().clear();
    assert!(fire_hint(&mut root, path));
    assert_eq!(*sent.borrow(), vec![peek], "and the pick does the override");
}

/// `on_press_to` sends to a sender of the caller's own — what a described tree uses — and the pick
/// still carries the intent.
#[test]
fn on_press_to_goes_to_the_senders_own_sink() {
    let host = sink();
    let own: Sent = Rc::new(RefCell::new(Vec::new()));
    let out = own.clone();
    let mut root = Flex::row().child(
        Button::new("Close").on_press_to(close(), move |intent| out.borrow_mut().push(intent)),
    );
    lay(&mut root);
    let at = centre(root.base().children[0].as_ref());
    click_at(&mut root, at, PointerButton::Left);
    assert_eq!(*own.borrow(), vec![close()]);
    assert!(host.borrow().is_empty(), "nothing went to the host's sink");
    let targets = heca_grid_ui::collect_hints(&root);
    assert_eq!(hint_intent(&root, &targets[0].0), Some(close()));
}
