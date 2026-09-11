//! The layer stack's own tests: what is on screen, what is in charge, and what order they read.

use super::*;
use heca_grid_ui::animation::Animation;
use heca_grid_ui::widgets::{Flex, Overlay};

/// **One frame, in the order `handle_about_to_wait` runs it.**
///
/// Snapshot who is leaving, let the **tree** tick (the surfaces are its children), then let the
/// registry retire whatever finished. Written as a helper rather than a registry method because
/// that is the real shape: the registry does not advance anything any more.
///
/// The order is the test. When the registry ticked the trees *as well as* the tree ticking them,
/// every surface advanced twice a frame and the finishing transition was consumed before the
/// registry looked — the exposé stuck mid-fade and stayed modal (Antonio, driving, 2026-08-31).
/// A test that called `reg.tick` alone could not see that, because it never ran the tree's walk.
fn frame(reg: &mut LayerRegistry, window: &mut heca_grid_ui::widgets::Flex, dt: f32) -> bool {
    let leaving = reg.leaving_before_tick(window);
    let ticked = window.tick(dt);
    let retired = reg.retire_finished_exits(window, &leaving);
    ticked || retired
}

/// A layer whose surface declares nothing: it appears and goes between two frames.
fn empty_root() -> Box<dyn Component> {
    Box::new(Flex::row())
}

/// **A surface that declares what it is** — the two things an overlay says about the application
/// underneath it, said the way a plugin author says them: one builder each, on the widget
/// (F003/P097/T499). The registry is told neither.
fn declaring(
    mut root: Box<dyn Component>,
    captures_keyboard: bool,
    lock: bool,
) -> Box<dyn Component> {
    // **Taking the keyboard is read from the tree**, not declared — a surface that wants keys holds
    // focus. These fixtures are bare `Flex`es with no focus of their own, so they override it; a
    // real overlay writes nothing and answers by being open.
    root.base_mut().captures_keyboard = Some(captures_keyboard);
    root.base_mut().lock = lock;
    root
}

/// **A real layer root**: a surface that names its own gesture, exactly as the exposé and a
/// plugin's panel do — one builder on the widget, nothing said to the registry.
fn fading_root() -> Box<dyn Component> {
    Box::new(Overlay::new().panel(Flex::row()).animation(Animation::Fade))
}

/// The same, for a surface that arrives by pulling back — the exposé's own gesture.
fn zooming_root() -> Box<dyn Component> {
    Box::new(
        Overlay::new()
            .panel(Flex::row())
            .animation(Animation::Zoom.from(1.3)),
    )
}

/// What scale a layer's surface is drawing itself at this frame.
fn scale_of(window: &heca_grid_ui::widgets::Flex, id: LayerId) -> f32 {
    crate::chrome::surface_node(window, id)
        .and_then(|n| n.presence())
        .map_or(1.0, |p| p.frame().scale)
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
    // repaint (Antonio, driving, 2026-08-19). This assertion used to read `!frame(&mut reg, &mut window, ..)`,
    // which is that bug written down.
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

/// **A `Modal`-band layer covers whatever it declared** — the property no call site can get
/// wrong (F003/P086/T371). It demands a decision, so acts on the panes behind it are refused by
/// construction; the alternative is trusting a `bool` at every `insert`, and the one that
/// passed `false` let `prefix+x` raise the close-pane confirm with a context menu open (user,
/// 2026-07-30).
#[test]
fn coverage_is_what_the_surface_declared_not_what_its_place_implies() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    // Takes the keyboard, declares it covers nothing — and is believed.
    reg.insert(
        LayerId(7),
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, false),
        &mut window,
    );
    assert!(
        !reg.content_covered(&window),
        "a modal that says it covers nothing covers nothing — the band used to overrule this",
    );

    // The decision-demanding surfaces declare it themselves, which is where it belongs:
    // `open_modal` and `insert_menu_layer` both pass `true`.
    reg.insert(
        LayerId(8),
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    assert!(
        reg.content_covered(&window),
        "and a surface that says it covers, does"
    );
}

