use super::*;

pub(super) fn build_scene(
    root: &dyn Component,
    overlays: &dyn Component,
    theme: &Theme,
    w: f32,
    h: f32,
    show_clip_demo: bool,
) -> Scene {
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
        // Background fill as the first scene command (not a clear pass): scissored to
        // the damage region it clears only the changed area, so the rest of the
        // persistent scene texture is preserved for damage-region redraw.
        cx.rect(
            Rectangle::from_size(Size::new(w as f64, h as f64)),
            theme.colors.background,
            None,
            0.0,
            None,
        );
        root.paint(&mut cx);
        // The overlay layer paints after (= above) the scrolled page; its widgets
        // anchor to the viewport, not to the page's scroll offset.
        overlays.paint(&mut cx);
    }
    // Clip-primitive demo (task B), toggled by `c`: a centered overlay viewport
    // whose text content is taller than the box and offset by a fractional amount,
    // wrapped in `with_clip`. The renderer scissors it, so the top and bottom lines
    // are sliced cleanly at the panel edges instead of spilling out — the editor's
    // pixel-scroll foundation. Painted into the overlay layer so it sits on top.
    if show_clip_demo {
        let mut cx = PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
        let (pw, ph) = (240.0, 150.0);
        let panel = Rectangle::new(
            Point::new((w as f64 - pw) * 0.5, (h as f64 - ph) * 0.5),
            Size::new(pw, ph),
        );
        cx.with_overlay(|cx| {
            cx.rect(
                panel,
                theme.colors.surface,
                Some(heca_grid_ui::scene::Border {
                    color: theme.colors.accent,
                    width: theme.colors.border_width,
                }),
                theme.colors.border_radius,
                None,
            );
            // Title sits above the clipped region (not clipped).
            cx.text(
                Rectangle::new(
                    Point::new(panel.loc.x + 12.0, panel.loc.y + 8.0),
                    Size::new(panel.size.w - 24.0, 16.0),
                ),
                "CLIP VIEWPORT  (c)",
                theme.colors.accent,
                11.0,
                TextAlign::Start,
                TextStyle::BOLD,
            );
            let inner = Rectangle::new(
                Point::new(panel.loc.x + 12.0, panel.loc.y + 30.0),
                Size::new(panel.size.w - 24.0, panel.size.h - 42.0),
            );
            cx.with_clip(inner, |cx| {
                // Start ~half a line above the top edge so line 00 is sliced; the 12
                // lines overflow the bottom so the last line is sliced too.
                for i in 0..12 {
                    let y = inner.loc.y - 9.0 + i as f64 * 18.0;
                    cx.text(
                        Rectangle::new(Point::new(inner.loc.x, y), Size::new(inner.size.w, 16.0)),
                        &format!("clip line {i:02} — sliced at the edges"),
                        theme.colors.foreground,
                        12.0,
                        TextAlign::Start,
                        TextStyle::REGULAR,
                    );
                }
            });
        });
    }
    scene.push(DrawCommand::Scanline(ScanlineCmd {
        rect: Rectangle::from_size(Size::new(w as f64, h as f64)),
        color: theme.colors.accent,
        spacing: 3.0,
        opacity: theme.colors.intensity.scanline_opacity(),
    }));
    scene
}

