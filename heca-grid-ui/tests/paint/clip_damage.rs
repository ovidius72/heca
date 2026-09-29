use super::*;

#[test]
fn paint_cx_culls_offscreen_content_but_not_headless() {
    let theme = Theme::default();
    let vp = Size::new(800.0, 600.0);
    let off = Rectangle::new(Point::new(10.0, 5000.0), Size::new(100.0, 40.0)); // far below
    let on = Rectangle::new(Point::new(10.0, 10.0), Size::new(100.0, 40.0));

    // On-screen content is painted.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        cx.rect(on, theme.colors.surface, None, 0.0, None);
    }
    assert_eq!(scene.len(), 1, "on-screen rect is painted");

    // Content fully outside the viewport (rect + text) emits nothing.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        cx.rect(off, theme.colors.surface, None, 0.0, None);
        cx.text(
            off,
            "hidden",
            theme.colors.foreground,
            15.0,
            TextAlign::Start,
            TextStyle::REGULAR,
        );
    }
    assert!(scene.is_empty(), "content far below the viewport is culled");

    // With no viewport set (headless / tests) nothing is ever culled.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.rect(off, theme.colors.surface, None, 0.0, None);
    }
    assert_eq!(
        scene.len(),
        1,
        "no viewport ⇒ no culling (headless default)"
    );
}

#[test]
fn with_clip_wraps_body_draws_in_push_and_pop_clip() {
    use heca_grid_ui::PaintCx;

    let theme = Theme::default();
    let mut scene = Scene::new();
    let clip = Rectangle::new(Point::new(0.0, 0.0), Size::new(50.0, 50.0));
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(100.0, 100.0));
        cx.with_clip(clip, |cx| {
            cx.rect(
                Rectangle::new(Point::new(5.0, 5.0), Size::new(10.0, 10.0)),
                theme.colors.surface,
                None,
                0.0,
                None,
            );
        });
    }

    let cmds: Vec<&DrawCommand> = scene.iter().collect();
    assert!(
        matches!(cmds.first(), Some(DrawCommand::PushClip(r)) if *r == clip),
        "the body is opened by a PushClip carrying the clip rect"
    );
    assert!(
        matches!(cmds.last(), Some(DrawCommand::PopClip)),
        "the clip is popped after the body"
    );
    assert!(
        cmds.iter().any(|c| matches!(c, DrawCommand::Rect(_))),
        "the clipped rect sits between the push and pop"
    );
}

#[test]
fn focused_input_requests_a_timed_caret_redraw_not_continuous() {
    use heca_grid_ui::{Component, FocusManager, Input};

    let mut input = Input::new().value("hi");
    LayoutEngine::new().compute(&mut input, Size::new(200.0, 60.0));

    // The caret is never a continuous animation: tick reports no animating frame.
    assert!(
        !input.tick(0.016),
        "an input never drives the continuous redraw loop"
    );
    // Unfocused: nothing to redraw on a timer.
    assert_eq!(
        input.next_redraw(),
        None,
        "an unfocused input asks for no timed redraw"
    );

    // Focused: it schedules a wake at its next caret toggle (within a half period),
    // so the host sleeps until then instead of redrawing every frame.
    let mut focus = FocusManager::new();
    focus.advance(&mut input, true);
    let nr = input
        .next_redraw()
        .expect("a focused input schedules a timed caret redraw");
    assert!(
        nr > 0.0 && nr <= BLINK_PERIOD_HALF + 1e-3,
        "caret wake is within the half blink period, got {nr}"
    );
}
const BLINK_PERIOD_HALF: f32 = 0.5;

#[test]
fn collect_damage_unions_dirty_widgets_then_clears_flags() {
    use heca_grid_ui::{Component, collect_damage};

    let mut ui = Flex::row()
        .child(Button::primary("A"))
        .child(Button::secondary("B"));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));

    // A fresh tree needs its first paint; collecting reports damage and clears flags.
    assert!(
        collect_damage(&ui).is_some(),
        "a fresh tree needs its first paint"
    );
    assert!(
        collect_damage(&ui).is_none(),
        "flags cleared → no damage on the next collect"
    );

    // Marking one widget dirty → damage covers (at least) that widget's bounds.
    let b = ui.base().children[1].base().bounds;
    ui.base().children[1].base().mark_needs_paint();
    let d = collect_damage(&ui).expect("a marked widget reports damage");
    assert!(
        d.loc.x <= b.loc.x && d.loc.x + d.size.w >= b.loc.x + b.size.w,
        "damage horizontally covers the marked widget"
    );
}
