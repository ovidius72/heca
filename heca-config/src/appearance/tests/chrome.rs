use super::super::*;

#[test]
fn floating_terminal_knobs_are_independent_of_tiled() {
    // Tiled translucent while floating defaults to opaque.
    let cfg = AppearanceConfig {
        terminal: TerminalAppearance {
            transparency: 95,
            floating_transparency: 0,
            floating_blur: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!((cfg.terminal_opacity() - 0.05).abs() < 1e-6);
    assert!((cfg.terminal_floating_opacity() - 1.0).abs() < f32::EPSILON);
    assert!((cfg.terminal_floating_blur_radius()).abs() < f32::EPSILON);

    // Floating knobs can be set independently of the tiled ones.
    let cfg = AppearanceConfig {
        terminal: TerminalAppearance {
            floating_transparency: 50,
            floating_blur: 25,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!((cfg.terminal_floating_opacity() - 0.5).abs() < 1e-6);
    assert!((cfg.terminal_floating_blur_radius() - 12.0).abs() < 1e-6); // 25% of 48px
}

#[test]
fn floating_border_color_resolves_config_then_float_accent() {
    let mocha = crate::theme::load("mocha");

    // Default: no config override -> theme.float_accent.
    let cfg = AppearanceConfig::default();
    assert_eq!(
        cfg.effective_pane_floating_border_color(&mocha),
        mocha.float_accent
    );

    // Config override wins.
    let cfg = AppearanceConfig {
        pane: PaneAppearance {
            floating_border_color: Some(Color::new(1, 2, 3, 255)),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        cfg.effective_pane_floating_border_color(&mocha),
        Color::new(1, 2, 3, 255)
    );
}

/// The overlay-panel frame is a config/theme decision, never hardcoded: an
/// `[appearance] overlay_border_style` override wins, otherwise the theme's
/// `overlay_frame` (themes default `bracketed`).
#[test]
fn overlay_border_style_overrides_the_theme_else_inherits_it() {
    let mut theme = Theme::default();

    let dflt = AppearanceConfig::default();
    assert_eq!(dflt.overlay_border_style, None, "unset by default");
    assert_eq!(
        dflt.effective_overlay_border_style(&theme),
        FrameStyle::Bracketed,
        "inherits the theme (bracket reticle)"
    );

    // The theme, not the config, moves it when the user hasn't overridden.
    theme.overlay_frame = FrameStyle::None;
    assert_eq!(
        dflt.effective_overlay_border_style(&theme),
        FrameStyle::None
    );

    // A config override wins over the theme, and parses snake_case.
    let cfg: AppearanceConfig = toml::from_str("overlay_border_style = \"bordered\"")
        .expect("overlay_border_style should parse snake_case");
    assert_eq!(cfg.overlay_border_style, Some(BorderStyle::Bordered));
    assert_eq!(
        cfg.effective_overlay_border_style(&theme),
        FrameStyle::Bordered,
        "config override beats the theme"
    );
}

#[test]
fn border_style_parses_snake_case_and_resolves_defaults() {
    let cfg: AppearanceConfig =
        toml::from_str("[pane]\nborder_style = \"bracketed\"\n[sidebar]\nborder_style = \"none\"")
            .expect("border styles should parse snake_case");
    assert_eq!(cfg.pane.border_style, Some(BorderStyle::Bracketed));
    assert_eq!(cfg.sidebar.border_style, Some(BorderStyle::None));
    assert_eq!(cfg.effective_pane_border_style(), BorderStyle::Bracketed);
    assert_eq!(cfg.effective_sidebar_border_style(), BorderStyle::None);

    // Defaults match the current app look: panes bordered, sidebar bracketed.
    let dflt = AppearanceConfig::default();
    assert_eq!(dflt.effective_pane_border_style(), BorderStyle::Bordered);
    assert_eq!(
        dflt.effective_sidebar_border_style(),
        BorderStyle::Bracketed
    );
}

#[test]
fn sidebar_border_width_resolves_config_then_global_then_theme() {
    let mocha = crate::theme::load("mocha");

    // Unset → falls back to the global border_width (here the theme value).
    let dflt = AppearanceConfig::default();
    assert_eq!(
        dflt.effective_sidebar_border_width(&mocha),
        dflt.effective_border_width(&mocha)
    );

    // Global border_width set, sidebar unset → sidebar inherits the global.
    let cfg = AppearanceConfig {
        border_width: Some(4.0),
        ..Default::default()
    };
    assert_eq!(cfg.effective_sidebar_border_width(&mocha), 4.0);

    // Sidebar-specific override wins over the global, and is clamped to [0, 10].
    let cfg = AppearanceConfig {
        border_width: Some(4.0),
        sidebar: SidebarAppearance {
            border_width: Some(99.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(cfg.effective_sidebar_border_width(&mocha), 10.0);
}

#[test]
fn sidebar_background_and_radius_resolve_config_then_fallback() {
    let mocha = crate::theme::load("mocha");
    let theme_surface = Color::new(9, 9, 9, 255);

    // Unset → the theme-derived surface passed in / the theme radius.
    let dflt = AppearanceConfig::default();
    assert_eq!(
        dflt.effective_sidebar_background_color(theme_surface),
        theme_surface
    );
    assert_eq!(
        dflt.effective_sidebar_border_radius(&mocha),
        mocha.border_radius
    );

    // Config override wins (radius clamped to [0, 40]).
    let cfg = AppearanceConfig {
        sidebar: SidebarAppearance {
            background_color: Some(Color::new(1, 2, 3, 255)),
            border_radius: Some(99.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        cfg.effective_sidebar_background_color(theme_surface),
        Color::new(1, 2, 3, 255)
    );
    assert_eq!(cfg.effective_sidebar_border_radius(&mocha), 40.0);
}

#[test]
fn global_border_width_drives_chrome_and_pane_fallback() {
    let theme = crate::theme::Theme::default();
    // Unset: global + pane both fall back to the theme width.
    let dflt = AppearanceConfig::default();
    assert_eq!(dflt.effective_border_width(&theme), theme.border_width);
    assert_eq!(dflt.effective_pane_border_width(&theme), theme.border_width);

    // Global override drives both chrome and the pane fallback.
    let g = AppearanceConfig {
        border_width: Some(3.0),
        ..Default::default()
    };
    assert_eq!(g.effective_border_width(&theme), 3.0);
    assert_eq!(g.effective_pane_border_width(&theme), 3.0);

    // pane.border_width overrides the global for panes only.
    let p = AppearanceConfig {
        border_width: Some(3.0),
        pane: PaneAppearance {
            border_width: Some(1.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(p.effective_border_width(&theme), 3.0);
    assert_eq!(p.effective_pane_border_width(&theme), 1.0);
}

#[test]
fn focus_border_width_resolves_and_clamps() {
    // Unset: focus defaults to 1.5 (visible regardless of the decorative border).
    let dflt = AppearanceConfig::default();
    assert!((dflt.effective_focus_border_width() - 1.5).abs() < f32::EPSILON);

    // Explicit value passes through; out-of-range clamps to [0, 10].
    let cfg = AppearanceConfig {
        focus_border_width: Some(99.0),
        ..Default::default()
    };
    assert_eq!(cfg.effective_focus_border_width(), 10.0);
}

#[test]
fn pane_chrome_values_are_clamped_to_safe_ranges() {
    let theme = crate::theme::Theme::default();

    // Explicit config values above the cap clamp down into range.
    let over = AppearanceConfig {
        pane: PaneAppearance {
            border_width: Some(999.0),
            border_radius: Some(88.0),
            padding: Some(999.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(over.effective_pane_border_width(&theme), 10.0);
    assert_eq!(over.effective_pane_border_radius(&theme), 20.0);
    assert_eq!(over.effective_pane_padding(&theme), 20.0);

    // Negative values clamp to the lower bound (0).
    let under = AppearanceConfig {
        pane: PaneAppearance {
            border_width: Some(-5.0),
            border_radius: Some(-2.0),
            padding: Some(-1.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(under.effective_pane_border_width(&theme), 0.0);
    assert_eq!(under.effective_pane_border_radius(&theme), 0.0);
    assert_eq!(under.effective_pane_padding(&theme), 0.0);

    // Defaults (None) fall back to the theme then clamp — theme defaults
    // (border_width 1.0, border_radius 6.0) are already in range, and pane
    // padding defaults to 4.0.
    let dflt = AppearanceConfig::default();
    assert_eq!(dflt.effective_pane_border_width(&theme), theme.border_width);
    assert_eq!(
        dflt.effective_pane_border_radius(&theme),
        theme.border_radius
    );
    assert_eq!(dflt.effective_pane_padding(&theme), 4.0);

    // In-range values pass through unchanged.
    let mid = AppearanceConfig {
        pane: PaneAppearance {
            border_width: Some(4.0),
            border_radius: Some(15.0),
            padding: Some(8.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(mid.effective_pane_border_width(&theme), 4.0);
    assert_eq!(mid.effective_pane_border_radius(&theme), 15.0);
    assert_eq!(mid.effective_pane_padding(&theme), 8.0);
}

#[test]
fn pane_gap_floored_at_border_width_only_when_borders_visible() {
    let theme = crate::theme::Theme::default();

    // Borders ON (default style "bordered"): gap 0 floors up to the border
    // width so the two adjacent pane borders don't overlap on the shared edge.
    let zero = AppearanceConfig {
        pane: PaneAppearance {
            gap: Some(0.0),
            border_width: Some(3.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(zero.effective_pane_gap(&theme), 3.0);

    // A gap wider than the border is left untouched.
    let wide = AppearanceConfig {
        pane: PaneAppearance {
            gap: Some(12.0),
            border_width: Some(3.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(wide.effective_pane_gap(&theme), 12.0);

    // border_style = "none": no border is drawn, so gap 0 stays truly flush.
    let flush = AppearanceConfig {
        pane: PaneAppearance {
            gap: Some(0.0),
            border_style: Some(BorderStyle::None),
            border_width: Some(3.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(flush.effective_pane_gap(&theme), 0.0);
}
