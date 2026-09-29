//! **The widget keymap** — `[keys.widgets]` → the `heca_grid_ui::Keymap` every interactive widget
//! consults, and the conversion from an app key chord to a grid-ui one.

use crate::keymap::KeyCombo;

/// Convert a parsed [`KeyCombo`] into a renderer-agnostic `heca_grid_ui` chord
/// (`GridKey` + `Modifiers`), or `None` for a key name grid-ui does not model. Handles both
/// key-name sources: config strings parsed by `KeyCombo::parse` (lowercase, e.g. `"arrowdown"`)
/// **and** live events from `build_event_combo`/`normalize_key_text`, which name a `NamedKey`
/// via `{:?}` (capitalised, e.g. `"ArrowDown"`, `"Tab"`, `"Enter"`). The name is matched
/// **case-insensitively** so both resolve; modifiers map straight across (`super_` → `meta`).
pub(crate) fn combo_to_grid(
    combo: &KeyCombo,
) -> Option<(heca_grid_ui::GridKey, heca_grid_ui::Modifiers)> {
    // The **name** table is `GridKey::from_name`'s, not this function's: a config file, a platform
    // event and an RPC string all name a key, and each surface used to keep its own list of which
    // key a name meant. What is left here is the part that really is the app's — its normalised
    // chord, which carries the macOS physical-key fallback for `Ctrl+letter`.
    let key = heca_grid_ui::GridKey::from_name(&combo.key)?;
    let mods = heca_grid_ui::Modifiers {
        ctrl: combo.ctrl,
        alt: combo.alt,
        shift: combo.shift,
        meta: combo.super_,
    };
    Some((key, mods))
}

/// Build the **widget keymap** (`widget-keys-config`): the `[keys.widgets]` bindings resolved
/// into a `heca_grid_ui::Keymap` (`key chord → WidgetIntent`), the single host-owned map every
/// interactive widget/overlay consults. User bindings win when set (arrays replace wholesale),
/// else the bundled default. These names are **not** `WmAction`s, so `build_keymap` skips them —
/// they live only here and never hijack normal-mode input. `edit_*` entries are bound **first**
/// so a focused `Input` wins the `Ctrl+h` overload (field-first). See `docs/widgets.md` + README.
pub fn build_widget_keymap(config: &heca_config::theme::Config) -> heca_grid_ui::Keymap {
    use heca_grid_ui::WidgetIntent;
    let defaults = heca_config::theme::KeysConfig::default();
    // Edit shortcuts bound before the nav overload (so `Ctrl+h` resolves EditDeleteBack first).
    let entries = [
        ("edit_delete_back", WidgetIntent::EditDeleteBack),
        (
            "edit_delete_to_line_start",
            WidgetIntent::EditDeleteToLineStart,
        ),
        ("edit_select_all", WidgetIntent::EditSelectAll),
        ("item_previous", WidgetIntent::ItemPrevious),
        ("item_next", WidgetIntent::ItemNext),
        ("menu_up", WidgetIntent::MenuUp),
        ("menu_down", WidgetIntent::MenuDown),
        ("menu_history_up", WidgetIntent::MenuHistoryUp),
        ("menu_history_down", WidgetIntent::MenuHistoryDown),
        ("activate", WidgetIntent::Activate),
        ("dismiss", WidgetIntent::Dismiss),
    ];
    let mut km = heca_grid_ui::Keymap::new();
    for (name, intent) in entries {
        let value = config
            .keys
            .widgets
            .get(name)
            .or_else(|| defaults.widgets.get(name));
        if let Some(value) = value {
            for key_str in value.keys() {
                if let Some((key, mods)) = combo_to_grid(&KeyCombo::parse(key_str.trim())) {
                    km.bind(key, mods, intent);
                }
            }
        }
    }
    km
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_keymap_maps_default_bindings_by_axis() {
        use heca_grid_ui::{GridKey, Modifiers, WidgetIntent};
        let km = build_widget_keymap(&heca_config::theme::Config::default());
        let ctrl = Modifiers {
            ctrl: true,
            ..Default::default()
        };
        let meta = Modifiers {
            meta: true,
            ..Default::default()
        };
        let none = Modifiers::default();
        // Horizontal item_* (←/→, Ctrl+h/l).
        assert_eq!(
            km.resolve(GridKey::ArrowRight, none),
            &[WidgetIntent::ItemNext]
        );
        assert_eq!(
            km.resolve(GridKey::Char('l'), ctrl),
            &[WidgetIntent::ItemNext]
        );
        assert_eq!(
            km.resolve(GridKey::ArrowLeft, none),
            &[WidgetIntent::ItemPrevious]
        );
        // Vertical menu_* (↑/↓, Ctrl+k/j) — what an open context menu / palette / Select
        // navigates by (see `ContextMenu::event`, which acts on these intents).
        assert_eq!(
            km.resolve(GridKey::ArrowDown, none),
            &[WidgetIntent::MenuDown]
        );
        assert_eq!(km.resolve(GridKey::ArrowUp, none), &[WidgetIntent::MenuUp]);
        assert_eq!(
            km.resolve(GridKey::Char('j'), ctrl),
            &[WidgetIntent::MenuDown]
        );
        assert_eq!(
            km.resolve(GridKey::Char('k'), ctrl),
            &[WidgetIntent::MenuUp]
        );
        // Shared + edits.
        assert_eq!(km.resolve(GridKey::Enter, none), &[WidgetIntent::Activate]);
        assert_eq!(km.resolve(GridKey::Escape, none), &[WidgetIntent::Dismiss]);
        assert_eq!(
            km.resolve(GridKey::Char('u'), ctrl),
            &[WidgetIntent::EditDeleteToLineStart]
        );
        assert_eq!(
            km.resolve(GridKey::Char('a'), ctrl),
            &[WidgetIntent::EditSelectAll]
        );
        assert_eq!(
            km.resolve(GridKey::Char('a'), meta),
            &[WidgetIntent::EditSelectAll]
        );
        // The Ctrl+h overload: edit-first (field-first), then the horizontal nav.
        assert_eq!(
            km.resolve(GridKey::Char('h'), ctrl),
            &[WidgetIntent::EditDeleteBack, WidgetIntent::ItemPrevious],
        );
    }

    #[test]
    fn widget_key_names_are_not_wm_actions() {
        // The [keys.widgets] names live only in the widget keymap — NOT WmActions, so
        // `build_keymap` skips them and they never bind into normal/global.
        for name in [
            "item_previous",
            "item_next",
            "menu_up",
            "menu_down",
            "activate",
            "dismiss",
            "edit_delete_back",
            "edit_delete_to_line_start",
            "edit_select_all",
        ] {
            assert!(
                crate::input::resolve_action(name, &std::collections::HashMap::new()).is_none(),
                "{name} must not be a WmAction (widget-keys-config is a separate map)",
            );
        }
    }
}
