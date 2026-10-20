use super::*;

/// **A dissolving layer is removed only after its dissolve.** Escape and a widget's own
/// dismiss both go through `overlay::resolve`, which *removes*; hiding was the only path that
/// faded. So the map dissolved on a click and cut on Escape — two dismissals, two behaviours.
/// **Re-showing a surface that is leaving does not resurrect it.**
///
/// A layer is re-registered and re-shown every time the session changes underneath it, and a
/// leaving layer is still visible — so cancelling its fade here made the map snap back to full
/// opacity and start its exit over: a fast fade-in immediately before it vanished. The exposé
/// hits this on the ordinary path, because it dismisses itself and *then* focuses the pane you
/// chose, and focusing rebuilds the map mid-exit.
#[test]
fn re_showing_a_leaving_layer_does_not_cancel_its_exit() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(fading_root(), true, true),
        &mut window,
    );
    reg.show(&mut window, id);

    reg.hide(&mut window, id);
    frame(&mut reg, &mut window, 0.1);
    let mid = reg.opacity(&window, id);
    assert!(mid < 1.0, "it is on its way out: {mid}");

    // The rebuild: same layer, still visible, shown again.
    reg.show(&mut window, id);
    assert_eq!(
        reg.opacity(&window, id),
        mid,
        "the exit carries on from where it was"
    );
    assert!(reg.get(id).is_some(), "and it is still leaving");
}

#[test]
fn removing_a_fading_layer_waits_for_the_fade() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(fading_root(), true, true),
        &mut window,
    );
    reg.show(&mut window, id);
    while frame(&mut reg, &mut window, 0.05) {} // let the arrival play, as a few frames would

    reg.remove(&mut window, id);
    assert!(reg.any_visible(), "it is on its way out, not gone");
    assert!(frame(&mut reg, &mut window, 0.05), "still dissolving");
    assert!(
        reg.opacity(&window, id) < 1.0,
        "and visibly on its way: {}",
        reg.opacity(&window, id)
    );
    while reg.opacity(&window, id) > 0.05 {
        assert!(frame(&mut reg, &mut window, 0.05), "still dissolving");
    }

    // **The tick that finishes the exit still asks for a frame** — the one that paints the
    // absence. Both effects report themselves done here, so without it nothing requests
    // another frame and the last frame actually drawn is the one before, still faintly there:
    // the map stayed on the glass at about a tenth opacity until some other input forced a
    // repaint. This assertion used to read `!frame(&mut reg, &mut window,.)`, which is that bug
    // written down.
    assert!(
        frame(&mut reg, &mut window, 0.05),
        "one more frame, to paint it gone"
    );
    assert!(!reg.any_visible(), "the layer is gone for good");
    assert!(
        !frame(&mut reg, &mut window, 0.05),
        "and now it asks for nothing"
    );
}

/// **A dissolving layer stops being in charge the moment it is dismissed**, even though it is
/// still on screen.
///
/// The exposé closes by dismissing itself and *then* focusing the pane you chose. While the
/// dissolve still counted as coverage, that focus was refused by `Domain::Overlay` for the
/// whole animation — Enter and Space did nothing and the log filled with `blocked intent from
/// Keyboard`. Adding a fade must not make a surface hold onto input it has already given up.
#[test]
fn a_dissolving_layer_no_longer_covers_the_content() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(fading_root(), true, true),
        &mut window,
    );
    reg.show(&mut window, id);
    assert!(reg.content_covered(&window), "up and in charge");

    reg.hide(&mut window, id);
    assert!(
        !reg.content_covered(&window),
        "dismissed — a picture now, not a modal"
    );
    assert!(reg.any_visible(), "and still painted while it dissolves");
    assert!(
        reg.top_modal_id(&window).is_none(),
        "so it takes no more input either"
    );
}

/// A layer with no dissolve declared still goes at once — a menu that lingers reads as lag.
#[test]
fn removing_a_plain_layer_is_immediate() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    reg.show(&mut window, id);
    reg.remove(&mut window, id);
    assert!(!reg.any_visible());
}

#[test]
fn on_demand_starts_hidden_persistent_starts_visible() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let a = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, false),
        &mut window,
    );
    let b = reg.add(
        None,
        LayerKind::Persistent,
        declaring(empty_root(), false, false),
        &mut window,
    );
    let vis: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
    assert_eq!(
        vis,
        vec![b],
        "on-demand hidden until shown; persistent visible"
    );
    reg.show(&mut window, a);
    // Both visible now. `b` was registered second, so it is the later sibling and in front.
    let vis: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
    assert_eq!(vis, vec![b, a], "a later sibling is in front");
    reg.hide(&mut window, a);
    assert_eq!(reg.visible_front_to_back().len(), 1);
}

