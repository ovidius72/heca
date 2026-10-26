use super::*;

#[test]
fn sibling_surfaces_do_not_displace_a_centered_overlay() {
    for viewport in [Size::new(420.0, 400.0), Size::new(900.0, 700.0)] {
        let mut root = Flex::column().width(viewport.w as f32).height(viewport.h as f32)
            .child(Label::new("Unrelated content"))
            .child(Overlay::new().panel(Label::new("Closed sibling")))
            .child(Overlay::new().panel(Label::new("Centered surface"))
                .panel_size(Length::Px(200.0), Length::Px(100.0)).default_open(true));
        crate::LayoutEngine::new().compute(&mut root, viewport);
        let panel = root.base().children[2].base().children[0].base().bounds;
        assert_eq!(Point::new(panel.loc.x + panel.size.w / 2.0,
            panel.loc.y + panel.size.h / 2.0), Point::new(viewport.w / 2.0, viewport.h / 2.0),
            "a sibling must not move a viewport-centered surface");
    }
}

/// The size lands on the panel child whichever order the builders are called
/// in — `panel()` clears and re-pushes the child, so the overlay has to keep
/// the size and re-apply it.
#[test]
fn panel_size_applies_regardless_of_builder_order() {
    let want_w = Length::Percent(0.6);
    let want_h = Length::Px(420.0);

    // size first, then panel
    let a = Overlay::new()
        .panel_size(want_w, want_h)
        .panel(Flex::column().child(Label::new("body")));
    let style = &a.base.children[0].base().style.layout;
    assert_eq!(style.width, want_w);
    assert_eq!(style.height, want_h);

    // panel first, then size
    let b = Overlay::new()
        .panel(Flex::column().child(Label::new("body")))
        .panel_size(want_w, want_h);
    let style = &b.base.children[0].base().style.layout;
    assert_eq!(style.width, want_w);
    assert_eq!(style.height, want_h);
}

/// **Regression guard.** A panel must never be larger than the viewport it is
/// centered in: an oversized one gets cut off by the window edge on *both*
/// sides (a dialog bigger than the window — user-reported). The cap is
/// unconditional, so it also protects a content-sized panel, not just a
/// `panel_size`d one.
#[test]
fn panel_never_exceeds_the_viewport() {
    use crate::widgets::ScrollRegion;
    // Ask for a panel far bigger than the viewport we lay out in.
    let mut o = Overlay::new()
        .panel_size(Length::Px(4000.0), Length::Px(3000.0))
        .panel(ScrollRegion::new().child(Label::new("tall")))
        .default_open(true);
    crate::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
    let panel = o.panel_bounds();
    assert!(
        panel.size.w <= 800.0,
        "panel width capped, got {}",
        panel.size.w
    );
    assert!(
        panel.size.h <= 600.0,
        "panel height capped, got {}",
        panel.size.h
    );
}

/// Unset (the default) leaves the panel hugging its own content — the sizing
/// API must not silently impose a size on every existing overlay.
#[test]
fn panel_size_is_opt_in() {
    let o = Overlay::new().panel(Flex::column().child(Label::new("body")));
    let style = &o.base.children[0].base().style.layout;
    assert_eq!(style.width, Length::Auto, "untouched by default");
    assert_eq!(style.height, Length::Auto);
}

/// End-to-end: an anchored overlay bakes the placement into the panel child's
/// bounds on layout (so paint/hit-testing follow), and it is idempotent.
#[test]
fn anchored_overlay_places_panel_child_on_layout() {
    let anchor = Rectangle::new(Point::new(40.0, 100.0), Size::new(120.0, 30.0));
    let mut o = Overlay::new()
        .blocking(false)
        .anchored(anchor)
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true);
    // Simulate a layout pass: taffy placed the panel somewhere with a real size.
    o.base.children[0].base_mut().bounds =
        Rectangle::new(Point::new(300.0, 300.0), Size::new(120.0, 80.0));
    o.viewport.set(Size::new(800.0, 600.0));
    o.on_layout();
    let placed = o.panel_bounds();
    assert_eq!(
        placed.loc,
        Point::new(40.0, 134.0),
        "panel anchored below trigger"
    );
    // Idempotent: a second on_layout must not compound the offset.
    o.on_layout();
    assert_eq!(o.panel_bounds().loc, placed.loc, "re-placing is idempotent");
}

