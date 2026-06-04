//! Registry and keymap construction helpers.
//!
//! This module owns config-driven action/keymap wiring so `main.rs` can stay
//! focused on application lifecycle and event dispatch.

use crate::actions::ActionRegistry;
use crate::handlers::*;
use crate::input::{self, WmAction, action_from_name, build_action};
use crate::keymap::{KeyCombo, KeymapRegistry};
use std::collections::HashMap;

/// Build the keymap registry from a config.
pub fn build_keymap(config: &heca_config::theme::Config) -> KeymapRegistry {
    let mut keymap = KeymapRegistry::new();

    // ── Load bindings from [keys] flat map ──
    let default_keys = heca_config::theme::KeysConfig::default();
    let mut merged_bindings = default_keys.bindings.clone();
    for (k, v) in &config.keys.bindings {
        merged_bindings.insert(k.clone(), v.clone());
    }
    for (action_name, value) in &merged_bindings {
        let Some(action) = action_from_name(action_name) else {
            continue;
        };
        for key_str in value.keys() {
            let trimmed = key_str.trim();
            if trimmed.starts_with("prefix+") {
                let rest = trimmed.strip_prefix("prefix+").unwrap().trim();
                let combo = KeyCombo::parse(rest);
                keymap.bind("normal", combo, action.clone());
            } else {
                let combo = KeyCombo::parse(trimmed);
                keymap.bind("global", combo, action.clone());
            }
        }
    }

    // ── Apply unbinds ──
    for combo_str in config.keys.unbind.keys() {
        let trimmed = combo_str.trim();
        if trimmed.starts_with("prefix+") {
            let rest = trimmed.strip_prefix("prefix+").unwrap().trim();
            let combo = KeyCombo::parse(rest);
            keymap.unbind("normal", &combo);
        } else {
            let combo = KeyCombo::parse(trimmed);
            keymap.unbind("global", &combo);
        }
    }

    // ── Sidebar-mode bindings (hardcoded for now) ──
    let sidebar_bindings = vec![
        ("j", WmAction::SidebarDown),
        ("k", WmAction::SidebarUp),
        ("h", WmAction::SidebarLeftNav),
        ("l", WmAction::SidebarRightNav),
        ("Tab", WmAction::SidebarExpandToggle),
        ("Space", WmAction::SidebarExpandToggle),
        ("b", WmAction::SidebarLeft),
        ("Enter", WmAction::SidebarRightNav),
    ];
    for (key, action) in sidebar_bindings {
        keymap.bind("sidebar", KeyCombo::parse(key), action);
    }

    // ── Custom command bindings from [[keys.command]] ──
    for cmd_cfg in &config.keys.command {
        let action = WmAction::SpawnCommand {
            command: cmd_cfg.command.clone(),
        };
        let trimmed = cmd_cfg.key.trim();
        if trimmed.starts_with("prefix+") {
            let rest = trimmed.strip_prefix("prefix+").unwrap().trim();
            keymap.bind("normal", KeyCombo::parse(rest), action);
        } else {
            keymap.bind("global", KeyCombo::parse(trimmed), action);
        }
    }

    keymap
}

/// Build mode keymaps and triggers from config.
pub fn build_modes(
    config: &heca_config::theme::Config,
) -> (
    HashMap<String, KeymapRegistry>,
    HashMap<String, (KeyCombo, bool)>,
) {
    let mut mode_keymaps = HashMap::new();
    let mut mode_triggers: HashMap<String, (KeyCombo, bool)> = HashMap::new();

    // Start with default modes so built-in modes (resize, etc.) are always available.
    let default_keys = heca_config::theme::KeysConfig::default();
    let modes_to_load: Vec<_> = default_keys
        .mode
        .iter()
        .chain(config.keys.mode.iter())
        .cloned()
        .collect();

    for mode_cfg in &modes_to_load {
        let mut mode_map = KeymapRegistry::new();
        for binding in &mode_cfg.bindings {
            let action = if let Some(unit) = action_from_name(&binding.action) {
                unit
            } else if let Some(built) = build_action(&binding.action, &binding.args) {
                built
            } else {
                continue;
            };
            let combo = KeyCombo::parse(&binding.keys);
            mode_map.bind(&mode_cfg.name, combo, action);
        }
        mode_keymaps.insert(mode_cfg.name.clone(), mode_map);
        let trigger_trimmed = mode_cfg.trigger.trim();
        let trigger_combo = if trigger_trimmed.starts_with("prefix+") {
            let rest = trigger_trimmed.strip_prefix("prefix+").unwrap().trim();
            KeyCombo::parse(rest)
        } else {
            KeyCombo::parse(trigger_trimmed)
        };
        mode_triggers.insert(mode_cfg.name.clone(), (trigger_combo, mode_cfg.sticky));
    }
    (mode_keymaps, mode_triggers)
}