/// **Below the `Modal` band, coverage is what the layer declared — `modal` says nothing about
/// it.** The two answer different questions: `modal` is "does this take the keyboard",
/// `lock` is "may actions still touch the panes". Conflating them made one surface
/// impossible to describe — the exposé takes the keyboard *and* is a map of the panes, so while
/// `modal` implied coverage it refused every act on the pane it exists to let you choose.
#[test]
fn an_overlay_that_takes_the_keyboard_can_still_declare_it_covers_nothing() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let map = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, map);
    assert!(
        !reg.content_covered(&window),
        "a map of the panes does not cover them"
    );
    assert_eq!(
        reg.top_modal_id(&window),
        Some(map),
        "and it still owns the keyboard"
    );
}

/// The one input `Domain::Overlay` reads: a *visible* covering layer, and only that
/// (F003/P086/T371). A layer that covers but is hidden is not covering anything.
#[test]
fn coverage_is_reported_only_while_the_layer_is_visible() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let corner = reg.add(
        None,
        LayerKind::Persistent,
        declaring(empty_root(), false, false),
        &mut window,
    );
    assert!(
        !reg.content_covered(&window),
        "a non-covering layer covers nothing"
    );

    let over = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    assert!(!reg.content_covered(&window), "on-demand starts hidden");
    reg.show(&mut window, over);
    assert!(
        reg.content_covered(&window),
        "shown, and it covers the panes"
    );
    reg.hide(&mut window, over);
    assert!(!reg.content_covered(&window));
    reg.remove(&mut window, corner);
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

/// **z is the position in the tree, so order is a pre-order walk of it.** Two rules, and both
/// fall out of comparing the z-paths rather than being written anywhere: a later sibling is in
/// front of an earlier one, and a child is in front of its parent.
///
/// This replaced `band_orders_front_to_back`, which asserted a five-variant enum's ranking —
/// a stored z one step removed, and the thing §6 of the surface-compositor model forbids.
#[test]
fn z_is_a_path_so_a_child_is_in_front_of_its_parent_and_a_later_sibling_of_an_earlier() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let first = reg.add(
        None,
        LayerKind::Persistent,
        declaring(empty_root(), false, false),
        &mut window,
    );
    let second = reg.add(
        None,
        LayerKind::Persistent,
        declaring(empty_root(), false, false),
        &mut window,
    );
    // Opened BY `first`, so it hangs from it — and sits above it without outranking `second`'s
    // own children, because a path is compared left to right.
    let childs_child = reg.add(
        Some(first),
        LayerKind::Persistent,
        declaring(empty_root(), false, false),
        &mut window,
    );

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
    let mut window = crate::chrome::new_window_root();
    let panel = reg.add(
        None,
        LayerKind::Persistent,
        declaring(empty_root(), false, false),
        &mut window,
    );
    let unrelated = reg.add(
        None,
        LayerKind::Persistent,
        declaring(empty_root(), false, false),
        &mut window,
    );
    let modal = reg.add(
        Some(panel),
        LayerKind::Persistent,
        declaring(empty_root(), true, false),
        &mut window,
    );

    let order: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
    assert_eq!(order, vec![unrelated, modal, panel]);
    assert_eq!(
        reg.top_modal_id(&window),
        Some(modal),
        "and input agrees with the picture, because both read the one order",
    );
}

