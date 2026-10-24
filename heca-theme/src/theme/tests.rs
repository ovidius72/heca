use super::*;

/// The active pane's halo is a theme token, with the values the app used to hard-code.
#[test]
fn the_active_glow_is_a_theme_token_with_todays_defaults() {
    let theme = Theme::default();
    assert_eq!(theme.active_glow_radius, 10.0);
    assert_eq!(theme.drag_image_alpha, 0.6);
    assert_eq!(theme.drag_edge_width, 3.0);
    assert_eq!(theme.drag_edge_color, None);
    assert_eq!(theme.drag_image_scale, 0.5);
    assert_eq!(theme.active_glow_strength, 0.55);
}

#[test]
fn default_theme_is_grid_tron() {
    let theme = Theme::default();
    assert_eq!(theme.name, "Grid Tron");
}

#[test]
fn workspace_washes_are_optional_in_bundled_and_custom_themes() {
    for source in [include_str!("../themes/grid_tron.toml"), include_str!("../themes/mocha.toml"), include_str!("../themes/latte.toml")] {
        let theme: Theme = toml::from_str(source).unwrap();
        assert_eq!(theme.active_wash_alpha, 0.0);
        let custom = source.lines().filter(|line| !line.starts_with("active_wash_alpha"))
            .collect::<Vec<_>>().join("\n");
        let theme: Theme = toml::from_str(&custom).unwrap();
        assert_eq!(theme.active_wash_alpha, 0.0, "omitted token must stay off");
    }
}

#[test]
fn control_radius_is_half_of_radius() {
    let theme = Theme::grid_tron();
    assert!((theme.control_radius() - 2.0).abs() < f32::EPSILON);
}

/// **The picker's letters follow the theme, and a theme may pin them**.
///
/// Unset, they wear the accent — which is the colour they always had, so a theme that says
/// nothing looks exactly as before. A theme that names one wins, and because it is a theme token
/// rather than a user setting it travels with the theme and moves on a reload.
#[test]
fn the_picker_letters_follow_the_accent_until_a_theme_names_a_colour() {
    let mut theme = Theme::grid_tron();
    theme.hint_color = None;
    assert_eq!(
        theme.effective_hint_color(),
        theme.accent,
        "saying nothing keeps the colour the letters always had",
    );

    let named = Color::new(0xff, 0xb8, 0x6c, 0xff);
    theme.hint_color = Some(named);
    assert_eq!(theme.effective_hint_color(), named);
    assert_ne!(
        named, theme.accent,
        "…and it is not merely the accent again"
    );
}

/// A theme file may carry the token, and one that omits it still parses — every bundled theme
/// does.
#[test]
fn a_theme_file_may_name_the_picker_colour_and_may_omit_it() {
    let bundled: Theme = toml::from_str(include_str!("../themes/latte.toml"))
        .expect("bundled latte.toml must parse into Theme");
    assert_eq!(
        bundled.effective_hint_color(),
        bundled.accent,
        "a theme that names none falls back to its own accent",
    );

    let named: Theme = toml::from_str(&format!(
        "{}\nhint_color = \"#ffb86c\"\n",
        include_str!("../themes/latte.toml")
    ))
    .expect("a theme naming hint_color must parse");
    assert_eq!(named.hint_color, Some(Color::new(0xff, 0xb8, 0x6c, 0xff)));
}

#[test]
fn focus_ring_is_theme_aware_and_respects_override() {
    // Dark theme (grid_tron): the derived ring is *lighter* than the accent (shifted toward the
    // light foreground) so it separates from the accent border.
    let mut dark = Theme::grid_tron();
    dark.focus_ring = None;
    let d = dark.effective_focus_ring();
    assert_ne!(d, dark.accent);
    assert!(
        d.luminance() > dark.accent.luminance(),
        "dark theme: ring brighter than accent"
    );

    // Light theme (latte): the SAME logic derives a *darker* ring (shifted toward the dark
    // foreground) — the direction flips automatically, no hardcoded light/dark.
    let light: Theme = toml::from_str(include_str!("../themes/latte.toml"))
        .expect("bundled latte.toml must parse into Theme");
    let l = light.effective_focus_ring();
    assert!(
        l.luminance() < light.accent.luminance(),
        "light theme: ring darker than accent"
    );

    // Any tone derives via the same helper (this is how the destructive/danger ring is built).
    assert_eq!(
        dark.focus_ring_tone(dark.danger),
        dark.danger
            .lerp(dark.foreground, FOCUS_RING_CONTRAST_FACTOR)
    );

    // The `focus_ring` token overrides only the default/accent case, verbatim.
    dark.focus_ring = Some(Color::rgb(10, 20, 30));
    assert_eq!(dark.effective_focus_ring(), Color::rgb(10, 20, 30));
}

#[test]
fn chrome_background_tokens_derive_from_background_when_unset() {
    let mut theme = Theme::grid_tron();
    theme.left_sidebar_background = None;
    theme.right_sidebar_background = None;
    theme.top_bottom_pane_background = None;

    assert_ne!(theme.effective_left_sidebar_background(), theme.background);
    assert_ne!(theme.effective_right_sidebar_background(), theme.background);
    assert_ne!(
        theme.effective_top_bottom_pane_background(),
        theme.background
    );
}

#[test]
fn glow_level_radius_scales() {
    assert!((GlowLevel::None.radius_scale()).abs() < f32::EPSILON);
    assert!((GlowLevel::Thin.radius_scale() - 0.5).abs() < f32::EPSILON);
    assert!((GlowLevel::Medium.radius_scale() - 1.0).abs() < f32::EPSILON);
    assert!((GlowLevel::Large.radius_scale() - 2.0).abs() < f32::EPSILON);
}

