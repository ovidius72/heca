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

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{ContextMenu, Glyph, Icon, Menu, MenuAnchor, MenuItem};
use heca_grid_ui::{Component, Event, LayoutEngine, Point, PointerButton, Size};
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
        .map(|m| m.entry_labels().first().cloned().flatten().unwrap_or_default())
        .collect()
}

fn menu_named(first: &str) -> ContextMenu {
    let first = first.to_string();
    ContextMenu::new("test-menu").child(
        Menu::new("Test", "a menu").child(MenuItem::new().label(first).on_click(|| {})),
    )
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
    assert_eq!(labels(&opened), vec!["Rename"]);
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
    assert_eq!(labels(&opened), vec!["Rename"]);
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
        labels(&opened),
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
    assert_eq!(labels(&opened), vec!["New workspace"]);
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
            .context_menu(move |_at| {
                menu_named(if r.get() { "Use default name" } else { "Rename" })
            }),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    right_click(&mut root, AT);
    renamed.set(true);
    right_click(&mut root, AT);
    assert_eq!(labels(&opened), vec!["Rename", "Use default name"]);
}

/// A widget that wants to show **something other than** its declared menu says so, by name:
/// `prevent_default`, exactly as a browser does.
///
/// The claim is a call, not a side effect of registering a handler.
///
/// ⚠️ **It used to be `stop_propagation` that cancelled the menu**, and that was a trap: stopping
/// the walk is about who *else* sees the event, not about what the framework does afterwards. A
/// widget that stopped the walk for an unrelated reason — to keep something behind from also
/// reacting — silently lost its own declared menu, with nothing failing and no warning. The two
/// are separate levers now.
#[test]
fn a_widget_that_prevents_the_default_beats_the_declaration() {
    let opened = recording_sink();
    let hits = Rc::new(std::cell::Cell::new(0));
    let h = hits.clone();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .on_right_click(move |e| {
                h.set(h.get() + 1);
                e.prevent_default();
            })
            .context_menu(menu_named("Rename")),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    right_click(&mut root, AT);
    assert_eq!(hits.get(), 1, "the widget's own handler ran");
    assert!(
        opened.borrow().is_empty(),
        "and it said not to open the declared one"
    );
}

/// **Stopping the walk does NOT cancel the menu** — the whole point of splitting the two.
///
/// This is the trap that prompted the split: a widget answers its right-click and stops the event
/// reaching anything behind it, and its own menu still opens, because it never said otherwise.
#[test]
fn stopping_the_walk_does_not_withhold_the_declared_menu() {
    let opened = recording_sink();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .on_right_click(|e| e.stop_propagation())
            .context_menu(menu_named("Rename")),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    right_click(&mut root, AT);
    assert_eq!(
        labels(&opened),
        vec!["Rename"],
        "nobody said prevent_default, so the menu the widget declared still opens",
    );
}

/// …and a handler that only **watches** the click gets both: it runs, and the declared menu still
/// opens. Watching without claiming used to need a second, differently-shaped API.
#[test]
fn a_handler_that_does_not_claim_the_click_still_lets_the_menu_open() {
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
    assert_eq!(hits.get(), 1, "the observer ran");
    assert_eq!(opened.borrow().len(), 1, "and the menu it did not claim still opened");
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
        !heca_grid_ui::open_for_keyboard(&root, None),
        "nothing is focused, so nothing opens — not the root's menu",
    );

    // Focus the label *inside* the row: the menu is still the row's.
    root.base_mut().children[0].base_mut().children[0]
        .base_mut()
        .focused
        .set(true);
    assert!(heca_grid_ui::open_for_keyboard(&root, None));
    assert_eq!(labels(&opened), vec!["Rename"]);
}

/// Any trigger at all: `menu::show` is public, so a left click, a long press or a timer opens a
/// menu through the same path the declaration uses. A parallel path for the convenient case is how
/// one widget's `hide()` came to fade while its `remove()` cut.
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
    assert_eq!(labels(&opened), vec!["Deploy to staging"]);
}

