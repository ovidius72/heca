//! A row's background at rest, by name — `RowVariant` — and the hand-set style that wins over it.

mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, LayoutEngine, Size, Theme};

/// The rect a row paints for its own background, if any (the first filled rect at its bounds).
fn background(row: Row) -> Option<(heca_grid_ui::Color, f32)> {
    let mut root = Flex::column().child(row);
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));
    let bounds = root.base().children[0].base().bounds;
    let scene = common::paint_via_child(&root, &Theme::default());
    scene.iter().find_map(|cmd| match cmd {
        DrawCommand::Rect(r) if r.rect == bounds && r.fill.a > 0 => Some((r.fill, r.radius)),
        _ => None,
    })
}

#[test]
fn each_variant_paints_its_theme_look_and_plain_paints_none() {
    let theme = Theme::default();
    let label = || Label::new("zsh");
    assert_eq!(
        background(Row::new().child(label())),
        None,
        "Plain: no background at rest"
    );
    assert_eq!(
        background(Row::new().variant(RowVariant::Tinted).child(label())),
        Some((
            theme
                .colors
                .foreground
                .with_alpha_f32(theme.colors.card_background_alpha),
            theme.colors.control_radius()
        )),
    );
    assert_eq!(
        background(Row::new().variant(RowVariant::Filled).child(label())),
        Some((theme.colors.surface, theme.colors.control_radius())),
    );
}

#[test]
fn a_hand_set_background_or_radius_wins_over_the_variant() {
    let theme = Theme::default();
    let painted = background(
        Row::new()
            .variant(RowVariant::Tinted)
            .background(theme.colors.danger)
            .child(Label::new("x")),
    );
    assert_eq!(painted.map(|(fill, _)| fill), Some(theme.colors.danger));
    let painted = background(
        Row::new()
            .variant(RowVariant::Filled)
            .radius(1.0)
            .child(Label::new("x")),
    );
    assert_eq!(painted.map(|(_, radius)| radius), Some(1.0));
}

#[test]
fn a_variant_brings_its_air_and_a_later_padding_wins() {
    let width = |row: Row| {
        let mut root = Flex::row().child(row);
        LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));
        root.base().children[0].base().bounds.size.w
    };
    let plain = width(Row::new().child(Label::new("zsh")));
    let tinted = width(
        Row::new()
            .variant(RowVariant::Tinted)
            .child(Label::new("zsh")),
    );
    let filled = width(
        Row::new()
            .variant(RowVariant::Filled)
            .child(Label::new("zsh")),
    );
    assert!(
        tinted > filled && filled > plain,
        "Sm air > Xs air > none: {tinted} {filled} {plain}"
    );
    let overridden = width(
        Row::new()
            .variant(RowVariant::Tinted)
            .padding(Spacing::Xs)
            .child(Label::new("zsh")),
    );
    assert_eq!(overridden, filled, "a padding set after the variant wins");
}