/// **A surface that names itself is reachable by that name**. ⚠️ Ran red first.
///
/// The third of the three doors — `default_open` for the starting value, `open_when` for state you
/// hold, and *point at it by name* — was never connected to anything. A widget's `key` is the name
/// the component chose about itself, but nothing read it when the surface was seated, and worse,
/// seating **overwrote** it with the host's internal slot. So
/// `Dialog::new("..").key("confirm")` declared a name that was destroyed on the way onto the
/// screen, and the only surfaces `show_layer` could reach were the ones the host had registered by
/// hand under a name typed in a second place — two answers to one question, and a plugin could
/// give neither.
#[test]
fn a_surface_that_names_itself_can_be_reached_by_that_name() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();

    let mut root = empty_root();
    root.base_mut().key = Some("confirm".to_string());
    let id = reg.reserve_id();
    reg.insert(id, None, LayerKind::OnDemand, root, &mut window);

    assert_eq!(
        reg.by_name("heca.confirm"),
        Some(id),
        "the surface's own key becomes the name `show_layer` resolves, with the owner half stamped \
         here so it cannot be forged",
    );
    assert_eq!(
        crate::chrome::surface_node(&window, id).and_then(|n| n.base().key.clone()),
        Some("confirm".to_string()),
        "and seating it leaves the author's own name alone — it used to be overwritten by the \
         host's internal slot, so the declaration was gone the moment it went on screen",
    );
}

/// **A surface that names nothing stays anonymous, and that is fine.**
///
/// `key` is optional everywhere in this codebase and a surface is no exception: one that declares
/// no name is reached by the id its opener kept, exactly as a dropdown or a modal always has been.
/// Guarding it because the tempting fix for the test above is to require a key.
#[test]
fn a_surface_that_names_nothing_is_still_a_perfectly_good_surface() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();

    let id = reg.reserve_id();
    reg.insert(id, None, LayerKind::OnDemand, empty_root(), &mut window);

    assert_eq!(reg.name_of(id), None, "no name declared, so none invented");
    reg.show(&mut window, id);
    assert!(
        reg.visible_front_to_back().iter().any(|l| l.id == id),
        "and it is on screen like any other surface",
    );
}

/// **A described layer can be named, so something can point at it** (F003/P097/T502).
///
/// `show_layer` / `hide_layer` address a surface **by name** — that is how a key binding, the
/// palette or RPC reaches one, none of which could ever know a runtime `LayerId`. Until this,
/// `add_named` took a native tree and `add_view` took a description and wrote no name at all, so a
/// plugin could have a surface it could put up and then nothing that could point at it: the two
/// halves were reachable one at a time and never together.
#[test]
fn a_described_layer_can_be_named_so_an_action_can_reach_it() {
    use heca_view::{ViewNode, WidgetKind};
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();

    let slot = reg.reserve_id();
    let named = reg.add_view(
        slot,
        Some("heca.confirm".to_string()),
        None,
        LayerKind::OnDemand,
        ViewNode::new(WidgetKind::Label),
        declaring(empty_root(), false, true),
        &mut window,
    );

    assert_eq!(
        reg.by_name("heca.confirm"),
        Some(named),
        "a described surface answers to its name, so `show_layer heca.confirm` reaches it",
    );
    reg.show(&mut window, named);
    let layers = reg.visible_front_to_back();
    let layer = layers
        .iter()
        .find(|l| l.id == named)
        .expect("the layer is registered");
    assert!(
        layer.node.is_some(),
        "and naming it did not cost it its description — that is what a theme reload re-realizes \
         from",
    );
}

/// **A described layer is a real layer.** It sorts, shows, hides and covers exactly like a
/// native one — the arm decides where the tree came from, never how the stack treats it.
#[test]
fn a_view_layer_behaves_like_any_other_and_keeps_its_description() {
    use heca_view::{ViewNode, WidgetKind};
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let native = reg.add(
        None,
        LayerKind::Persistent,
        declaring(empty_root(), false, false),
        &mut window,
    );
    let node = ViewNode::new(WidgetKind::Label);
    let slot = reg.reserve_id();
    let described = reg.add_view(
        slot,
        None,
        None,
        LayerKind::Persistent,
        node,
        declaring(empty_root(), false, true),
        &mut window,
    );

    let order: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
    assert_eq!(
        order,
        vec![described, native],
        "band decides order, not the content arm"
    );
    assert!(
        reg.content_covered(&window),
        "a described layer declares coverage like any other"
    );

    // The description is KEPT, not thrown away once realized: it is what a theme reload or a
    // plugin update re-realizes from.
    let layer = reg
        .visible_front_to_back()
        .into_iter()
        .find(|l| l.id == described)
        .expect("the described layer is in the stack");
    assert!(layer.node.is_some(), "the ViewNode survives realization");
    assert!(
        crate::chrome::surface_node(&window, described).is_some(),
        "and its realized tree is a node in the window root, where the one walk finds it",
    );
    assert_eq!(layer.node.as_ref().map(|n| n.kind), Some(WidgetKind::Label),);

    // And a native layer has no description to offer — the arms are not interchangeable.
    let native_layer = reg
        .visible_front_to_back()
        .into_iter()
        .find(|l| l.id == native)
        .expect("the native layer is in the stack");
    assert!(
        native_layer.node.is_none(),
        "a natively-built surface was never described"
    );
}

