//! The layer stack's own tests: what is on screen, what is in charge, and what order they read.

use super::*;
use heca_grid_ui::animation::Animation;
use heca_grid_ui::widgets::{Flex, Overlay};

/// A layer whose surface declares nothing: it appears and goes between two frames.
fn empty_root() -> Box<dyn Component> {
    Box::new(Flex::row())
}

/// **A real layer root**: a surface that names its own gesture, exactly as the exposé and a
/// plugin's panel do — one builder on the widget, nothing said to the registry.
fn fading_root() -> Box<dyn Component> {
    Box::new(Overlay::new().panel(Flex::row()).animation(Animation::Fade))
}

/// The same, for a surface that arrives by pulling back — the exposé's own gesture.
fn zooming_root() -> Box<dyn Component> {
    Box::new(Overlay::new().panel(Flex::row()).animation(Animation::Zoom.from(1.3)))
}

/// What scale a layer's surface is drawing itself at this frame.
fn scale_of(reg: &LayerRegistry, id: LayerId) -> f32 {
    reg.get(id)
        .and_then(|l| l.root().presence())
        .map_or(1.0, |p| p.frame().scale)
}

/// **A frost is only paid for while the layer asking for it is up.** The blur is a full-frame
/// GPU pass, so a hidden exposé must not keep the renderer running it, and a plain dialog must
/// never trigger one it did not ask for.
#[test]
fn only_a_visible_layer_that_asked_for_it_wants_a_frost() {
    let mut reg = LayerRegistry::default();
    let plain = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
    let frosted = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
    reg.set_backdrop(frosted, LayerBackdrop::Frosted);

    assert!(!reg.wants_frost(), "both are hidden — nothing to frost behind");
    reg.show(plain);
    assert!(!reg.wants_frost(), "a plain layer does not summon a blur pass");
    reg.show(frosted);
    assert!(reg.wants_frost());
    reg.hide(frosted);
    assert!(!reg.wants_frost(), "hidden again, and the pass stops with it");
}

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
    let id = reg.add(None, LayerKind::OnDemand, true, true, fading_root());
    reg.show(id);

    reg.hide(id);
    reg.tick(0.1);
    let mid = reg.opacity(id);
    assert!(mid < 1.0, "it is on its way out: {mid}");

    // The rebuild: same layer, still visible, shown again.
    reg.show(id);
    assert_eq!(reg.opacity(id), mid, "the exit carries on from where it was");
    assert!(reg.get(id).is_some_and(|l| l.is_leaving()), "and it is still leaving");
}

#[test]
fn removing_a_fading_layer_waits_for_the_fade() {
    let mut reg = LayerRegistry::default();
    let id = reg.add(None, LayerKind::OnDemand, true, true, fading_root());
    reg.show(id);
    while reg.tick(0.05) {} // let the arrival play, as a few frames would

    reg.remove(id);
    assert!(reg.any_visible(), "it is on its way out, not gone");
    assert!(reg.tick(0.05), "still dissolving");
    assert!(reg.opacity(id) < 1.0, "and visibly on its way: {}", reg.opacity(id));
    while reg.opacity(id) > 0.05 {
        assert!(reg.tick(0.05), "still dissolving");
    }

    // **The tick that finishes the exit still asks for a frame** — the one that paints the
    // absence. Both effects report themselves done here, so without it nothing requests
    // another frame and the last frame actually drawn is the one before, still faintly there:
    // the map stayed on the glass at about a tenth opacity until some other input forced a
    // repaint (Antonio, driving, 2026-08-19). This assertion used to read `!reg.tick(..)`,
    // which is that bug written down.
    assert!(reg.tick(0.05), "one more frame, to paint it gone");
    assert!(!reg.any_visible(), "the layer is gone for good");
    assert!(!reg.tick(0.05), "and now it asks for nothing");
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
    let id = reg.add(None, LayerKind::OnDemand, true, true, fading_root());
    reg.show(id);
    assert!(reg.content_covered(), "up and in charge");

    reg.hide(id);
    assert!(!reg.content_covered(), "dismissed — a picture now, not a modal");
    assert!(reg.any_visible(), "and still painted while it dissolves");
    assert!(reg.top_modal_id().is_none(), "so it takes no more input either");
}

/// A layer with no dissolve declared still goes at once — a menu that lingers reads as lag.
#[test]
fn removing_a_plain_layer_is_immediate() {
    let mut reg = LayerRegistry::default();
    let id = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
    reg.show(id);
    reg.remove(id);
    assert!(!reg.any_visible());
}