/// **A value or a closure, one method.** Both spellings reach the same slot, so the choice is about
/// when the rows are built and never about which builder to remember.
#[test]
fn a_menu_is_declared_as_a_value_or_as_a_closure() {
    let opened = recording_sink();
    let ctx = menu_named("Rename");
    let mut root = Flex::row()
        .child(
            Row::new()
                .width(Length::Px(100.0))
                .height(Length::Px(40.0))
                .context_menu(ctx.clone()),
        )
        .child(
            Row::new()
                .width(Length::Px(100.0))
                .height(Length::Px(40.0))
                .context_menu(move |_at| ctx.clone()),
        );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    right_click(&mut root, AT);
    right_click(&mut root, Point::new(150.0, 10.0));
    assert_eq!(labels(&opened), vec!["Rename", "Rename"]);
}

/// **A menu value can be declared on several rows**, because it is `Clone` — that is the whole
/// reason the composed content is a builder behind an `Rc` rather than an owned subtree.
#[test]
fn one_menu_value_serves_every_row_in_a_list() {
    let opened = recording_sink();
    let ctx = menu_named("Close");
    let mut root = Flex::row();
    for _ in 0..3 {
        root = root.child(
            Row::new()
                .width(Length::Px(100.0))
                .height(Length::Px(40.0))
                .context_menu(ctx.clone()),
        );
    }
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 40.0));

    right_click(&mut root, AT);
    right_click(&mut root, Point::new(150.0, 10.0));
    right_click(&mut root, Point::new(250.0, 10.0));
    assert_eq!(labels(&opened), vec!["Close", "Close", "Close"]);
}

/// **A composed row is built again for every opening.** An owned subtree can be handed over once;
/// this is the test that says the second right-click still has a menu to show.
#[test]
fn a_composed_row_survives_being_opened_twice() {
    let built = Rc::new(std::cell::Cell::new(0u32));
    let b = built.clone();
    let ctx = ContextMenu::new("pane-menu").child(
        Menu::new("Pane", "what you can do").child(
            MenuItem::new()
                .child(move || {
                    b.set(b.get() + 1);
                    Icon::new(Glyph::Trash)
                })
                .on_click(|| {}),
        ),
    );

    let _opened = recording_sink();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .context_menu(ctx),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    right_click(&mut root, AT);
    assert_eq!(built.get(), 1, "the subtree is built when the menu opens");
    right_click(&mut root, AT);
    assert_eq!(built.get(), 2, "and built again for the second opening");
}

/// **Composed content wins over the sugar**, the same precedence `Button` has — and both forms end
/// up as real children the layout engine sizes, which is what keeps this widget free of a layout
/// implementation of its own.
#[test]
fn composed_content_wins_over_the_label_and_both_become_real_children() {
    let menu = Menu::new("Pane", "what you can do")
        .child(MenuItem::new().label("Rename").on_click(|| {}))
        .child(
            MenuItem::new()
                .label("ignored")
                .child(|| Label::new("Close"))
                .on_click(|| {}),
        );
    let mut panel = MenuAnchor::At(Point::new(0.0, 0.0))
        .open(ContextMenu::new("pane-menu").child(menu));
    LayoutEngine::new().compute(&mut panel, Size::new(400.0, 400.0));

    assert_eq!(
        panel.base().children.len(),
        2,
        "one real child per row, laid out by the engine",
    );
    for (i, row) in panel.base().children.iter().enumerate() {
        let b = row.base().bounds;
        assert!(
            b.size.w > 0.0 && b.size.h > 0.0,
            "row {i} was measured by the layout engine: {b:?}",
        );
    }
}

/// **The anchor comes out of the event.** A pointer event answers with the cursor, a widget-bounds
/// event answers with the widget — and an event carrying neither opens nothing rather than
/// guessing a corner of the screen.
#[test]
fn the_anchor_is_read_from_the_event_that_asked_for_the_menu() {
    use heca_grid_ui::event::PointerEvent;
    use heca_grid_ui::Rectangle;

    let at = Point::new(30.0, 40.0);
    assert_eq!(
        MenuAnchor::from_event(&Event::RightClick(PointerEvent::at(at))),
        Some(MenuAnchor::At(at)),
        "a pointer event anchors at the cursor",
    );

    let bounds = Rectangle::new(Point::new(4.0, 8.0), Size::new(100.0, 20.0));
    let no_pos = Event::Widget(heca_grid_ui::component::WidgetIntent::Activate);
    assert_eq!(
        MenuAnchor::from_event(&no_pos),
        None,
        "an event with neither a position nor a target shows nothing",
    );

    // The panel hangs off the bottom edge, so the row it is *about* stays readable.
    let panel = MenuAnchor::Under(bounds).open(ContextMenu::new("m"));
    assert_eq!(
        panel.anchor_signal().get_untracked(),
        Point::new(bounds.loc.x, bounds.loc.y + bounds.size.h),
    );
}

