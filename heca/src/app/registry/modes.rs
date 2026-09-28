//! **The mode keymaps** — `[[keys.mode]]`, their triggers, and the modes entered by focus rather
//! than by a key. Owns nothing about what a floor guarantees; it only asks [`super::floors`] to
//! assert one.

use super::binding::{Written, bind_with_conflict_tracking};
use super::floors::assert_escape_floor;
use super::names::action_ref_from_config;
use crate::app::conflicts::Conflicts;
use crate::keymap::{BindingIndex, KeyCombo, KeymapRegistry};
use std::collections::{BTreeMap, HashMap};

/// The built-in mode keymaps that are **entered by focus rather than by a key**, so they never take
/// a trigger — see the note at the trigger site in [`build_modes`].
pub(super) const UNTRIGGERED_MODES: &[&str] = &[
    "sidebar",
    crate::app::input::FOCUS_LAYER,
    crate::app::input::LAYER_FLOOR,
];

/// Build mode keymaps and triggers from config.
pub fn build_modes(
    config: &heca_config::theme::Config,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) -> (
    HashMap<String, KeymapRegistry>,
    HashMap<String, (KeyCombo, bool)>,
) {
    let mut mode_keymaps = HashMap::new();
    let mut mode_triggers: HashMap<String, (KeyCombo, bool)> = HashMap::new();

    let default_keys = heca_config::theme::KeysConfig::default();
    let mut merged_modes: BTreeMap<String, heca_config::theme::KeyModeConfig> = default_keys
        .mode
        .iter()
        .map(|mode| (mode.name.clone(), mode.clone()))
        .collect();

    for user_mode in &config.keys.mode {
        if let Some(existing) = merged_modes.get_mut(&user_mode.name) {
            existing.trigger = user_mode.trigger.clone();
            existing.sticky = user_mode.sticky;
            existing.bindings.extend(user_mode.bindings.clone());
        } else {
            merged_modes.insert(user_mode.name.clone(), user_mode.clone());
        }
    }

    for mode_cfg in merged_modes.values() {
        let mut mode_map = KeymapRegistry::new();
        let label = format!("[[keys.mode]] {}", mode_cfg.name);
        for binding in &mode_cfg.bindings {
            // Same rule as the flat bindings: an unresolvable name is a Dynamic ref, not a dropped
            // binding. A mode binding's `args` become the Intent's args.
            let action = action_ref_from_config(&binding.action, &binding.args);
            // **One action, however many keys reach it** — `keys = ["q", "Ctrl+q"]`, a
            // comma-separated string, or a single combo. The same `BindingValue` every other
            // binding table takes, so a mode block does not need a repeated entry per key.
            for key_str in binding.keys.keys() {
                bind_with_conflict_tracking(
                    &mut mode_map,
                    &mode_cfg.name,
                    KeyCombo::parse(key_str),
                    action.clone(),
                    Written {
                        action: &binding.action,
                        layer: &label,
                        key: key_str,
                    },
                    conflicts,
                    index,
                );
            }
        }
        assert_escape_floor(&mut mode_map, &mode_cfg.name, conflicts, index);
        mode_keymaps.insert(mode_cfg.name.clone(), mode_map);
        // These two are **entered by focus, not by a key**: `sidebar` by `sidebar_focus` or a
        // click, `focus` by focusing a dock. A trigger would make them reachable as a plain
        // `InputMode::Mode`, where their bindings resolve against a focus nothing is holding — a
        // mode that looks entered and does nothing. So a trigger is refused even when a user
        // config supplies one.
        if !UNTRIGGERED_MODES.contains(&mode_cfg.name.as_str()) {
            let trigger_trimmed = mode_cfg.trigger.trim();
            let trigger_combo = if trigger_trimmed.starts_with("prefix+") {
                let rest = trigger_trimmed.strip_prefix("prefix+").unwrap().trim();
                KeyCombo::parse(rest)
            } else {
                KeyCombo::parse(trigger_trimmed)
            };
            mode_triggers.insert(mode_cfg.name.clone(), (trigger_combo, mode_cfg.sticky));
        }
    }

    // The layer floor is not a `[[keys.mode]]` block anybody writes — a layer is entered by being
    // shown, not by pressing something — so it is created here when no config declared it. A user
    // who *does* write one gets extra keys for the front-most layer, with the floor re-asserted over
    // them by the loop above.
    if !mode_keymaps.contains_key(crate::app::input::LAYER_FLOOR) {
        let mut floor = KeymapRegistry::new();
        assert_escape_floor(&mut floor, crate::app::input::LAYER_FLOOR, conflicts, index);
        mode_keymaps.insert(crate::app::input::LAYER_FLOOR.to_string(), floor);
    }

    (mode_keymaps, mode_triggers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::conflicts::Conflicts;
    use crate::input::WmAction;
    use crate::keymap::{BindingIndex, KeyCombo};
    use heca_config::keys::BindingValue;
    use heca_config::theme::{KeyModeConfig, ModeBindingConfig};
    use std::collections::HashMap;

    #[test]
    fn default_font_size_modes_build_with_triggers_and_keys() {
        use crate::input::FontZoomStep;
        let config = heca_config::theme::Config::default();
        let (mode_keymaps, mode_triggers) =
            build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        // Both modes exist, are sticky, and are entered by prefix+! / prefix+@.
        let (app_trigger, app_sticky) = mode_triggers
            .get("app_font_size")
            .expect("app_font_size mode trigger");
        assert!(app_sticky, "app_font_size must be sticky");
        assert_eq!(*app_trigger, KeyCombo::parse("!"));
        let (pane_trigger, pane_sticky) = mode_triggers
            .get("pane_font_size")
            .expect("pane_font_size mode trigger");
        assert!(pane_sticky, "pane_font_size must be sticky");
        assert_eq!(*pane_trigger, KeyCombo::parse("@"));

        // Inner keys resolve to the right actions (k/ArrowUp = bigger, j = smaller,
        // 0 = reset) — whole-app for app_font_size, focused pane for pane_font_size.
        let app = mode_keymaps.get("app_font_size").unwrap();
        assert_eq!(
            app.resolve_builtin("app_font_size", &KeyCombo::parse("k")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            app.resolve_builtin("app_font_size", &KeyCombo::parse("ArrowUp")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            app.resolve_builtin("app_font_size", &KeyCombo::parse("j")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::Out
            })
        );
        assert_eq!(
            app.resolve_builtin("app_font_size", &KeyCombo::parse("0")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::Reset
            })
        );

        let pane = mode_keymaps.get("pane_font_size").unwrap();
        assert_eq!(
            pane.resolve_builtin("pane_font_size", &KeyCombo::parse("k")),
            Some(&WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            pane.resolve_builtin("pane_font_size", &KeyCombo::parse("0")),
            Some(&WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::Reset
            })
        );
    }

    #[test]
    fn default_selection_mode_bindings_resolve() {
        let config = heca_config::theme::Config::default();
        let (mode_keymaps, _) =
            build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let keymap = mode_keymaps
            .get("selection")
            .expect("selection mode exists");

        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("h")),
            Some(&WmAction::SelectionLeft)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("ArrowLeft")),
            Some(&WmAction::SelectionLeft)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("l")),
            Some(&WmAction::SelectionRight)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("ArrowUp")),
            Some(&WmAction::SelectionUp)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("ArrowDown")),
            Some(&WmAction::SelectionDown)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("y")),
            Some(&WmAction::CopySelection)
        );
        // BeginSelection: direct keys in selection mode.
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("v")),
            Some(&WmAction::BeginSelection)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("Space")),
            Some(&WmAction::BeginSelection)
        );
        // ToggleSelectionEndpoint.
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("o")),
            Some(&WmAction::ToggleSelectionEndpoint)
        );
    }

    /// **One action, however many keys reach it — in every binding table.**
    ///
    /// `keys` takes one combo, several comma-separated, or a list, and the three are the same
    /// binding. It was a bare `String` in `[[keys.mode.bindings]]` alone, so a mode block needed a
    /// repeated entry per key while `[keys]` beside it took a list — the same capability spelled
    /// two ways in two tables (Antonio, 2026-08-21: *"why not `keys = ["q", "Ctrl+q"]`?"*).
    #[test]
    fn a_binding_takes_one_key_a_list_or_a_comma_separated_string() {
        let mode_with = |keys: BindingValue| {
            let mut config = heca_config::theme::Config::default();
            config.keys.mode.push(heca_config::keys::KeyModeConfig {
                name: "spelling".to_string(),
                trigger: "".to_string(),
                sticky: true,
                bindings: vec![heca_config::keys::ModeBindingConfig {
                    action: "close_overlay".to_string(),
                    keys,
                    args: Default::default(),
                }],
            });
            let (modes, _) =
                build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
            let map = modes.get("spelling").expect("the mode");
            ["q", "Ctrl+q"].map(|k| {
                map.resolve_builtin("spelling", &KeyCombo::parse(k))
                    == Some(&WmAction::CloseOverlay { overlay: None })
            })
        };

        assert_eq!(
            mode_with(BindingValue::Many(vec!["q".into(), "Ctrl+q".into()])),
            [true, true],
            "a list binds every key in it",
        );
        assert_eq!(
            mode_with(BindingValue::Single("q, Ctrl+q".into())),
            [true, true],
            "…and so does one comma-separated string",
        );
        assert_eq!(
            mode_with(BindingValue::Single("q".into())),
            [true, false],
            "…while a single combo binds exactly itself",
        );
    }

    /// The focus layer ships the keys the **widgets** answer, and is never enterable by a trigger
    /// — it is entered by focusing a dock (F003/P085/T352).
    #[test]
    fn the_focus_layer_ships_the_scroll_keys_and_the_way_out() {
        let config = heca_config::theme::Config::default();
        let (mode_keymaps, mode_triggers) =
            build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let focus = mode_keymaps
            .get(crate::app::input::FOCUS_LAYER)
            .expect("the focus layer is a built-in mode keymap");

        for (key, want) in [
            ("PageUp", WmAction::ScrollPageUp),
            ("PageDown", WmAction::ScrollPageDown),
            ("Home", WmAction::ScrollToTop),
            ("End", WmAction::ScrollToBottom),
            ("Alt+PageUp", WmAction::ScrollPageLeft),
            ("Alt+PageDown", WmAction::ScrollPageRight),
            ("Alt+Home", WmAction::ScrollToLeftEdge),
            ("Alt+End", WmAction::ScrollToRightEdge),
            ("Escape", WmAction::UnfocusDock),
        ] {
            assert_eq!(
                focus.resolve_builtin(crate::app::input::FOCUS_LAYER, &KeyCombo::parse(key)),
                Some(&want),
                "{key} must reach {want:?} while a dock holds the keyboard",
            );
        }
        assert!(
            !mode_triggers.contains_key(crate::app::input::FOCUS_LAYER),
            "focus is entered by focusing a dock, never by a key",
        );
    }

    /// …and a user trigger on it is refused, exactly as it is for `sidebar`: the mode's bindings
    /// only mean anything while a dock is focused, so an enterable copy would be a dead mode.
    #[test]
    fn a_user_trigger_on_the_focus_layer_is_refused() {
        let mut config = heca_config::theme::Config::default();
        config.keys.mode.push(KeyModeConfig {
            name: crate::app::input::FOCUS_LAYER.to_string(),
            trigger: "prefix+Shift+f".to_string(),
            sticky: true,
            bindings: Vec::new(),
        });
        let (_, mode_triggers) =
            build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        assert!(!mode_triggers.contains_key(crate::app::input::FOCUS_LAYER));
    }

    #[test]
    fn user_mode_with_same_name_merges_with_defaults() {
        let mut config = heca_config::theme::Config::default();
        config.keys.mode.push(KeyModeConfig {
            name: "resize".to_string(),
            trigger: "prefix+r".to_string(),
            sticky: true,
            bindings: vec![ModeBindingConfig {
                action: "resize_increase".to_string(),
                keys: BindingValue::Single("x".to_string()),
                args: HashMap::new(),
            }],
        });

        let (mode_keymaps, mode_triggers) =
            build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let resize = mode_keymaps.get("resize").expect("resize mode exists");

        assert!(
            resize
                .resolve_builtin("resize", &KeyCombo::parse("h"))
                .is_some()
        );
        assert_eq!(
            resize.resolve_builtin("resize", &KeyCombo::parse("x")),
            Some(&WmAction::ResizeIncrease)
        );
        assert_eq!(
            mode_triggers.get("resize"),
            Some(&(KeyCombo::parse("r"), true))
        );
    }
}
