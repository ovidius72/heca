//! **One click path.** A control is acted on by one thing — a real `Click` delivered to it — and
//! everything that can press one (the mouse, Enter, Space, a letter pick, a dialog, a collapsed
//! row) reaches the handlers wired with `.on_click(|ev| ..)`.
//!
//! These are written the way an author writes: wire the event form, then press the control any way
//! there is.

mod common;

use common::{click_at, give_keyboard};
use heca_grid_ui::menu::install_menu_sink;
use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{
    BadgeButton, ButtonGroup, Choice, Dialog, IconButton, Item, Toast,
};
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

fn key(root: &mut dyn Component, key: GridKey) -> Handled {
    heca_grid_ui::dispatch(root, &Event::Key { key, pressed: true })
}

fn lay(root: &mut dyn Component) {
    LayoutEngine::new().compute(root, Size::new(400.0, 200.0));
}

/// Every control that composes its content, each wired the **event** way and nothing else.
fn controls(seen: &Seen) -> Vec<(&'static str, Box<dyn Component>)> {
    vec![
        ("Button", Box::new(Button::new("Go").on_click(recorder(seen)))),
        (
            "IconButton",
            Box::new(IconButton::new(Icon::new(Glyph::Plus)).on_click(recorder(seen))),
        ),
        ("BadgeButton", Box::new(BadgeButton::new("Go").on_click(recorder(seen)))),
        ("Row", Box::new(Row::new().child(Label::new("Go")).on_click(recorder(seen)))),
        ("Item", Box::new(Item::new("Go").on_click(recorder(seen)))),
        ("Choice", Box::new(Choice::labeled("go", "Go").on_click(recorder(seen)))),
        (
            "RailCell",
            Box::new(RailCell::new(Icon::new(Glyph::Plus)).on_click(recorder(seen))),
        ),
        ("Toast", Box::new(Toast::info("Go").on_click(recorder(seen)))),
    ]
}

/// **Enter and Space on a focused control are a real `Click`**, delivered to the handler wired
/// with `.on_click(|ev| ..)` — the browser's `<button>`. Not one of the eight has a private action
/// any more, so this is the only way Enter can reach a handler; if it did nothing, the keyboard
/// would be dead on all of them.
#[test]
fn enter_and_space_on_a_focused_control_deliver_a_click_to_its_handler() {
    for (label, pressed) in [("Enter", GridKey::Enter), ("Space", GridKey::Space)] {
        let log = seen();
        for (name, mut control) in controls(&log) {
            log.borrow_mut().clear();
            lay(control.as_mut());
            give_keyboard(control.as_mut());
            key(control.as_mut(), pressed);
            assert_eq!(
                *log.borrow(),
                vec![EventKind::Click],
                "{label} on a focused {name} must deliver exactly one Click to its on_click handler"
            );
        }
    }
}

/// **A letter that is not Enter or Space does nothing**, and a control that is not enabled does
/// nothing for either.
#[test]
fn other_keys_and_disabled_controls_deliver_nothing() {
    let log = seen();
    for (name, mut control) in controls(&log) {
        lay(control.as_mut());
        give_keyboard(control.as_mut());
        key(control.as_mut(), GridKey::Char('x'));
        assert!(log.borrow().is_empty(), "a letter is not a click on a {name}");
    }
    let log = seen();
    for (name, control) in controls(&log) {
        let mut control = control;
        control.base().disabled.set(true);
        lay(control.as_mut());
        give_keyboard(control.as_mut());
        key(control.as_mut(), GridKey::Enter);
        assert!(log.borrow().is_empty(), "a disabled {name} answers nothing");
    }
}

/// The mouse reaches the same handler, as the same event.
#[test]
fn the_pointer_reaches_the_same_handlers() {
    let log = seen();
    for (name, mut control) in controls(&log) {
        log.borrow_mut().clear();
        lay(control.as_mut());
        let b = control.base().bounds;
        click_at(
            control.as_mut(),
            Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0),
            PointerButton::Left,
        );
        assert_eq!(*log.borrow(), vec![EventKind::Click], "a click on a {name}");
    }
}

