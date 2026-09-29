use super::*;

#[test]
fn button_derives_border_width_and_radius_from_theme() {
    use heca_grid_ui::Button;

    // A theme with a distinctive radius + border width.
    let mut theme = Theme::default();
    theme.colors.border_radius = 10.0;
    theme.colors.border_width = 2.0;
    let expected_radius = theme.colors.control_radius();

    let mut btn = Button::primary("OK");
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(&mut btn, Size::new(300.0, 80.0));
    let scene = common::paint(&btn, &theme);

    // The button's background box uses the surface fill; it must round to the
    // theme's control radius and stroke at the theme's border width — not the
    // old hardcoded 0.0 / 1.5.
    let bg = scene
        .iter()
        .find_map(|cmd| match cmd {
            DrawCommand::Rect(r) if r.fill == theme.colors.surface => Some(*r),
            _ => None,
        })
        .expect("button paints a surface-filled background box");
    assert_eq!(
        bg.radius, expected_radius,
        "button corner radius follows theme.colors.control_radius()"
    );
    assert_eq!(
        bg.border.expect("primary button has a border").width,
        theme.colors.border_width,
        "button border width follows theme.colors.border_width",
    );

    // border_width == 0 → no border drawn (borders off, like every surface).
    theme.colors.border_width = 0.0;
    let mut btn = Button::primary("OK");
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(&mut btn, Size::new(300.0, 80.0));
    let scene = common::paint(&btn, &theme);
    let bg = scene
        .iter()
        .find_map(|cmd| match cmd {
            DrawCommand::Rect(r) if r.fill == theme.colors.surface => Some(*r),
            _ => None,
        })
        .expect("button still paints its background box");
    assert!(
        bg.border.is_none(),
        "border_width == 0 means no button border"
    );
}

#[test]
fn bracket_frame_zero_border_draws_nothing_nonzero_draws_reticle() {
    // `border_width == 0` means borders off everywhere — `bracket_frame` draws
    // NOTHING (no hairline). A container that needs definition at 0 carries a fill,
    // not a forced border. This keeps the bracket frame consistent with every other
    // widget's border gate.
    let mut theme = Theme::default();
    theme.colors.border_width = 0.0;
    let rect = Rectangle::new(Point::new(10.0, 10.0), Size::new(200.0, 120.0));

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame(rect);
    }
    assert!(scene.is_empty(), "border_width == 0 draws no frame at all");

    // With a real border: one dimmed continuous accent line tracing the perimeter,
    // plus four bright accent corners — each redrawn clipped to its corner box.
    theme.colors.border_width = 2.0;
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame(rect);
    }
    let bright_corners = scene.iter().filter(|c| matches!(
        c, DrawCommand::Rect(r) if r.border.is_some_and(|b| b.color == theme.colors.accent && b.width > 0.0)
    )).count();
    let dim_line = scene.iter().any(|c| matches!(
        c, DrawCommand::Rect(r) if r.border.is_some_and(|b| b.color.a < theme.colors.accent.a && b.width > 0.0)
    ));
    let clips = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::PushClip(_)))
        .count();
    assert_eq!(
        bright_corners, 4,
        "border>0 draws four bright accent corner brackets"
    );
    assert!(
        dim_line,
        "border>0 traces a dimmed continuous accent line under the corners"
    );
    assert_eq!(
        clips, 4,
        "each bright corner is clipped to its own corner box"
    );
}

#[test]
fn bracket_frame_with_honors_explicit_width_independent_of_theme() {
    // `bracket_frame_with` sizes the reticle from the passed width/radius, not the
    // theme — the seam that lets a bracketed sidebar honor `sidebar_border_width`
    // even when the global/theme border is 0. (Issue 1.)
    let mut theme = Theme::default();
    theme.colors.border_width = 0.0; // global borders OFF
    let rect = Rectangle::new(Point::new(10.0, 10.0), Size::new(200.0, 120.0));

    // Theme says 0, but an explicit width of 3 still draws the reticle.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame_with(rect, 3.0, 8.0);
    }
    let bright_corners = scene.iter().filter(|c| matches!(
        c, DrawCommand::Rect(r) if r.border.is_some_and(|b| b.color == theme.colors.accent && b.width > 0.0)
    )).count();
    assert_eq!(
        bright_corners, 4,
        "explicit width draws the reticle even when theme.colors.border_width == 0"
    );

    // An explicit width of 0 draws nothing, regardless of the theme.
    theme.colors.border_width = 5.0;
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame_with(rect, 0.0, 8.0);
    }
    assert!(
        scene.is_empty(),
        "explicit width 0 draws no reticle even with theme border on"
    );
}
