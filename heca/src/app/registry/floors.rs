//! **Every panel has a way out** — and by default it is `Escape`.
//!
//! What leaves a panel depends on its kind, and each kind has a **floor**, a `[[keys.mode]]` block:
//!
//! | floor | the way out | the action |
//! |---|---|---|
//! | [`FOCUS_LAYER`](crate::app::input::FOCUS_LAYER) | a focused dock hands the keyboard back to the panes | `unfocus_dock` |
//! | [`LAYER_FLOOR`](crate::app::input::LAYER_FLOOR) | the front-most layer closes itself | `close_overlay` |
//!
//! The shipped file binds `Escape` there, as an ordinary binding the user can move: bind the action
//! to another key and give `Escape` up with `unbind = ["Escape"]` in the same block. What can never
//! happen is a panel with **no** key to leave it by, so after the merge each floor is checked: the
//! action must be bound in the floor, or behind the prefix (`prefix+<key>` works while any panel
//! holds the keyboard). A config that leaves a floor without one is refused for that floor, `Escape`
//! is put back, and the user is told — a mistake in a keybindings file must not strand the keyboard
//! with only the mouse to get out.
//!
//! (The panes have no floor on purpose: a key nothing claims belongs to the program running in
//! them, so `Escape` still means what it means inside vim.)

use super::binding::{Written, bind_with_conflict_tracking};
use crate::app::conflicts::{Conflicts, WayOutRefused};
use crate::input::WmAction;
use crate::keymap::{ActionRef, BindingIndex, KeyCombo, KeymapRegistry, LEADER_LAYER, WrittenKey};
use heca_config::theme::Config;

/// The key put back when a floor is left without one.
const RESTORED_KEY: &str = "Escape";

/// What leaves one kind of panel.
struct WayOut {
    /// The panel, in words, for the report.
    panel: &'static str,
    /// The action's config name.
    name: &'static str,
    action: ActionRef,
}

fn way_out_of(floor: &str) -> Option<WayOut> {
    match floor {
        crate::app::input::FOCUS_LAYER => Some(WayOut {
            panel: "a focused dock",
            name: "unfocus_dock",
            action: ActionRef::Builtin(WmAction::UnfocusDock),
        }),
        crate::app::input::LAYER_FLOOR => Some(WayOut {
            panel: "an overlay",
            name: "close_overlay",
            action: ActionRef::Builtin(WmAction::CloseOverlay { overlay: None }),
        }),
        _ => None,
    }
}

/// Whether the config binds `action` behind the prefix key, where it works from every panel.
fn bound_behind_the_prefix(config: &Config, action: &str) -> bool {
    let shipped = heca_config::theme::KeysConfig::default();
    let mut flat = shipped.bindings.clone();
    flat.extend(config.keys.bindings.clone());
    let given_up = |key: &str| {
        config
            .keys
            .unbind
            .keys()
            .any(|u| WrittenKey::parse(u) == WrittenKey::parse(key))
    };
    let behind_prefix =
        |key: &str| WrittenKey::parse(key).layer() == LEADER_LAYER && !given_up(key);
    flat.get(action)
        .is_some_and(|keys| keys.keys().iter().any(|k| behind_prefix(k)))
        || shipped
            .bind
            .iter()
            .chain(&config.keys.bind)
            .filter(|b| b.action == action)
            .any(|b| b.keys.keys().iter().any(|k| behind_prefix(k)))
}

/// Check that the floor `mode` still has a way out, and put `Escape` back (and say so) if not. A
/// no-op for every mode that is not a floor.
pub(super) fn assert_way_out(
    map: &mut KeymapRegistry,
    mode: &str,
    config: &Config,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) {
    let Some(way_out) = way_out_of(mode) else {
        return;
    };
    let in_the_floor = map
        .bindings_in_mode(mode)
        .is_some_and(|bound| bound.values().any(|a| *a == way_out.action));
    if in_the_floor || bound_behind_the_prefix(config, way_out.name) {
        return;
    }
    conflicts.way_out(WayOutRefused {
        surface: way_out.panel.to_string(),
        action: way_out.name.to_string(),
        restored: RESTORED_KEY.to_string(),
    });
    bind_with_conflict_tracking(
        map,
        mode,
        KeyCombo::parse(RESTORED_KEY),
        way_out.action,
        Written {
            action: way_out.name,
            layer: "built-in (every panel has a way out)",
            key: RESTORED_KEY,
        },
        conflicts,
        index,
    );
}