/// **Wiring the event is what makes each of them a control** — focusable, and (a toast card
/// aside, whose action and × take their own presses) one click target. Declared once by each
/// widget's role; no builder sets the flags by hand.
#[test]
fn wiring_a_click_makes_every_control_focusable_and_one_click_target() {
    for (name, control) in controls(&seen()) {
        assert!(control.focusable(), "{name} is a focus stop once wired");
        assert!(control.base().activatable, "{name} wears a pick letter once wired");
        assert_eq!(
            control.base().one_click_target,
            name != "Toast",
            "{name}: one click target, unless it is a card holding controls"
        );
    }
    for plain in [
        Box::new(Row::new()) as Box<dyn Component>,
        Box::new(Item::new("x")),
        Box::new(Choice::labeled("v", "x")),
        Box::new(RailCell::new(Icon::new(Glyph::Plus))),
        Box::new(IconButton::new(Icon::new(Glyph::Plus))),
        Box::new(Toast::info("x")),
    ] {
        assert!(!plain.focusable(), "nothing wired, nothing to focus");
        assert!(!plain.base().one_click_target, "nothing wired, nothing to claim");
    }
}

/// **An icon button wired the event way is a control**: focusable, and one click target. These came
/// from its private `on_click`, which is gone — the generic one now does the same, from one place.
#[test]
fn an_icon_button_with_an_event_handler_is_focusable_and_one_click_target() {
    let plain = IconButton::new(Icon::new(Glyph::Plus));
    assert!(!plain.focusable(), "nothing wired, nothing to focus");
    let wired = IconButton::new(Icon::new(Glyph::Plus)).on_click(|_| {});
    assert!(wired.focusable());
    assert!(wired.base().one_click_target);
    assert!(wired.base().activatable, "and it wears a pick letter");
}

/// **A container that only listens is not a control.** A pane shell and a dock both put an
/// `on_click` on a big area full of other controls; wiring it must not make the area focusable or
/// make it claim the press, or the button inside would never be pressed.
#[test]
fn a_container_with_a_click_handler_still_lets_its_child_button_take_the_press() {
    let outer = seen();
    let inner = seen();
    let mut shell = Flex::column()
        .padding(20.0)
        .child(Button::new("Close").on_click(recorder(&inner)))
        .on_click(recorder(&outer));
    assert!(!shell.focusable(), "a listener is not a focus stop");
    assert!(!shell.base().one_click_target, "and does not claim the press");
    lay(&mut shell);

    let b = shell.base().children[0].base().bounds;
    click_at(
        &mut shell,
        Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0),
        PointerButton::Left,
    );
    assert_eq!(*inner.borrow(), vec![EventKind::Click], "the child button was pressed");
    assert!(outer.borrow().is_empty(), "the shell did not take the press meant for it");

    click_at(&mut shell, Point::new(2.0, 2.0), PointerButton::Left);
    assert_eq!(*outer.borrow(), vec![EventKind::Click], "its own padding still reaches the shell");
}

/// **A letter pick is the same click.** It used to be a second synthesised-click path of its own.
#[test]
fn a_pick_delivers_the_same_click() {
    let log = seen();
    let mut root = Flex::row().child(Button::new("Go").on_click(recorder(&log)));
    lay(&mut root);
    let targets = heca_grid_ui::collect_hints(&root);
    assert_eq!(targets.len(), 1, "the button is lettered for having a click");
    assert!(heca_grid_ui::fire_hint(&mut root, &targets[0].0));
    assert_eq!(*log.borrow(), vec![EventKind::Click]);
}

