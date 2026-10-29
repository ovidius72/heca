//! An overlay names how it arrives and leaves, and what it asks of what is behind it.

use heca_view::WidgetId;

use super::*;

/// **A described surface arrives and leaves exactly as a native one does**.
///
/// The declarative half of `Overlay::animation`: a plugin that can only send JSON names a
/// built-in and gets the same gesture native code gets — one door, two spellings. The surface
/// then holds itself on screen for the whole of its exit, which is the behaviour that makes an
/// animated dismissal possible at all.
#[test]
fn a_described_overlay_names_how_it_arrives_and_leaves() {
    let emit: IntentEmitter = Rc::new(|_| {});
    let mut forms = FormBindings::default();
    let theme = Theme::default();

    let node = ViewNode::new(WidgetId::Overlay)
        .prop("animation", PropValue::Text("zoom_fade".into()))
        .prop("default_open", PropValue::Bool(true))
        .child(ViewNode::new(WidgetId::Label).text("MAP"));
    let mut surface = realize(&node, &theme, &emit, &mut forms).expect("valid described test widget");

    assert!(
        surface.presence().is_some(),
        "a described overlay is a surface a host can drive"
    );
    surface.close();
    assert!(
        surface.presence().is_some_and(|p| p.is_leaving()),
        "the named animation plays on the way out",
    );
    let mut frames = 0;
    while surface.tick(1.0 / 60.0) && frames < 600 {
        frames += 1;
    }
    assert!(frames > 1, "it played rather than cutting: {frames} frames");
    assert!(
        !surface.presence().is_some_and(|p| p.is_leaving()),
        "and then it is gone"
    );

    // Untrusted input stays total: an unknown name leaves the surface with its default.
    let unknown = ViewNode::new(WidgetId::Overlay)
        .prop("animation", PropValue::Text("supernova".into()))
        .child(ViewNode::new(WidgetId::Label).text("MAP"));
    let mut cut = realize(&unknown, &theme, &emit, &mut forms).expect("valid described test widget");
    cut.show();
    cut.close();
    assert!(
        !cut.presence().is_some_and(|p| p.is_leaving()),
        "a cut, not a panic"
    );
}

/// **A described surface can ask for the frost the exposé uses** — the plugin half of the
/// backdrop.
///
/// It matters because the blur is GPU work: if a plugin could not *describe* it, frosting would
/// be reachable only from native code and every plugin overlay would sit flat over a sharp
/// session. It asks for the effect and never a radius — strength is the theme's, so one theme
/// answers for every surface at once.
#[test]
fn a_described_overlay_can_ask_for_the_frost_behind_it() {
    let emit: IntentEmitter = Rc::new(|_| {});
    let mut forms = FormBindings::default();
    let theme = Theme::default();

    let mut backdrops = |frosted: bool| {
        let node = ViewNode::new(WidgetId::Overlay)
            .prop("frosted", PropValue::Bool(frosted))
            .prop("default_open", PropValue::Bool(true))
            .child(ViewNode::new(WidgetId::Label).text("MAP"));
        let mut w = realize(&node, &theme, &emit, &mut forms).expect("valid described test widget");
        heca_grid_ui::LayoutEngine::new()
            .compute(w.as_mut(), heca_core::layout::Size::new(800.0, 600.0));
        let mut scene = heca_grid_ui::Scene::new();
        {
            let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme)
                .with_viewport(heca_core::layout::Size::new(800.0, 600.0));
            w.paint(&mut cx);
        }
        scene
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    heca_grid_ui::scene::DrawCommand::Host(h)
                        if matches!(h.draw, heca_grid_ui::scene::HostDraw::Backdrop { .. })
                )
            })
            .count()
    };

    assert_eq!(
        backdrops(true),
        1,
        "the described surface asked for its blur"
    );
    assert_eq!(
        backdrops(false),
        0,
        "and one that did not ask pays for no pass"
    );
}
