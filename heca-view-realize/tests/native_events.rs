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
