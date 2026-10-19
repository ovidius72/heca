use super::*;
use crate::component::{dispatch, Component, Event, Handled};
use crate::event::WidgetIntent;
use crate::reactive::SignalUpdate;
use std::cell::Cell;
use std::rc::Rc;

/// **A focused button answers the activate key**.
///
/// It answered `Event::Click` and nothing else. `Dialog` documented this as the way it works —
/// "when a button itself is focused, Enter is delivered straight to it (it consumes it)" — and
/// fell back to firing its **primary** action for the case where it was not consumed. It never
/// was consumed, so every Enter went to the primary action instead of the focused button: Tab
/// to Cancel, press Enter, and you got OK.
///
/// Keys reach the focus owner, so this needs no focus test of its own.
#[test]
fn a_focused_button_answers_the_activate_key() {
    let fired = Rc::new(Cell::new(0));
    let count = fired.clone();
    let mut b = Button::new("Cancel").on_click(move || count.set(count.get() + 1));
    b.base().focus(false);

    assert_eq!(
        dispatch(&mut b, &Event::Widget(WidgetIntent::Activate)),
        Handled::Yes,
    );
    assert_eq!(fired.get(), 1, "the key ran the button's own click");
}

/// **A disabled button answers nothing**, by the key as by the mouse.
#[test]
fn a_disabled_button_ignores_the_activate_key() {
    let fired = Rc::new(Cell::new(0));
    let count = fired.clone();
    let mut b = Button::new("Cancel").on_click(move || count.set(count.get() + 1));
    b.base().focus(false);
    b.base_mut().disabled.set(true);

    assert_eq!(
        dispatch(&mut b, &Event::Widget(WidgetIntent::Activate)),
        Handled::No,
    );
    assert_eq!(fired.get(), 0);
}