/// **A `Modal`-band layer covers whatever it declared** — the property no call site can get
/// wrong (F003/P086/T371). It demands a decision, so acts on the panes behind it are refused by
/// construction; the alternative is trusting a `bool` at every `insert`, and the one that
/// passed `false` let `prefix+x` raise the close-pane confirm with a context menu open (user,
/// 2026-07-30).
#[test]
fn coverage_is_what_the_surface_declared_not_what_its_place_implies() {
    let mut reg = LayerRegistry::default();
    // Takes the keyboard, declares it covers nothing — and is believed.
    reg.insert(LayerId(7), None, LayerKind::OnDemand, true, false, empty_root());
    assert!(
        !reg.content_covered(),
        "a modal that says it covers nothing covers nothing — the band used to overrule this",
    );

    // The decision-demanding surfaces declare it themselves, which is where it belongs:
    // `open_modal` and `insert_menu_layer` both pass `true`.
    reg.insert(LayerId(8), None, LayerKind::OnDemand, true, true, empty_root());
    assert!(reg.content_covered(), "and a surface that says it covers, does");
}

/// **Below the `Modal` band, coverage is what the layer declared — `modal` says nothing about
/// it.** The two answer different questions: `modal` is "does this take the keyboard",
/// `covers_content` is "may actions still touch the panes". Conflating them made one surface
/// impossible to describe — the exposé takes the keyboard *and* is a map of the panes, so while
/// `modal` implied coverage it refused every act on the pane it exists to let you choose.
#[test]
fn an_overlay_that_takes_the_keyboard_can_still_declare_it_covers_nothing() {
    let mut reg = LayerRegistry::default();
    let map = reg.add(None, LayerKind::OnDemand, true, false, empty_root());
    reg.show(map);
    assert!(!reg.content_covered(), "a map of the panes does not cover them");
    assert_eq!(reg.top_modal_id(), Some(map), "and it still owns the keyboard");
}

/// The one input `Domain::Overlay` reads: a *visible* covering layer, and only that
/// (F003/P086/T371). A layer that covers but is hidden is not covering anything.
#[test]
fn coverage_is_reported_only_while_the_layer_is_visible() {
    let mut reg = LayerRegistry::default();
    let corner = reg.add(None, LayerKind::Persistent, false, false, empty_root());
    assert!(!reg.content_covered(), "a non-covering layer covers nothing");

    let over = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
    assert!(!reg.content_covered(), "on-demand starts hidden");
    reg.show(over);
    assert!(reg.content_covered(), "shown, and it covers the panes");
    reg.hide(over);
    assert!(!reg.content_covered());
    reg.remove(corner);
}

#[test]
fn on_demand_starts_hidden_persistent_starts_visible() {
    let mut reg = LayerRegistry::default();
    let a = reg.add(None, LayerKind::OnDemand, true, false, empty_root());
    let b = reg.add(None, LayerKind::Persistent, false, false, empty_root());
    let vis: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
    assert_eq!(vis, vec![b], "on-demand hidden until shown; persistent visible");
    reg.show(a);
    // Both visible now. `b` was registered second, so it is the later sibling and in front.
    let vis: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
    assert_eq!(vis, vec![b, a], "a later sibling is in front");
    reg.hide(a);
    assert_eq!(reg.visible_front_to_back().len(), 1);
}

/// **z is the position in the tree, so order is a pre-order walk of it.** Two rules, and both
/// fall out of comparing the z-paths rather than being written anywhere: a later sibling is in
/// front of an earlier one, and a child is in front of its parent.
///
/// This replaced `band_orders_front_to_back`, which asserted a five-variant enum's ranking —
/// a stored z one step removed, and the thing §6 of the surface-compositor model forbids.
#[test]
fn z_is_a_path_so_a_child_is_in_front_of_its_parent_and_a_later_sibling_of_an_earlier() {
    let mut reg = LayerRegistry::default();
    let first = reg.add(None, LayerKind::Persistent, false, false, empty_root());
    let second = reg.add(None, LayerKind::Persistent, false, false, empty_root());
    // Opened BY `first`, so it hangs from it — and sits above it without outranking `second`'s
    // own children, because a path is compared left to right.
    let childs_child = reg.add(Some(first), LayerKind::Persistent, false, false, empty_root());

    let order: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
    assert_eq!(
        order,
        vec![second, childs_child, first],
        "[1] > [0,0] > [0] — lexicographic on the path",
    );
}

