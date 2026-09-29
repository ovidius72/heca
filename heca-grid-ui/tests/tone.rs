mod common;

use heca_grid_ui::{LayoutEngine, Size, Theme};

// --- A container tints the controls inside it (F003/P096/T484) ---------------

/// Paint `w` inside a container that published `tone`, and report the hues it drew its chrome in.
fn chrome_hues(
    w: &mut dyn heca_grid_ui::Component,
    tone: Option<heca_grid_ui::Color>,
) -> Vec<(u8, u8, u8)> {
    use heca_grid_ui::{DrawCommand, PaintCx, Scene};
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        // **Through `paint_child`, never `paint`** — the chokepoint is what applies a widget's own
        // published hue, so painting directly here would test a path no widget is ever drawn by.
        match tone {
            Some(t) => cx.with_accent(t, |cx| heca_grid_ui::paint_child(w, cx)),
            None => heca_grid_ui::paint_child(w, &mut cx),
        }
    }
    scene
        .base_layer()
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) => r.border.map(|b| (b.color.r, b.color.g, b.color.b)),
            _ => None,
        })
        .collect()
}

/// **A control follows the hue its container published.** Without this a `Retry` inside a danger
/// notification would be theme-accent blue: a card cannot pass a colour at build time, because the
/// theme only exists at paint and changes on reload.
#[test]
fn a_button_takes_the_tone_its_container_published() {
    use heca_grid_ui::{Button, Color, LayoutExt, Length};

    // A Primary carries its accent border at rest; an Outline's is muted until hover, so it would
    // show nothing to compare without driving an animation.
    let mut plain = Button::primary("Retry")
        .width(Length::Px(90.0))
        .height(Length::Px(28.0));
    LayoutEngine::new().compute(&mut plain, Size::new(200.0, 60.0));
    let rest = chrome_hues(&mut plain, None);

    let tone = Color::rgb(240, 80, 60);
    let toned = chrome_hues(&mut plain, Some(tone));

    assert!(
        !rest.is_empty(),
        "a primary button draws a border to compare"
    );
    assert_ne!(rest, toned, "the published tone must reach the chrome");
    assert!(
        toned
            .iter()
            .any(|&(r, g, b)| (r, g, b) == (tone.r, tone.g, tone.b)),
        "and it is the tone that was published, not some blend of it: {toned:?}",
    );
}

/// **Retro-compatibility, stated as a test.** Nothing publishes a control tone by default, so a
/// control outside such a container paints exactly what it always did. This is what makes the
/// channel safe to add to a library where six widgets already publish a *content* colour.
#[test]
fn a_control_outside_a_publishing_container_is_unchanged() {
    use heca_grid_ui::{Glyph, Icon, IconButton, LayoutExt, Length};

    let mut b = IconButton::new(Icon::new(Glyph::Close))
        .active(true) // a held-on frame, so it draws chrome at rest
        .width(Length::Px(28.0))
        .height(Length::Px(28.0));
    LayoutEngine::new().compute(&mut b, Size::new(60.0, 60.0));
    let theme = Theme::default();
    let accent = theme.colors.accent;
    let hues = chrome_hues(&mut b, None);
    assert!(
        !hues.is_empty(),
        "the control must draw a bordered hue to compare"
    );
    assert!(
        hues.iter()
            .all(|&(r, g, b)| (r, g, b) == (accent.r, accent.g, accent.b)),
        "with nothing published, the theme accent is still the hue: {hues:?}",
    );
}

/// An explicit tone is the control's own decision and outranks the container's.
#[test]
fn an_explicit_tone_wins_over_the_container() {
    use heca_grid_ui::{Color, Glyph, Icon, IconButton, LayoutExt, Length};

    let own = Color::rgb(10, 200, 120);
    let mut b = IconButton::new(Icon::new(Glyph::Close))
        .accent(own)
        .active(true)
        .width(Length::Px(28.0))
        .height(Length::Px(28.0));
    LayoutEngine::new().compute(&mut b, Size::new(60.0, 60.0));
    let hues = chrome_hues(&mut b, Some(Color::rgb(240, 80, 60)));
    assert!(
        !hues.is_empty(),
        "the control must draw a bordered hue to compare"
    );
    assert!(
        hues.iter()
            .all(|&(r, g, b)| (r, g, b) == (own.r, own.g, own.b)),
        "the control's own tone must win: {hues:?}",
    );
}