/// **The owner half is unforgeable.** An author supplies only the short name, so a plugin has
/// nowhere to write a prefix — and a short name carrying its own dot is rejected rather than
/// joined, or `expose.thing` would read as if `docker.expose` owned it.
#[test]
fn a_layer_name_is_stamped_from_its_owner_and_cannot_be_forged() {
    assert_eq!(
        layer_name("docker", "expose").as_deref(),
        Some("docker.expose")
    );
    assert_eq!(
        layer_name(HOST_OWNER, "expose").as_deref(),
        Some("heca.expose")
    );
    assert_eq!(
        layer_name("docker", "expose.thing"),
        None,
        "no smuggled second segment"
    );
    assert_eq!(
        layer_name("docker", "heca.expose"),
        None,
        "cannot claim another namespace"
    );
    assert_eq!(layer_name("docker", ""), None);
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

/// **Input goes to the layer the user sees in front, and there is only one order to read.**
///
/// Painting used to sort by band while input took "the last one added", so the two could name
/// different layers — a dialog was drawn over the exposé while the map quietly took the
/// pointer. Both now read the z-path, so disagreeing is not expressible.
#[test]
fn input_goes_to_the_front_most_surface_not_the_last_one_added() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let map = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, map);
    // Raised FROM the map, so it is the map's child and above it.
    let dialog = reg.add(
        Some(map),
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    reg.show(&mut window, dialog);

    assert_eq!(
        reg.top_modal_id(&window),
        Some(dialog),
        "a surface opened from the map sits above the map",
    );

    // Among siblings, the later one is in front.
    let second_dialog = reg.add(
        Some(map),
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    reg.show(&mut window, second_dialog);
    assert_eq!(
        reg.top_modal_id(&window),
        Some(second_dialog),
        "same parent, later wins"
    );

    // And a layer on its way out never holds the input.
    reg.remove(&mut window, second_dialog);
    assert_eq!(
        reg.top_modal_id(&window),
        Some(dialog),
        "a dissolving layer is not the target"
    );
}

/// **The active context follows the exclusive surfaces**, so an overlay knows what to hang
/// from without any caller telling it. Closing one restores the context beneath it.
#[test]
fn the_active_context_follows_the_modal_surfaces() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    assert_eq!(
        reg.current(),
        None,
        "the base context: panes, sidebar and floats together"
    );

    let map = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, map);
    assert_eq!(reg.current(), Some(map));

    let dialog = reg.add(
        reg.current(),
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    reg.show(&mut window, dialog);
    assert_eq!(
        reg.current(),
        Some(dialog),
        "a modal raised from the map takes the context"
    );
    assert_eq!(
        reg.get(dialog).and_then(|l| l.parent),
        Some(map),
        "and hangs from it"
    );

    reg.hide(&mut window, dialog);
    assert_eq!(
        reg.current(),
        Some(map),
        "closing it hands the context back"
    );
    reg.hide(&mut window, map);
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
    let mut window = crate::chrome::new_window_root();
    let name = layer_name(HOST_OWNER, "expose").expect("valid");
    let slot = reg.slot_for_name(&name);
    let map = reg.add_named(
        slot,
        name.clone(),
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, map);
    // A confirm dialog opens ON TOP of it.
    let dialog = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    reg.show(&mut window, dialog);
    assert_eq!(
        reg.top_modal_id(&window),
        Some(dialog),
        "the dialog is the input target"
    );

    // The session changes under both, so the map is rebuilt.
    let slot = reg.slot_for_name(&name);
    let rebuilt = reg.add_named(
        slot,
        name.clone(),
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, rebuilt);

    assert_eq!(
        reg.top_modal_id(&window),
        Some(dialog),
        "the rebuilt map must NOT take the keyboard and the pointer from the dialog above it",
    );
    assert_eq!(
        reg.by_name(&name),
        Some(rebuilt),
        "and the name still resolves to the new tree"
    );
}