/// **A surface goes above whatever opened it, wherever that is.** The deciding case for
/// plugins: a panel opens a modal, and the modal must sit above *that* panel — not above
/// whatever happens to have been registered last.
#[test]
fn an_overlay_sits_above_its_opener_not_above_the_newest_layer() {
    let mut reg = LayerRegistry::default();
    let panel = reg.add(None, LayerKind::Persistent, false, false, empty_root());
    let unrelated = reg.add(None, LayerKind::Persistent, false, false, empty_root());
    let modal = reg.add(Some(panel), LayerKind::Persistent, true, false, empty_root());

    let order: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
    assert_eq!(order, vec![unrelated, modal, panel]);
    assert_eq!(
        reg.top_modal_id(),
        Some(modal),
        "and input agrees with the picture, because both read the one order",
    );
}

/// **A described layer is a real layer.** It sorts, shows, hides and covers exactly like a
/// native one — the arm decides where the tree came from, never how the stack treats it.
#[test]
fn a_view_layer_behaves_like_any_other_and_keeps_its_description() {
    use heca_view::{ViewNode, WidgetKind};
    let mut reg = LayerRegistry::default();
    let native = reg.add(None, LayerKind::Persistent, false, false, empty_root());
    let node = ViewNode::new(WidgetKind::Label);
    let slot = reg.reserve_id();
    let described = reg.add_view(
        slot,
        None,
        LayerKind::Persistent,
        false,
        true,
        node,
        empty_root(),
    );

    let order: Vec<LayerId> =
        reg.visible_front_to_back().iter().map(|l| l.id).collect();
    assert_eq!(order, vec![described, native], "band decides order, not the content arm");
    assert!(reg.content_covered(), "a described layer declares coverage like any other");

    // The description is KEPT, not thrown away once realized: it is what a theme reload or a
    // plugin update re-realizes from.
    let layer = reg
        .visible_front_to_back()
        .into_iter()
        .find(|l| l.id == described)
        .expect("the described layer is in the stack");
    assert!(layer.content.node().is_some(), "the ViewNode survives realization");
    assert!(layer.realized.is_some(), "and its realized tree is registered beside it");
    assert_eq!(
        layer.content.node().map(|n| n.kind),
        Some(WidgetKind::Label),
    );

    // And a native layer has no description to offer — the arms are not interchangeable.
    let native_layer = reg
        .visible_front_to_back()
        .into_iter()
        .find(|l| l.id == native)
        .expect("the native layer is in the stack");
    assert!(native_layer.content.node().is_none());
}

/// **The owner half is unforgeable.** An author supplies only the short name, so a plugin has
/// nowhere to write a prefix — and a short name carrying its own dot is rejected rather than
/// joined, or `expose.thing` would read as if `docker.expose` owned it.
#[test]
fn a_layer_name_is_stamped_from_its_owner_and_cannot_be_forged() {
    assert_eq!(layer_name("docker", "expose").as_deref(), Some("docker.expose"));
    assert_eq!(layer_name(HOST_OWNER, "expose").as_deref(), Some("heca.expose"));
    assert_eq!(layer_name("docker", "expose.thing"), None, "no smuggled second segment");
    assert_eq!(layer_name("docker", "heca.expose"), None, "cannot claim another namespace");
    assert_eq!(layer_name("docker", ""), None);
}

