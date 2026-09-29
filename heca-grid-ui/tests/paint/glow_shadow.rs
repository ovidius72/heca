use super::*;

#[test]
fn glow_none_suppresses_glow() {
    use heca_grid_ui::GlowLevel;
    // Glow is owned solely by `glow_size` now (intensity controls only scanlines),
    // so `GlowLevel::None` — not `Intensity::Off` — is what suppresses the glow.
    let mut theme = Theme::default();
    theme.colors.glow_size = GlowLevel::None;

    let root = Surface::new().glow(Color::rgb(64, 224, 255));
    let scene = common::paint(&root, &theme);

    let glow_present = scene.iter().any(|c| match c {
        DrawCommand::Rect(r) => r.glow.is_some(),
        _ => false,
    });
    assert!(
        !glow_present,
        "glow must be suppressed when glow_size is None"
    );

    // And with a glow size set, the glow survives.
    theme.colors.glow_size = GlowLevel::Medium;
    let scene2 = common::paint(&Surface::new().glow(Color::rgb(64, 224, 255)), &theme);
    let glow_present2 = scene2
        .iter()
        .any(|c| matches!(c, DrawCommand::Rect(r) if r.glow.is_some()));
    assert!(glow_present2, "glow present when glow_size is Medium");
}

#[test]
fn glow_strength_scales_with_glow_size() {
    use heca_grid_ui::GlowLevel;
    // `glow_size` owns glow STRENGTH (alpha), not just radius: the painted glow's
    // intensity scales by the level's `strength_scale` (thin 0.5×, medium 1.0×,
    // large 1.6×) through the single `scaled_glow` chokepoint, so config drives it.
    let intensity_at = |level: GlowLevel| -> f32 {
        let mut theme = Theme::default();
        theme.colors.glow_size = level;
        let scene = common::paint(&Surface::new().glow(Color::rgb(64, 224, 255)), &theme);
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) => r.glow.as_ref().map(|g| g.intensity),
                _ => None,
            })
            .expect("glow present")
    };
    let medium = intensity_at(GlowLevel::Medium);
    let thin = intensity_at(GlowLevel::Thin);
    let large = intensity_at(GlowLevel::Large);
    assert!(medium > 0.0, "medium glow intensity must be positive");
    // Assert against the curve itself (Thin must sit strictly between None and
    // Medium — its exact value is a tuning knob, e.g. 0.5→0.75 when Thin read
    // the same as None on the faint rest glows).
    assert!(
        (thin - medium * GlowLevel::Thin.strength_scale()).abs() < 1e-4,
        "thin follows its strength_scale"
    );
    assert!(
        thin > 0.0 && thin < medium,
        "thin sits between none and medium"
    );
    assert!(
        (large - medium * GlowLevel::Large.strength_scale()).abs() < 1e-4,
        "large follows its strength_scale"
    );
}

#[test]
fn drop_shadow_emits_a_shadow_rect_and_respects_zero_alpha() {
    use heca_grid_ui::scene::Shadow;
    let theme = Theme::default();
    let rect = Rectangle::new(Point::new(50.0, 50.0), Size::new(120.0, 80.0));

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.drop_shadow(
            rect,
            8.0,
            Shadow {
                color: theme.shadow_color(),
                radius: 24.0,
                dx: 0.0,
                dy: 10.0,
            },
        );
    }
    let sh = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Rect(r) => r.shadow,
            _ => None,
        })
        .expect("drop_shadow emits a rect carrying a Shadow");
    assert_eq!(
        (sh.radius, sh.dy),
        (24.0, 10.0),
        "shadow blur + offset are threaded through"
    );

    // A fully-transparent shadow (alpha 0) or zero radius is a no-op.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.drop_shadow(
            rect,
            8.0,
            Shadow {
                color: theme.shadow_color().with_alpha(0),
                radius: 24.0,
                dx: 0.0,
                dy: 10.0,
            },
        );
    }
    assert!(
        scene.is_empty(),
        "a zero-alpha shadow draws nothing (shadows-off)"
    );
}