/// **A rebuild does not replay the arrival.** A layer is re-registered and re-shown whenever
/// the session changes under it, and an animation that restarts every time reads as the surface
/// flickering open again — which is what `prefix+j` behind the exposé did.
#[test]
fn re_showing_a_visible_layer_does_not_restart_its_zoom() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let name = layer_name(HOST_OWNER, "expose").expect("valid");
    let slot = reg.slot_for_name(&name);
    let id = reg.add_named(
        slot,
        name.clone(),
        None,
        LayerKind::OnDemand,
        declaring(zooming_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, id);
    assert!(
        scale_of(&window, id) > 1.0,
        "opening animates: {}",
        scale_of(&window, id)
    );

    // Let it finish, the way a few frames would.
    while frame(&mut reg, &mut window, 0.05) {}
    assert_eq!(scale_of(&window, id), 1.0, "settled at life size");

    // The session changes: the layer is rebuilt — a **fresh tree**, which on its own would
    // arrive all over again — and re-shown.
    let slot = reg.slot_for_name(&name);
    let rebuilt = reg.add_named(
        slot,
        name.clone(),
        None,
        LayerKind::OnDemand,
        declaring(zooming_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, rebuilt);

    assert_eq!(
        scale_of(&window, rebuilt),
        1.0,
        "a rebuild must not replay the arrival"
    );
    assert!(
        !frame(&mut reg, &mut window, 0.05),
        "and nothing is animating"
    );
}

/// **Showing a layer puts its surface on screen** — the whole chain, because every link in it
/// is new: `show` states that the surface is open, the surface opens itself and starts its
/// arrival, and *it* is what paints. A surface that stays closed here draws nothing at all,
/// which is the one failure mode of moving the animation onto the widget that no unit test of
/// the parts would catch.
#[test]
fn showing_a_layer_puts_its_surface_on_screen() {
    use heca_grid_ui::{LayoutEngine, PaintCx, Scene, Size, Theme};

    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(zooming_root(), true, true),
        &mut window,
    );
    let theme = Theme::default();
    let viewport = Size::new(800.0, 600.0);

    let painted = |window: &heca_grid_ui::widgets::Flex, id: LayerId| {
        let mut scene = Scene::new();
        let node = crate::chrome::surface_node(window, id).expect("the surface is in the tree");
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(viewport);
        heca_grid_ui::paint_child(node, &mut cx);
        !scene.is_empty()
    };

    assert!(
        !painted(&window, id),
        "registered but not shown: nothing on screen"
    );

    reg.show(&mut window, id);
    if let Some(node) = crate::chrome::surface_node_mut(&mut window, id) {
        LayoutEngine::new().compute(node.as_mut(), viewport);
    }
    assert!(painted(&window, id), "shown: the surface draws itself");

    reg.hide(&mut window, id);
    assert!(painted(&window, id), "…and keeps drawing while it leaves");
    while frame(&mut reg, &mut window, 0.05) {}
    assert!(!reg.any_visible(), "…until the gesture has played out");
}

/// **A rebuild mid-gesture carries the gesture, rather than restarting or snapping it.**
///
/// The harder half of the rule above: the exposé is re-registered *while* it is arriving (the
/// session changes under it on the very frames the map is opening), and the replacement is a
/// fresh tree that on its own would begin at the far end again. What is handed over is the
/// surface's whole `Presence`, so the arrival continues from exactly where the old tree had
/// got to.
#[test]
fn a_rebuild_part_way_through_an_arrival_carries_it_on() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let name = layer_name(HOST_OWNER, "expose").expect("valid");
    let slot = reg.slot_for_name(&name);
    let id = reg.add_named(
        slot,
        name.clone(),
        None,
        LayerKind::OnDemand,
        declaring(zooming_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, id);
    frame(&mut reg, &mut window, 0.05);
    frame(&mut reg, &mut window, 0.05);
    let mid = scale_of(&window, id);
    assert!(mid > 1.0 && mid < 1.3, "part way in: {mid}");

    let slot = reg.slot_for_name(&name);
    let rebuilt = reg.add_named(
        slot,
        name.clone(),
        None,
        LayerKind::OnDemand,
        declaring(zooming_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, rebuilt);
    assert_eq!(
        scale_of(&window, rebuilt),
        mid,
        "the arrival carried on from where it was"
    );
    assert!(frame(&mut reg, &mut window, 0.05), "…and is still going");
    assert!(
        scale_of(&window, rebuilt) < mid,
        "…toward life size, not back to the start"
    );
}