/// **A rebuild does not replay the arrival.** A layer is re-registered and re-shown whenever
/// the session changes under it, and an animation that restarts every time reads as the surface
/// flickering open again — which is what `prefix+j` behind the exposé did.
#[test]
fn re_showing_a_visible_layer_does_not_restart_its_zoom() {
    let mut reg = LayerRegistry::default();
    let name = layer_name(HOST_OWNER, "expose").expect("valid");
    let slot = reg.slot_for_name(&name);
    let id = reg.add_named(
        slot, name.clone(), None, LayerKind::OnDemand, true, false, zooming_root(),
    );
    reg.show(id);
    assert!(scale_of(&reg, id) > 1.0, "opening animates: {}", scale_of(&reg, id));

    // Let it finish, the way a few frames would.
    while reg.tick(0.05) {}
    assert_eq!(scale_of(&reg, id), 1.0, "settled at life size");

    // The session changes: the layer is rebuilt — a **fresh tree**, which on its own would
    // arrive all over again — and re-shown.
    let slot = reg.slot_for_name(&name);
    let rebuilt = reg.add_named(
        slot, name.clone(), None, LayerKind::OnDemand, true, false, zooming_root(),
    );
    reg.show(rebuilt);

    assert_eq!(scale_of(&reg, rebuilt), 1.0, "a rebuild must not replay the arrival");
    assert!(!reg.tick(0.05), "and nothing is animating");
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
    let id = reg.add(None, LayerKind::OnDemand, true, true, zooming_root());
    let theme = Theme::default();
    let viewport = Size::new(800.0, 600.0);

    let painted = |reg: &LayerRegistry, id: LayerId| {
        let mut scene = Scene::new();
        let layer = reg.get(id).expect("the layer");
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(viewport);
        heca_grid_ui::paint_child(layer.root(), &mut cx);
        !scene.is_empty()
    };

    assert!(!painted(&reg, id), "registered but not shown: nothing on screen");

    reg.show(id);
    if let Some(l) = reg.get_mut(id) {
        LayoutEngine::new().compute(l.root_mut().as_mut(), viewport);
    }
    assert!(painted(&reg, id), "shown: the surface draws itself");

    reg.hide(id);
    assert!(painted(&reg, id), "…and keeps drawing while it leaves");
    while reg.tick(0.05) {}
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
    let name = layer_name(HOST_OWNER, "expose").expect("valid");
    let slot = reg.slot_for_name(&name);
    let id = reg.add_named(
        slot, name.clone(), None, LayerKind::OnDemand, true, false, zooming_root(),
    );
    reg.show(id);
    reg.tick(0.05);
    reg.tick(0.05);
    let mid = scale_of(&reg, id);
    assert!(mid > 1.0 && mid < 1.3, "part way in: {mid}");

    let slot = reg.slot_for_name(&name);
    let rebuilt = reg.add_named(
        slot, name.clone(), None, LayerKind::OnDemand, true, false, zooming_root(),
    );
    reg.show(rebuilt);
    assert_eq!(scale_of(&reg, rebuilt), mid, "the arrival carried on from where it was");
    assert!(reg.tick(0.05), "…and is still going");
    assert!(scale_of(&reg, rebuilt) < mid, "…toward life size, not back to the start");
}

/// **The frost dissolves with the surface it is under.** The blur is stamped *behind* the
/// layer, so a map that dissolves over a sharp backdrop and then cuts it away in one frame at
/// the end reads as the app snapping back into focus a beat too late.
#[test]
fn the_frosted_backdrop_follows_its_surface_out() {
    let mut reg = LayerRegistry::default();
    let id = reg.add(None, LayerKind::OnDemand, true, true, fading_root());
    reg.set_backdrop(id, LayerBackdrop::Frosted);
    reg.show(id);
    while reg.tick(0.05) {} // the arrival plays; the blur comes up with it
    assert_eq!(reg.frost_opacity(), 1.0, "up, and the blur is full strength");

    reg.hide(id);
    reg.tick(0.1);
    let frost = reg.frost_opacity();
    assert!(frost > 0.0 && frost < 1.0, "the blur is going with it: {frost}");
}

/// **Input goes to the layer the user sees in front, and there is only one order to read.**
///
/// Painting used to sort by band while input took "the last one added", so the two could name
/// different layers — a dialog was drawn over the exposé while the map quietly took the
/// pointer. Both now read the z-path, so disagreeing is not expressible.
#[test]
fn input_goes_to_the_front_most_surface_not_the_last_one_added() {
    let mut reg = LayerRegistry::default();
    let map = reg.add(None, LayerKind::OnDemand, true, false, empty_root());
    reg.show(map);
    // Raised FROM the map, so it is the map's child and above it.
    let dialog = reg.add(Some(map), LayerKind::OnDemand, true, true, empty_root());
    reg.show(dialog);

    assert_eq!(
        reg.top_modal_id(),
        Some(dialog),
        "a surface opened from the map sits above the map",
    );

    // Among siblings, the later one is in front.
    let second_dialog = reg.add(Some(map), LayerKind::OnDemand, true, true, empty_root());
    reg.show(second_dialog);
    assert_eq!(reg.top_modal_id(), Some(second_dialog), "same parent, later wins");

    // And a layer on its way out never holds the input.
    reg.remove(second_dialog);
    assert_eq!(reg.top_modal_id(), Some(dialog), "a dissolving layer is not the target");
}

/// **The active context follows the exclusive surfaces**, so an overlay knows what to hang
/// from without any caller telling it. Closing one restores the context beneath it.
#[test]
fn the_active_context_follows_the_modal_surfaces() {
    let mut reg = LayerRegistry::default();
    assert_eq!(reg.current(), None, "the base context: panes, sidebar and floats together");

    let map = reg.add(None, LayerKind::OnDemand, true, false, empty_root());
    reg.show(map);
    assert_eq!(reg.current(), Some(map));

    let dialog = reg.add(reg.current(), LayerKind::OnDemand, true, true, empty_root());
    reg.show(dialog);
    assert_eq!(reg.current(), Some(dialog), "a modal raised from the map takes the context");
    assert_eq!(reg.get(dialog).and_then(|l| l.parent), Some(map), "and hangs from it");

    reg.hide(dialog);
    assert_eq!(reg.current(), Some(map), "closing it hands the context back");
    reg.hide(map);
    assert_eq!(reg.current(), None, "and back to the base context");
}