/// A named layer is addressable without knowing its `LayerId` — which is a runtime counter no
/// keybinding or RPC call could know. Re-registering the name replaces it, as a remount should.
#[test]
fn a_named_layer_is_addressable_and_re_registering_replaces_it() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let anonymous = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), false, false),
        &mut window,
    );
    let name = layer_name(HOST_OWNER, "expose").expect("valid");

    let slot = reg.slot_for_name(&name);
    let first = reg.add_named(
        slot,
        name.clone(),
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), false, true),
        &mut window,
    );
    assert_eq!(reg.by_name(&name), Some(first));
    assert!(
        !reg.is_visible_named(&window, &name),
        "OnDemand starts hidden"
    );

    reg.show(&mut window, first);
    assert!(reg.is_visible_named(&window, &name));

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
        slot,
        name.clone(),
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), false, true),
        &mut window,
    );
    // **And it keeps its ID.** Changed 2026-08-12 (F003/P082/T416): a rebuild used to mint a
    // new one, so anything holding a `LayerId` across a session change — a plugin's handle, and
    // every intent sink inside the layer's own tree, which names the layer it lives in — was
    // left pointing at a layer that no longer existed. "The same layer with fresh content" has
    // to mean the same id, or the sentence is only about the stack position.
    assert_eq!(
        second, first,
        "a rebuild is the same layer, so it is the same id"
    );
    assert_eq!(
        reg.by_name(&name),
        Some(second),
        "the name follows the new registration"
    );
    assert!(
        reg.is_visible_named(&window, &name),
        "a rebuild of a layer that is up leaves it up — the caller does not re-show it",
    );

    // An anonymous layer answers to no name, and is untouched by a named registration.
    assert_eq!(reg.by_name("heca.nothing"), None);
    reg.show(&mut window, anonymous);
    assert!(
        reg.visible_front_to_back()
            .iter()
            .any(|l| l.id == anonymous),
        "the unnamed layer is still in the stack after two named registrations",
    );
}

/// **A key reaches a surface because the surface is a node in the tree, not because the host
/// picked it** (F003/P097/T495).
///
/// This is the assertion the keyboard half of that task rests on. The host used to look the
/// front-most modal up in this registry and dispatch straight into its node; it now hands the key
/// to the window root and lets delivery follow focus. That only works if an open surface actually
/// holds focus where it is placed — and until this task nothing exercised it, because the old path
/// reached the surface whether it held focus or not.
///
/// So the failure it guards against is silent and total: a dialog that answers no key at all, with
/// every test still green. A `Handled::No` here means the focus half of the seating is broken.
/// **Fix the focus** — do not reintroduce a predicate that offers keys to a widget which does not
/// hold them (`routes_own_subtree` / `takes_raw_keys` / `takes_text_input` were deleted for
/// exactly that).
///
/// A `Dialog` is the shape the app really places (`OverlayHost::open_modal` builds one), and Tab
/// is the key it answers itself — so this asks the same question the user asks by pressing Tab in
/// a confirm dialog.
#[test]
fn a_key_dispatched_at_the_window_root_reaches_a_placed_surface() {
    use heca_grid_ui::widgets::{Button, Dialog};
    use heca_grid_ui::{Event, GridKey, Handled};

    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(
            Box::new(
                Dialog::new("Close pane?")
                    .action(Button::new("Cancel"))
                    .action(Button::new("OK"))
                    .default_open(true),
            ),
            true,
            true,
        ),
        &mut window,
    );
    reg.show(&mut window, id);

    assert_eq!(
        heca_grid_ui::dispatch(
            &mut window,
            &Event::Key {
                key: GridKey::Tab,
                pressed: true,
            },
        ),
        Handled::Yes,
        "the key never reached the surface: an open dialog must hold focus where it is placed, \
         so delivery finds it without the host naming it",
    );
}

