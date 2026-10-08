//! The focus door: who holds the keyboard, and the ways it moves.
//!
//! One meaning of `focused` — the widget the keyboard is aimed at, one per tree — written only by
//! `Base::focus` / `Base::blur` and the signal a surface follows. These tests state the rules in the authoring API a plugin would
//! use: place widgets, click them, hand a surface a signal to follow.

mod common;

use common::press_at;
use heca_grid_ui::prelude::*;
use heca_grid_ui::reactive::{SignalGet, SignalUpdate};
use heca_grid_ui::{FocusManager, LayoutEngine, Point, Size, holds_keyboard};
use std::cell::Cell;
use std::rc::Rc;

fn centre(root: &dyn Component, child: usize) -> Point {
    let b = root.base().children[child].base().bounds;
    Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0)
}

fn enter(root: &mut dyn Component) {
    heca_grid_ui::dispatch(
        root,
        &Event::Key {
            key: GridKey::Enter,
            pressed: true,
        },
    );
}

/// Two buttons that count their own activations: `(row, a_count, b_count)`.
fn two_buttons() -> (Flex, Rc<Cell<u32>>, Rc<Cell<u32>>) {
    let (a, b) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (sa, sb) = (a.clone(), b.clone());
    let mut ui = Flex::row()
        .child(Button::primary("A").on_click(move || sa.set(sa.get() + 1)))
        .child(Button::secondary("B").on_click(move || sb.set(sb.get() + 1)));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));
    (ui, a, b)
}

/// **There is one holder per tree.** Focusing a widget lets go of the one that had it, wherever each
/// sits — never two widgets both "focused" and a tiebreak between them.
#[test]
fn focusing_a_widget_lets_go_of_the_one_that_had_the_keyboard() {
    let (mut ui, a, b) = two_buttons();

    ui.base().children[0].base().focus(false);
    ui.base().children[1].base().focus(false);
    enter(&mut ui);
    assert_eq!((a.get(), b.get()), (0, 1), "B was focused last");
    assert!(!ui.base().children[0].base().is_focused(), "and A let go");

    ui.base().children[0].base().focus(false);
    enter(&mut ui);
    assert_eq!(
        (a.get(), b.get()),
        (1, 1),
        "A was focused last, though B is the later sibling"
    );
    assert!(!ui.base().children[1].base().is_focused(), "and B let go");
}

/// **A press elsewhere takes the keyboard from anything**, a surface a host decides included. The
/// surface's own signal is the host's and is not touched — it just no longer owns the keys.
#[test]
fn a_press_elsewhere_takes_the_keyboard_from_a_followed_surface() {
    let open = signal(true);
    let count = Rc::new(Cell::new(0));
    let sink = count.clone();
    let mut ui = Flex::row()
        .child(Item::new("surface").on_activate(|| {}))
        .child(Button::primary("B").on_click(move || sink.set(sink.get() + 1)));
    ui.base_mut().children[0].base_mut().follow_focus(open);
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));
    assert!(holds_keyboard(&ui), "open, so it holds the keyboard");

    let at = centre(&ui, 1);
    press_at(&mut ui, at, PointerButton::Left);
    enter(&mut ui);

    assert_eq!(count.get(), 1, "the button clicked owns the keys");
    assert!(
        !ui.base().children[0].base().is_focused(),
        "the surface let go"
    );
    assert!(
        open.get_untracked(),
        "but its signal is the host's, untouched"
    );
}

/// **A modal gives the keyboard back to its opener.** Click a control, a surface opens over it and
/// takes the keys, and when it closes they return to the control — the browser's "restore focus to
/// the element that opened the dialog".
#[test]
fn a_modal_returns_the_keyboard_to_the_widget_that_held_it_when_it_opened() {
    let open = signal(false);
    let (b, layer) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (sb, sl) = (b.clone(), layer.clone());
    let mut ui = Flex::row()
        .child(Button::primary("B").on_click(move || sb.set(sb.get() + 1)))
        .child(Item::new("layer").on_activate(move || sl.set(sl.get() + 1)));
    ui.base_mut().children[1]
        .base_mut()
        .follow_focus_modal(open);
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));

    let at = centre(&ui, 0);
    press_at(&mut ui, at, PointerButton::Left);
    open.set(true);
    enter(&mut ui);
    assert_eq!((b.get(), layer.get()), (0, 1), "the layer, not the button");

    open.set(false);
    enter(&mut ui);
    assert_eq!((b.get(), layer.get()), (1, 1), "closed: back to the button");
}