/// Paint `o` into a fresh scene and report every host request it recorded, split by band.
///
/// The split is the point: a backdrop must land in the **base** commands, never the deferred
/// overlay band, because the host performs the requests between flushing the two.
fn host_draws(o: &Overlay) -> (Vec<HostCmd>, Vec<HostCmd>) {
    let theme = crate::theme::Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(800.0, 600.0));
        o.paint(&mut cx);
    }
    let host = |s: &Scene| -> Vec<HostCmd> {
        s.iter()
            .filter_map(|c| match c {
                DrawCommand::Host(h) => Some(h.clone()),
                _ => None,
            })
            .collect()
    };
    (host(&scene.base_layer()), host(&scene.overlay_layer()))
}

/// **A frosted surface blurs exactly what it occludes, and asks for it in the base band.**
///
/// The band is the whole mechanism: everything an overlay *draws* is deferred so it composites
/// above its siblings, but a backdrop is the opposite — it must be performed after what is beneath
/// it and before the surface itself. The host flushes base, performs the requests, then flushes the
/// overlay bands, so a request recorded in the wrong band blurs the wrong frame.
#[test]
fn a_frosted_overlay_records_its_blur_in_the_base_band_over_what_it_occludes() {
    let mut o = Overlay::new()
        .frosted(true)
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true);
    crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));

    let (base, overlay) = host_draws(&o);
    let radius = crate::theme::Theme::default().colors.overlay_frost_radius;
    assert_eq!(
        base,
        vec![HostCmd {
            draw: HostDraw::Backdrop {
                radius,
                corner: 0.0
            },
            rect: Rectangle::new(Point::new(0.0, 0.0), Size::new(800.0, 600.0)),
            alpha: 1.0,
            echo: false,
            rounded: None,
        }],
        "blocking, so it blurs the viewport it covers — and in the base band",
    );
    assert!(
        overlay.is_empty(),
        "a backdrop is never deferred with what the surface draws"
    );
}

/// **A surface that did not ask for a frost summons no blur pass.** The blur is a GPU pass over
/// the whole frame; a dialog that never wanted one must not be paying for it.
#[test]
fn a_plain_overlay_records_no_backdrop() {
    let o = open_overlay();
    assert!(host_draws(&o).0.is_empty());
}

/// **A non-blocking frosted surface blurs its panel, not the screen.** It occludes only its panel,
/// and the frost reads the same reach the input policy does — so the two can never disagree about
/// how far this surface goes.
#[test]
fn a_non_blocking_frost_reaches_only_as_far_as_the_panel() {
    let mut o = Overlay::new()
        .blocking(false)
        .frosted(true)
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true);
    crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));

    let (base, _) = host_draws(&o);
    assert_eq!(base.len(), 1);
    assert_eq!(
        base[0].rect,
        o.panel_bounds(),
        "the panel's reach, not the viewport's"
    );
}

/// **The backdrop dissolves with the surface that asked for it.**
///
/// Left at full strength it holds the whole session out of focus for the length of the fade and
/// then snaps sharp in one frame — the exact pop the fade exists to remove. It takes the
/// animation's *opacity* and nothing else: a blurred region that also zoomed would be blurring
/// somewhere the surface is not.
#[test]
fn a_frosted_overlays_backdrop_fades_with_it() {
    let mut o = Overlay::new()
        .frosted(true)
        .animation(Animation::ZoomFade)
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true);
    crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
    o.show();
    while o.tick(0.05) {}

    o.close();
    o.tick(0.05);
    let (base, _) = host_draws(&o);
    assert_eq!(base.len(), 1, "still asking while it leaves");
    assert!(
        base[0].alpha < 1.0,
        "and asking more faintly: {}",
        base[0].alpha
    );
    assert_eq!(
        base[0].rect,
        Rectangle::new(Point::new(0.0, 0.0), Size::new(800.0, 600.0)),
        "the zoom moves the panel, never the region being blurred",
    );
}
