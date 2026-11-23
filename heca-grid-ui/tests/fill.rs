//! **A background named by its theme colour follows the theme** — `Fill::theme("surface")`.

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Fill, LayoutEngine, PaintCx, Scene, Size, Theme};

fn fills(surface: &Surface, theme: &Theme) -> Vec<heca_grid_ui::Color> {
    let mut scene = Scene::new();
    heca_grid_ui::paint_child(surface, &mut PaintCx::new(&mut scene, theme));
    scene
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Rect(rect) => Some(rect.fill),
            _ => None,
        })
        .collect()
}

/// The same surface, painted under two themes with no rebuild, takes each theme's colour; a
/// colour written as a colour stays that colour; a name that is not a theme colour paints nothing.
#[test]
fn a_theme_fill_is_read_when_drawn_and_follows_the_theme() {
    let (mut dark, mut light) = (Theme::default(), Theme::default());
    dark.colors.surface = heca_grid_ui::Color::rgb(0x10, 0x10, 0x18);
    light.colors.surface = heca_grid_ui::Color::rgb(0xf0, 0xf0, 0xe8);

    let mut named = Surface::new()
        .background(Fill::theme("surface"))
        .width(Length::Px(40.0))
        .height(Length::Px(20.0));
    LayoutEngine::new().compute(&mut named, Size::new(100.0, 100.0));
    assert_eq!(fills(&named, &dark), [dark.colors.surface]);
    assert_eq!(
        fills(&named, &light),
        [light.colors.surface],
        "the same widget, the new theme"
    );

    let fixed_colour = heca_grid_ui::Color::rgb(0xff, 0, 0);
    let mut fixed = Surface::new()
        .background(fixed_colour)
        .width(Length::Px(40.0))
        .height(Length::Px(20.0));
    LayoutEngine::new().compute(&mut fixed, Size::new(100.0, 100.0));
    assert_eq!(
        fills(&fixed, &light),
        [fixed_colour],
        "a colour is not a theme name"
    );

    let mut unknown = Surface::new()
        .background(Fill::theme("chartreuse"))
        .width(Length::Px(40.0))
        .height(Length::Px(20.0));
    LayoutEngine::new().compute(&mut unknown, Size::new(100.0, 100.0));
    assert!(
        fills(&unknown, &dark).is_empty(),
        "not a colour of the theme: no fill"
    );
}

fn rects(surface: &Surface, theme: &Theme) -> Vec<heca_grid_ui::scene::RectCmd> {
    let mut scene = Scene::new();
    heca_grid_ui::paint_child(surface, &mut PaintCx::new(&mut scene, theme));
    scene
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Rect(rect) => Some(*rect),
            _ => None,
        })
        .collect()
}

/// **`elevation` gives a surface the panel look of a menu** — a drop shadow, then the theme's
/// surface fill at the theme's corner radius — read when drawn, so the same widget follows a new
/// theme. Without it the surface paints nothing of its own.
#[test]
fn an_elevated_surface_wears_the_panel_look_and_follows_the_theme() {
    use heca_grid_ui::widgets::PanelElevation;
    let mut panel = Surface::new()
        .elevation(PanelElevation::Panel)
        .width(Length::Percent(0.5))
        .height(Length::Percent(0.5));
    LayoutEngine::new().compute(&mut panel, Size::new(100.0, 100.0));

    let (mut one, mut two) = (Theme::default(), Theme::default());
    one.colors.surface = heca_grid_ui::Color::rgb(0x10, 0x10, 0x18);
    one.colors.border_radius = 3.0;
    two.colors.surface = heca_grid_ui::Color::rgb(0x30, 0x20, 0x10);
    two.colors.border_radius = 9.0;
    for theme in [&one, &two] {
        let drawn = rects(&panel, theme);
        assert!(drawn.iter().any(|r| r.shadow.is_some()), "a drop shadow");
        assert!(
            drawn
                .iter()
                .any(|r| r.fill == theme.colors.surface && r.radius == theme.colors.border_radius),
            "the theme's surface at the theme's radius: {drawn:?}"
        );
    }

    let mut plain = Surface::new()
        .width(Length::Percent(0.5))
        .height(Length::Percent(0.5));
    LayoutEngine::new().compute(&mut plain, Size::new(100.0, 100.0));
    assert!(rects(&plain, &one).is_empty(), "no elevation, no panel");
}

/// **A theme colour at a theme opacity, and a colour the theme derives** — both read when drawn:
/// `Fill::theme("background").alpha("scrim")` is the theme's background at its scrim alpha, and
/// a derived colour the theme leaves unset paints what the theme derives for it.
#[test]
fn a_theme_fill_takes_a_theme_alpha_and_a_derived_colour() {
    let mut theme = Theme::default();
    theme.colors.background = heca_grid_ui::Color::rgb(0x20, 0x30, 0x40);
    theme.colors.interaction.scrim = 90;
    theme.colors.top_bottom_pane_background = None;
    let painted = |fill: heca_grid_ui::ThemeFill| {
        let mut surface = Surface::new()
            .background(fill)
            .width(Length::Px(40.0))
            .height(Length::Px(20.0));
        LayoutEngine::new().compute(&mut surface, Size::new(100.0, 100.0));
        fills(&surface, &theme)
    };
    assert_eq!(
        painted(Fill::theme("background").alpha("scrim")),
        [theme.colors.background.with_alpha(90)],
        "the background at the scrim's opacity"
    );
    assert_eq!(
        painted(Fill::theme("top_bottom_pane_background")),
        [theme.colors.effective_top_bottom_pane_background()],
        "a derived colour paints what the theme derives"
    );
    assert!(
        painted(Fill::theme("background").alpha("chartreuse")).is_empty(),
        "an opacity that is not the theme's paints nothing"
    );
}
