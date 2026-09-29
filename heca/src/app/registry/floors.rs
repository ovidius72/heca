//! **The keys that are always there** — `Escape` acts on the surface in front of you, and no
//! config can remove it. Owns nothing about config tables or handlers.

use super::binding::{Written, bind_with_conflict_tracking};
use crate::app::conflicts::Conflicts;
use crate::input::WmAction;
use crate::keymap::{ActionRef, BindingIndex, KeyCombo, KeymapRegistry};

/// **`Escape` is a guarantee, not a default** (F003/P086/T363; generalized by F003/P082/T428).
///
/// Escape acts on the surface in front of you and gives the keyboard back to what was under it —
/// the way it does in every other application. What that means depends on the kind of surface, and
/// neither spelling is a line a config can drop:
///
/// | floor | what `Escape` means there |
/// |---|---|
/// | [`FOCUS_LAYER`](crate::app::input::FOCUS_LAYER) | a focused dock hands the keyboard back to the panes |
/// | [`LAYER_FLOOR`](crate::app::input::LAYER_FLOOR) | the front-most layer closes itself |
///
/// (The panes have no floor on purpose: a key nothing claims belongs to the program running in
/// them, so `Escape` still means what it means inside vim.)
///
/// It is re-asserted **after** the merge rather than left to the file because `[[keys.mode]]` arrays
/// are replaced wholesale by a user's config, so a block that simply forgot the line would strand
/// the keyboard with only the mouse to get out. Bound through the same door as everything else:
/// putting something *else* on `Escape` in one of these layers is a real collision and comes out in
/// the report rather than silently losing. Binding the same action to further keys is untouched —
/// this adds a floor, not a ceiling.
pub(super) fn assert_escape_floor(
    map: &mut KeymapRegistry,
    mode: &str,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) {
    let (action, name) = match mode {
        crate::app::input::FOCUS_LAYER => {
            (ActionRef::Builtin(WmAction::UnfocusDock), "unfocus_dock")
        }
        crate::app::input::LAYER_FLOOR => (
            ActionRef::Builtin(WmAction::CloseOverlay { overlay: None }),
            "close_overlay",
        ),
        _ => return,
    };
    bind_with_conflict_tracking(
        map,
        mode,
        KeyCombo::parse("Escape"),
        action,
        Written {
            action: name,
            layer: "built-in (Escape acts on the focused surface)",
            key: "Escape",
        },
        conflicts,
        index,
    );
}

#[cfg(test)]
mod tests {

    use crate::app::conflicts::{Conflicts, format_combo};
    use crate::app::registry::build_modes;
    use crate::input::WmAction;
    use crate::keymap::{BindingIndex, KeyCombo};
    use heca_config::keys::BindingValue;

    use std::collections::HashMap;

    /// **`Esc` is a floor, not a default.** `[[keys.mode]]` arrays are replaced wholesale by a
    /// user's config, so a `focus` block that forgot this line would strand the keyboard in a dock
    /// with only the mouse to get out. It is re-asserted after the merge.
    #[test]
    fn escape_always_releases_a_focused_container() {
        let mut config = heca_config::theme::Config::default();
        // A user's `focus` layer that keeps the paging keys and drops the way out.
        config.keys.mode = vec![heca_config::theme::KeyModeConfig {
            name: crate::app::input::FOCUS_LAYER.to_string(),
            trigger: String::new(),
            sticky: true,
            bindings: vec![heca_config::theme::ModeBindingConfig {
                action: "scroll_page_up".to_string(),
                keys: BindingValue::Single("PageUp".to_string()),
                args: HashMap::new(),
            }],
        }];
        let (modes, _) = build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let focus = &modes[crate::app::input::FOCUS_LAYER];

        assert_eq!(
            focus.resolve_builtin(crate::app::input::FOCUS_LAYER, &KeyCombo::parse("Escape")),
            Some(&WmAction::UnfocusDock),
            "a container can always be left, whatever the config says",
        );
    }

    /// Putting something *else* on `Escape` in the focus layer is a real collision — the guarantee
    /// wins, and the user is told rather than left wondering.
    #[test]
    fn taking_escape_in_the_focus_layer_is_reported() {
        let mut config = heca_config::theme::Config::default();
        config.keys.mode = vec![heca_config::theme::KeyModeConfig {
            name: crate::app::input::FOCUS_LAYER.to_string(),
            trigger: String::new(),
            sticky: true,
            bindings: vec![heca_config::theme::ModeBindingConfig {
                action: "scroll_to_top".to_string(),
                keys: BindingValue::Single("Escape".to_string()),
                args: HashMap::new(),
            }],
        }];
        let mut conflicts = Conflicts::default();
        let (modes, _) = build_modes(&config, &mut conflicts, &mut BindingIndex::new());

        assert_eq!(
            modes[crate::app::input::FOCUS_LAYER]
                .resolve_builtin(crate::app::input::FOCUS_LAYER, &KeyCombo::parse("Escape")),
            Some(&WmAction::UnfocusDock),
            "the guarantee wins",
        );
        // Two lines, both true: the user's `Escape` displaced the shipped default, and the
        // guarantee then displaced the user's. A key this consequential deserves the noise.
        assert!(
            conflicts
                .keys
                .iter()
                .any(|c| format_combo(&c.combo).eq_ignore_ascii_case("escape")),
            "and it is not silent: {:?}",
            conflicts.keys,
        );
    }

    /// **The layer twin of the dock's floor** (F003/P082/T428). A layer that declares nothing —
    /// which is every layer today, and every layer a plugin will ship before it thinks about keys —
    /// still closes on `Escape`. Nothing in any config declares this mode, so it is built from
    /// nothing, exactly as a plugin's layer will find it.
    #[test]
    fn escape_always_closes_the_front_most_layer() {
        let config = heca_config::theme::Config::default();
        let (modes, triggers) =
            build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        assert_eq!(
            modes[crate::app::input::LAYER_FLOOR]
                .resolve_builtin(crate::app::input::LAYER_FLOOR, &KeyCombo::parse("Escape")),
            Some(&WmAction::CloseOverlay { overlay: None }),
            "a layer can always be closed, whatever it declared",
        );
        assert!(
            !triggers.contains_key(crate::app::input::LAYER_FLOOR),
            "a layer is entered by being shown, never by a key",
        );
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
        // up: `:q` in vim stopped at the colon, in every terminal (Antonio, driving, 2026-08-20).
        // They live in the `layer` floor now, which is consulted only while a layer holds the
        // keyboard — and `Ctrl+q` is `quoted-insert` in emacs and readline, so shadowing it
        // globally was wrong twice over.
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
                    .get(crate::app::input::LAYER_FLOOR)
                    .and_then(|m| m
                        .resolve_builtin(crate::app::input::LAYER_FLOOR, &KeyCombo::parse(key))),
                Some(&WmAction::CloseOverlay { overlay: None }),
                "{key} closes the front-most overlay while one holds the keyboard",
            );
        }
    }
}
