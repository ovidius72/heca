use super::*;

/// A `Choice` carries a **value** (what it means) independently of its **content** (what it shows).
/// The `labeled` sugar builds exactly the child a caller would compose by hand — one content model.
#[test]
fn choice_carries_a_value_and_composes_its_content() {
    let sugar = Choice::labeled("high", "HIGH");
    assert_eq!(sugar.value(), "high", "the value is what the option means");
    assert_eq!(
        sugar.base().children.len(),
        1,
        "labeled sugar → one Label child"
    );

    // Composed: arbitrary content, and the value is unchanged by it.
    let composed = Choice::new("high").child(
        Flex::row()
            .child(Icon::new(Glyph::Warning))
            .child(Label::new("HIGH")),
    );
    assert_eq!(composed.value(), "high");
    assert_eq!(composed.base().children.len(), 1, "one composed subtree");
    assert_eq!(
        composed.base().children[0].base().children.len(),
        2,
        "icon + label inside"
    );

    // Empty is legal — content is the caller's business.
    assert_eq!(Choice::new("v").base().children.len(), 0);
}

/// The option's **state color is inherited** by its unstyled content: a selected option's label
/// turns accent without the label knowing anything about selection.
#[test]
fn choice_content_inherits_the_selected_state_color() {
    let theme = Theme::default();
    let color_of = |mut c: Choice| -> Color {
        LayoutEngine::new().compute(&mut c, Size::new(200.0, 60.0));
        let scene = common::paint(&c, &theme);
        scene
            .iter()
            .find_map(|cmd| match cmd {
                DrawCommand::Text(t) => Some(t.color),
                _ => None,
            })
            .expect("the option paints its composed label")
    };

    assert_eq!(
        color_of(Choice::labeled("a", "A").selected(true)),
        theme.colors.accent
    );
    assert_eq!(color_of(Choice::labeled("a", "A")), theme.colors.foreground);
    // An explicit child color opts out of the inheritance.
    let pinned = color_of(
        Choice::new("a")
            .child(Label::new("A").color(theme.colors.danger))
            .selected(true),
    );
    assert_eq!(pinned, theme.colors.danger, "an explicit child color wins");
}

/// An option is **one** Tab stop, whatever it composes — its content is content, not focus targets.
#[test]
fn a_choice_is_one_tab_stop_whatever_it_contains() {
    let mut ui = Flex::row()
        .child(
            Choice::labeled("a", "A")
                .child(Toggle::new())
                .on_activate(|| {}),
        )
        .child(Choice::labeled("b", "B").on_activate(|| {}));

    let mut focus = FocusManager::new();
    focus.advance(&mut ui, true);
    let first = focus.focused(&mut ui);
    focus.advance(&mut ui, true);
    assert_ne!(
        focus.focused(&mut ui),
        first,
        "each option is its own Tab stop"
    );
    focus.advance(&mut ui, true);
    assert_eq!(
        focus.focused(&mut ui),
        first,
        "exactly two focusables — focus wraps"
    );
}

/// Containers must resolve a pick from the children's **real bounds**, never from row arithmetic,
/// so what is drawn and what is clickable can never disagree.
#[test]
fn choice_at_resolves_a_pick_from_real_bounds() {
    let mut list = Flex::column()
        .child(Choice::labeled("a", "AAA").on_activate(|| {}))
        .child(Choice::labeled("b", "BBB").on_activate(|| {}));
    LayoutEngine::new().compute(&mut list, Size::new(200.0, 200.0));

    let second = list.base().children[1].base().bounds;
    let inside_second = Point::new(second.loc.x + 2.0, second.loc.y + 2.0);
    assert_eq!(
        heca_grid_ui::widgets::choice_at(&list.base().children, inside_second),
        Some(1)
    );

    let miss = Point::new(second.loc.x - 50.0, second.loc.y - 500.0);
    assert_eq!(
        heca_grid_ui::widgets::choice_at(&list.base().children, miss),
        None,
        "a miss picks nothing"
    );
}
