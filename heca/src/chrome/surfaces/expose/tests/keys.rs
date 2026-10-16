//! The map's keys: delete letters and its own picker, from the shipped defaults.

use super::*;

/// **The letters reach the cards** — the whole path, from the shipped file through the built
/// keymaps to the list [`register`] hands the cards.
///
/// The gap this closed was not academic: the entry parsed, the keymap built, and the lookup
/// still came back **empty**, because the index records the qualified id
/// (`heca.expose.delete_pane`) while the lookup asked for the short name. The letters silently
/// did nothing and `x` fell through to the host, which reported an action it had never heard of
/// Asserting the entry alone — which the test below does — could not see it: both ends were
/// right and the join was wrong.
#[test]
fn the_shipped_delete_letters_reach_the_cards() {
    let mut index = crate::keymap::BindingIndex::new();
    crate::app::registry::build_component_keymaps(
        &heca_config::theme::Config::default(),
        &mut crate::app::conflicts::Conflicts::default(),
        &mut index,
    );
    let shortcuts =
        crate::chrome::ActionShortcuts::from_index(&index, crate::shortcut::KeyStyle::default());
    let name = crate::chrome::surfaces::layers::layer_name(crate::chrome::surfaces::layers::HOST_OWNER, SURFACE)
        .expect("a valid layer name");
    let found = ExposeDeleteKeys {
        pane: shortcuts.in_surface(&name, "delete_pane").to_vec(),
        column: shortcuts.in_surface(&name, "delete_column").to_vec(),
        workspace: shortcuts.in_surface(&name, "delete_workspace").to_vec(),
    };
    assert_eq!(
        found,
        shipped_keys(),
        "the map's cards must actually receive the letters the defaults bind",
    );
}

/// **The letters are config, and this is the file they live in.** They were once literals in this
/// module; the test above would happily keep passing with a default file
/// that bound nothing, and the map would then have no delete keys at all.
#[test]
fn the_shipped_defaults_bind_the_maps_delete_keys() {
    let keys = heca_config::theme::KeysConfig::default();
    let name = crate::chrome::surfaces::layers::layer_name(crate::chrome::surfaces::layers::HOST_OWNER, SURFACE)
        .expect("a valid layer name");
    let entry = keys
        .surfaces()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("keybindings.default.toml must ship a surface layer for {name}"));
    for (action, expected) in [
        ("delete_pane", "x"),
        ("delete_column", "r"),
        ("delete_workspace", "d"),
    ] {
        let bound = entry
            .bindings
            .get(action)
            .unwrap_or_else(|| panic!("{name} must bind {action}"));
        assert_eq!(bound.keys(), vec![expected.to_string()], "{action}");
    }
}

/// **The shipped config binds the map's own action**, not a built-in it borrowed. A name that
/// no widget declares binds fine, dispatches, and does nothing — indistinguishable from a typo —
/// so the file and the declaration are held together here.
#[test]
fn the_shipped_defaults_bind_the_maps_own_pick_action() {
    let config = heca_config::theme::KeysConfig::default();
    let entry = config
        .surfaces()
        .find(|e| e.name == "heca.expose")
        .expect("the map has a [[keys.surface]] entry");
    let short = PICK_ACTION
        .strip_prefix("heca.expose.")
        .expect("the action is namespaced by its surface");
    assert!(
        entry.bindings.contains_key(short),
        "`{short}` is bound in the shipped defaults: {:?}",
        entry.bindings.keys().collect::<Vec<_>>(),
    );
}