#[test]
fn glow_level_strength_scales() {
    // GlowLevel is the sole owner of glow strength. The curve started as the
    // former Intensity::glow_scale() values; `Thin` was since raised 0.5 → 0.75
    // because at 0.5× strength on a 0.5× radius the faint rest glows read
    // as None. `Thin` stays "thin" via its 0.5× *radius*, not a faded alpha.
    assert!((GlowLevel::None.strength_scale()).abs() < f32::EPSILON);
    assert!((GlowLevel::Thin.strength_scale() - 0.75).abs() < f32::EPSILON);
    assert!((GlowLevel::Medium.strength_scale() - 1.0).abs() < f32::EPSILON);
    assert!((GlowLevel::Large.strength_scale() - 1.6).abs() < f32::EPSILON);
}

#[test]
fn intensity_scanline_opacity_is_separate_from_glow() {
    // Intensity owns scanlines only; it no longer exposes glow_scale().
    // Pin exact values (not just ordering) so an accidental curve change is caught.
    assert!((Intensity::Off.scanline_opacity()).abs() < f32::EPSILON);
    assert!((Intensity::Low.scanline_opacity() - 0.05).abs() < 1e-6);
    assert!((Intensity::Medium.scanline_opacity() - 0.11).abs() < 1e-6);
    assert!((Intensity::Heavy.scanline_opacity() - 0.20).abs() < 1e-6);
    // Ordering still holds as a sanity check.
    assert!(Intensity::Medium.scanline_opacity() > Intensity::Low.scanline_opacity());
    assert!(Intensity::Heavy.scanline_opacity() > Intensity::Medium.scanline_opacity());
}

#[test]
fn glow_level_parse_case_insensitive() {
    assert_eq!(GlowLevel::parse("NONE"), GlowLevel::None);
    assert_eq!(GlowLevel::parse("thin"), GlowLevel::Thin);
    assert_eq!(GlowLevel::parse("LARGE"), GlowLevel::Large);
    assert_eq!(GlowLevel::parse("unknown"), GlowLevel::Medium);
}

#[test]
fn intensity_next_cycles() {
    assert_eq!(Intensity::Off.next(), Intensity::Low);
    assert_eq!(Intensity::Low.next(), Intensity::Medium);
    assert_eq!(Intensity::Medium.next(), Intensity::Heavy);
    assert_eq!(Intensity::Heavy.next(), Intensity::Off);
}

#[test]
fn gradient_colors_derive_from_background_when_unset() {
    let mut theme = Theme::grid_tron();
    theme.background_gradient_top = None;
    theme.background_gradient_bottom = None;
    // Top falls back to the theme background.
    assert_eq!(theme.effective_background_gradient_top(), theme.background);
    // Bottom falls back to a slightly darkened background (not the raw
    // background — subtle vertical depth).
    assert_ne!(
        theme.effective_background_gradient_bottom(),
        theme.background
    );
}

/// Bundled themes ship explicit z=0 gradient colors in their TOMLs. This pins
/// that contract so a dropped key is caught (the resolver would silently fall
/// back to the derived single-color gradient otherwise). Iterates the loader's
/// bundled-theme map so a newly added bundled theme is covered automatically.
#[test]
fn bundled_themes_ship_explicit_gradient_colors() {
    for name in crate::loader::bundled_themes().keys() {
        let theme = crate::load_theme(name);
        assert!(
            theme.background_gradient_top.is_some(),
            "{name} should set background_gradient_top in its TOML"
        );
        assert!(
            theme.background_gradient_bottom.is_some(),
            "{name} should set background_gradient_bottom in its TOML"
        );
        // Top and bottom differ so the gradient is actually a gradient
        // (not a flat fill that hides the blur).
        assert_ne!(
            theme.effective_background_gradient_top(),
            theme.effective_background_gradient_bottom(),
            "{name} gradient top and bottom should differ"
        );
    }
}

/// `Shadow.color` is a `Color` (not a `String`) - pinned so a regression to
/// `String` is caught. The bundled grid_tron TOML writes
/// `shadow = { color = "#000000", alpha = 0.3, blur = 8.0 }`; serde must parse
/// the hex string into a `Color` via `Color`'s `try_from<String>`.
#[test]
fn shadow_color_is_color_from_toml_hex() {
    let theme = crate::load_theme("grid_tron");
    assert_eq!(theme.shadow.color, Color::rgb(0, 0, 0));
    assert!((theme.shadow.alpha - 0.3).abs() < f32::EPSILON);
    assert!((theme.shadow.blur - 8.0).abs() < f32::EPSILON);
}

/// Serde round-trip: a `Shadow` with a hex `color` deserializes to a `Color`
/// and serializes back, so the TOML config representation stays stable.
#[test]
fn shadow_serde_roundtrips_color() {
    let toml = "color = \"#89b4fa\"\nalpha = 0.25\nblur = 12.0\n";
    let s: Shadow = toml::from_str(toml).unwrap();
    assert_eq!(s.color, Color::rgb(0x89, 0xb4, 0xfa));
    assert!((s.alpha - 0.25).abs() < f32::EPSILON);
    assert!((s.blur - 12.0).abs() < f32::EPSILON);
    let reserialized: String = toml::to_string(&s).unwrap();
    let s2: Shadow = toml::from_str(&reserialized).unwrap();
    assert_eq!(s, s2);
}
