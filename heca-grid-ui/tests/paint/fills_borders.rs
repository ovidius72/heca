use super::*;

#[test]
fn paint_emits_background_rect_and_label_text() {
    let mut root = Surface::new()
        .background(Color::rgb(10, 10, 10))
        .child(Label::new("HI"));
    // Laid out before it is painted, because that is the only order the app ever paints in — and
    // since `a_widget_that_has_never_been_laid_out_paints_nothing` a child with no box draws
    // nothing at all, rather than drawing at the window's origin.
    LayoutEngine::new()
        .base_font(13.0)
        .compute(&mut root, Size::new(300.0, 120.0));

    let theme = Theme::default();
    let scene = common::paint(&root, &theme);

    let rects = common::rects(&scene).len();
    let texts = common::texts(&scene).len();
    assert_eq!(rects, 1, "container background should emit one rect");
    assert_eq!(texts, 1, "label should emit one text run");
}

#[test]
fn surface_paints_styled_rect_with_border() {
    let theme = Theme::default();
    let surface = Surface::new()
        .background(Color::rgb(12, 18, 24))
        .border(theme.colors.accent, 1.5)
        .glow(theme.colors.glow);

    let scene = common::paint(&surface, &theme);
    let has_bordered = scene
        .iter()
        .any(|c| matches!(c, DrawCommand::Rect(r) if r.border.is_some() && r.glow.is_some()));
    assert!(has_bordered, "surface should emit a bordered, glowing rect");
}

#[test]
fn bordered_pane_border_width_follows_theme_and_vanishes_at_zero() {
    // A default `Bordered` Pane with NO explicit `.border()` derives its border
    // from `theme.colors.border_width` (the global border control): a theme-colored
    // border when borders are on, and nothing at `border_width == 0`. This is the
    // consistency contract — the global control governs every container.
    use heca_grid_ui::Pane;
    // `explicit_border` width is the literal a caller passes to `.border()` — it must
    // be ignored in favour of the live theme width, so a build-time literal can't
    // survive a global border change (the showcase bug). `None` ⇒ no `.border()`.
    let border_rects = |theme: &Theme,
                        explicit: Option<(heca_grid_ui::Color, f32)>|
     -> Vec<heca_grid_ui::scene::RectCmd> {
        let mut p = Pane::new()
            .background(theme.colors.surface)
            .width(Length::Px(120.0))
            .height(Length::Px(80.0));
        if let Some((c, w)) = explicit {
            p = p.border(c, w);
        }
        LayoutEngine::new().compute(&mut p, Size::new(200.0, 200.0));
        LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
        let scene = common::paint(&p, theme);
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) if r.border.is_some_and(|b| b.width > 0.0) => Some(*r),
                _ => None,
            })
            .collect()
    };

    let mut theme = Theme::default();
    theme.colors.border_width = 2.0;
    let on = border_rects(&theme, None);
    assert!(
        on.iter().any(|r| r
            .border
            .is_some_and(|b| b.color == theme.colors.border && b.width == 2.0)),
        "Bordered pane draws theme.colors.border at theme.colors.border_width without an explicit .border()",
    );

    // An explicit `.border(accent, 9.0)` keeps the COLOR but the width follows the
    // theme (2.0), never the 9.0 literal.
    let explicit = border_rects(&theme, Some((theme.colors.accent, 9.0)));
    assert!(
        explicit.iter().any(|r| r
            .border
            .is_some_and(|b| b.color == theme.colors.accent && b.width == 2.0)),
        "explicit .border() supplies color only; width tracks theme.colors.border_width",
    );

    theme.colors.border_width = 0.0;
    assert!(
        border_rects(&theme, None).is_empty()
            && border_rects(&theme, Some((theme.colors.accent, 9.0))).is_empty(),
        "border_width == 0 leaves the Bordered pane with no visible border, even with an explicit .border()",
    );
}

#[test]
fn bordered_pane_border_width_override_is_independent_of_theme() {
    // `.border_width(w)` pins a Bordered pane's frame width regardless of the
    // global `theme.colors.border_width` — the seam that lets the sidebar shell carry its
    // own thickness (`[appearance] sidebar_border_width`). Color still resolves
    // from `.border(color, _)` when set, else `theme.colors.border`.
    use heca_grid_ui::{Color, Pane};
    let border_rects = |theme: &Theme,
                        override_w: Option<f32>,
                        explicit: Option<Color>|
     -> Vec<heca_grid_ui::scene::RectCmd> {
        let mut p = Pane::new()
            .background(theme.colors.surface)
            .border_width(override_w)
            .width(Length::Px(120.0))
            .height(Length::Px(80.0));
        if let Some(c) = explicit {
            p = p.border(c, 0.0);
        }
        LayoutEngine::new().compute(&mut p, Size::new(200.0, 200.0));
        LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
        let scene = common::paint(&p, theme);
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) if r.border.is_some_and(|b| b.width > 0.0) => Some(*r),
                _ => None,
            })
            .collect()
    };

    let mut theme = Theme::default();

    // Theme borders OFF, but the override forces a 3px frame in theme.colors.border.
    theme.colors.border_width = 0.0;
    let forced = border_rects(&theme, Some(3.0), None);
    assert!(
        forced.iter().any(|r| r
            .border
            .is_some_and(|b| b.color == theme.colors.border && b.width == 3.0)),
        "override draws its own width even when the global border is off",
    );

    // Override width + explicit color: width = override, color = explicit.
    let colored = border_rects(&theme, Some(3.0), Some(theme.colors.accent));
    assert!(
        colored.iter().any(|r| r
            .border
            .is_some_and(|b| b.color == theme.colors.accent && b.width == 3.0)),
        "override sets width; explicit .border() sets color",
    );

    // override = 0 ⇒ no border, even with the global border ON.
    theme.colors.border_width = 5.0;
    assert!(
        border_rects(&theme, Some(0.0), None).is_empty(),
        "override of 0 removes the border regardless of the global width",
    );
}

/// Paint `w` under `border_width == 0` and return every visible (width>0) Rect
/// border stroke it emitted.
fn visible_border_widths_at_zero<C: heca_grid_ui::Component>(mut w: C) -> Vec<f32> {
    let mut theme = Theme::default();
    theme.colors.border_width = 0.0;
    let vp = Size::new(400.0, 200.0);
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(&mut w, vp);
    let scene = common::paint_in(&w, &theme, vp);
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) => r.border.map(|b| b.width),
            _ => None,
        })
        .filter(|w| *w > 0.0)
        .collect()
}

#[test]
fn non_container_widgets_drop_their_border_at_zero_border_width() {
    use heca_grid_ui::{Alert, Badge, Button, ProgressBar, Toggle};
    // Guard against the recurring regression: a widget that hardcodes a border
    // stroke instead of routing it through the theme (cx.border / border_width).
    for (name, widths) in [
        (
            "button",
            visible_border_widths_at_zero(Button::primary("OK")),
        ),
        ("badge", visible_border_widths_at_zero(Badge::success("ON"))),
        (
            "alert",
            visible_border_widths_at_zero(Alert::warning("W").body("b")),
        ),
        (
            "progress",
            visible_border_widths_at_zero(ProgressBar::new().value(0.5)),
        ),
        (
            "toggle",
            visible_border_widths_at_zero(Toggle::new().on(true)),
        ),
    ] {
        assert!(
            widths.is_empty(),
            "{name}: expected no border at border_width=0, got {widths:?}"
        );
    }
}
