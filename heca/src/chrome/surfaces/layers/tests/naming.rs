use super::*;

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

/// **A described layer can be named, so something can point at it**.
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
    // ⚠️ Changed. It used to start hidden, which made every caller remember the
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
    // **And it keeps its ID.** Changed: a rebuild used to mint a
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
/// picked it**.
///
/// This is the assertion the keyboard half of that task rests on. The host used to look the
/// front-most modal up in this registry and dispatch straight into its node; it now hands the key
/// to the window root and lets delivery follow focus. That only works if an open surface actually
/// holds focus where it is placed — and before this test nothing exercised it, because the old path
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