/// **A menu is its own panel, so seating it as a surface must not resize it** (F003/P097/T495).
///
/// `place_surface` gives every surface the whole viewport, which is right for an `Overlay` — a
/// layer fills the window and positions a panel *inside* itself. A `ContextMenu` is not shaped
/// that way: the widget **is** the panel (`panel_base` sets its own direction, padding and width
/// bounds, and its rows are its children), so handing it a full-viewport box stretches the panel
/// down the whole window.
///
/// Two things go wrong at once, and only one of them is visible: the panel is drawn floor to
/// ceiling, **and** its hit area becomes the whole window — so with a menu open, every press
/// anywhere lands on the menu instead of what is under the cursor, and a right-click elsewhere
/// opens nothing (Antonio, driving, 2026-09-01).
#[test]
fn a_menu_seated_as_a_surface_keeps_its_own_height() {
    use heca_core::layout::Size;
    use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
    use heca_grid_ui::LayoutEngine;

    let viewport = Size::new(1400.0, 900.0);
    let menu = ContextMenu::new("pane")
        .child(
            Menu::new("Pane", "")
                .child(MenuItem::new().label("New column"))
                .child(MenuItem::new().label("New pane"))
                .child(MenuItem::new().label("Close pane")),
        )
        .default_open(true);

    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(Box::new(menu), true, true),
        &mut window,
    );
    reg.show(&mut window, id);
    LayoutEngine::new().compute(&mut window, viewport);

    let h = crate::chrome::surface_node(&window, id)
        .expect("the menu is seated")
        .base()
        .bounds
        .size
        .h;
    assert!(
        h < viewport.h / 2.0,
        "the menu was stretched to the seat instead of keeping its own height: {h} of {}",
        viewport.h,
    );
}

/// An open layer, built the way `fading_root` builds one.
fn fading_root_open() -> Overlay {
    let mut o = Overlay::new().panel(Flex::row());
    o.show();
    o
}

/// The other half of the seat, and the silent one: **a layer-shaped surface still fills the
/// window**.
///
/// The seat leaves both sizes to the surface, so this is what would break if a layer ever stopped
/// declaring its own — and it would break invisibly, as a surface that paints and hit-tests in a
/// box the size of its content while still believing it owns the screen.
#[test]
fn a_layer_seated_as_a_surface_still_fills_the_window() {
    use heca_core::layout::Size;
    use heca_grid_ui::LayoutEngine;

    let viewport = Size::new(1400.0, 900.0);
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(Box::new(fading_root_open()), true, true),
        &mut window,
    );
    reg.show(&mut window, id);
    LayoutEngine::new().compute(&mut window, viewport);

    assert_eq!(
        crate::chrome::surface_node(&window, id)
            .expect("the layer is seated")
            .base()
            .bounds
            .size,
        viewport,
        "a layer declares its own full-viewport size and the seat must not shrink it",
    );
}