#[cfg(test)]
mod tests {

    use crate::app::conflicts::Conflicts;
    use crate::app::input::{FOCUS_LAYER, LAYER_FLOOR};
    use crate::app::registry::build_modes;
    use crate::input::WmAction;
    use crate::keymap::{BindingIndex, KeyCombo, KeymapRegistry};
    use heca_config::keys::BindingValue;
    use heca_config::theme::{Config, KeyModeConfig, ModeBindingConfig};

    use std::collections::HashMap;

    fn built(config: &Config) -> (HashMap<String, KeymapRegistry>, Conflicts) {
        let mut conflicts = Conflicts::default();
        let (modes, _) = build_modes(config, &mut conflicts, &mut BindingIndex::new());
        (modes, conflicts)
    }

    fn resolves(
        modes: &HashMap<String, KeymapRegistry>,
        floor: &str,
        key: &str,
    ) -> Option<WmAction> {
        modes[floor]
            .resolve_builtin(floor, &KeyCombo::parse(key))
            .cloned()
    }

    /// One block for one floor: the keys it gives up and the ones it binds the way out to.
    fn floor_block(floor: &str, unbind: &[&str], action: &str, keys: &[&str]) -> KeyModeConfig {
        KeyModeConfig {
            name: floor.to_string(),
            trigger: String::new(),
            sticky: true,
            bindings: vec![ModeBindingConfig {
                action: action.to_string(),
                keys: BindingValue::Many(keys.iter().map(|k| k.to_string()).collect()),
                args: HashMap::new(),
            }],
            unbind: unbind.iter().map(|k| k.to_string()).collect(),
        }
    }

    /// **As shipped, every panel is left with `Escape`** — a dock hands the keyboard back, a layer
    /// closes — and nothing about it is special-cased in code: it is two ordinary bindings in the
    /// shipped file.
    #[test]
    fn as_shipped_escape_leaves_a_dock_and_closes_a_layer() {
        let (modes, conflicts) = built(&Config::default());

        assert_eq!(
            resolves(&modes, FOCUS_LAYER, "Escape"),
            Some(WmAction::UnfocusDock)
        );
        assert_eq!(
            resolves(&modes, LAYER_FLOOR, "Escape"),
            Some(WmAction::CloseOverlay { overlay: None })
        );
        assert!(conflicts.way_outs.is_empty(), "{:?}", conflicts.way_outs);
    }

    /// **The way out can be moved**: bind the action to another key and give `Escape` up in the same
    /// block. Escape then belongs to nobody in the floor, so it reaches what is typed into.
    #[test]
    fn the_way_out_of_a_dock_can_move_to_another_key() {
        let mut config = Config::default();
        config.keys.mode = vec![floor_block(
            FOCUS_LAYER,
            &["Escape"],
            "unfocus_dock",
            &["Ctrl+g"],
        )];
        let (modes, conflicts) = built(&config);

        assert_eq!(resolves(&modes, FOCUS_LAYER, "Escape"), None, "given up");
        assert_eq!(
            resolves(&modes, FOCUS_LAYER, "Ctrl+g"),
            Some(WmAction::UnfocusDock),
            "moved"
        );
        assert!(conflicts.way_outs.is_empty(), "{:?}", conflicts.way_outs);
    }

