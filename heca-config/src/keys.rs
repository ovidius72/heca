use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
//  BindingValue
// ═══════════════════════════════════════════════════════════════════════════════

/// A single binding value, either a single string or a list of key combos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BindingValue {
    Single(String),
    Many(Vec<String>),
}

impl BindingValue {
    pub fn keys(&self) -> Vec<&str> {
        match self {
            BindingValue::Single(s) => s
                .split(',')
                .map(|p| p.trim())
                .filter(|p| !p.is_empty())
                .collect(),
            BindingValue::Many(v) => v
                .iter()
                .map(|s| s.as_str())
                .filter(|p| !p.is_empty())
                .collect(),
        }
    }
}

/// Map of action names to their keybinding values.
pub type KeybindingMap = HashMap<String, BindingValue>;

// ═══════════════════════════════════════════════════════════════════════════════
//  CommandKeybindConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// A keybinding that spawns an external command (rather than triggering a WM action).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandKeybindConfig {
    pub key: String,
    pub command: String,
    #[serde(default = "default_command_type")]
    pub command_type: String,
}

fn default_command_type() -> String {
    "pane".to_string()
}

// ═══════════════════════════════════════════════════════════════════════════════
//  ModeBindingConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// A single action binding inside a custom input mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ModeBindingConfig {
    pub action: String,
    /// The key(s) for this binding. Also accepts `key` (singular) for convenience.
    #[serde(alias = "key")]
    pub keys: String,
    /// Arguments for parameterized actions.
    /// e.g. `args = { target = "column", axis = "x", amount = "50" }`
    /// → WmAction::Resize { target: Column, axis: X, amount: 50.0 }
    #[serde(default)]
    pub args: HashMap<String, String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  KeyModeConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// A custom input mode with its own keybinding namespace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyModeConfig {
    pub name: String,
    pub trigger: String,
    /// If true, stay in this mode until Esc or another mode trigger.
    /// If false, execute one binding and exit to Normal.
    #[serde(default = "default_mode_sticky")]
    pub sticky: bool,
    #[serde(default)]
    pub bindings: Vec<ModeBindingConfig>,
}

fn default_mode_sticky() -> bool {
    true
}

// ═══════════════════════════════════════════════════════════════════════════════
//  KeysConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// The top-level keybinding configuration, holding prefix, flat bindings,
/// command bindings, and custom input modes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeysConfig {
    /// The prefix key that activates prefix mode.
    /// Accepts both `prefix` and `prefix_key` for compatibility.
    #[serde(default = "default_prefix_key", alias = "prefix_key")]
    pub prefix: String,
    /// Flat action bindings (any key not named "prefix", "command", "mode", or "unbind").
    #[serde(flatten)]
    pub bindings: KeybindingMap,
    /// Key combos to remove from the keymap (e.g. to free a default binding).
    /// Each entry is a combo string like "prefix+w" or "Alt+1".
    #[serde(default)]
    pub unbind: HashMap<String, bool>,
    /// Custom command bindings.
    #[serde(default)]
    pub command: Vec<CommandKeybindConfig>,
    /// Custom input modes.
    #[serde(default)]
    pub mode: Vec<KeyModeConfig>,
}

fn default_prefix_key() -> String {
    "ctrl+b".to_string()
}