/// A modal that closes **after the user clicked elsewhere** does not snatch the keyboard back: the
/// opener is only restored if the modal still had the keyboard to give.
#[test]
fn a_modal_that_lost_the_keyboard_does_not_take_it_back_on_close() {
    let open = signal(false);
    let (a, b) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (sa, sb) = (a.clone(), b.clone());
    let mut ui = Flex::row()
        .child(Button::primary("A").on_click(move || sa.set(sa.get() + 1)))
        .child(Button::primary("B").on_click(move || sb.set(sb.get() + 1)))
        .child(Item::new("layer").on_activate(|| {}));
    ui.base_mut().children[2]
        .base_mut()
        .follow_focus_modal(open);
    LayoutEngine::new().compute(&mut ui, Size::new(600.0, 100.0));

    let at = centre(&ui, 0);
    press_at(&mut ui, at, PointerButton::Left); // A holds it
    open.set(true);
    heca_grid_ui::settle_focus(&mut ui); // the layer takes it, remembering A
    let at = centre(&ui, 1);
    press_at(&mut ui, at, PointerButton::Left); // the user clicks B anyway
    open.set(false);
    enter(&mut ui);

    assert_eq!((a.get(), b.get()), (0, 1), "B keeps the keyboard");
}

/// The opener is given the keyboard back only when the surface **had it to give**: if the keyboard
/// was taken off the surface and left nowhere, closing it does not resurrect the opener.
#[test]
fn a_modal_that_no_longer_holds_the_keyboard_does_not_hand_it_to_the_opener() {
    let open = signal(false);
    let a = Rc::new(Cell::new(0));
    let sa = a.clone();
    let mut ui = Flex::row()
        .child(Button::primary("A").on_click(move || sa.set(sa.get() + 1)))
        .child(Item::new("layer").on_activate(|| {}));
    ui.base_mut().children[1]
        .base_mut()
        .follow_focus_modal(open);
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));

    let at = centre(&ui, 0);
    press_at(&mut ui, at, PointerButton::Left); // A holds it
    open.set(true);
    heca_grid_ui::settle_focus(&mut ui); // the layer takes it, remembering A
    ui.base().children[1].base().blur(); // …and the keyboard is taken off it, to nobody
    open.set(false);
    enter(&mut ui);

    assert_eq!(
        a.get(),
        0,
        "A is not woken by a surface that was not holding it"
    );
}

/// **A region told it holds the keyboard does not steal it from what is inside it.** The user
/// clicked a control in the dock and the host then marks the dock as the keyboard's: the keys stay
/// with the control.
#[test]
fn a_region_that_is_told_it_holds_the_keyboard_leaves_it_with_the_control_inside() {
    let dock = signal(false);
    let count = Rc::new(Cell::new(0));
    let sink = count.clone();
    let mut ui = Flex::column().child(
        Flex::row().child(Button::primary("inside").on_click(move || sink.set(sink.get() + 1))),
    );
    ui.base_mut().children[0].base_mut().follow_focus(dock);
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));

    let inside = ui.base().children[0].base().children[0].base().bounds;
    let at = Point::new(
        inside.loc.x + inside.size.w / 2.0,
        inside.loc.y + inside.size.h / 2.0,
    );
    press_at(&mut ui, at, PointerButton::Left);
    dock.set(true);
    enter(&mut ui);

    assert_eq!(count.get(), 1, "the control inside still owns the key");
}

