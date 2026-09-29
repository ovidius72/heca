use super::*;

// ── which button a control owns ───────────────────────────────────────────────────────────────

/// **A control claims the primary press and nothing else.** A right-click has to reach whatever
/// answers one — the widget itself, or the host behind it — and a control that swallowed every
/// button is how a right-click on a sidebar row opened no menu at all while the pane behind it
/// worked fine.
#[test]
fn a_control_takes_the_left_press_and_lets_every_other_button_past() {
    let mut root = Flex::row().child(
        Button::primary("DEREZ")
            .width(Length::Px(100.0))
            .height(Length::Px(50.0)),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 50.0));

    assert_eq!(
        heca_grid_ui::dispatch(
            &mut root,
            &Event::pointer_pressed(LEFT, PointerButton::Left)
        ),
        Handled::Yes,
        "the button owns the press that would activate it",
    );
    let _ = heca_grid_ui::dispatch(
        &mut root,
        &Event::pointer_released(LEFT, PointerButton::Left),
    );
    assert_eq!(
        heca_grid_ui::dispatch(
            &mut root,
            &Event::pointer_pressed(LEFT, PointerButton::Right)
        ),
        Handled::No,
        "a right press carries on to whoever answers right-clicks",
    );
}

/// The claim is a **declaration**, not nine copies of a match arm: it lives on `Base`, the router
/// applies it, and a widget that declares it never writes the claim out.
#[test]
fn one_click_target_is_declared_not_written_out() {
    let button = Button::primary("A");
    let row = Row::new().on_activate(|| {});
    let plain = Label::new("text");
    assert!(button.base().one_click_target);
    assert!(row.base().one_click_target, "an interactive row is one too");
    assert!(
        !plain.base().one_click_target,
        "and a plain leaf claims nothing",
    );
}

/// **Click away to close.** A press that lands anywhere else reaches an open menu through its own
/// hooks — the same two the widget uses for everything — so a popup dismisses itself with no
/// geometry of its own and nothing for the host to arrange.
#[test]
fn a_press_outside_an_open_menu_dismisses_it() {
    use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
    let dismissed = Rc::new(std::cell::Cell::new(false));
    let d = dismissed.clone();
    let mut menu = ContextMenu::new("test-menu")
        .child(Menu::new("Test", "a menu").child(MenuItem::new().label("Close").on_click(|| {})))
        .anchor(Point::new(40.0, 40.0))
        .on_dismiss(move || d.set(true))
        .default_open(true);
    LayoutEngine::new().compute(&mut menu, Size::new(400.0, 300.0));
    // Paint once so the menu caches a viewport and places its panel, as a host frame would.
    let theme = Theme::default();
    let _ = common::paint_in(&menu, &theme, Size::new(400.0, 300.0));

    press_at(&mut menu, Point::new(380.0, 290.0), PointerButton::Left);
    assert!(dismissed.get(), "a press away from the panel closed it");
}
