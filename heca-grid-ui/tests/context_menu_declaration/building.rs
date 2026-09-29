use super::*;

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
                menu_named(if r.get() {
                    "Use default name"
                } else {
                    "Rename"
                })
            }),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    click_at(&mut root, AT, PointerButton::Right);
    renamed.set(true);
    click_at(&mut root, AT, PointerButton::Right);
    assert_eq!(labels(&opened), vec!["Rename", "Use default name"]);
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

    click_at(&mut root, AT, PointerButton::Right);
    click_at(&mut root, Point::new(150.0, 10.0), PointerButton::Right);
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

    click_at(&mut root, AT, PointerButton::Right);
    click_at(&mut root, Point::new(150.0, 10.0), PointerButton::Right);
    click_at(&mut root, Point::new(250.0, 10.0), PointerButton::Right);
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

    click_at(&mut root, AT, PointerButton::Right);
    assert_eq!(built.get(), 1, "the subtree is built when the menu opens");
    click_at(&mut root, AT, PointerButton::Right);
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
    let mut panel =
        MenuAnchor::At(Point::new(0.0, 0.0)).open(ContextMenu::new("pane-menu").child(menu));
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
    click_at(&mut root, AT, PointerButton::Right);

    let keys = opened.borrow()[0].quick_pick_keys();
    assert_eq!(keys[1], Some('s'), "an explicit key is kept");
    assert_eq!(
        keys[2], None,
        "a disabled row cannot be picked, so it gets no letter"
    );
    assert!(
        keys[0].is_some() && keys[3].is_some(),
        "every enabled row got one: {keys:?}"
    );
    assert_ne!(keys[0], keys[3], "and no letter is handed out twice");
    assert!(
        keys[0] != Some('s') && keys[3] != Some('s'),
        "the explicit letter was not reused"
    );
}