/// **The ⋮ row clicks the collapsed button.** There is one handler — the button's — and the menu
/// row runs it on the `Click` the pointer would have delivered.
#[test]
fn the_overflow_row_runs_the_collapsed_buttons_own_click_handler() {
    let menu: Rc<RefCell<Option<ContextMenu>>> = Rc::new(RefCell::new(None));
    let sink = menu.clone();
    install_menu_sink(move |m, _anchor, _subject| *sink.borrow_mut() = Some(m));

    let log: Rc<RefCell<Vec<(&'static str, EventKind)>>> = Rc::new(RefCell::new(Vec::new()));
    let named = |name: &'static str| {
        let log = log.clone();
        move |cx: &mut EventCx<'_>| log.borrow_mut().push((name, cx.event().kind()))
    };
    let group = ButtonGroup::new()
        .size(WidgetSize::Small)
        .child(Button::new("Split").icon(Glyph::Plus).on_click(named("split")))
        .child(Button::new("Zoom").icon(Glyph::FrameCorners).on_click(named("zoom")))
        .child(Button::new("Close").icon(Glyph::Minus).on_click(named("close")));
    // Too narrow for three icon buttons: what does not fit moves behind the ⋮.
    let mut parent = Flex::row().width(Length::Px(40.0)).child(group);
    for _ in 0..4 {
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut parent, Size::new(40.0, 60.0));
    }
    let strip = parent.base().children[0].as_ref();
    let hidden: Vec<usize> = (0..3)
        .filter(|i| strip.base().children[*i].base().style.layout.hidden)
        .collect();
    assert!(!hidden.is_empty(), "something collapsed");

    // Open the ⋮ the way a user does: click it.
    let trigger = strip.base().children.last().unwrap().base().bounds;
    let at = Point::new(trigger.loc.x + trigger.size.w / 2.0, trigger.loc.y + trigger.size.h / 2.0);
    let _ = heca_grid_ui::dispatch(&mut parent, &Event::pointer_moved(at));
    click_at(&mut parent, at, PointerButton::Left);
    let opened = menu.borrow_mut().take().expect("clicking the ⋮ opened its menu");

    let rows = opened.menu().clone().into_items();
    assert_eq!(rows.len(), hidden.len(), "one row per collapsed button");
    let names = ["split", "zoom", "close"];
    for (row, index) in rows.iter().zip(&hidden) {
        log.borrow_mut().clear();
        row.activate();
        assert_eq!(
            *log.borrow(),
            vec![(names[*index], EventKind::Click)],
            "the row for button {index} clicks that button"
        );
    }
}

/// **A dialog presses its primary button by clicking it** — Enter from a field that did not take
/// it — and Enter on a focused button presses *that* button, not the primary one.
#[test]
fn a_dialog_clicks_its_primary_button_and_enter_on_a_button_clicks_that_button() {
    let log: Rc<RefCell<Vec<(&'static str, EventKind)>>> = Rc::new(RefCell::new(Vec::new()));
    let named = |name: &'static str| {
        let log = log.clone();
        move |cx: &mut EventCx<'_>| log.borrow_mut().push((name, cx.event().kind()))
    };
    let mut dialog = Dialog::new("Rename pane")
        .body(Input::new().value("term"))
        .action(Button::new("OK").on_click(named("ok")))
        .action(Button::new("Cancel").on_click(named("cancel")))
        .default_open(true);
    lay(&mut dialog);

    heca_grid_ui::dispatch(&mut dialog, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(*log.borrow(), vec![("ok", EventKind::Click)], "the primary button was clicked");

    // Focus Cancel and press Enter: Cancel is clicked, OK is not.
    log.borrow_mut().clear();
    let cancel = heca_grid_ui::component::area(&dialog, "dialog-actions")
        .map(|row| row.base().children[1].base().focus(true));
    assert!(cancel.is_some(), "the dialog's actions row has the Cancel button");
    heca_grid_ui::dispatch(
        &mut dialog,
        &Event::Key {
            key: GridKey::Enter,
            pressed: true,
        },
    );
    assert_eq!(*log.borrow(), vec![("cancel", EventKind::Click)]);
}
