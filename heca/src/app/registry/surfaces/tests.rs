//! Tests for [`super`].

use super::*;
use crate::app::conflicts::Conflicts;
use crate::app::registry::names::binding_arg_problems;
use crate::app::registry::testing::*;
use crate::input::WmAction;
use crate::keymap::{ActionRef, BindingIndex, KeyCombo, KeymapRegistry};
use heca_config::keys::BindingValue;
use std::collections::HashMap;

/// A component's layer resolves under its **name**, and binds any action id — its own or an
/// existing built-in it simply reuses.
#[test]
fn a_component_layer_binds_its_own_actions_and_existing_ones() {
    let config = with_layer(layer_of(
        "docker",
        None,
        &[("docker.restart_selected", "r"), ("next_pane", "n")],
    ));
    let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    let docker = maps
        .get("docker")
        .expect("an id-less entry is keyed by name");

    match docker.resolve("docker", &KeyCombo::parse("r")) {
        Some(ActionRef::Dynamic(intent)) => {
            assert_eq!(intent.action, "docker.restart_selected")
        }
        other => panic!("its own action resolves at press time: {other:?}"),
    }
    assert_eq!(
        docker.resolve_builtin("docker", &KeyCombo::parse("n")),
        Some(&WmAction::NextPane),
        "an existing action is bound here, not redeclared",
    );
}

/// **The table merges per key**: overriding one binding keeps every other default. Two entries
/// for the same component are now just two entries in the array — the builder layers them.
#[test]
fn overriding_one_binding_keeps_the_rest() {
    let config = with_layers(vec![
        layer_of(
            "docker",
            None,
            &[
                ("docker.restart_selected", "r"),
                ("docker.stop_selected", "s"),
            ],
        ),
        layer_of("docker", None, &[("docker.stop_selected", "x")]),
    ]);
    let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    let docker = &maps["docker"];

    assert!(
        docker.resolve("docker", &KeyCombo::parse("x")).is_some(),
        "the overridden action moved to its new key",
    );
    assert!(
        docker.resolve("docker", &KeyCombo::parse("r")).is_some(),
        "and every other default survived",
    );
}

/// **Binding names are short.** `cursor_up` under `name = "workspaces"` is
/// `workspaces.cursor_up`; a built-in keeps its catalog id, because a component binds those
/// rather than redeclaring them; and writing the id out in full is never wrong.
#[test]
fn a_short_name_means_the_components_own_action() {
    let config = with_layer(layer_of(
        "workspaces",
        None,
        &[
            ("cursor_up", "k"),
            ("next_pane", "n"),
            ("workspaces.peek_selected", "Space"),
        ],
    ));
    let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    let layer = &maps["workspaces"];

    match layer.resolve("workspaces", &KeyCombo::parse("k")) {
        Some(ActionRef::Dynamic(intent)) => assert_eq!(intent.action, "workspaces.cursor_up"),
        other => panic!("a short name is qualified with the component: {other:?}"),
    }
    assert_eq!(
        layer.resolve_builtin("workspaces", &KeyCombo::parse("n")),
        Some(&WmAction::NextPane),
        "a built-in keeps its own id — it is bound here, not redeclared",
    );
    match layer.resolve("workspaces", &KeyCombo::parse("Space")) {
        Some(ActionRef::Dynamic(intent)) => {
            assert_eq!(
                intent.action, "workspaces.peek_selected",
                "never qualified twice"
            )
        }
        other => panic!("an already-qualified name is left alone: {other:?}"),
    }
}