/// **The rows stack.** A menu is a vertical list, and the declared path always clones — so
/// anything `Clone` drops is missing from every declared menu while a directly built one is fine.
///
/// It happened: `Clone` rebuilt a bare `Base`, losing `Direction::Column` (whose default is `Row`),
/// so declared menus laid their rows out **side by side** while the host's own dropdown stayed a
/// vertical list. Two menus, one widget, two shapes on screen. Thirteen tests here passed, because
/// none of them looked at where the rows ended up (Antonio, 2026-08-07, with screenshots).
#[test]
fn rows_stack_vertically() {
    let opened = recording_sink();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .context_menu(
                ContextMenu::new("pane-menu").child(
                    Menu::new("Pane", "what you can do")
                        .child(MenuItem::new().label("Rename").on_click(|| {}))
                        .child(MenuItem::new().label("Close").on_click(|| {})),
                ),
            ),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));
    right_click(&mut root, AT);

    // Laid out **in place**: cloning it again would hand back a fresh, unrealized panel — this is
    // the one the host opened, which is already a clone of what the widget declared.
    let mut panels = opened.borrow_mut();
    let panel = &mut panels[0];
    LayoutEngine::new().compute(panel, Size::new(600.0, 600.0));
    let rows: Vec<_> = panel.base().children.iter().map(|c| c.base().bounds).collect();

    assert_eq!(rows.len(), 2, "one child per row");
    assert!(
        rows[1].loc.y > rows[0].loc.y,
        "the second row must sit BELOW the first — a menu is a vertical list. Got {rows:?}",
    );
    assert!(
        (rows[1].loc.x - rows[0].loc.x).abs() < 0.5,
        "rows share a left edge; they are stacked, not side by side. Got {rows:?}",
    );
}

/// **The menu hands out the quick-pick letters**, not its caller. Two hosts each wrote
/// `let mut letters = 'a'..='z'` beside their own loop, so the same menu had keycaps built one way
/// and none built the other.
#[test]
fn the_menu_assigns_quick_pick_letters() {
    let opened = recording_sink();
    let mut root = Flex::row().child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .context_menu(
                ContextMenu::new("m").child(
                    Menu::new("Pane", "what you can do")
                        .child(MenuItem::new().label("Rename").on_click(|| {}))
                        .child(MenuItem::new().label("Split").key('s').on_click(|| {}))
                        .child(MenuItem::new().label("Nope").enabled(false).on_click(|| {}))
                        .child(MenuItem::new().label("Close").on_click(|| {})),
                ),
            ),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));
    right_click(&mut root, AT);

    let keys = opened.borrow()[0].quick_pick_keys();
    assert_eq!(keys[1], Some('s'), "an explicit key is kept");
    assert_eq!(keys[2], None, "a disabled row cannot be picked, so it gets no letter");
    assert!(keys[0].is_some() && keys[3].is_some(), "every enabled row got one: {keys:?}");
    assert_ne!(keys[0], keys[3], "and no letter is handed out twice");
    assert!(keys[0] != Some('s') && keys[3] != Some('s'), "the explicit letter was not reused");
}

/// **A menu is clamped on its very first frame.** Placement happens during layout, and the
/// viewport used to be learned one pass later, from the paint — so a menu opened near an edge was
/// drawn at the raw anchor and then jumped once it had been clamped (Antonio, 2026-08-10). The
/// layout pass publishes the viewport it was given, so the first placement is the right one.
#[test]
fn a_menu_near_the_edge_is_clamped_on_the_first_layout_pass() {
    let viewport = Size::new(400.0, 300.0);
    let mut menu = ContextMenu::new("m")
        .child(
            Menu::new("Pane", "what you can do")
                .child(MenuItem::new().label("Rename").on_click(|| {}))
                .child(MenuItem::new().label("Close").on_click(|| {})),
        )
        .default_open(true);
    // Anchored hard against the bottom-right corner: unclamped, the panel would hang off-screen.
    menu.anchor_signal().set(heca_core::layout::Point::new(390.0, 290.0));

    LayoutEngine::new().compute(&mut menu, viewport);

    let b = menu.base().bounds;
    assert!(
        b.loc.x + b.size.w <= viewport.w + 0.5 && b.loc.y + b.size.h <= viewport.h + 0.5,
        "the panel is inside the viewport on the first pass, with no second frame to correct it: {b:?}",
    );
}

