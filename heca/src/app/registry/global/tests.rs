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
        cwd: None,
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
            terminal: None,
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
            terminal: None,
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
            terminal: None,
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

    // One pair of keys per job: `prefix+u` / `prefix+d` are free for the user.
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
            query: None,
            ask: None,
            arguments: None
        })
    );
}

#[test]
fn default_session_palette_binding_uses_the_registered_action_and_hash_key() {
    use crate::keymap::LEADER_LAYER;
    use winit::keyboard::{Key, KeyCode, ModifiersState, PhysicalKey};
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    let registry = crate::app::registry::build_registry();
    for (key, mode) in [("o", "pane"), ("Ctrl+o", "workspace")] {
        let action = WmAction::CommandPalette {
            mode: Some(mode.into()),
            query: None,
            ask: None,
            arguments: None,
        };
        assert!(registry.has_handler(&action));
        assert_eq!(
            keymap.resolve_builtin(LEADER_LAYER, &KeyCombo::parse(key)),
            Some(&action),
            "{key}"
        );
    }
    let hash = crate::app::keyboard::build_event_combo(
        &Key::Character("#".into()),
        &PhysicalKey::Code(KeyCode::Digit3),
        "#",
        ModifiersState::SHIFT,
    );
    // The session list is its own action — the same one the palette's "Sessions" row and
    // `heca rpc list_sessions` run — so the row shows this key.
    // It belongs to the server, which answers with the list; the window opens the `#` list on it.
    assert!(crate::server::sessions::owns(
        crate::input::WmActionKind::ListSessions
    ));
    assert_eq!(
        keymap.resolve_builtin(LEADER_LAYER, &hash),
        Some(&WmAction::ListSessions)
    );
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let (intent, _) = crate::rpc::parse_rpc("command_palette mode=session")
        .unwrap()
        .into_intent(&catalog)
        .unwrap();
    assert_eq!(
        crate::input::resolve_action(
            &intent.action,
            &crate::app::interaction::intent_args_as_strings(&intent)
        ),
        Some(WmAction::CommandPalette {
            mode: Some("session".into()),
            query: None,
            ask: None,
            arguments: None,
        })
    );
}

#[test]
fn the_notification_history_opens_from_its_default_key_and_from_rpc() {
    use crate::keymap::LEADER_LAYER;
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    let registry = crate::app::registry::build_registry();

    assert!(registry.has_handler(&WmAction::NotificationHistory));
    assert_eq!(
        keymap.resolve_builtin(LEADER_LAYER, &KeyCombo::parse("Ctrl+Alt+n")),
        Some(&WmAction::NotificationHistory),
        "prefix+Ctrl+Alt+n",
    );
    // The neighbours keep their keys: the new one took a free combination.
    assert_eq!(
        keymap.resolve_builtin(LEADER_LAYER, &KeyCombo::parse("Ctrl+n")),
        Some(&WmAction::WorkspaceNext),
    );
    assert_eq!(
        keymap.resolve_builtin(LEADER_LAYER, &KeyCombo::parse("n")),
        Some(&WmAction::NotificationPick)
    );

    // Named, it is in the catalog with no arguments, so RPC and the palette reach it by that name.
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let meta = catalog
        .all()
        .find(|meta| meta.name == "notification_history")
        .expect("described");
    assert!(meta.args.is_empty());
    let (intent, _) = crate::rpc::parse_rpc("notification_history")
        .unwrap()
        .into_intent(&catalog)
        .unwrap();
    assert_eq!(
        crate::input::resolve_action(
            &intent.action,
            &crate::app::interaction::intent_args_as_strings(&intent)
        ),
        Some(WmAction::NotificationHistory),
    );
}

#[test]
fn the_default_session_palette_key_can_be_overridden_or_unbound() {
    use crate::keymap::LEADER_LAYER;
    let config = heca_config::loader::config_from_layers(
        r##"
[[keys.bind]]
keys = "prefix+#"
action = "command_palette"
args = { mode = "pane" }
"##,
    )
    .unwrap();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    assert_eq!(
        keymap.resolve_builtin(LEADER_LAYER, &KeyCombo::parse("#")),
        Some(&WmAction::CommandPalette {
            mode: Some("pane".into()),
            query: None,
            ask: None,
            arguments: None
        })
    );
    assert_eq!(
        keymap.resolve_builtin(LEADER_LAYER, &KeyCombo::parse("Ctrl+o")),
        Some(&WmAction::CommandPalette {
            mode: Some("workspace".into()),
            query: None,
            ask: None,
            arguments: None
        })
    );
    let config =
        heca_config::loader::config_from_layers("[keys.unbind]\n\"prefix+#\" = true\n").unwrap();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    assert!(
        keymap
            .resolve_builtin(LEADER_LAYER, &KeyCombo::parse("#"))
            .is_none()
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
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Shift+q")),
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
            query: None,
            ask: None,
            arguments: None
        })
    );
}

