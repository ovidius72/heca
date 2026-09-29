use super::*;

// ── handlers ──────────────────────────────────────────────────────────────────────────────────

/// The builder side: a handler on any widget, with no widget-specific support for it.
#[test]
fn a_handler_on_any_widget_hears_its_own_clicks() {
    let hits = Rc::new(std::cell::Cell::new(0));
    let h = hits.clone();
    let menus = Rc::new(std::cell::Cell::new(0));
    let m = menus.clone();
    let mut root = Flex::row().child(
        Label::new("plain")
            .width(Length::Px(100.0))
            .height(Length::Px(50.0))
            .on_click(move |_| h.set(h.get() + 1))
            .on_right_click(move |_| m.set(m.get() + 1)),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 50.0));

    click_at(&mut root, LEFT, PointerButton::Left);
    assert_eq!(hits.get(), 1, "a Label with a handler is a click target");
    let _ = heca_grid_ui::dispatch(
        &mut root,
        &Event::pointer_pressed(LEFT, PointerButton::Right),
    );
    let _ = heca_grid_ui::dispatch(
        &mut root,
        &Event::pointer_released(LEFT, PointerButton::Right),
    );
    assert_eq!(menus.get(), 1, "and it owns its own right-click");
    assert_eq!(hits.get(), 1, "which did not also count as a click");
}

/// `stop_propagation` is the named opt-out: the handler takes the event, and the widget's own
/// behaviour does not run.
#[test]
fn stop_propagation_takes_the_event_from_the_widget_itself() {
    let fired = Rc::new(std::cell::Cell::new(false));
    let f = fired.clone();
    let mut button = Button::primary("DEREZ")
        .on_click(move || f.set(true))
        .on(EventKind::Click, |cx| cx.stop_propagation());
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));
    let b = button.base().bounds;
    click_at(
        &mut button,
        Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0),
        PointerButton::Left,
    );
    assert!(!fired.get(), "the handler owned the click");
}

/// Mount fires once, on the first layout pass that sees the widget — the first moment it is both
/// in a live tree and somewhere in it.
#[test]
fn mount_fires_once_when_the_tree_is_first_laid_out() {
    let mounted = Rc::new(std::cell::Cell::new(0));
    let m = mounted.clone();
    let mut root = Flex::row().child(Label::new("x").on_mount(move |_| m.set(m.get() + 1)));
    assert_eq!(mounted.get(), 0, "nothing happens on construction");
    LayoutEngine::new().compute(&mut root, Size::new(100.0, 50.0));
    assert_eq!(mounted.get(), 1);
    LayoutEngine::new().compute(&mut root, Size::new(100.0, 50.0));
    assert_eq!(mounted.get(), 1, "and not again on the next pass");
}

/// Unmount fires when the tree that held the widget is thrown away — the moment to release
/// whatever it registered with the host.
#[test]
fn unmount_fires_when_the_widget_is_dropped() {
    let gone = Rc::new(std::cell::Cell::new(false));
    let g = gone.clone();
    {
        let _root = Flex::row().child(Label::new("x").on_unmount(move |_| g.set(true)));
        assert!(!gone.get());
    }
    assert!(gone.get(), "the rebuilt-away tree said so on its way out");
}

/// Focus is an **event**, not only a signal: the moment it arrives and the moment it leaves are
/// both things a widget can act on.
#[test]
fn focus_and_blur_reach_their_widget() {
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
    let (a, b) = (log.clone(), log.clone());
    let mut root = Flex::row()
        .child(
            Button::primary("A")
                .on_focus_gained(move |_| a.borrow_mut().push("focus"))
                .on_focus_lost(move |_| b.borrow_mut().push("blur")),
        )
        .child(Button::primary("B"));
    LayoutEngine::new().compute(&mut root, Size::new(400.0, 80.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut root, true);
    focus.advance(&mut root, true);
    assert_eq!(*log.borrow(), vec!["focus", "blur"]);
}
