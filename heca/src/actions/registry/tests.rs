use super::*;
use crate::actions::testing::dyn_meta;
use crate::input::WmAction;
use heca_grid_ui::Glyph;

#[test]
fn test_registry_dispatch() {
    let mut registry = ActionRegistry::new();
    fn dummy_handler(state: &mut crate::app_state::AppState, _action: &WmAction) {
        state.needs_redraw = true;
    }
    registry.register(crate::input::WmActionKind::FocusLeft, dummy_handler);
    assert!(registry.has_handler(&WmAction::FocusLeft));
    assert!(!registry.has_handler(&WmAction::FocusRight));
}

/// A name-keyed action joins the SAME catalog as the built-ins — so it gets a label, an icon and
/// introspection exactly like `close` does. This is the whole point of the owned ripple: before
/// it, a plugin action could only be dispatched, never rendered.
#[test]
fn a_dynamic_action_lives_in_the_same_catalog_as_the_builtins() {
    use crate::app::interaction::ActionPolicy;
    let mut registry = ActionRegistry::new();
    let mut catalog = ActionCatalog::with_builtins();
    let builtins = catalog.all().count();

    let handle = register_dynamic(
        &mut registry,
        &mut catalog,
        dyn_meta("plugin.docker.restart", ActionPolicy::Global),
        Some(std::rc::Rc::new(|_state, _intent| {})),
    );

    assert_eq!(
        handle,
        Ok(ActionHandle("plugin.docker.restart".to_string()))
    );
    assert_eq!(catalog.all().count(), builtins + 1);
    assert_eq!(
        catalog.label("plugin.docker.restart"),
        Some("Restart Container")
    );
    assert_eq!(catalog.icon("plugin.docker.restart"), Some(Glyph::Trash));
    assert_eq!(
        catalog.policy("plugin.docker.restart"),
        Some(ActionPolicy::Global),
        "the router reads the DECLARED policy"
    );
    assert!(!catalog.is_builtin("plugin.docker.restart"));
    assert!(
        registry.has_dynamic_handler("plugin.docker.restart"),
        "the host can run it"
    );
    // A built-in is still a built-in, beside it in the same catalog.
    assert!(catalog.is_builtin("close"));
    assert!(catalog.find("nope.not.a.thing").is_none());
}

/// **Introspection says whose verb it is** (F003/P085/T358). Without an owner, `list-actions`
/// returns one flat list in which a component's actions are indistinguishable from the app's,
/// and a tool has no way to group or scope them.
#[test]
fn introspection_names_the_component_that_declared_an_action() {
    use crate::app::interaction::ActionPolicy;
    let mut registry = ActionRegistry::new();
    let mut catalog = ActionCatalog::with_builtins();
    let mut meta = dyn_meta("docker.restart_selected", ActionPolicy::ContainerFocused);
    // What `register_provider_actions` stamps from the provider being registered.
    meta.owner = Some("docker".to_string());
    let _ = register_dynamic(&mut registry, &mut catalog, meta, None);

    let info = catalog
        .describe("docker.restart_selected")
        .expect("a declared action is describable");
    assert_eq!(info.owner.as_deref(), Some("docker"));
    assert_eq!(
        catalog.describe("close").and_then(|i| i.owner),
        None,
        "a built-in belongs to the app itself",
    );
    // And it survives the wire form, which is the only reason it exists.
    let json = serde_json::to_string(&info).expect("ActionInfo is serializable");
    assert!(json.contains("\"owner\":\"docker\""), "{json}");
}

/// `unregister` (the provider unmounting and dropping its handle) retires BOTH halves — the
/// handler and the metadata — so the id is no longer dispatchable or renderable.
#[test]
fn unregister_retires_both_the_handler_and_the_metadata() {
    use crate::app::interaction::ActionPolicy;
    let mut registry = ActionRegistry::new();
    let mut catalog = ActionCatalog::with_builtins();
    let _ = register_dynamic(
        &mut registry,
        &mut catalog,
        dyn_meta("plugin.docker.restart", ActionPolicy::TiledOnly),
        Some(std::rc::Rc::new(|_state, _intent| {})),
    );

    assert!(unregister_dynamic(
        &mut registry,
        &mut catalog,
        "plugin.docker.restart"
    ));
    assert!(catalog.find("plugin.docker.restart").is_none());
    assert!(!registry.has_dynamic_handler("plugin.docker.restart"));
    // Idempotent: retiring it twice is not an error.
    assert!(!unregister_dynamic(
        &mut registry,
        &mut catalog,
        "plugin.docker.restart"
    ));
}

/// Re-registering an id (a provider remounting) REPLACES the entry rather than duplicating it.
#[test]
fn re_registering_an_id_replaces_it() {
    use crate::app::interaction::ActionPolicy;
    let mut registry = ActionRegistry::new();
    let mut catalog = ActionCatalog::with_builtins();
    let before = catalog.all().count();
    let _ = register_dynamic(
        &mut registry,
        &mut catalog,
        dyn_meta("plugin.x", ActionPolicy::Global),
        None,
    );
    let mut second = dyn_meta("plugin.x", ActionPolicy::TiledOnly);
    second.label = "Second".to_string();
    let _ = register_dynamic(&mut registry, &mut catalog, second, None);

    assert_eq!(
        catalog.all().count(),
        before + 1,
        "replaced, not duplicated"
    );
    assert_eq!(catalog.label("plugin.x"), Some("Second"));
    assert_eq!(catalog.policy("plugin.x"), Some(ActionPolicy::TiledOnly));
}

/// A DECLARATIVE action (no host handler — its owner is a WASM plugin) is declared and
/// policy-classified, but the host cannot run it. Dispatching it is a no-op, never a crash.
#[test]
fn a_declarative_action_is_declared_but_has_no_host_handler() {
    use crate::app::interaction::ActionPolicy;
    let mut registry = ActionRegistry::new();
    let mut catalog = ActionCatalog::with_builtins();
    let _ = register_dynamic(
        &mut registry,
        &mut catalog,
        dyn_meta("plugin.wasm.thing", ActionPolicy::FocusedPaneLocal),
        None, // no host handler
    );
    assert!(catalog.find("plugin.wasm.thing").is_some(), "declared");
    assert!(
        !registry.has_dynamic_handler("plugin.wasm.thing"),
        "but the host cannot run it"
    );
    assert_eq!(
        catalog.policy("plugin.wasm.thing"),
        Some(ActionPolicy::FocusedPaneLocal)
    );
}

/// A dynamic action can neither shadow nor retire a built-in.
#[test]
fn a_dynamic_action_cannot_retire_a_builtin() {
    let mut registry = ActionRegistry::new();
    let mut catalog = ActionCatalog::with_builtins();
    assert!(!unregister_dynamic(&mut registry, &mut catalog, "close"));
    assert!(catalog.find("close").is_some(), "built-in survives");
}