/// **A `[[keys.command]]` says where its program starts** — its `cwd` reaches the action as
/// written, and an entry without one leaves it unset (the server reads unset as the pane's folder).
#[test]
fn a_key_commands_cwd_reaches_its_action() {
    use crate::keymap::LEADER_LAYER;
    let config = heca_config::loader::config_from_layers(
        r##"
[[keys.command]]
keys = "prefix+F7"
command = "lazygit"
cwd = "git"

[[keys.command]]
keys = "prefix+F8"
command = "htop"
"##,
    )
    .unwrap();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    let cwd_of = |key: &str| match keymap.resolve_builtin(LEADER_LAYER, &KeyCombo::parse(key)) {
        Some(WmAction::SpawnCommand { cwd, .. }) => cwd.clone(),
        other => panic!("{key} is not a command: {other:?}"),
    };
    assert_eq!(cwd_of("F7"), Some("git".to_string()));
    assert_eq!(cwd_of("F8"), None);
}

/// **`sidebar_pane_view` has a key and a name**, like every action: `prefix+Shift+b` goes to the next
/// view, and `heca rpc sidebar_pane_view view=compact` picks one by name.
#[test]
fn the_sidebar_view_has_a_default_key_and_an_rpc_door() {
    use heca_config::appearance::PaneView;
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    assert_eq!(
        keymap.resolve_builtin(crate::keymap::LEADER_LAYER, &KeyCombo::parse("Shift+b")),
        Some(&WmAction::SidebarPaneView { view: None }),
        "the bare key goes to the next view"
    );
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let resolved = |line: &str| {
        let (intent, _) = crate::rpc::parse_rpc(line)
            .unwrap()
            .into_intent(&catalog)
            .unwrap();
        crate::input::resolve_action(
            &intent.action,
            &crate::app::interaction::intent_args_as_strings(&intent),
        )
    };
    assert_eq!(
        resolved("sidebar_pane_view view=compact"),
        Some(WmAction::SidebarPaneView {
            view: Some(PaneView::Compact)
        })
    );
    assert_eq!(
        resolved("sidebar_pane_view"),
        Some(WmAction::SidebarPaneView { view: None })
    );
}

/// **No key without the prefix opens a menu by default.** heca holds other programs: a global
/// `Shift+F10` or Menu key would be taken before the program in the focused pane ever saw it. The
/// context menu is `prefix+>`.
#[test]
fn the_shipped_context_menu_key_is_behind_the_prefix_only() {
    use crate::keymap::{DIRECT_LAYER, LEADER_LAYER};
    let config = heca_config::theme::Config::default();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    for key in ["Shift+F10", "ContextMenu"] {
        assert!(
            keymap
                .resolve(DIRECT_LAYER, &KeyCombo::parse(key))
                .is_none(),
            "{key} must reach the focused program"
        );
    }
    assert_eq!(
        keymap.resolve_builtin(LEADER_LAYER, &KeyCombo::parse(">")),
        Some(&WmAction::OpenContextMenu)
    );
}

/// **A sequence is bound as its keys in order, and only after the prefix** — `prefix+Ctrl+s r`
/// renames the session: after the prefix, `Ctrl+s` begins it (nothing is bound to it alone) and `r`
/// completes it. The palette shows it as written.
#[test]
fn the_shipped_session_keys_are_a_sequence_after_the_prefix() {
    use crate::keymap::LEADER_LAYER;
    let config = heca_config::theme::Config::default();
    let mut index = BindingIndex::new();
    let keymap = build_keymap(&config, &mut Conflicts::default(), &mut index);
    let shift_s = KeyCombo::parse("Ctrl+s");
    let first = keymap.step(LEADER_LAYER, std::slice::from_ref(&shift_s));
    assert!(first.continues, "Ctrl+s begins the session keys");
    assert_eq!(first.complete, None);
    let done = keymap.step(LEADER_LAYER, &[shift_s, KeyCombo::parse("r")]);
    assert!(!done.continues);
    assert_eq!(
        done.complete,
        Some(crate::keymap::ActionRef::Builtin(WmAction::RenameSession {
            session: None
        }))
    );
    assert!(
        index["rename_session"]
            .iter()
            .any(|b| b.key == "prefix+Ctrl+s r"),
        "shown as written"
    );
}
