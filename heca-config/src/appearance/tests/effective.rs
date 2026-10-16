use super::super::*;

#[test]
fn amounts_map_to_derived_values() {
    let cfg = AppearanceConfig {
        transparency: 25,
        blur: 50,
        terminal: TerminalAppearance {
            transparency: 40,
            ..Default::default()
        },
        vibrancy: Vibrancy::Sidebar,
        ..Default::default()
    };
    assert!((cfg.opacity() - 0.75).abs() < 1e-6);
    assert!(cfg.is_transparent());
    assert!((cfg.blur_radius() - 24.0).abs() < 1e-6); // 50% of 48px
    assert!((cfg.terminal_opacity() - 0.6).abs() < 1e-6);
    assert_eq!(cfg.os_vibrancy(), Some(Vibrancy::Sidebar));
}

#[test]
fn z0_background_knobs_map_to_derived_values() {
    let cfg = AppearanceConfig {
        background_blur: 50,
        background_transparency: 25,
        ..Default::default()
    };
    // blur 50 → 50% of MAX_BLUR_PX (24.0).
    assert!((cfg.background_blur_radius() - 24.0).abs() < 1e-6);
    // transparency 25 → alpha 0.75.
    assert!((cfg.background_alpha() - 0.75).abs() < 1e-6);

    // Over-cap clamps.
    let over = AppearanceConfig {
        background_blur: 200,
        background_transparency: 200,
        ..Default::default()
    };
    assert!((over.background_blur_radius() - MAX_BLUR_PX).abs() < 1e-6);
    assert!((over.background_alpha() - 0.0).abs() < f32::EPSILON);
}

#[test]
fn z0_gradient_default_resolves_to_theme_colors() {
    let mocha = crate::theme::load("mocha");
    let cfg = AppearanceConfig::default();
    // No config override → inherits the theme's explicit TOML gradient
    // colors (mocha ships them).
    let top = cfg.effective_background_gradient_top(&mocha);
    let bottom = cfg.effective_background_gradient_bottom(&mocha);
    assert_eq!(top, mocha.background_gradient_top.unwrap());
    assert_eq!(bottom, mocha.background_gradient_bottom.unwrap());
    assert_ne!(top, bottom);
}

#[test]
fn z0_gradient_config_override_wins_over_theme() {
    let mocha = crate::theme::load("mocha");
    let cfg = AppearanceConfig {
        background_gradient_top: Some(Color::new(1, 2, 3, 255)),
        background_gradient_bottom: Some(Color::new(4, 5, 6, 255)),
        ..Default::default()
    };
    assert_eq!(
        cfg.effective_background_gradient_top(&mocha),
        Color::new(1, 2, 3, 255)
    );
    assert_eq!(
        cfg.effective_background_gradient_bottom(&mocha),
        Color::new(4, 5, 6, 255)
    );
}

#[test]
fn z0_gradient_unset_theme_fields_fall_back_to_derived() {
    let mocha = crate::theme::load("mocha");
    let mut bare = mocha.clone();
    bare.background_gradient_top = None;
    bare.background_gradient_bottom = None;
    let cfg = AppearanceConfig::default();
    // top = background; bottom = darker(background) (≠ background).
    assert_eq!(
        cfg.effective_background_gradient_top(&bare),
        bare.background
    );
    assert_ne!(
        cfg.effective_background_gradient_bottom(&bare),
        bare.background
    );
}

#[test]
fn effect_token_defaults_unset() {
    let cfg = AppearanceConfig::default();
    assert_eq!(cfg.glow_size, None);
    assert_eq!(cfg.intensity, None);
}

#[test]
fn effect_token_resolvers_inherit_theme_when_unset() {
    let mocha = crate::theme::load("mocha");
    let cfg = AppearanceConfig::default();
    // mocha ships glow_size = GlowLevel::None (the "no glow" variant),
    // intensity = Intensity::Off. (Not to be confused with cfg.glow_size,
    // the Option<GlowLevel> field, which is Option::None for a default config.)
    assert_eq!(cfg.effective_glow_size(&mocha), mocha.glow_size);
    assert_eq!(cfg.effective_intensity(&mocha), mocha.intensity);

    // grid_tron ships glow_size = Medium, intensity = Medium.
    let grid_tron = crate::theme::load("grid_tron");
    assert_eq!(cfg.effective_glow_size(&grid_tron), grid_tron.glow_size);
    assert_eq!(cfg.effective_intensity(&grid_tron), grid_tron.intensity);
}

/// **The picker's letters: theme first, config on top** — the same two-step every effect token
/// takes, so a user who wants one colour across every theme sets it once.
#[test]
fn the_picker_colour_falls_back_to_the_theme_then_to_the_accent() {
    let grid_tron = crate::theme::load("grid_tron");
    let cfg = AppearanceConfig::default();
    assert_eq!(
        cfg.effective_hint_color(&grid_tron),
        grid_tron.accent,
        "no setting and no theme token: the letters wear the accent",
    );

    let mut themed = grid_tron.clone();
    themed.hint_color = Some(Color::rgb(0xff, 0xb8, 0x6c));
    assert_eq!(
        cfg.effective_hint_color(&themed),
        Color::rgb(0xff, 0xb8, 0x6c),
        "a theme that names one wins over the accent",
    );

    let over = AppearanceConfig {
        hint_color: Some(Color::rgb(0x00, 0xff, 0x00)),
        ..Default::default()
    };
    assert_eq!(
        over.effective_hint_color(&themed),
        Color::rgb(0x00, 0xff, 0x00),
        "and the setting wins over the theme",
    );
}

#[test]
fn effect_token_config_override_wins_over_theme() {
    let mocha = crate::theme::load("mocha");
    // mocha defaults: glow_size = None, intensity = Off.
    let cfg = AppearanceConfig {
        glow_size: Some(GlowLevel::Large),
        intensity: Some(Intensity::Heavy),
        ..Default::default()
    };
    assert_eq!(cfg.effective_glow_size(&mocha), GlowLevel::Large);
    assert_eq!(cfg.effective_intensity(&mocha), Intensity::Heavy);
    // The theme value is untouched — override is read-only at resolve time.
    assert_eq!(mocha.glow_size, GlowLevel::None);
    assert_eq!(mocha.intensity, Intensity::Off);
}

#[test]
fn effect_tokens_parse_snake_case() {
    // Tests the serde contract of the re-exported heca-theme types
    // (`GlowLevel`/`Intensity` derive Deserialize with `#[serde(rename_all =
    // "snake_case")]`) at the config boundary where TOML values flow into
    // `AppearanceConfig`. If the enum serde representation changes in
    // `heca-theme`, this test breaks — which is the intended signal here.
    #[derive(Deserialize)]
    struct Wrapper {
        glow_size: GlowLevel,
        intensity: Intensity,
    }
    let w: Wrapper = toml::from_str(
        r#"glow_size = "large"
intensity = "heavy""#,
    )
    .expect("effect tokens should parse snake_case");
    assert_eq!(w.glow_size, GlowLevel::Large);
    assert_eq!(w.intensity, Intensity::Heavy);

    let off: Wrapper = toml::from_str(
        r#"glow_size = "none"
intensity = "off""#,
    )
    .expect("off values should parse");
    assert_eq!(off.glow_size, GlowLevel::None);
    assert_eq!(off.intensity, Intensity::Off);
}
