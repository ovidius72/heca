use super::*;

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
                .child(
                    MenuItem::new()
                        .label("Rename")
                        .key('r')
                        .on_click(move || r.set(1)),
                )
                .child(MenuItem::new().label("Close").on_click(|| {})),
        )
        .on_dismiss(move || d.set(true))
        .default_open(true);
    LayoutEngine::new().compute(&mut menu, Size::new(400.0, 300.0));

    // The quick-pick letter, as a raw key.
    heca_grid_ui::dispatch(
        &mut menu,
        &heca_grid_ui::Event::Key {
            key: heca_grid_ui::GridKey::Char('r'),
            pressed: true,
        },
    );
    assert_eq!(ran.get(), 1, "the quick-pick letter reached the menu");

    // …and the intent the host resolves Esc into, on a menu that is still open (running an entry
    // closes the one above).
    let d2 = dismissed.clone();
    let mut menu = ContextMenu::new("m")
        .child(
            Menu::new("Pane", "what you can do")
                .child(MenuItem::new().label("Rename").on_click(|| {})),
        )
        .on_dismiss(move || d2.set(true))
        .default_open(true);
    LayoutEngine::new().compute(&mut menu, Size::new(400.0, 300.0));
    heca_grid_ui::dispatch(
        &mut menu,
        &heca_grid_ui::Event::Widget(WidgetIntent::Dismiss),
    );
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
    assert_eq!(
        targets.len(),
        2,
        "only the widgets that declared one: {targets:?}"
    );
    assert!(
        targets[0].1.size.h > 0.0,
        "each carries the rect its letter goes over"
    );

    assert!(heca_grid_ui::hint::fire_hint(&mut root, &targets[1].0));
    assert_eq!(
        *picked.borrow(),
        vec!["second"],
        "the pick ran the closure that row was built with"
    );

    // A path into a tree that no longer has that widget is not an error.
    assert!(!heca_grid_ui::hint::fire_hint(&mut root, &[99]));
}
