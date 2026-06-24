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
    #[serde(alias = "keys")]
    pub key: String,
    pub command: String,
    #[serde(default = "default_command_kind", alias = "command_type")]
    pub kind: String,
    #[serde(default)]
    pub float: bool,
    #[serde(default)]
    pub close_pane: bool,
    #[serde(default)]
    pub keep_on_error: bool,
    #[serde(default)]
    pub keep_on_success: bool,
}

fn default_command_kind() -> String {
    "terminal".to_string()
}

impl CommandKeybindConfig {
    pub fn validate(&self) -> Result<(), String> {
        match self.kind.as_str() {
            "terminal" | "app" | "plugin" => Ok(()),
            other => Err(format!(
                "invalid [[keys.command]].kind '{other}' (expected one of: terminal, app, plugin)"
            )),
        }
    }
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
    /// The default keybindings are parsed from the embedded
    /// `keybindings.default.toml` — that file is the single source of truth.
    fn default() -> Self {
        crate::loader::parse_default_keys()
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
        assert!(cfg.bindings.contains_key("move_pane_to_column_pick"));
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
