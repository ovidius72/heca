//! **Every use of a widget follows one set of rules** — a click of any button, or a run of them:
//! a disabled widget is not used, one control uses each event (the innermost that listens to it),
//! and only that control shows it was pressed. Containers that are not controls still hear the
//! event as it rises.

mod common;

use common::click_at;
use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Input, Row};
use heca_grid_ui::{Base, Button, Component, LayoutEngine, PointerButton, Size};
use std::cell::RefCell;
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

/// A control that writes down when the framework says it was pressed.
struct Pressable {
    base: Base,
    name: &'static str,
    log: Log,
}

impl Pressable {
    fn new(name: &'static str, log: &Log) -> Self {
        Self { base: Base::new(), name, log: log.clone() }
    }
}

impl Component for Pressable {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn is_control(&self) -> bool {
        true
    }
    fn show_press(&mut self) {
        self.log.borrow_mut().push(format!("{} pressed", self.name));
    }
}

impl LayoutExt for Pressable {}
impl Parent for Pressable {}

fn centre_of(root: &dyn Component, key: &str) -> heca_core::layout::Point {
    let b = heca_grid_ui::node_with_key(root, key).expect("keyed").base().bounds;
    heca_core::layout::Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0)
}

fn double_click(root: &mut dyn Component, at: heca_core::layout::Point) {
    click_at(root, at, PointerButton::Left);
    click_at(root, at, PointerButton::Left);
}

fn lay(root: &mut dyn Component) {
    LayoutEngine::new().compute(root, Size::new(400.0, 200.0));
}

fn heard(log: &Log, who: &'static str) -> impl FnMut(&mut heca_grid_ui::EventCx<'_>) + 'static {
    let log = log.clone();
    move |_| log.borrow_mut().push(who.to_string())
}

#[test]
fn a_double_click_is_used_by_the_button_not_also_by_the_row_around_it() {
    let log: Log = Rc::default();
    let mut row = Row::new()
        .on_double_click(heard(&log, "row"))
        .child(Button::new("Go").key("btn").on_double_click(heard(&log, "button")));
    lay(&mut row);
    let at = centre_of(&row, "btn");
    double_click(&mut row, at);
    assert_eq!(*log.borrow(), ["button"]);
}

#[test]
fn a_disabled_button_hears_no_double_click() {
    let log: Log = Rc::default();
    let mut button = Button::new("Go").key("btn").disabled(true).on_double_click(heard(&log, "button"));
    lay(&mut button);
    let at = centre_of(&button, "btn");
    double_click(&mut button, at);
    assert!(log.borrow().is_empty(), "{:?}", log.borrow());
}

#[test]
fn a_field_keeps_its_double_click_from_the_row_around_it() {
    let log: Log = Rc::default();
    let mut row = Row::new()
        .on_double_click(heard(&log, "row"))
        .child(Input::new().key("field").value("hello world"));
    lay(&mut row);
    let at = centre_of(&row, "field");
    double_click(&mut row, at);
    assert!(log.borrow().is_empty(), "the field selects the word itself: {:?}", log.borrow());
}

#[test]
fn a_right_click_a_button_does_not_listen_to_reaches_the_row() {
    let log: Log = Rc::default();
    let mut row = Row::new()
        .on_right_click(heard(&log, "row"))
        .child(Button::new("Go").key("btn").on_click(heard(&log, "button")));
    lay(&mut row);
    let at = centre_of(&row, "btn");
    click_at(&mut row, at, PointerButton::Right);
    assert_eq!(*log.borrow(), ["row"]);
}

#[test]
fn a_click_in_a_field_inside_a_clickable_row_is_the_rows_and_focuses_the_field() {
    let log: Log = Rc::default();
    let mut row = Row::new()
        .on_click(heard(&log, "row"))
        .child(Input::new().key("field").value("hello"));
    lay(&mut row);
    let at = centre_of(&row, "field");
    click_at(&mut row, at, PointerButton::Left);
    assert_eq!(*log.borrow(), ["row"]);
    assert!(heca_grid_ui::node_with_key(&row, "field").unwrap().base().is_focused());
}

/// **Only the control that used the event shows the press.** The outer control listens to double
/// clicks only: a single click is the inner one's, and the outer one does not flash for it; the
/// double click the inner one does not listen to is the outer one's.
#[test]
fn only_the_control_that_used_the_event_shows_the_press() {
    let log: Log = Rc::default();
    let mut outer = Pressable::new("outer", &log)
        .width(200.0)
        .height(100.0)
        .on_double_click(heard(&log, "outer heard"))
        .child(
            Pressable::new("inner", &log)
                .key("inner")
                .width(60.0)
                .height(30.0)
                .on_click(heard(&log, "inner heard")),
        );
    lay(&mut outer);
    let at = centre_of(&outer, "inner");
    click_at(&mut outer, at, PointerButton::Left);
    assert_eq!(*log.borrow(), ["inner pressed", "inner heard"], "a single click");
    log.borrow_mut().clear();
    click_at(&mut outer, at, PointerButton::Left);
    assert_eq!(
        *log.borrow(),
        ["inner pressed", "inner heard", "outer pressed", "outer heard"],
        "the second click of a pair: the click is the inner one's, the double click the outer one's",
    );
}