impl Default for KeysConfig {
    fn default() -> Self {
        let mut bindings = HashMap::new();
        use BindingValue::*;

        // ── Navigation ──
        bindings.insert(
            "focus_left".to_string(),
            Many(vec!["prefix+h".to_string(), "prefix+ArrowLeft".to_string()]),
        );
        bindings.insert(
            "focus_right".to_string(),
            Many(vec!["prefix+l".to_string(), "prefix+ArrowRight".to_string()]),
        );
        bindings.insert(
            "focus_up".to_string(),
            Many(vec!["prefix+k".to_string(), "prefix+ArrowUp".to_string()]),
        );
        bindings.insert(
            "focus_down".to_string(),
            Many(vec!["prefix+j".to_string(), "prefix+ArrowDown".to_string()]),
        );

        // ── Splits ──
        bindings.insert(
            "split_horizontal".to_string(),
            Single("prefix+Enter".to_string()),
        );
        bindings.insert("split_vertical".to_string(), Single("prefix+v".to_string()));
        bindings.insert("zoom_column".to_string(), Single("prefix+z".to_string()));

        // ── Resize ──
        bindings.insert(
            "resize_increase".to_string(),
            Single("prefix+=".to_string()),
        );
        bindings.insert(
            "resize_decrease".to_string(),
            Single("prefix+-".to_string()),
        );
        bindings.insert(
            "pane_height_increase".to_string(),
            Single("prefix+Shift+=".to_string()),
        );
        bindings.insert(
            "pane_height_decrease".to_string(),
            Single("prefix+Shift+-".to_string()),
        );

        // ── Pane operations ──
        bindings.insert("close".to_string(), Single("prefix+x".to_string()));
        bindings.insert("float".to_string(), Single("prefix+f".to_string()));

        // ── Quick select / swap ──
        bindings.insert("pane_select".to_string(), Single("prefix+q".to_string()));
        // Swap (keyboard): Shift+M swaps panes
        bindings.insert(
            "swap_pane".to_string(),
            Single("prefix+Shift+m".to_string()),
        );
        // Swap + focus (keyboard): M swaps and then focuses the swapped pane
        bindings.insert(
            "swap_and_focus_pane".to_string(),
            Single("prefix+m".to_string()),
        );
        // Take: move target pane to bottom of active column
        bindings.insert("pane_take".to_string(), Single("prefix+t".to_string()));
        // Take + focus: same but focuses the moved pane
        bindings.insert(
            "pane_take_and_focus".to_string(),
            Single("prefix+Shift+t".to_string()),
        );

        // ── Tabs ──
        bindings.insert("next_pane".to_string(), Single("prefix+]".to_string()));
        bindings.insert("prev_pane".to_string(), Single("prefix+[".to_string()));

        // ── Sidebars ──
        bindings.insert("sidebar_left".to_string(), Single("prefix+b".to_string()));
        bindings.insert("sidebar_right".to_string(), Single("prefix+.".to_string()));
        bindings.insert("sidebar_focus".to_string(), Single("prefix+e".to_string()));
        bindings.insert(
            "sidebar_expand_toggle".to_string(),
            Single("prefix+Tab".to_string()),
        );

        // ── Sidebar tree UI collapse (global, works even when sidebar hidden) ──
        bindings.insert(
            "toggle_current_workspace_collapsed".to_string(),
            Single("prefix+<".to_string()),
        );
        bindings.insert(
            "toggle_current_column_collapsed".to_string(),
            Single("prefix+(".to_string()),
        );

        // ── Workspace navigation ──
        bindings.insert(
            "workspace_prev".to_string(),
            Many(vec!["prefix+u".to_string(), "prefix+Ctrl+p".to_string()]),
        );
        bindings.insert(
            "workspace_next".to_string(),
            Many(vec!["prefix+d".to_string(), "prefix+Ctrl+n".to_string()]),
        );

        // ── Focus toggle ──
        bindings.insert(
            "focus_toggle_local".to_string(),
            Single("prefix+i".to_string()),
        );
        bindings.insert(
            "focus_toggle_global".to_string(),
            Single("prefix+Shift+l".to_string()),
        );

        // ── Workspace / naming ──
        bindings.insert(
            "create_workspace".to_string(),
            Single("prefix+w".to_string()),
        );
        bindings.insert(
            "rename_workspace".to_string(),
            Single("prefix+Shift+w".to_string()),
        );
        bindings.insert("rename_pane".to_string(), Single("prefix+$".to_string()));
        bindings.insert(
            "rename_column".to_string(),
            Single("prefix+Shift+c".to_string()),
        );

        // ── Command palette ──
        bindings.insert(
            "command_palette".to_string(),
            Single("prefix+p".to_string()),
        );

        // ── Config reload ──
        bindings.insert(
            "reload_config".to_string(),
            Single("prefix+Shift+r".to_string()),
        );

        // ── Move pane to column (NIRI-style) ──
        bindings.insert(
            "move_pane_left".to_string(),
            Single("prefix+Ctrl+[".to_string()),
        );
        bindings.insert(
            "move_pane_right".to_string(),
            Single("prefix+Ctrl+]".to_string()),
        );

        // ── Move column to workspace (vertical movement) ──
        bindings.insert(
            "move_column_up".to_string(),
            Single("prefix+Ctrl+Shift+k".to_string()),
        );
        bindings.insert(
            "move_column_down".to_string(),
            Single("prefix+Ctrl+Shift+j".to_string()),
        );

        // ── Swap position (Ctrl+nav) ──
        bindings.insert("swap_left".to_string(), Single("prefix+Ctrl+h".to_string()));
        bindings.insert(
            "swap_right".to_string(),
            Single("prefix+Ctrl+l".to_string()),
        );
        bindings.insert("swap_up".to_string(), Single("prefix+Ctrl+k".to_string()));
        bindings.insert("swap_down".to_string(), Single("prefix+Ctrl+j".to_string()));

        // ── Default resize mode ──
        let mut mode = Vec::new();
        let mut resize_bindings = Vec::new();
        use ModeBindingConfig as Mbc;
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "h".to_string(),
            args: [
                ("target".to_string(), "column".to_string()),
                ("axis".to_string(), "x".to_string()),
                ("amount".to_string(), "-50".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "l".to_string(),
            args: [
                ("target".to_string(), "column".to_string()),
                ("axis".to_string(), "x".to_string()),
                ("amount".to_string(), "50".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "j".to_string(),
            args: [
                ("target".to_string(), "pane".to_string()),
                ("axis".to_string(), "y".to_string()),
                ("amount".to_string(), "-40".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "k".to_string(),
            args: [
                ("target".to_string(), "pane".to_string()),
                ("axis".to_string(), "y".to_string()),
                ("amount".to_string(), "40".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "Left".to_string(),
            args: [
                ("target".to_string(), "column".to_string()),
                ("axis".to_string(), "x".to_string()),
                ("amount".to_string(), "-50".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "Right".to_string(),
            args: [
                ("target".to_string(), "column".to_string()),
                ("axis".to_string(), "x".to_string()),
                ("amount".to_string(), "50".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "Down".to_string(),
            args: [
                ("target".to_string(), "pane".to_string()),
                ("axis".to_string(), "y".to_string()),
                ("amount".to_string(), "-40".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "Up".to_string(),
            args: [
                ("target".to_string(), "pane".to_string()),
                ("axis".to_string(), "y".to_string()),
                ("amount".to_string(), "40".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        mode.push(KeyModeConfig {
            name: "resize".to_string(),
            trigger: "prefix+r".to_string(),
            sticky: true,
            bindings: resize_bindings,
        });

        let sidebar_bindings = vec![
            Mbc {
                action: "sidebar_up".to_string(),
                keys: "k".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_down".to_string(),
                keys: "j".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_up".to_string(),
                keys: "Up".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_down".to_string(),
                keys: "Down".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_up".to_string(),
                keys: "ArrowUp".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_down".to_string(),
                keys: "ArrowDown".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_left_nav".to_string(),
                keys: "h".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_right_nav".to_string(),
                keys: "l".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_left_nav".to_string(),
                keys: "Left".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_right_nav".to_string(),
                keys: "Right".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_left_nav".to_string(),
                keys: "ArrowLeft".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_right_nav".to_string(),
                keys: "ArrowRight".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_create_workspace".to_string(),
                keys: "w".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_create_column".to_string(),
                keys: "c".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_split_in_column".to_string(),
                keys: "v".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_zoom_selected_column".to_string(),
                keys: "z".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_delete_selected".to_string(),
                keys: "d".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_expand_toggle".to_string(),
                keys: "Tab".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_right_nav".to_string(),
                keys: "Space".to_string(),
                args: HashMap::new(),
            },
            Mbc {
                action: "sidebar_left".to_string(),
                keys: "b".to_string(),
                args: HashMap::new(),
            },
        ];
        mode.push(KeyModeConfig {
            name: "sidebar".to_string(),
            trigger: String::new(),
            sticky: true,
            bindings: sidebar_bindings,
        });

        Self {
            prefix: default_prefix_key(),
            bindings,
            unbind: HashMap::new(),
            command: Vec::new(),
            mode,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binding_value_single() {
        let v = BindingValue::Single("h, Left".to_string());
        assert_eq!(v.keys(), vec!["h", "Left"]);
    }

    #[test]
    fn test_binding_value_many() {
        let v = BindingValue::Many(vec!["h".to_string(), "Left".to_string()]);
        assert_eq!(v.keys(), vec!["h", "Left"]);
    }

    #[test]
    fn test_binding_value_empty_ignored() {
        let v = BindingValue::Single("h, , Left".to_string());
        assert_eq!(v.keys(), vec!["h", "Left"]);
    }

    #[test]
    fn test_keys_config_default_prefix() {
        let cfg = KeysConfig::default();
        assert_eq!(cfg.prefix, "ctrl+b");
    }

    #[test]
    fn test_keys_config_has_default_bindings() {
        let cfg = KeysConfig::default();
        assert!(cfg.bindings.contains_key("focus_left"));
        assert!(cfg.bindings.contains_key("split_horizontal"));
        assert!(cfg.bindings.contains_key("zoom_column"));
        assert!(cfg.bindings.contains_key("rename_column"));
        assert!(cfg.bindings.contains_key("close"));
    }

    #[test]
    fn test_keys_config_has_default_sidebar_mode() {
        let cfg = KeysConfig::default();
        let sidebar = cfg
            .mode
            .iter()
            .find(|mode| mode.name == "sidebar")
            .expect("sidebar mode should exist by default");
        assert!(sidebar.bindings.iter().any(|b| b.keys == "j"));
        assert!(sidebar.bindings.iter().any(|b| b.keys == "Space"));
    }
}
