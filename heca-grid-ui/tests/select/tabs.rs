use super::*;

#[test]
fn tabs_menu_nav_and_click_change_selection() {
    use heca_grid_ui::{Action, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut tabs =
        Tabs::new(["ALPHA", "BETA", "GAMMA"]).on_change(move |a| sink.borrow_mut().push(a));
    give_keyboard(&mut tabs);
    LayoutEngine::new().compute(&mut tabs, Size::new(600.0, 60.0));

    use heca_grid_ui::WidgetIntent;
    assert_eq!(tabs.index(), 0);
    // A horizontal selector: nav arrives as `ItemPrevious`/`ItemNext` (the host maps the
    // configurable item keys — ←/Ctrl+h → previous, →/Ctrl+l → next). No literal keys.
    heca_grid_ui::dispatch(&mut tabs, &Event::Widget(WidgetIntent::ItemNext));
    assert_eq!(tabs.index(), 1);
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("tab-change", SignalData::Usize(1))),
    );

    heca_grid_ui::dispatch(&mut tabs, &Event::Widget(WidgetIntent::ItemNext));
    assert_eq!(tabs.index(), 2);
    let before = log.borrow().len();
    heca_grid_ui::dispatch(&mut tabs, &Event::Widget(WidgetIntent::ItemNext));
    assert_eq!(tabs.index(), 2, "ItemNext clamps at the last tab");
    assert_eq!(
        log.borrow().len(),
        before,
        "no event emitted when selection is unchanged"
    );

    // A click near the left edge selects the first tab again.
    let b = tabs.base().bounds;
    click_at(
        &mut tabs,
        Point::new(b.loc.x + 2.0, b.loc.y + b.size.h / 2.0),
        PointerButton::Left,
    );
    assert_eq!(tabs.index(), 0, "click selects the hit tab");
}

#[test]
fn tabs_underline_slides_toward_the_selected_tabs_bounds() {
    let theme = Theme::default();
    // The tabs paint their own pills, so pick the underline out by its thickness — it is the only
    // 2px-tall rect in the strip.
    let underline = |t: &Tabs| {
        let scene = common::paint(t, &theme);
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) if (r.rect.size.h - 2.0).abs() < 0.01 => Some(r.rect),
                _ => None,
            })
            .expect("the underline is painted")
    };
    let mut tabs = Tabs::new(["ALPHA", "BETA", "GAMMA"]);
    give_keyboard(&mut tabs);
    LayoutEngine::new().compute(&mut tabs, Size::new(600.0, 60.0));

    // It starts on the selected tab — snapped to that child's real bounds, not slid in from the
    // origin — and it is exactly as wide as the tab.
    let first = tabs.base().children[0].base().bounds;
    let u0 = underline(&tabs);
    assert!((u0.loc.x - first.loc.x).abs() < 0.01, "starts on tab 0");
    assert!((u0.size.w - first.size.w).abs() < 0.01, "as wide as tab 0");

    heca_grid_ui::dispatch(
        &mut tabs,
        &Event::Widget(heca_grid_ui::WidgetIntent::ItemNext),
    );
    let mid = underline(&tabs);
    assert!(
        mid.loc.x == u0.loc.x,
        "it does not jump: the slide happens in tick",
    );
    for _ in 0..40 {
        tabs.tick(0.016);
    }

    // …and it lands on the *bounds* of the newly selected tab, whatever that tab contains.
    let second = tabs.base().children[1].base().bounds;
    let u1 = underline(&tabs);
    assert!(u1.loc.x > u0.loc.x, "it slid right");
    assert!(
        (u1.loc.x - second.loc.x).abs() < 0.01 && (u1.size.w - second.size.w).abs() < 0.01,
        "it tracks the selected tab's real bounds",
    );
}

#[test]
fn tabs_sugar_builds_choice_children_and_composed_tabs_carry_their_content() {
    let theme = Theme::default();

    // The string constructor is sugar: every tab is a `Choice` child whose value is the text.
    let sugar = Tabs::new(["ALPHA", "BETA"]);
    assert_eq!(sugar.base().children.len(), 2, "one child per tab");
    assert_eq!(
        sugar.base().children[1].text_summary().as_deref(),
        Some("BETA"),
    );

    // A composed tab: an icon, a label and a count Badge — none of which a char-count could have
    // measured, and all of which the underline must now span.
    let mut tabs = Tabs::empty()
        .tab(Choice::labeled("files", "FILES"))
        .tab(
            Choice::new("issues")
                .child(Icon::new(Glyph::Warning))
                .child(Label::new("ISSUES"))
                .child(Badge::new("3")),
        )
        .selected(1);
    LayoutEngine::new().compute(&mut tabs, Size::new(600.0, 60.0));

    assert_eq!(tabs.selected_label(), "ISSUES");
    let issues = tabs.base().children[1].base().bounds;
    let files = tabs.base().children[0].base().bounds;
    assert!(
        issues.size.w > files.size.w,
        "the composed tab measures wider than the plain one (it holds more)",
    );

    // The selected tab tints its whole content through the `Choice` — with no wiring from Tabs.
    let scene = common::paint(&tabs, &theme);
    let issues_color = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) if t.text == "ISSUES" => Some(t.color),
            _ => None,
        })
        .expect("the composed tab paints its label");
    assert_eq!(
        issues_color, theme.colors.accent,
        "the selected tab's content is accent-tinted",
    );
}