/// **A rebuild keeps the layer's place, so it cannot climb over what opened above it.**
///
/// `top_modal_root_mut` takes the *last* active modal in the stack, so position is what "on
/// top" means. Re-registering by removing and re-appending therefore promoted a layer above
/// everything opened since — which is how deleting a pane from the exposé did nothing at all:
/// the confirm dialog opened above the map, the session change rebuilt the map, the rebuild put
/// it back on top of the dialog, and the click on "Close" went to the map (Antonio, driving,
/// 2026-08-11).
#[test]
fn re_registering_a_layer_does_not_promote_it_above_a_newer_one() {
    let mut reg = LayerRegistry::default();
    let name = layer_name(HOST_OWNER, "expose").expect("valid");
    let slot = reg.slot_for_name(&name);
    let map = reg.add_named(
        slot, name.clone(), None, LayerKind::OnDemand, true, false, empty_root(),
    );
    reg.show(map);
    // A confirm dialog opens ON TOP of it.
    let dialog = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
    reg.show(dialog);
    assert_eq!(reg.top_modal_id(), Some(dialog), "the dialog is the input target");

    // The session changes under both, so the map is rebuilt.
    let slot = reg.slot_for_name(&name);
    let rebuilt = reg.add_named(
        slot, name.clone(), None, LayerKind::OnDemand, true, false, empty_root(),
    );
    reg.show(rebuilt);

    assert_eq!(
        reg.top_modal_id(),
        Some(dialog),
        "the rebuilt map must NOT take the keyboard and the pointer from the dialog above it",
    );
    assert_eq!(reg.by_name(&name), Some(rebuilt), "and the name still resolves to the new tree");
}

/// A named layer is addressable without knowing its `LayerId` — which is a runtime counter no
/// keybinding or RPC call could know. Re-registering the name replaces it, as a remount should.
#[test]
fn a_named_layer_is_addressable_and_re_registering_replaces_it() {
    let mut reg = LayerRegistry::default();
    let anonymous = reg.add(None, LayerKind::OnDemand, false, false, empty_root());
    let name = layer_name(HOST_OWNER, "expose").expect("valid");

    let slot = reg.slot_for_name(&name);
    let first = reg.add_named(
        slot, name.clone(), None, LayerKind::OnDemand, false, true, empty_root(),
    );
    assert_eq!(reg.by_name(&name), Some(first));
    assert!(!reg.is_visible_named(&name), "OnDemand starts hidden");

    reg.show(first);
    assert!(reg.is_visible_named(&name));

    // A remount registers the same name again: one layer, the new one — and it **stays up**.
    //
    // ⚠️ Changed 2026-08-11. It used to start hidden, which made every caller remember the
    // `let was_visible = …; if was_visible { show(id) }` dance around its own rebuild — and got
    // it subtly wrong: re-showing restarted the entry animation, so the exposé zoomed open
    // again on every keystroke that changed the session behind it. A re-registration is the
    // same layer with fresh content, so it keeps its place, its visibility and any animation in
    // flight.
    let slot = reg.slot_for_name(&name);
    let second = reg.add_named(
        slot, name.clone(), None, LayerKind::OnDemand, false, true, empty_root(),
    );
    // **And it keeps its ID.** Changed 2026-08-12 (F003/P082/T416): a rebuild used to mint a
    // new one, so anything holding a `LayerId` across a session change — a plugin's handle, and
    // every intent sink inside the layer's own tree, which names the layer it lives in — was
    // left pointing at a layer that no longer existed. "The same layer with fresh content" has
    // to mean the same id, or the sentence is only about the stack position.
    assert_eq!(second, first, "a rebuild is the same layer, so it is the same id");
    assert_eq!(reg.by_name(&name), Some(second), "the name follows the new registration");
    assert!(
        reg.is_visible_named(&name),
        "a rebuild of a layer that is up leaves it up — the caller does not re-show it",
    );

    // An anonymous layer answers to no name, and is untouched by a named registration.
    assert_eq!(reg.by_name("heca.nothing"), None);
    reg.show(anonymous);
    assert!(
        reg.visible_front_to_back().iter().any(|l| l.id == anonymous),
        "the unnamed layer is still in the stack after two named registrations",
    );
}
