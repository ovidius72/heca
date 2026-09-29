use super::*;

/// **The anchor comes out of the event.** A pointer event answers with the cursor, a widget-bounds
/// event answers with the widget — and an event carrying neither opens nothing rather than
/// guessing a corner of the screen.
#[test]
fn the_anchor_is_read_from_the_event_that_asked_for_the_menu() {
    use heca_grid_ui::Rectangle;
    use heca_grid_ui::event::PointerEvent;

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
    click_at(&mut root, AT, PointerButton::Right);

    // Laid out **in place**: cloning it again would hand back a fresh, unrealized panel — this is
    // the one the host opened, which is already a clone of what the widget declared.
    let mut panels = opened.borrow_mut();
    let panel = &mut panels[0];
    LayoutEngine::new().compute(panel, Size::new(600.0, 600.0));
    let rows: Vec<_> = panel
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds)
        .collect();

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
    menu.anchor_signal()
        .set(heca_core::layout::Point::new(390.0, 290.0));

    LayoutEngine::new().compute(&mut menu, viewport);

    let b = menu.base().bounds;
    assert!(
        b.loc.x + b.size.w <= viewport.w + 0.5 && b.loc.y + b.size.h <= viewport.h + 0.5,
        "the panel is inside the viewport on the first pass, with no second frame to correct it: {b:?}",
    );
}