/// `global_focus` must **not** land in the container's own layer: that layer is only consulted
/// while the container already holds focus, so a key to *take* focus placed there could never
/// fire. It goes into the flat map, aimed at this container.
#[test]
fn global_focus_goes_to_the_global_map_not_the_containers_own_layer() {
    let config = with_layer(layer_of(
        "docker",
        None,
        &[("global_focus", "prefix+d"), ("restart_selected", "r")],
    ));
    let mut index = BindingIndex::new();
    let (layers, global) = build_component_keymaps(&config, &mut Conflicts::default(), &mut index);

    assert!(
        layers["docker"]
            .resolve("docker", &KeyCombo::parse("d"))
            .is_none(),
        "a key that takes focus is useless in the layer that needs focus first",
    );
    assert!(
        layers["docker"]
            .resolve("docker", &KeyCombo::parse("r"))
            .is_some(),
        "…while its ordinary bindings are untouched",
    );

    let mut flat = KeymapRegistry::new();
    bind_global_focus(&mut flat, &global, &mut Conflicts::default(), &mut index);
    assert_eq!(
        flat.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("d")),
        // **`ToggleDock`, not `FocusDock`** — `global_focus` is press-again-to-leave, and that
        // toggle belongs to the binding rather than to the verb (F003/P082/T444). `FocusDock`
        // only focuses, so a click, an RPC call and the palette cannot release a dock by asking
        // to focus it.
        Some(&WmAction::ToggleDock {
            dock: Some("docker".to_string())
        }),
        "`prefix+d` lands in the leader map, aimed at this container",
    );
    assert!(
        index["toggle_dock"]
            .iter()
            .any(|b| b.key == "prefix+d" && b.layer.contains("docker")),
        "and --keys-show can say where it came from: {index:?}",
    );
}

/// An `id` aims the key at **one seating**; without one it names the component, and
/// `placement_for` resolves that at press time to the seating the user was last in.
#[test]
fn an_id_aims_global_focus_at_one_placement() {
    let config = with_layers(vec![
        layer_of("docker", None, &[("global_focus", "prefix+d")]),
        layer_of(
            "docker",
            Some("docker.right"),
            &[("global_focus", "prefix+Shift+d")],
        ),
    ]);
    let (_, global) =
        build_component_keymaps(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    let mut flat = KeymapRegistry::new();
    bind_global_focus(
        &mut flat,
        &global,
        &mut Conflicts::default(),
        &mut BindingIndex::new(),
    );

    assert_eq!(
        flat.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Shift+d")),
        Some(&WmAction::ToggleDock {
            dock: Some("docker.right".to_string())
        }),
        "the narrowed entry names its placement",
    );
    assert_eq!(
        flat.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("d")),
        Some(&WmAction::ToggleDock {
            dock: Some("docker".to_string())
        }),
        "the id-less entry names the component, resolved at press time",
    );
}

/// Two components asking for one combo is exactly what the report exists to say out loud.
#[test]
fn two_containers_claiming_one_global_focus_is_reported() {
    let config = with_layers(vec![
        layer_of("docker", None, &[("global_focus", "prefix+d")]),
        layer_of("notes", None, &[("global_focus", "prefix+d")]),
    ]);
    let (_, global) =
        build_component_keymaps(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    let mut conflicts = Conflicts::default();
    bind_global_focus(
        &mut KeymapRegistry::new(),
        &global,
        &mut conflicts,
        &mut BindingIndex::new(),
    );
    assert_eq!(conflicts.keys.len(), 1, "one combo, two claimants");
}

/// **The shipped defaults must not make heca complain about themselves at startup.**
///
/// Every id a `[[keys.surface]]` entry actually binds is checked for missing arguments here,
/// the way `log_arg_problems` checks it at load — which is silenced under `cfg!(test)`, so
/// nothing else in this suite can see it. The exposé's `delete_column = "r"` printed *"missing
/// required argument 'ws_idx' … cannot be built and will do nothing when pressed"* on every
/// launch while working perfectly, because the probe that decides whether to qualify a name was
/// reporting on the name it went on to reject.
#[test]
fn no_shipped_surface_binding_reports_an_argument_problem() {
    let defaults = heca_config::theme::KeysConfig::default();
    for entry in defaults.surfaces() {
        for written in entry.bindings.keys() {
            let no_args = HashMap::new();
            let id = crate::app::registry::surface_action_id(&entry.name, written, &no_args);
            let problems = binding_arg_problems(&id, &no_args);
            assert!(
                problems.is_empty(),
                "[[keys.surface]] {} binds {written} → {id}, which the loader would then \
                 report as unusable: {problems:?}",
                entry.name,
            );
        }
    }
}

/// The reason the shape is an array: an `id` narrows an entry to one placement, and that
/// placement's layer carries the id-less base underneath it — so the mount lookup alone is
/// enough at press time.
#[test]
fn an_id_layers_over_the_base_for_one_placement_only() {
    let config = with_layers(vec![
        layer_of(
            "docker",
            None,
            &[
                ("docker.restart_selected", "r"),
                ("docker.stop_selected", "s"),
            ],
        ),
        layer_of(
            "docker",
            Some("docker.right"),
            &[("docker.stop_selected", "x")],
        ),
    ]);
    let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());

    let right = &maps["docker.right"];
    assert!(
        right
            .resolve("docker.right", &KeyCombo::parse("x"))
            .is_some(),
        "the narrowed binding applies to this placement",
    );
    assert!(
        right
            .resolve("docker.right", &KeyCombo::parse("r"))
            .is_some(),
        "and the base came with it — one lookup, no merging at press time",
    );

    let base = &maps["docker"];
    assert!(
        base.resolve("docker", &KeyCombo::parse("s")).is_some(),
        "every other placement still has the base binding",
    );
    assert!(
        base.resolve("docker", &KeyCombo::parse("x")).is_none(),
        "and never sees what was written for one seating",
    );
}