/// **A region that lets go and takes it again goes back to where it was inside.** Leave the dock
/// and return, and you are in the terminal you were in, not on the dock's frame.
#[test]
fn a_region_returns_the_keyboard_to_the_control_it_held_when_it_last_let_go() {
    let dock = signal(true);
    let count = Rc::new(Cell::new(0));
    let sink = count.clone();
    let mut ui = Flex::column().child(
        Flex::row().child(Button::primary("inside").on_click(move || sink.set(sink.get() + 1))),
    );
    ui.base_mut().children[0].base_mut().follow_focus(dock);
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));
    let inside = ui.base().children[0].base().children[0].base().bounds;
    let at = Point::new(
        inside.loc.x + inside.size.w / 2.0,
        inside.loc.y + inside.size.h / 2.0,
    );
    press_at(&mut ui, at, PointerButton::Left);

    dock.set(false);
    enter(&mut ui);
    assert_eq!(count.get(), 0, "left the dock: the keyboard is nobody's");

    dock.set(true);
    enter(&mut ui);
    assert_eq!(count.get(), 1, "back in the dock: back on the control");
}

/// **A press inside the widget that holds the keyboard keeps it there.** A container that can hold
/// the keyboard itself (a card grid, an open dialog) is held; a press on a control inside it moves
/// the keyboard to the control, which is the deepest owner.
#[test]
fn a_press_inside_a_focused_container_focuses_the_control_inside() {
    let count = Rc::new(Cell::new(0));
    let sink = count.clone();
    let mut ui = Flex::column().child(
        Flex::row().child(Button::primary("inside").on_click(move || sink.set(sink.get() + 1))),
    );
    // A container that can hold the keyboard itself: focusable, and not a single click target, so a
    // press inside it lands on what is inside.
    ui.base_mut().children[0].base_mut().focusable = true;
    ui.base().children[0].base().focus(false);
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));
    assert!(holds_keyboard(&ui));

    let inside = ui.base().children[0].base().children[0].base().bounds;
    let at = Point::new(
        inside.loc.x + inside.size.w / 2.0,
        inside.loc.y + inside.size.h / 2.0,
    );
    press_at(&mut ui, at, PointerButton::Left);
    enter(&mut ui);

    assert_eq!(count.get(), 1, "the control inside owns the key");
}

/// A press on the widget that already owns the keyboard is not a focus change, so nothing is told
/// it gained focus a second time (an `Input` selecting its text on focus would otherwise re-select
/// on every click inside it).
#[test]
fn a_press_on_the_widget_that_holds_the_keyboard_does_not_refocus_it() {
    let gained = Rc::new(Cell::new(0));
    let sink = gained.clone();
    let mut ui =
        Flex::row().child(Button::primary("A").on_focus_gained(move |_| sink.set(sink.get() + 1)));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));
    let at = centre(&ui, 0);

    press_at(&mut ui, at, PointerButton::Left);
    press_at(&mut ui, at, PointerButton::Left);

    assert_eq!(gained.get(), 1, "focused once, however often it is clicked");
}

/// **Tab continues from the widget that was clicked** — the manager keeps no position of its own, so
/// a click and the keyboard can never disagree about where focus is.
#[test]
fn tab_continues_from_the_widget_that_was_clicked() {
    let mut ui = Flex::row()
        .child(Button::primary("A"))
        .child(Button::primary("B"))
        .child(Button::primary("C"));
    LayoutEngine::new().compute(&mut ui, Size::new(600.0, 100.0));
    let mut focus = FocusManager::new();

    let at = centre(&ui, 1);
    press_at(&mut ui, at, PointerButton::Left);
    focus.advance(&mut ui, true);

    assert_eq!(focus.focused(&mut ui), Some(2), "after B comes C, not A");
}

/// A widget placed in a tree is focusable by being focusable — nothing is declared on it for a click
/// to reach the keyboard.
#[test]
fn a_clicked_row_with_no_wiring_holds_the_keyboard() {
    let mut ui = Flex::column().child(Row::new().child(Label::new("row")).on_activate(|| {}));
    LayoutEngine::new().compute(&mut ui, Size::new(200.0, 80.0));
    assert!(!holds_keyboard(&ui));
    let at = centre(&ui, 0);
    press_at(&mut ui, at, PointerButton::Left);
    assert!(holds_keyboard(&ui));
}
