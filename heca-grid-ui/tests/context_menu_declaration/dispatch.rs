use super::*;

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

    click_at(&mut root, AT, PointerButton::Right);
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
    click_at(
        &mut root,
        Point::new(label.loc.x + 2.0, label.loc.y + label.size.h / 2.0),
        PointerButton::Right,
    );
    assert_eq!(labels(&opened), vec!["Rename"]);
}

/// **Nearest wins, and it stops there.** Two declarations on one path are not merged — a menu is a
/// statement about one thing, and stitching two together would make its entries mean different
/// targets in the same list.
#[test]
fn the_innermost_declaration_wins_and_menus_are_never_merged() {
    let opened = recording_sink();
    let mut root = Flex::row().context_menu(menu_named("New workspace")).child(
        Row::new()
            .width(Length::Px(100.0))
            .height(Length::Px(40.0))
            .context_menu(menu_named("Rename")),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 40.0));

    click_at(&mut root, AT, PointerButton::Right);
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
    let mut bare = Flex::row().child(Row::new().width(Length::Px(100.0)).height(Length::Px(40.0)));
    LayoutEngine::new().compute(&mut bare, Size::new(200.0, 40.0));
    click_at(&mut bare, AT, PointerButton::Right);
    assert!(opened.borrow().is_empty(), "no declaration, no menu");

    let opened = recording_sink();
    let mut with_root = Flex::row()
        // A real size, as the region this would be mounted in gives it: the root's menu covers the
        // gaps *inside the root*, and a point outside every widget is outside the tree.
        .width(Length::Px(400.0))
        .height(Length::Px(40.0))
        .context_menu(menu_named("New workspace"))
        .child(Row::new().width(Length::Px(100.0)).height(Length::Px(40.0)));
    LayoutEngine::new().compute(&mut with_root, Size::new(400.0, 40.0));
    // A point past the row — the "empty space" case, with nothing declared about empty space.
    click_at(
        &mut with_root,
        Point::new(300.0, 10.0),
        PointerButton::Right,
    );
    assert_eq!(labels(&opened), vec!["New workspace"]);
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

    click_at(&mut root, AT, PointerButton::Right);
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

    click_at(&mut root, AT, PointerButton::Right);
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

    click_at(&mut root, AT, PointerButton::Right);
    assert_eq!(hits.get(), 1, "the observer ran");
    assert_eq!(
        opened.borrow().len(),
        1,
        "and the menu it did not claim still opened"
    );
}