    /// **A floor is never left with no key.** Giving `Escape` up and binding nothing in its place is
    /// refused for that floor: `Escape` is back and the report says so.
    #[test]
    fn giving_up_escape_with_nothing_in_its_place_is_refused_and_reported() {
        for (floor, action) in [
            (FOCUS_LAYER, "unfocus_dock"),
            (LAYER_FLOOR, "close_overlay"),
        ] {
            let mut config = Config::default();
            config.keys.mode = vec![KeyModeConfig {
                bindings: vec![],
                ..floor_block(floor, &["Escape"], action, &[])
            }];
            // The layer floor also ships `q` and `Ctrl+q`; take those too.
            if floor == LAYER_FLOOR {
                config.keys.mode[0]
                    .unbind
                    .extend(["q".into(), "Ctrl+q".into()]);
            }
            let (modes, conflicts) = built(&config);

            assert!(
                resolves(&modes, floor, "Escape").is_some(),
                "{floor}: Escape is put back"
            );
            assert_eq!(conflicts.way_outs.len(), 1, "{floor}: and it is not silent");
            assert_eq!(conflicts.way_outs[0].action, action);
        }
    }

    /// Behind the prefix is a way out too: it works from every panel. So `Escape` can be given up
    /// when `unfocus_dock` lives at `prefix+Escape`.
    #[test]
    fn a_way_out_behind_the_prefix_counts() {
        let mut config = Config::default();
        config.keys.bindings.insert(
            "unfocus_dock".into(),
            BindingValue::Single("prefix+Escape".into()),
        );
        config.keys.mode = vec![KeyModeConfig {
            bindings: vec![],
            ..floor_block(FOCUS_LAYER, &["Escape"], "unfocus_dock", &[])
        }];
        let (modes, conflicts) = built(&config);

        assert_eq!(resolves(&modes, FOCUS_LAYER, "Escape"), None, "given up");
        assert!(conflicts.way_outs.is_empty(), "{:?}", conflicts.way_outs);
    }

    /// Taking `Escape` for something else, without moving the way out anywhere, leaves the floor
    /// with no key for it: the guarantee wins and the user is told.
    #[test]
    fn taking_escape_for_something_else_is_refused_and_reported() {
        let mut config = Config::default();
        config.keys.mode = vec![floor_block(
            FOCUS_LAYER,
            &["Escape"],
            "scroll_to_top",
            &["Escape"],
        )];
        let (modes, conflicts) = built(&config);

        assert_eq!(
            resolves(&modes, FOCUS_LAYER, "Escape"),
            Some(WmAction::UnfocusDock),
            "the guarantee wins"
        );
        assert_eq!(conflicts.way_outs.len(), 1);
    }

    /// **`Escape` must not be a global binding** (F003/P082/T428). The global map is the fallback
    /// for what no surface in front claimed, so an `Escape` in it outranks all three floors at once
    /// — which is how `close_overlay` came to eat the key while a dock held the keyboard, closing
    /// nothing because no overlay was up.
    ///
    /// **Nor is `close_overlay`**, for exactly the same reason — see below.
    #[test]
    fn escape_is_never_a_global_binding() {
        let keymaps = crate::app::registry::build_keymaps(
            &heca_config::theme::Config::default(),
            &mut Conflicts::default(),
        );

        assert_eq!(
            keymaps
                .flat
                .resolve(crate::keymap::DIRECT_LAYER, &KeyCombo::parse("Escape")),
            None,
            "Escape belongs to the surface that holds the keyboard, never to the whole app",
        );
        // **And neither is a way out of an overlay.** `close_overlay` shipped as a global `q` +
        // `Ctrl+q`, so both were taken from the program in the pane whether or not an overlay was
        // up: `:q` in vim stopped at the colon, in every terminal. They live in the `layer` floor
        // now, which is consulted only while a layer holds the keyboard — and `Ctrl+q` is
        // `quoted-insert` in emacs and readline, so shadowing it globally was wrong twice over.
        for key in ["q", "Ctrl+q"] {
            assert_eq!(
                keymaps
                    .flat
                    .resolve_builtin(crate::keymap::DIRECT_LAYER, &KeyCombo::parse(key)),
                None,
                "{key} belongs to the program in the pane, not to the whole app",
            );
            assert_eq!(
                keymaps
                    .modes
                    .get(LAYER_FLOOR)
                    .and_then(|m| m.resolve_builtin(LAYER_FLOOR, &KeyCombo::parse(key))),
                Some(&WmAction::CloseOverlay { overlay: None }),
                "{key} closes the front-most overlay while one holds the keyboard",
            );
        }
    }
}
