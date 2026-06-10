//! Registry and keymap construction helpers.
//!
//! This module owns config-driven action/keymap wiring so `main.rs` can stay
//! focused on application lifecycle and event dispatch.

use crate::actions::ActionRegistry;
use crate::handlers::*;
use crate::input::{self, WmAction, action_from_name, build_action};
use crate::keymap::{KeyCombo, KeymapRegistry};
use heca_core::layout::PaneId;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug)]
struct BindingConflict {
    mode: String,
    combo: KeyCombo,
    previous_action: WmAction,
    previous_source: String,
    new_action: WmAction,
    new_source: String,
}

fn bind_with_conflict_tracking(
    keymap: &mut KeymapRegistry,
    mode: &str,
    combo: KeyCombo,
    action: WmAction,
    source: String,
    conflicts: &mut Vec<BindingConflict>,
) {
    if let Some(previous_action) = keymap.resolve(mode, &combo).cloned()
        && previous_action != action
    {
        conflicts.push(BindingConflict {
            mode: mode.to_string(),
            combo: combo.clone(),
            previous_action,
            previous_source: "existing binding".to_string(),
            new_action: action.clone(),
            new_source: source,
        });
    }
    keymap.bind(mode, combo, action);
}

fn format_combo(combo: &KeyCombo) -> String {
    let mut parts = Vec::new();
    if combo.ctrl {
        parts.push("Ctrl".to_string());
    }
    if combo.shift {
        parts.push("Shift".to_string());
    }
    if combo.alt {
        parts.push("Alt".to_string());
    }
    if combo.super_ {
        parts.push("Super".to_string());
    }
    parts.push(combo.key.clone());
    parts.join("+")
}

fn log_conflicts(kind: &str, conflicts: &[BindingConflict]) {
    if conflicts.is_empty() || cfg!(test) {
        return;
    }

    eprintln!(
        "[heca] detected {} keybinding conflict(s):",
        conflicts.len()
    );
    for conflict in conflicts {
        eprintln!(
            "[heca] {} conflict in mode '{}': '{}' => {:?} ({}) overwritten by {:?} ({})",
            kind,
            conflict.mode,
            format_combo(&conflict.combo),
            conflict.previous_action,
            conflict.previous_source,
            conflict.new_action,
            conflict.new_source,
        );
    }
}

