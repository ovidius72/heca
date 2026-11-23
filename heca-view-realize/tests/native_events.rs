//! A described node reaches every native event, by the same name.

use heca_grid_ui::{Event, LayoutEngine, PointerButton, Size, Theme};
use heca_view::build::{self as view, Style as _};
use heca_view::{Intent, ViewEvent, ViewNode};
use heca_view_realize::{FormBindings, IntentEmitter, realize};
use std::{cell::RefCell, rc::Rc};

/// **A node binds `right_click` and a right click sends its intent** — the line a plugin writes,
/// realized as the native listener with nothing per widget kind.
#[test]
fn a_described_right_click_sends_its_intent() {
    let node: ViewNode = view::Label::new("zsh")
        .width(120.0)
        .height(30.0)
        .on(ViewEvent::RightClick, Intent::new("docker.menu"))
        .into();
    let sent = Rc::new(RefCell::new(Vec::new()));
    let seen = sent.clone();
    let emit: IntentEmitter = Rc::new(move |intent| seen.borrow_mut().push(intent.action));
    let mut realized = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    )
    .expect("a label realizes");
    LayoutEngine::new().compute(realized.as_mut(), Size::new(200.0, 60.0));

    let b = realized.base().bounds;
    let at = heca_grid_ui::Point::new(b.loc.x + 10.0, b.loc.y + 10.0);
    let _ = heca_grid_ui::dispatch(
        realized.as_mut(),
        &Event::pointer_pressed(at, PointerButton::Right),
    );
    let _ = heca_grid_ui::dispatch(
        realized.as_mut(),
        &Event::pointer_released(at, PointerButton::Right),
    );
    assert_eq!(*sent.borrow(), ["docker.menu"]);
}

/// What happens for each native event a description can name — enough of it to be delivered.
fn sample(kind: heca_grid_ui::EventKind) -> Event {
    use heca_grid_ui::EventKind as K;
    use heca_grid_ui::event::{DragEvent, Modifiers, PointerEvent};
    let p = PointerEvent::at(heca_grid_ui::Point::new(1.0, 1.0));
    let drag = || DragEvent {
        item: "x".into(),
        pos: p.pos,
        modifiers: Modifiers::default(),
        side: heca_grid_ui::DropSide::Onto,
    };
    match kind {
        K::DoubleClick => Event::DoubleClick(p),
        K::TripleClick => Event::TripleClick(p),
        K::RightClick => Event::RightClick(p),
        K::MiddleClick => Event::MiddleClick(p),
        K::PointerEnter => Event::PointerEnter(p),
        K::PointerLeave => Event::PointerLeave(p),
        K::Focus => Event::Focus,
        K::Blur => Event::Blur,
        K::Key => Event::Key {
            key: heca_grid_ui::GridKey::Char('x'),
            pressed: true,
        },
        K::Scroll => Event::Scroll(p),
        K::EdgeScroll => Event::EdgeScroll(p),
        K::DragStart => Event::DragStart(drag()),
        K::Drag => Event::Drag(drag()),
        K::DragEnd => Event::DragEnd(drag()),
        K::DragEnter => Event::DragEnter(drag()),
        K::DragOver => Event::DragOver(drag()),
        K::DragLeave => Event::DragLeave(drag()),
        K::Drop => Event::Drop(drag()),
        other => unreachable!("{other:?} is no described event: add its sample here"),
    }
}

/// **Every native event a description can name reaches the intent bound to it** — not only the
/// table read both ways, but the event happening and the intent arriving, on a node with no event
/// of its own. A name added to the table without reaching its listener fails here.
#[test]
fn every_native_event_reaches_the_intent_bound_to_its_name() {
    for event in ViewEvent::ALL {
        let Some(kind) = heca_view_realize::events::native(*event) else {
            continue;
        };
        let node: ViewNode = view::Label::new("x")
            .width(100.0)
            .height(30.0)
            .on(*event, Intent::new(event.name()))
            .into();
        let sent = Rc::new(RefCell::new(Vec::new()));
        let seen = sent.clone();
        let emit: IntentEmitter = Rc::new(move |intent| seen.borrow_mut().push(intent.action));
        let mut realized = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        )
        .expect("a label realizes");
        LayoutEngine::new().compute(realized.as_mut(), Size::new(200.0, 60.0));
        // A key goes to whoever holds the keyboard, as in a browser.
        realized.base().focus(false);
        let _ = heca_grid_ui::deliver(realized.as_mut(), &sample(kind));
        assert!(
            sent.borrow().iter().any(|a| a == event.name()),
            "`{}` bound on a node never fired when {kind:?} happened (got {:?})",
            event.name(),
            sent.borrow(),
        );
    }
}

/// **A name the table does not have is ignored**, never a panic: a plugin ships its description as
/// data, and a typo must not take the window down.
#[test]
fn an_unknown_event_name_is_ignored() {
    // The typed builder cannot misspell an event; a description that arrives as data can, and
    // lands on the node with the name it was given.
    let node = ViewNode::from(view::Label::new("x")).on("not_an_event", Intent::new("x"));
    let sent = Rc::new(RefCell::new(Vec::new()));
    let seen = sent.clone();
    let emit: IntentEmitter = Rc::new(move |intent| seen.borrow_mut().push(intent.action));
    let mut realized = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    )
    .expect("a label with an unknown event still realizes");
    LayoutEngine::new().compute(realized.as_mut(), Size::new(200.0, 60.0));
    for kind in [
        heca_grid_ui::EventKind::Focus,
        heca_grid_ui::EventKind::RightClick,
    ] {
        let _ = heca_grid_ui::deliver(realized.as_mut(), &sample(kind));
    }
    assert!(
        sent.borrow().is_empty(),
        "nothing is bound, nothing is sent"
    );
}
