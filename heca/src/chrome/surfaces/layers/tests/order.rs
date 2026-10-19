use super::*;

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

/// **A rebuild keeps the layer's place, so it cannot climb over what opened above it.**
///
/// `top_modal_root_mut` takes the *last* active modal in the stack, so position is what "on
/// top" means. Re-registering by removing and re-appending therefore promoted a layer above
/// everything opened since — which is how deleting a pane from the exposé did nothing at all:
/// the confirm dialog opened above the map, the session change rebuilt the map, the rebuild put
/// it back on top of the dialog, and the click on "Close" went to the map.
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