/// **A seated surface must not stand between the pointer and the page where it draws nothing**
/// (F003/P097/T495).
///
/// The notification stack's box is the whole window on purpose — that is how a corner means the
/// *screen's* corner — and it sits above the chrome, because child order is z-order. Take the
/// default input surface and the consequence is total and silent: every press anywhere in the app
/// hit-tests to the stack, so the chrome under it stops answering the mouse and a sidebar row can
/// never be right-clicked (Antonio, driving, 2026-09-01).
///
/// The guard is here, on the seating, rather than only in the widget: this is the arrangement that
/// makes it dangerous — a window-sized node in front of everything — and any future surface seated
/// the same way is subject to the same rule.
#[test]
fn a_surface_that_draws_nothing_does_not_take_the_pointer_from_the_chrome() {
    use heca_core::layout::{Point, Size};
    use heca_grid_ui::reactive::signal;
    use heca_grid_ui::style::Length;
    use heca_grid_ui::widgets::{KeyHintGroup, ToastStack};
    use heca_grid_ui::{LayoutEngine, LayoutExt};

    let viewport = Size::new(1280.0, 800.0);
    let mut window = crate::chrome::new_window_root();
    // The chrome: a plain child of the window root, and the thing the pointer must reach.
    crate::chrome::seat_chrome(
        &mut window,
        Flex::row()
            .width(Length::Percent(1.0))
            .height(Length::Percent(1.0)),
    );
    // The stack, seated **exactly as the app seats it** — wrapped in its picker group, and empty,
    // as it is nearly always. The wrapper is the point: a first version of this guard placed a bare
    // stack, passed, and the app was still broken, because what the pointer actually meets is the
    // decorator hugging it. A guard for a seating must build the arrangement the app seats.
    crate::chrome::place_surface(
        &mut window,
        "heca.notifications",
        Box::new(KeyHintGroup::new_boxed(Box::new(ToastStack::new(signal(
            Vec::new(),
        ))))),
    );
    LayoutEngine::new().compute(&mut window, viewport);

    let path = heca_grid_ui::hit_test(&window, Point::new(146.0, 171.0))
        .expect("the chrome is under the pointer");
    assert_eq!(
        path.first(),
        Some(&0),
        "the pointer reached child {:?} — the chrome is child 0 and an empty surface must not \
         claim the point in front of it",
        path.first(),
    );
}

/// **A dismissed surface declares nothing** (F003/P097/T499, found by Antonio driving).
///
/// Hiding a surface calls `node.close()` and leaves it **seated** in the window root —
/// `remove_surface` runs only when it is destroyed — so presence in the tree says nothing about
/// being in charge. The exposé and the command palette both declare `lock` and `modal`,
/// so a reader that takes a seated surface's declaration at face value applies a hidden exposé's
/// occluders and treats a hidden modal as the active context.
///
/// That is exactly what happened: the `prefix+/` stack stopped asking the registry for its visible
/// layers and started walking the tree's children, and lost the `visible` filter it had been
/// getting for free — **every letter in the app disappeared**, the universal picker and `prefix+q`
/// alike, in every state.
#[test]
fn a_hidden_surfaces_declaration_is_not_live() {
    let mut window = Flex::column();
    let mut reg = LayerRegistry::default();

    // An on-demand surface that declares BOTH things a dismissed one must stop declaring — said
    // on the widget, which is the only place they live now.
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    let slot = crate::chrome::surface_slot(id);

    // On-demand starts hidden, and its node is already seated.
    assert!(
        reg.declaration_at(&slot).is_some(),
        "the surface is seated and registered whether or not it is up",
    );
    assert!(
        !reg.slot_is_live(&window, &slot),
        "a surface that is not up declares nothing — reading it anyway is what blanked \
         every letter in the app",
    );

    reg.show(&mut window, id);
    assert!(
        reg.slot_is_live(&window, &slot),
        "a surface that is up declares what it declared",
    );
    let node = crate::chrome::surface_node(&window, id).expect("seated");
    assert!(
        node.base().lock,
        "and coverage is the widget's own declaration, not a record the registry kept",
    );

    reg.hide(&mut window, id);
    assert!(
        !reg.slot_is_live(&window, &slot),
        "dismissed again: seated, still registered, and declaring nothing",
    );
}