/// Build the action registry and register individual handlers for all actions.
pub fn build_registry() -> ActionRegistry {
    let mut registry = ActionRegistry::new();

    // ── Navigation ──
    registry.register(&WmAction::FocusLeft, handle_focus_left);
    registry.register(&WmAction::FocusRight, handle_focus_right);
    registry.register(&WmAction::FocusUp, handle_focus_up);
    registry.register(&WmAction::FocusDown, handle_focus_down);
    registry.register(&WmAction::NextPane, handle_next_pane);
    registry.register(&WmAction::PrevPane, handle_prev_pane);
    registry.register(&WmAction::WorkspaceNext, handle_workspace_next);
    registry.register(&WmAction::WorkspacePrev, handle_workspace_prev);
    registry.register(&WmAction::FocusToggleLocal, handle_focus_toggle_local);
    registry.register(&WmAction::FocusToggleGlobal, handle_focus_toggle_global);
    registry.register(&WmAction::FocusPane { pane_id: 0 }, handle_focus_pane);
    registry.register(
        &WmAction::FocusWorkspace { ws_idx: 0 },
        handle_focus_workspace,
    );

    // ── Layout ──
    registry.register(&WmAction::SplitHorizontal, handle_split_horizontal);
    registry.register(&WmAction::SplitVertical, handle_split_vertical);
    registry.register(&WmAction::ZoomColumn, handle_zoom_column);
    registry.register(&WmAction::ResizeIncrease, handle_resize_increase);
    registry.register(&WmAction::ResizeDecrease, handle_resize_decrease);
    registry.register(&WmAction::PaneHeightIncrease, handle_pane_height_increase);
    registry.register(&WmAction::PaneHeightDecrease, handle_pane_height_decrease);
    registry.register(&WmAction::SwapLeft, handle_swap_left);
    registry.register(&WmAction::SwapRight, handle_swap_right);
    registry.register(&WmAction::SwapUp, handle_swap_up);
    registry.register(&WmAction::SwapDown, handle_swap_down);
    registry.register(&WmAction::MovePaneLeft, handle_move_pane_left);
    registry.register(&WmAction::MovePaneRight, handle_move_pane_right);
    registry.register(&WmAction::MoveColumnUp, handle_move_column_up);
    registry.register(&WmAction::MoveColumnDown, handle_move_column_down);
    registry.register(&WmAction::Swap { a_id: 0, b_id: 0 }, handle_swap_param);
    registry.register(
        &WmAction::Move {
            pane_id: 0,
            target_col: 0,
        },
        handle_move_param,
    );
    registry.register(
        &WmAction::MovePaneToWorkspace {
            pane_id: 0,
            ws_idx: 0,
        },
        handle_move_pane_to_workspace,
    );
    registry.register(
        &WmAction::MovePaneToColumn {
            pane_id: 0,
            ws_idx: 0,
            col_idx: 0,
        },
        handle_move_pane_to_column,
    );
    registry.register(
        &WmAction::MoveColumnToWorkspace {
            col_idx: 0,
            ws_idx: 0,
            focus: true,
        },
        handle_move_column_to_workspace,
    );
    registry.register(
        &WmAction::Resize {
            target: input::ResizeTarget::Column,
            axis: input::ResizeAxis::X,
            amount: 0.0,
        },
        handle_resize,
    );
    registry.register(
        &WmAction::ResizeTo {
            target: input::ResizeTarget::Column,
            width: 0.0,
            height: 0.0,
        },
        handle_resize_to,
    );

    // ── Pane ──
    registry.register(&WmAction::Float, handle_float);
    registry.register(&WmAction::ClosePane, handle_close_pane);
    registry.register(&WmAction::PaneSelect, handle_pane_select);
    registry.register(&WmAction::SwapPane, handle_swap_pane);
    registry.register(&WmAction::SwapAndFocusPane, handle_swap_and_focus_pane);
    registry.register(&WmAction::PaneTake, handle_pane_take);
    registry.register(&WmAction::PaneTakeAndFocus, handle_pane_take_and_focus);
    registry.register(
        &WmAction::TakePane {
            pane_id: 0,
            focus_after: false,
        },
        handle_take_pane,
    );
    registry.register(&WmAction::RenamePane, handle_rename_pane);
    registry.register(&WmAction::RenameColumn, handle_rename_column);
    registry.register(
        &WmAction::FloatAt {
            pane_id: 0,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        handle_float_at,
    );
    registry.register(
        &WmAction::ClosePaneById { pane_id: 0 },
        handle_close_pane_by_id,
    );
    registry.register(
        &WmAction::RenameTarget {
            pane_id: 0,
            name: String::new(),
        },
        handle_rename_target,
    );

    // ── Workspace ──
    registry.register(&WmAction::CreateWorkspace, handle_create_workspace);
    registry.register(&WmAction::RenameWorkspace, handle_rename_workspace);

    // ── Sidebar / Chrome ──
    registry.register(&WmAction::SidebarLeft, handle_sidebar_left);
    registry.register(&WmAction::SidebarRight, handle_sidebar_right);
    registry.register(&WmAction::SidebarFocus, handle_sidebar_focus);
    registry.register(&WmAction::SidebarUp, handle_sidebar_up);
    registry.register(&WmAction::SidebarDown, handle_sidebar_down);
    registry.register(&WmAction::SidebarLeftNav, handle_sidebar_left_nav);
    registry.register(&WmAction::SidebarRightNav, handle_sidebar_right_nav);
    registry.register(&WmAction::SidebarExpandToggle, handle_sidebar_expand_toggle);

    // ── System ──
    registry.register(&WmAction::CommandPalette, handle_command_palette);
    registry.register(
        &WmAction::SpawnCommand {
            command: String::new(),
        },
        handle_spawn_command,
    );
    registry.register(&WmAction::ReloadConfig, handle_reload_config);

    // ── Sidebar-specific (parameterized) ──
    registry.register(
        &WmAction::AddPaneToColumn {
            ws_idx: 0,
            col_idx: 0,
        },
        handle_add_pane_to_column,
    );

    // ── Destructive ──
    registry.register(
        &WmAction::DeleteColumn {
            ws_idx: 0,
            col_idx: 0,
        },
        handle_delete_column,
    );
    registry.register(
        &WmAction::DeleteWorkspace { ws_idx: 0 },
        handle_delete_workspace,
    );

    // ── Take ──
    // (registered above with PaneTake/PaneTakeAndFocus)

    // ── Mode ──
    registry.register(
        &WmAction::EnterMode {
            name: String::new(),
        },
        handle_enter_mode,
    );

    registry
}

#[cfg(test)]
mod tests {
    use super::build_keymap;
    use crate::input::WmAction;
    use crate::keymap::KeyCombo;

    #[test]
    fn default_ctrl_k_binding_stays_swap_up() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config);

        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("Ctrl+k")),
            Some(&WmAction::SwapUp)
        );
        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("Ctrl+j")),
            Some(&WmAction::SwapDown)
        );
        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("Ctrl+Shift+k")),
            Some(&WmAction::MoveColumnUp)
        );
        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("Ctrl+Shift+j")),
            Some(&WmAction::MoveColumnDown)
        );
    }

    #[test]
    fn default_workspace_aliases_include_ctrl_p_and_ctrl_n() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config);

        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("u")),
            Some(&WmAction::WorkspacePrev)
        );
        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("d")),
            Some(&WmAction::WorkspaceNext)
        );
        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("Ctrl+p")),
            Some(&WmAction::WorkspacePrev)
        );
        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("Ctrl+n")),
            Some(&WmAction::WorkspaceNext)
        );
    }
}
