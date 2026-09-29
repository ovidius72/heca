//! **A context menu is declared on the widget it belongs to** — the rules, held to.
//!
//! Getting a menu onto a row used to take four things, three of them invisible: a `context_path`
//! mapping a row key to a menu-id string, a builder registered for that id, the items themselves,
//! and — the one nobody would think of — a `.key(..)` on the row, because the host resolved
//! "what did you right-click" from a *position* and read the answer off `Base::key`. A
//! workspace header had the first three and not the fourth: right-clicking it opened nothing while
//! panes and columns worked, with no error and no failing test.
//!
//! What is left is one declaration on the widget. These tests are what says so.

#[path = "../common/mod.rs"]
mod common;

use common::click_at;
use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{ContextMenu, Glyph, Icon, Menu, MenuAnchor, MenuItem};
use heca_grid_ui::{Event, LayoutEngine, Point, PointerButton, Size};
use std::cell::RefCell;
use std::rc::Rc;

/// Every menu the sink was handed, **as the host opened it** — so a test can ask both *which* menu
/// opened and what it actually looks like, without building one itself.
type Opened = Rc<RefCell<Vec<ContextMenu>>>;

/// Install a sink that plays host: open what it is handed, and keep it.
///
/// Opening is the host's job and the step that realizes the rows into children — a sink that only
/// recorded a name would be testing a menu nobody ever showed. Tests below never anchor or open a
/// menu themselves; they write `.context_menu(..)` and right-click, like any author.
fn recording_sink() -> Opened {
    let opened: Opened = Rc::new(RefCell::new(Vec::new()));
    let sink = opened.clone();
    heca_grid_ui::install_menu_sink(move |menu, anchor, _subject| {
        sink.borrow_mut().push(anchor.open(menu));
    });
    opened
}

/// The first row's label of each menu that opened — the usual question here.
fn labels(opened: &Opened) -> Vec<String> {
    opened
        .borrow()
        .iter()
        .map(|m| {
            m.entry_labels()
                .first()
                .cloned()
                .flatten()
                .unwrap_or_default()
        })
        .collect()
}

fn menu_named(first: &str) -> ContextMenu {
    let first = first.to_string();
    ContextMenu::new("test-menu")
        .child(Menu::new("Test", "a menu").child(MenuItem::new().label(first).on_click(|| {})))
}

const AT: Point = Point { x: 20.0, y: 10.0 };

mod building;
mod dispatch;
mod geometry;
mod triggers;
