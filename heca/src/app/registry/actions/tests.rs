//! Tests for [`super`].

use super::*;

use crate::input::WmAction;

/// **Every catalogued action can actually run.** Metadata lives in `actions::builtins()` and
/// handlers in `build_registry()`, two places that could drift; this closes the direction that
/// matters — a descriptor whose action has no handler is an entry the whole UI advertises
/// (icon, label, command palette, `list-actions`) and that panics in debug when pressed.
///
/// The other direction — a handler with no descriptor — is already held by
/// `every_wm_action_variant_is_reachable_by_name` in `input.rs`, which walks the variants
/// rather than the names. Between them the two lists cannot fall out of step, which is the
/// property F003/P010/T005 exists to guarantee.
#[test]
fn every_catalogued_action_has_a_handler() {
    use crate::actions::builtins;
    use crate::args::{ArgSpec, sample_args};
    use crate::input::resolve_action;

    let registry = build_registry();
    let mut missing = Vec::new();
    for descriptor in builtins() {
        let args: Vec<ArgSpec> = descriptor
            .args
            .iter()
            .map(ArgSpec::from_descriptor)
            .collect();
        let Some(action) = resolve_action(descriptor.name, &sample_args(&args)) else {
            // Not this test's business: `every_wm_action_variant_is_reachable_by_name` owns it.
            continue;
        };
        // Overlay control is resolved by the dispatcher, never by a registered handler.
        if !action.is_overlay_control()
            && !registry.has_handler(&action)
            && !crate::server::ServerState::runs(action.kind())
        {
            missing.push(descriptor.name);
        }
    }
    assert!(
        missing.is_empty(),
        "these actions are catalogued — they have a label, an icon and a place in the command \
         palette — but no handler is registered for them, so pressing one panics in debug and \
         does nothing in release: {missing:#?}",
    );
}

/// A placement id is a BUILT-IN (it has a `WmAction` and a native handler), not a name-keyed
/// dynamic action — it must not fall through to `Dynamic` when its args are supplied.
#[test]
fn a_placement_id_is_a_builtin_not_a_dynamic_action() {
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let registry = build_registry();
    for name in [
        "chrome.container.move_to_region",
        "chrome.container.move_left_sidebar",
        "chrome.container.move_right_sidebar",
        "chrome.container.reorder_before",
        "chrome.container.reorder_after",
    ] {
        let meta = catalog
            .find(name)
            .unwrap_or_else(|| panic!("{name} is not in the catalog"));
        assert_eq!(meta.category, crate::actions::ActionCategory::Chrome);
        // Chrome placement acts on regions, not on the tiled/floating pane domain, so it stays
        // reachable in EITHER domain. `Global` is the only policy that survives a floating pane
        // owning the domain (`AlwaysAllowed` does NOT — it is a misnomer).
        assert_eq!(
            meta.policy,
            crate::app::interaction::ActionPolicy::Global,
            "{name} must stay reachable while a floating pane owns the domain",
        );
        assert!(catalog.is_builtin(name), "{name} is a built-in");
        assert!(
            !registry.has_dynamic_handler(name),
            "{name} runs through its WmAction, not by name",
        );
    }
}

#[test]
fn chrome_container_placement_actions_have_handlers() {
    // Every WmAction variant must have a registered handler (execute() panics
    // in debug otherwise) — assert the plugin-02 container actions are wired.
    let registry = super::build_registry();
    assert!(registry.has_handler(&WmAction::MoveContainerToRegion {
        container_id: String::new(),
        region: crate::chrome::RegionId::LeftSidebar,
    }));
    assert!(registry.has_handler(&WmAction::ReorderContainerBefore {
        container_id: String::new(),
        before_id: None,
    }));
    assert!(registry.has_handler(&WmAction::ReorderContainerAfter {
        container_id: String::new(),
        after_id: String::new(),
    }));
    assert!(registry.has_handler(&WmAction::SetRegionVisible {
        region: crate::chrome::RegionId::LeftSidebar,
        visible: crate::input::RegionVisibility::Show,
    }));
}

/// **An action that runs on the server has no window handler.** Two handlers for one kind would be
/// two paths over the same input, and the registry would run only the one the routing picks —
/// the other would rot unseen. A kind moves to the server by leaving here.
#[test]
fn a_server_action_has_no_window_handler() {
    use crate::input::WmActionKind;
    use strum::IntoEnumIterator;
    let registry = build_registry();
    let doubled: Vec<WmActionKind> = WmActionKind::iter()
        .filter(|kind| crate::server::ServerState::runs(*kind))
        .filter(|kind| registry.has_handler_for_kind(*kind))
        .collect();
    assert!(
        doubled.is_empty(),
        "these run on the server and still have a window handler: {doubled:?}"
    );
    assert!(
        WmActionKind::iter().any(crate::server::ServerState::runs),
        "no action runs on the server — the loop above checked nothing"
    );
}
