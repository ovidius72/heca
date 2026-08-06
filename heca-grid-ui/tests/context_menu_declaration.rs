//! **A context menu is declared on the widget it belongs to** — the rules, held to.
//!
//! Getting a menu onto a row used to take four things, three of them invisible: a `context_path`
//! mapping a row key to a menu-id string, a builder registered for that id, the items themselves,
//! and — the one nobody would think of — a `.nav_key(..)` on the row, because the host resolved
//! "what did you right-click" from a *position* and read the answer off `Base::nav_key`. A
//! workspace header had the first three and not the fourth: right-clicking it opened nothing while
//! panes and columns worked, with no error and no failing test.
//!
//! What is left is one declaration on the widget. These tests are what says so.

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Menu, MenuAnchor, MenuItem};
use heca_grid_ui::{Component, Event, LayoutEngine, Point, PointerButton, Size};
use std::cell::RefCell;
use std::rc::Rc;

/// Every menu the sink was handed, by the label of its first row — enough to say *which* menu
/// opened, which is the whole question in these tests.
type Opened = Rc<RefCell<Vec<String>>>;

/// Install a sink that records what opened. The host's one job is presenting the menu; a test can
/// be that host in three lines, which is the point of the seam.
fn recording_sink() -> Opened {
    let opened: Opened = Rc::new(RefCell::new(Vec::new()));
    let sink = opened.clone();
    heca_grid_ui::install_menu_sink(move |menu, _anchor| {
        sink.borrow_mut()
            .push(menu.item_labels().first().cloned().unwrap_or_default());
    });
    opened
}

fn menu_named(first: &str) -> Menu {
    let first = first.to_string();
    Menu::new("Test", "a menu").child(MenuItem::new(first).on_click(|| {}))
}

fn right_click(root: &mut dyn Component, pos: Point) {
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_pressed(pos, PointerButton::Right));
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_released(pos, PointerButton::Right));
}

const AT: Point = Point { x: 20.0, y: 10.0 };

/// The declaration is all there is: no row identity, no path, no registered builder.
#[test]
fn a_right_click_opens_the_menu_the_widget_declares() {
    let opened = recording_sink();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .context_menu(menu_named("Rename")),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    right_click(&mut root, AT);
    assert_eq!(*opened.borrow(), vec!["Rename"]);
}

/// **You click the label, the row's menu opens.** The menu belongs to the row; the label inside it
/// is not a separate thing to declare one on.
#[test]
fn the_click_bubbles_out_to_the_declaring_ancestor() {
    let opened = recording_sink();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .child(Label::new("pane-1"))
            .context_menu(menu_named("Rename")),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    // Aim at the label, not at the row's padding.
    let label = root.base().children[0].base().children[0].base().bounds;
    right_click(
        &mut root,
        Point::new(label.loc.x + 2.0, label.loc.y + label.size.h / 2.0),
    );
    assert_eq!(*opened.borrow(), vec!["Rename"]);
}

/// **Nearest wins, and it stops there.** Two declarations on one path are not merged — a menu is a
/// statement about one thing, and stitching two together would make its entries mean different
/// targets in the same list.
#[test]
fn the_innermost_declaration_wins_and_menus_are_never_merged() {
    let opened = recording_sink();
    let mut root = Flex::row()
        .context_menu(menu_named("New workspace"))
        .child(
            Row::new()
                .width(Length::Px(100.0))
                .height(Length::Px(40.0))
                .context_menu(menu_named("Rename")),
        );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    right_click(&mut root, AT);
    assert_eq!(
        *opened.borrow(),
        vec!["Rename"],
        "the row's menu, once — not the container's, and not both",
    );
}