/// **A container publishes its hue with one builder, and everything inside follows.**
///
/// The tone had consumers and no publishers: the only way to set one was for a widget to call
/// `with_control_tone` inside its own `paint`, so a plain container could not publish at all. The
/// remaining way to re-tint a subtree was to paint it under a *swapped theme* — which needs a
/// separate paint call per subtree, and is what stopped a container from painting its own children.
#[test]
fn a_container_publishes_its_hue_to_everything_inside_it() {
    use heca_grid_ui::widgets::Surface;
    use heca_grid_ui::{Button, Color, ComponentExt, LayoutExt, Parent};

    let tone = Color::rgb(240, 80, 60);
    let mut card = Surface::new()
        .accent(tone)
        .width(200)
        .height(60)
        .child(Button::primary("Retry").width(90).height(28));
    LayoutEngine::new().compute(&mut card, Size::new(200.0, 60.0));

    let hues = chrome_hues(&mut card, None);
    assert!(
        hues.iter()
            .any(|&(r, g, b)| (r, g, b) == (tone.r, tone.g, tone.b)),
        "the button inside was never told anything, and still follows: {hues:?}",
    );
}

/// **A declared meaning is not re-toned.** What a variant *names* is not decoration a container may
/// restyle — the same rule that keeps a destructive button destructive inside a warning-toned card.
#[test]
fn a_published_hue_does_not_redefine_what_a_variant_means() {
    use heca_grid_ui::widgets::Surface;
    use heca_grid_ui::{Badge, Color, ComponentExt, DrawCommand, LayoutExt, Parent};

    let tone = Color::rgb(240, 80, 60);
    let theme = Theme::default();
    let mut card = Surface::new()
        .accent(tone)
        .width(200)
        .height(60)
        .child(Badge::accent("3"));
    LayoutEngine::new().compute(&mut card, Size::new(200.0, 60.0));

    let scene = common::paint_via_child(&card, &theme);
    let fills: Vec<(u8, u8, u8)> = scene
        .base_layer()
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) => Some((r.fill.r, r.fill.g, r.fill.b)),
            _ => None,
        })
        .collect();
    assert!(!fills.is_empty(), "the badge must paint a fill to compare");
    assert!(
        !fills
            .iter()
            .any(|&(r, g, b)| (r, g, b) == (tone.r, tone.g, tone.b)),
        "an accent badge keeps the colour its variant names, not the container's: {fills:?}",
    );
}

/// **A widget's own hue and the hue it passes down are one property.** They were two — a private
/// `tone` field on `Button` and `IconButton`, beside the inherited channel — so the same word meant
/// "mine" on two widgets and "everything inside me" everywhere else.
#[test]
fn a_widgets_own_hue_outranks_the_one_it_inherits() {
    use heca_grid_ui::widgets::Surface;
    use heca_grid_ui::{Button, Color, ComponentExt, LayoutExt, Parent};

    let outer = Color::rgb(240, 80, 60);
    let own = Color::rgb(10, 200, 120);
    let mut card = Surface::new()
        .accent(outer)
        .width(200)
        .height(60)
        .child(Button::primary("Retry").accent(own).width(90).height(28));
    LayoutEngine::new().compute(&mut card, Size::new(200.0, 60.0));

    let hues = chrome_hues(&mut card, None);
    assert!(
        hues.iter()
            .any(|&(r, g, b)| (r, g, b) == (own.r, own.g, own.b)),
        "the innermost declaration wins: {hues:?}",
    );
    assert!(
        !hues
            .iter()
            .any(|&(r, g, b)| (r, g, b) == (outer.r, outer.g, outer.b)),
        "…and the container's hue does not also reach it: {hues:?}",
    );
}
