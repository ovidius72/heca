//! Tests for [`super`].

use super::*;

use crate::app::conflicts::{Conflicts, format_combo};
use crate::input::WmAction;
use crate::keymap::{ActionRef, BindingIndex, KeyCombo};

/// **A user's binding list adds to the defaults; it does not replace them.**
///
/// TOML replaces an array wholesale, so writing one `[[keys.command]]` used to delete every
/// default one — silently, with nothing to fail. `[[keys.bind]]` and `[[keys.surface.bind]]`
/// each carried a hand-written merge to dodge that; `[[keys.command]]` carried none, and the
/// only reason nobody lost a binding is that the shipped defaults have none uncommented yet.
///
/// Asserted against a **synthetic** default rather than the shipped file, so the guard says
/// something the day a default is added rather than passing on an empty list.
#[test]
fn a_users_command_binding_does_not_delete_the_default_ones() {
    use heca_config::theme::CommandKeybindConfig;
    let shipped = |key: &str, command: &str| CommandKeybindConfig {
        key: key.to_string(),
        command: command.to_string(),
        kind: "terminal".to_string(),
        float: false,
        close_pane: false,
        keep_on_error: false,
        keep_on_success: false,
    };

    let mut defaults = vec![shipped("prefix+g", "lazygit"), shipped("prefix+t", "btm")];
    let user = vec![shipped("prefix+t", "htop"), shipped("Alt+l", "lazydocker")];
    super::merge_by(&mut defaults, &user, |c| c.key.trim().to_string());

    let by_key = |key: &str| {
        defaults
            .iter()
            .find(|c| c.key == key)
            .map(|c| c.command.as_str())
    };
    assert_eq!(
        by_key("prefix+g"),
        Some("lazygit"),
        "an untouched default survives"
    );
    assert_eq!(
        by_key("prefix+t"),
        Some("htop"),
        "the same combo is overridden"
    );
    assert_eq!(by_key("Alt+l"), Some("lazydocker"), "a new combo is added");
    assert_eq!(defaults.len(), 3);
}

/// NON-REGRESSION: every binding in the DEFAULT keymap still resolves to a `Builtin` at load.
/// `build_keymap` no longer skips unresolvable names (they become `Dynamic`), so a typo — or a
/// rename — in `keybindings.default.toml` would silently degrade a real binding into a dynamic
/// one that no-ops at press. This test is what stops that.
#[test]
fn every_default_binding_still_resolves_to_a_builtin_at_load() {
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    for mode in [crate::keymap::LEADER_LAYER, crate::keymap::DIRECT_LAYER] {
        let Some(bindings) = keymap.bindings_in_mode(mode) else {
            continue;
        };
        for (combo, action) in bindings {
            assert!(
                matches!(action, ActionRef::Builtin(_)),
                "default binding {mode}:{} degraded to Dynamic — the action name in \
                 keybindings.default.toml does not resolve",
                format_combo(combo),
            );
        }
    }
}

#[test]
fn default_ctrl_k_binding_stays_swap_up() {
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Ctrl+k")),
        Some(&WmAction::SwapUp)
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Ctrl+j")),
        Some(&WmAction::SwapDown)
    );
    assert_eq!(
        keymap.resolve_builtin(
            crate::keymap::LEADER_LAYER,
            &KeyCombo::parse("Ctrl+Shift+k")
        ),
        Some(&WmAction::MoveColumnUp)
    );
    assert_eq!(
        keymap.resolve_builtin(
            crate::keymap::LEADER_LAYER,
            &KeyCombo::parse("Ctrl+Shift+j")
        ),
        Some(&WmAction::MoveColumnDown)
    );
}

#[test]
fn default_font_zoom_bindings_resolve_without_collision() {
    use crate::input::FontZoomStep;
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

    // Global (app-wide) branch: prefix+Ctrl+= / - / 0.
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Ctrl+=")),
        Some(&WmAction::AppFontZoom {
            step: FontZoomStep::In
        })
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Ctrl+-")),
        Some(&WmAction::AppFontZoom {
            step: FontZoomStep::Out
        })
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Ctrl+0")),
        Some(&WmAction::AppFontZoom {
            step: FontZoomStep::Reset
        })
    );

    // Focused-pane branch: prefix+Ctrl+Shift+= / - / 0 (pane_id resolved at
    // dispatch). Ctrl+Shift is used instead of Alt because macOS rewrites the
    // character under the Option key, so Alt+= would never match.
    assert_eq!(
        keymap.resolve_builtin(
            crate::keymap::LEADER_LAYER,
            &KeyCombo::parse("Ctrl+Shift+=")
        ),
        Some(&WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: FontZoomStep::In
        })
    );
    assert_eq!(
        keymap.resolve_builtin(
            crate::keymap::LEADER_LAYER,
            &KeyCombo::parse("Ctrl+Shift+-")
        ),
        Some(&WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: FontZoomStep::Out
        })
    );
    assert_eq!(
        keymap.resolve_builtin(
            crate::keymap::LEADER_LAYER,
            &KeyCombo::parse("Ctrl+Shift+0")
        ),
        Some(&WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: FontZoomStep::Reset
        })
    );

    // No collision: the bare `=`/`-` (resize) and Shift+`=` (pane height) keys
    // keep their original actions — the Ctrl / Ctrl+Shift variants are distinct.
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("=")),
        Some(&WmAction::ResizeIncrease)
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("-")),
        Some(&WmAction::ResizeDecrease)
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Shift+=")),
        Some(&WmAction::PaneHeightIncrease)
    );
    // The other Ctrl+Shift bindings (move column up/down) keep their actions.
    assert_eq!(
        keymap.resolve_builtin(
            crate::keymap::LEADER_LAYER,
            &KeyCombo::parse("Ctrl+Shift+k")
        ),
        Some(&WmAction::MoveColumnUp)
    );
}

#[test]
fn default_workspace_switch_is_ctrl_p_and_ctrl_n_only() {
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

    // One pair of keys per job: `prefix+u` / `prefix+d` are free for the user (Antonio,
    // 2026-10-03).
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("u")),
        None
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("d")),
        None
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Ctrl+p")),
        Some(&WmAction::WorkspacePrev)
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Ctrl+n")),
        Some(&WmAction::WorkspaceNext)
    );
}

#[test]
fn default_sidebar_global_collapse_bindings_exist() {
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("(")),
        Some(&WmAction::ToggleCurrentColumnCollapsed)
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("<")),
        Some(&WmAction::ToggleCurrentWorkspaceCollapsed)
    );
}

#[test]
fn default_pane_navigation_and_palette_bindings_are_separate() {
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("[")),
        Some(&WmAction::PrevPane)
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("]")),
        Some(&WmAction::NextPane)
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("p")),
        Some(&WmAction::CommandPalette {
            mode: None,
            query: None
        })
    );
}

#[test]
fn default_selection_bindings_resolve() {
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("s")),
        Some(&WmAction::EnterSelectionMode)
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Shift+s")),
        Some(&WmAction::ClearSelection)
    );
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("y")),
        Some(&WmAction::CopySelection)
    );
    // paste_clipboard is intentionally not given a default flat binding
    // to avoid colliding with established keys; users bind it in config.
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("p")),
        Some(&WmAction::CommandPalette {
            mode: None,
            query: None
        })
    );
}