/// **An open menu answers the keyboard when it is the root of the dispatch** — which is how a host
/// mounts it: the layer's root *is* the menu. Keys go to the focus owner, and an open menu is one.
#[test]
fn an_open_menu_mounted_as_a_layer_root_answers_dismiss_and_a_quick_pick() {
    use heca_grid_ui::WidgetIntent;
    let dismissed = Rc::new(std::cell::Cell::new(false));
    let d = dismissed.clone();
    let ran = Rc::new(std::cell::Cell::new(0u32));
    let r = ran.clone();

    let mut menu = ContextMenu::new("m")
        .child(
            Menu::new("Pane", "what you can do")
                .child(MenuItem::new().label("Rename").key('r').on_click(move || r.set(1)))
                .child(MenuItem::new().label("Close").on_click(|| {})),
        )
        .on_dismiss(move || d.set(true))
        .default_open(true);
    LayoutEngine::new().compute(&mut menu, Size::new(400.0, 300.0));

    // The quick-pick letter, as a raw key.
    heca_grid_ui::dispatch(&mut menu, &heca_grid_ui::Event::Key {
        key: heca_grid_ui::GridKey::Char('r'),
        pressed: true,
    });
    assert_eq!(ran.get(), 1, "the quick-pick letter reached the menu");

    // …and the intent the host resolves Esc into, on a menu that is still open (running an entry
    // closes the one above).
    let d2 = dismissed.clone();
    let mut menu = ContextMenu::new("m")
        .child(Menu::new("Pane", "what you can do").child(MenuItem::new().label("Rename").on_click(|| {})))
        .on_dismiss(move || d2.set(true))
        .default_open(true);
    LayoutEngine::new().compute(&mut menu, Size::new(400.0, 300.0));
    heca_grid_ui::dispatch(&mut menu, &heca_grid_ui::Event::Widget(WidgetIntent::Dismiss));
    assert!(dismissed.get(), "Dismiss reached the menu");
}

/// **`on_hint` is one line on the wrapper you were already using**, and the framework does the rest: it finds the widgets
/// that declared one, and running a pick runs the closure the author wrote — no id, no registry,
/// no host type at the call site (F004/P084/T399).
#[test]
fn a_hint_is_declared_on_the_wrapper_and_the_framework_finds_and_runs_it() {
    let picked = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let (a, b) = (picked.clone(), picked.clone());
    let mut root = Flex::column()
        .width(Length::Px(200.0))
        .child(
            heca_grid_ui::widgets::KeyHint::new(
                Row::new().width(Length::Px(200.0)).height(Length::Px(20.0)),
            )
            .on_hint(move || a.borrow_mut().push("first")),
        )
        .child(
            heca_grid_ui::widgets::KeyHint::new(
                Row::new()
                    .width(Length::Px(200.0))
                    .height(Length::Px(20.0))
                    .child(Label::new("nested")),
            )
            .on_hint(move || b.borrow_mut().push("second")),
        )
        // A widget that declares nothing is not a target.
        .child(Row::new().width(Length::Px(200.0)).height(Length::Px(20.0)));
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 60.0));

    let targets = heca_grid_ui::hint::collect_hints(&root);
    assert_eq!(targets.len(), 2, "only the widgets that declared one: {targets:?}");
    assert!(targets[0].1.size.h > 0.0, "each carries the rect its letter goes over");

    assert!(heca_grid_ui::hint::fire_hint(&mut root, &targets[1].0));
    assert_eq!(*picked.borrow(), vec!["second"], "the pick ran the closure that row was built with");

    // A path into a tree that no longer has that widget is not an error.
    assert!(!heca_grid_ui::hint::fire_hint(&mut root, &[99]));
}