/// A narrowing entry sees the **whole** base, wherever it was written — which is why the build
/// runs the id-less entries in a first pass rather than in file order.
#[test]
fn a_placement_is_seeded_from_the_finished_base() {
    let config = with_layers(vec![
        layer_of(
            "docker",
            Some("docker.right"),
            &[("docker.stop_selected", "x")],
        ),
        // Written *after* the placement entry, and still part of what it inherits.
        layer_of("docker", None, &[("docker.restart_selected", "r")]),
    ]);
    let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    assert!(
        maps["docker.right"]
            .resolve("docker.right", &KeyCombo::parse("r"))
            .is_some(),
        "file order does not decide what a placement inherits",
    );
}

/// An `id` naming a placement that will never mount is not an error: config is read before
/// anything mounts, so the layer is simply built and never asked for.
#[test]
fn an_id_for_a_placement_that_never_mounts_is_harmless() {
    let mut conflicts = Conflicts::default();
    let config = with_layer(layer_of("docker", Some("nowhere"), &[("docker.stop", "s")]));
    let maps = layers_only(&config, &mut conflicts, &mut BindingIndex::new());
    assert!(maps.contains_key("nowhere"));
    assert!(conflicts.is_empty(), "nothing to report");
}

/// **The array merges by `keys`** — the rule that makes the arg-carrying form usable at all.
/// Replacing the array wholesale (TOML's own rule for arrays) would mean adding one binding
/// silently drops every shipped default.
#[test]
fn an_arg_carrying_override_replaces_only_its_own_combo() {
    use heca_config::theme::ModeBindingConfig;
    let bind = |action: &str, keys: &str, cmd: &str| ModeBindingConfig {
        action: action.to_string(),
        keys: BindingValue::Single(keys.to_string()),
        args: HashMap::from([("command".to_string(), cmd.to_string())]),
    };
    let shipped = heca_config::theme::SurfaceKeysConfig {
        name: "docker".to_string(),
        bind: vec![
            bind("spawn_command", "t", "lazydocker"),
            bind("spawn_command", "g", "lazygit"),
        ],
        ..Default::default()
    };
    // The user rebinds `t` only, in a second entry the builder layers over the first.
    let user = heca_config::theme::SurfaceKeysConfig {
        name: "docker".to_string(),
        bind: vec![bind("spawn_command", "t", "ctop")],
        ..Default::default()
    };

    let maps = layers_only(
        &with_layers(vec![shipped, user]),
        &mut Conflicts::default(),
        &mut BindingIndex::new(),
    );
    let docker = &maps["docker"];
    match docker.resolve("docker", &KeyCombo::parse("t")) {
        Some(ActionRef::Builtin(WmAction::SpawnCommand { command, .. })) => {
            assert_eq!(command, "ctop", "exactly that combo was replaced")
        }
        other => panic!("unexpected: {other:?}"),
    }
    assert!(
        docker.resolve("docker", &KeyCombo::parse("g")).is_some(),
        "and the other arg-carrying default survived — the whole point of merging by key",
    );
}

/// `unbind` is keyed by the **combo**, so it retires a binding whatever it points at — never a
/// null or empty-string convention.
#[test]
fn a_component_layer_unbinds_by_combo() {
    let mut layer = layer_of("docker", None, &[("docker.stop_selected", "s")]);
    layer.unbind.insert("s".to_string(), true);
    let maps = layers_only(
        &with_layer(layer),
        &mut Conflicts::default(),
        &mut BindingIndex::new(),
    );
    assert!(
        maps["docker"]
            .resolve("docker", &KeyCombo::parse("s"))
            .is_none()
    );
}