/// **Nothing in the chain declares one ⇒ nothing opens.** No hidden fallback: a host that wants
/// "right-click empty space" puts a menu on the root, which needs no empty-space hit test.
#[test]
fn nothing_declared_opens_nothing_and_a_root_declaration_covers_the_gaps() {
    let opened = recording_sink();
    let mut bare = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0)),
    );
    LayoutEngine::new().compute(&mut bare, Size::new(200.0, 40.0));
    right_click(&mut bare, AT);
    assert!(opened.borrow().is_empty(), "no declaration, no menu");

    let opened = recording_sink();
    let mut with_root = Flex::row()
        // A real size, as the region this would be mounted in gives it: the root's menu covers the
        // gaps *inside the root*, and a point outside every widget is outside the tree.
        .width(Length::Px(400.0))
        .height(Length::Px(40.0))
        .context_menu(menu_named("New workspace"))
        .child(
            Row::new()
                .width(Length::Px(100.0))
                .height(Length::Px(40.0)),
        );
    LayoutEngine::new().compute(&mut with_root, Size::new(400.0, 40.0));
    // A point past the row — the "empty space" case, with nothing declared about empty space.
    right_click(&mut with_root, Point::new(300.0, 10.0));
    assert_eq!(*opened.borrow(), vec!["New workspace"]);
}

/// **The closure runs at trigger time**, so a menu describes the state it is opened in. Building
/// menus lazily was the only reason a host-owned builder registry existed.
#[test]
fn the_items_are_built_when_the_menu_opens_not_when_it_was_declared() {
    let opened = recording_sink();
    let renamed = Rc::new(std::cell::Cell::new(false));
    let r = renamed.clone();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .context_menu_built(move || {
                menu_named(if r.get() { "Use default name" } else { "Rename" })
            }),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    right_click(&mut root, AT);
    renamed.set(true);
    right_click(&mut root, AT);
    assert_eq!(*opened.borrow(), vec!["Rename", "Use default name"]);
}

/// A widget that answers its own right-click **wins**: the declared menu is what happens when
/// nothing claims the click, not something that overrides a widget's own behaviour.
#[test]
fn a_widget_that_handles_its_own_right_click_beats_the_declaration() {
    let opened = recording_sink();
    let hits = Rc::new(std::cell::Cell::new(0));
    let h = hits.clone();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .on_right_click(move |_| h.set(h.get() + 1))
            .context_menu(menu_named("Rename")),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    right_click(&mut root, AT);
    assert_eq!(hits.get(), 1, "the widget's own handler ran");
    assert!(opened.borrow().is_empty(), "and it owned the click");
}

/// The **keyboard** trigger: the same declaration, found from focus rather than from a position,
/// and anchored on the widget instead of on the pointer.
#[test]
fn the_keyboard_trigger_finds_the_same_declaration_from_focus() {
    let opened = recording_sink();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .child(Label::new("pane-1"))
            .context_menu(menu_named("Rename")),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    assert!(
        !heca_grid_ui::open_for_focused(&root),
        "nothing is focused, so nothing opens — not the root's menu",
    );

    // Focus the label *inside* the row: the menu is still the row's.
    root.base_mut().children[0].base_mut().children[0]
        .base_mut()
        .focused
        .set(true);
    assert!(heca_grid_ui::open_for_focused(&root));
    assert_eq!(*opened.borrow(), vec!["Rename"]);
}

/// Any trigger at all: `show_at` / `show_under` are public, so a left click, a long press or a
/// timer opens a menu through the same path the declaration uses. A parallel path for the
/// convenient case is how one widget's `hide()` came to fade while its `remove()` cut.
#[test]
fn a_menu_can_be_opened_by_any_trigger_the_author_invents() {
    let opened = recording_sink();
    let mut root = Flex::row().child(
        Button::new("More")
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .on_click(|| {
                heca_grid_ui::menu::show(
                    menu_named("Deploy to staging"),
                    MenuAnchor::At(Point::new(5.0, 5.0)),
                )
            }),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    let _ = heca_grid_ui::dispatch(&mut root, &Event::pointer_pressed(AT, PointerButton::Left));
    let _ = heca_grid_ui::dispatch(&mut root, &Event::pointer_released(AT, PointerButton::Left));
    assert_eq!(*opened.borrow(), vec!["Deploy to staging"]);
}
