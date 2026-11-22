//! **A dock's own keys reach the dock.** heca seats every dock in a focus scope that listens to
//! clicks (a click inside focuses the dock) and keeps the keyboard. When the dock holds the
//! keyboard, Enter and Space must not be taken by the tree as a "click": the dock declares them
//! itself (`activate_selected`, `peek_selected`), and heca consults the dock's keys only for a key
//! the tree did not take.

mod common;

use common::give_keyboard;
use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Flex, FocusScope, Row};
use heca_grid_ui::{Event, GridKey, Handled, LayoutEngine, Size, WidgetIntent};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn enter_and_space_in_a_focused_dock_are_left_to_the_dock() {
    let clicks = Rc::new(RefCell::new(0));
    let seen = clicks.clone();
    let mut dock = heca_grid_ui::WidgetModel::new(
        FocusScope::new(
            Flex::column().child(
                Row::new()
                    .key("row")
                    .child(heca_grid_ui::Label::new("pane")),
            ),
        )
        .on_click(move |_| *seen.borrow_mut() += 1)
        .keeps_keyboard(true),
    );
    LayoutEngine::new().compute(dock.as_mut(), Size::new(300.0, 200.0));
    give_keyboard(dock.as_mut());
    for (name, ev) in [
        (
            "Enter",
            Event::Key {
                key: GridKey::Enter,
                pressed: true,
            },
        ),
        (
            "Space",
            Event::Key {
                key: GridKey::Space,
                pressed: true,
            },
        ),
        ("activate", Event::Widget(WidgetIntent::Activate)),
    ] {
        let handled = heca_grid_ui::dispatch(dock.as_mut(), &ev);
        eprintln!(
            "DOCK {name}: handled = {handled:?}, dock wrapper clicks = {}",
            clicks.borrow()
        );
        assert_eq!(
            handled,
            Handled::No,
            "{name} is the dock's to answer, not a click"
        );
    }
    assert_eq!(
        *clicks.borrow(),
        0,
        "no key became a click on the dock wrapper"
    );
}