/// Build the keymap registry from a config.
pub fn build_keymap(config: &heca_config::theme::Config) -> KeymapRegistry {
    let mut keymap = KeymapRegistry::new();
    let mut conflicts = Vec::new();

    let default_keys = heca_config::theme::KeysConfig::default();
    let mut merged_bindings: BTreeMap<String, heca_config::theme::BindingValue> = default_keys
        .bindings
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
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
                bind_with_conflict_tracking(
                    &mut keymap,
                    "normal",
                    KeyCombo::parse(rest),
                    action.clone(),
                    format!("[keys] {action_name}"),
                    &mut conflicts,
                );
            } else {
                bind_with_conflict_tracking(
                    &mut keymap,
                    "global",
                    KeyCombo::parse(trimmed),
                    action.clone(),
                    format!("[keys] {action_name}"),
                    &mut conflicts,
                );
            }
        }
    }

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

    for cmd_cfg in &config.keys.command {
        let action = WmAction::SpawnCommand {
            command: cmd_cfg.command.clone(),
        };
        let trimmed = cmd_cfg.key.trim();
        if trimmed.starts_with("prefix+") {
            let rest = trimmed.strip_prefix("prefix+").unwrap().trim();
            bind_with_conflict_tracking(
                &mut keymap,
                "normal",
                KeyCombo::parse(rest),
                action,
                format!("[[keys.command]] {}", cmd_cfg.command),
                &mut conflicts,
            );
        } else {
            bind_with_conflict_tracking(
                &mut keymap,
                "global",
                KeyCombo::parse(trimmed),
                action,
                format!("[[keys.command]] {}", cmd_cfg.command),
                &mut conflicts,
            );
        }
    }

    log_conflicts("flat", &conflicts);
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
    let mut conflicts = Vec::new();

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
        for binding in &mode_cfg.bindings {
            let action = if let Some(unit) = action_from_name(&binding.action) {
                unit
            } else if let Some(built) = build_action(&binding.action, &binding.args) {
                built
            } else {
                continue;
            };
            bind_with_conflict_tracking(
                &mut mode_map,
                &mode_cfg.name,
                KeyCombo::parse(&binding.keys),
                action,
                format!("[keys.mode:{}] {}", mode_cfg.name, binding.action),
                &mut conflicts,
            );
        }
        mode_keymaps.insert(mode_cfg.name.clone(), mode_map);
        // SidebarNav is entered via SidebarFocus / mouse interaction, so the
        // built-in sidebar mode does not use a trigger entry here.
        if mode_cfg.name != "sidebar" {
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

    log_conflicts("mode", &conflicts);
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
    registry.register(&WmAction::FocusPane { pane_id: PaneId(0) }, handle_focus_pane);
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
    registry.register(&WmAction::Swap { a_id: PaneId(0), b_id: PaneId(0) }, handle_swap_param);
    registry.register(
        &WmAction::Move {
            pane_id: PaneId(0),
            target_col: 0,
        },
        handle_move_param,
    );
    registry.register(
        &WmAction::MovePaneToWorkspace {
            pane_id: PaneId(0),
            ws_idx: 0,
        },
        handle_move_pane_to_workspace,
    );
    registry.register(
        &WmAction::MovePaneToColumn {
            pane_id: PaneId(0),
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
            pane_id: PaneId(0),
            focus_after: false,
        },
        handle_take_pane,
    );
    registry.register(&WmAction::RenamePane, handle_rename_pane);
    registry.register(&WmAction::RenameColumn, handle_rename_column);
    registry.register(
        &WmAction::FloatAt {
            pane_id: PaneId(0),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        handle_float_at,
    );
    registry.register(
        &WmAction::ClosePaneById { pane_id: PaneId(0) },
        handle_close_pane_by_id,
    );
    registry.register(
        &WmAction::RenameTarget {
            pane_id: PaneId(0),
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
    registry.register(
        &WmAction::SidebarCreateWorkspace,
        handle_sidebar_create_workspace,
    );
    registry.register(&WmAction::SidebarCreateColumn, handle_sidebar_create_column);
    registry.register(
        &WmAction::SidebarSplitInColumn,
        handle_sidebar_split_in_column,
    );
    registry.register(
        &WmAction::SidebarZoomSelectedColumn,
        handle_sidebar_zoom_selected_column,
    );
    registry.register(
        &WmAction::SidebarDeleteSelected,
        handle_sidebar_delete_selected,
    );
    registry.register(
        &WmAction::CollapseCurrentWorkspace,
        handle_collapse_current_workspace,
    );
    registry.register(
        &WmAction::ExpandCurrentWorkspace,
        handle_expand_current_workspace,
    );
    registry.register(
        &WmAction::ToggleCurrentWorkspaceCollapsed,
        handle_toggle_current_workspace_collapsed,
    );
    registry.register(
        &WmAction::CollapseCurrentColumn,
        handle_collapse_current_column,
    );
    registry.register(
        &WmAction::ExpandCurrentColumn,
        handle_expand_current_column,
    );
    registry.register(
        &WmAction::ToggleCurrentColumnCollapsed,
        handle_toggle_current_column_collapsed,
    );

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
    use super::{build_keymap, build_modes};
    use crate::input::WmAction;
    use crate::keymap::KeyCombo;
    use heca_config::theme::{KeyModeConfig, ModeBindingConfig};
    use std::collections::HashMap;

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

    #[test]
    fn default_sidebar_global_collapse_bindings_exist() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config);

        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("(")),
            Some(&WmAction::ToggleCurrentColumnCollapsed)
        );
        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("<")),
            Some(&WmAction::ToggleCurrentWorkspaceCollapsed)
        );
    }

    #[test]
    fn default_pane_navigation_and_palette_bindings_are_separate() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config);

        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("[")),
            Some(&WmAction::PrevPane)
        );
        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("]")),
            Some(&WmAction::NextPane)
        );
        assert_eq!(
            keymap.resolve("normal", &KeyCombo::parse("p")),
            Some(&WmAction::CommandPalette)
        );
    }

    #[test]
    fn sidebar_mode_includes_arrow_aliases() {
        let config = heca_config::theme::Config::default();
        let (mode_keymaps, _) = build_modes(&config);
        let keymap = mode_keymaps.get("sidebar").expect("sidebar mode exists");

        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("ArrowUp")),
            Some(&WmAction::SidebarUp)
        );
        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("ArrowDown")),
            Some(&WmAction::SidebarDown)
        );
        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("ArrowLeft")),
            Some(&WmAction::SidebarLeftNav)
        );
        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("ArrowRight")),
            Some(&WmAction::SidebarRightNav)
        );
        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("Space")),
            Some(&WmAction::SidebarRightNav)
        );
    }

    #[test]
    fn sidebar_mode_includes_mutation_bindings() {
        let config = heca_config::theme::Config::default();
        let (mode_keymaps, _) = build_modes(&config);
        let keymap = mode_keymaps.get("sidebar").expect("sidebar mode exists");

        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("w")),
            Some(&WmAction::SidebarCreateWorkspace)
        );
        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("c")),
            Some(&WmAction::SidebarCreateColumn)
        );
        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("v")),
            Some(&WmAction::SidebarSplitInColumn)
        );
        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("z")),
            Some(&WmAction::SidebarZoomSelectedColumn)
        );
        assert_eq!(
            keymap.resolve("sidebar", &KeyCombo::parse("d")),
            Some(&WmAction::SidebarDeleteSelected)
        );
    }

    #[test]
    fn user_sidebar_mode_with_same_name_overrides_defaults() {
        let mut config = heca_config::theme::Config::default();
        config.keys.mode.push(KeyModeConfig {
            name: "sidebar".to_string(),
            trigger: "prefix+e".to_string(),
            sticky: true,
            bindings: vec![ModeBindingConfig {
                action: "sidebar_left_nav".to_string(),
                keys: "j".to_string(),
                args: HashMap::new(),
            }],
        });

        let (mode_keymaps, mode_triggers) = build_modes(&config);
        let sidebar = mode_keymaps.get("sidebar").expect("sidebar mode exists");

        assert_eq!(
            sidebar.resolve("sidebar", &KeyCombo::parse("j")),
            Some(&WmAction::SidebarLeftNav)
        );
        assert_eq!(
            sidebar.resolve("sidebar", &KeyCombo::parse("k")),
            Some(&WmAction::SidebarUp)
        );
        assert!(!mode_triggers.contains_key("sidebar"));
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
                keys: "x".to_string(),
                args: HashMap::new(),
            }],
        });

        let (mode_keymaps, mode_triggers) = build_modes(&config);
        let resize = mode_keymaps.get("resize").expect("resize mode exists");

        assert!(resize.resolve("resize", &KeyCombo::parse("h")).is_some());
        assert_eq!(
            resize.resolve("resize", &KeyCombo::parse("x")),
            Some(&WmAction::ResizeIncrease)
        );
        assert_eq!(
            mode_triggers.get("resize"),
            Some(&(KeyCombo::parse("r"), true))
        );
    }
}
