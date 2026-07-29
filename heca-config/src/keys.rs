use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
//  BindingValue
// ═══════════════════════════════════════════════════════════════════════════════

/// A single binding value: one combo, a list of combos, or — when the key under `[keys]` names a
/// **component kind** rather than an action — that component's whole binding layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BindingValue {
    Single(String),
    Many(Vec<String>),
    /// `[keys.docker]` — the bindings that apply **only while a component of that kind holds chrome
    /// focus**. A table, not an array, so it merges per key: overriding one binding keeps the rest
    /// (F003/P085/T355).
    ///
    /// It lives in the same map as the flat action bindings because that is the shape the user
    /// chose — `[keys.docker]`, not `[keys.component.docker]` — and a TOML table under `[keys]`
    /// cannot be told apart from an action binding by its name alone. The app separates them by
    /// **variant**: a table is a component layer, a string or list is an action binding.
    Component(ComponentKeysConfig),
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
            // A component layer binds nothing at the top level; its own entries do.
            BindingValue::Component(_) => Vec::new(),
        }
    }

    /// The component layer this value carries, if it is one.
    pub fn component(&self) -> Option<&ComponentKeysConfig> {
        match self {
            BindingValue::Component(c) => Some(c),
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  ComponentKeysConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// One component **kind's** binding layer — `[keys.<kind>]`.
///
/// Bindings belong to the *type*, not to a placement: write them once and every seating of that
/// component uses them, while cursor / scroll / focus stay per mount.
///
/// ```toml
/// [keys.docker]                 # a TABLE: merges per key, so overriding one keeps the rest
/// restart_selected = "r"        # the component's own declared action
/// next_pane        = "n"        # …or any EXISTING action id, simply bound here
///
/// [[keys.docker.bind]]          # the arg-carrying form; merged by `keys`, because arrays are
/// action = "spawn_command"      #   otherwise replaced wholesale and one entry would drop the rest
/// keys   = "t"
/// args   = { command = "lazydocker", float = "true" }
///
/// [keys.docker.unbind]          # explicit removal — never null/empty-string semantics
/// "s" = true
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ComponentKeysConfig {
    /// `action = "key"` entries. Merged per key by the ordinary table rules.
    #[serde(flatten)]
    pub bindings: KeybindingMap,
    /// `[[keys.<kind>.bind]]` — bindings that carry `args`. Merged **by `keys`**: a user entry with
    /// the same combo replaces that default and leaves the others alone. Without that rule the
    /// array would be replaced wholesale and binding one key would silently drop every other.
    #[serde(default)]
    pub bind: Vec<ModeBindingConfig>,
    /// `[keys.<kind>.unbind]` — combos to remove from this layer, keyed by the **combo**, so it
    /// retires a binding whatever it points at.
    #[serde(default)]
    pub unbind: HashMap<String, bool>,
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
    /// Flat action bindings (any key not named "prefix", "command", "mode", "unbind", or
    /// "widgets").
    #[serde(flatten)]
    pub bindings: KeybindingMap,
    /// Widget-internal keybindings (`[keys.widgets]`): the generic, cross-widget navigation +
    /// editing vocabulary (`item_next`/`item_previous`, `menu_up`/`menu_down`, `activate`,
    /// `dismiss`, `edit_*`) that the app resolves into a `heca_grid_ui::Keymap`. Applies only
    /// while an interactive widget/overlay is focused; never hijacks normal-mode input.
    #[serde(default)]
    pub widgets: KeybindingMap,
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

    /// `[keys.<kind>]` parses as a **component layer** and sits in the same map as the flat action
    /// bindings, told apart by its variant rather than by its name (F003/P085/T355).
    #[test]
    fn a_named_table_under_keys_is_a_component_layer() {
        let src = r#"
prefix = "ctrl+b"
focus_left = "prefix+h"
paste_clipboard = ["Super+v", "Ctrl+Shift+v"]

[keys.docker]
restart_selected = "r"
stop_selected = "s"

[[keys.docker.bind]]
action = "spawn_command"
keys = "t"
args = { command = "lazydocker" }

[keys.docker.unbind]
"s" = true
"#;
        // The embedded shape is `[keys]` at the top level of the file.
        let wrapper: HashMap<String, KeysConfig> =
            toml::from_str(&format!("[keys]\n{src}")).expect("parses");
        let keys = &wrapper["keys"];

        assert_eq!(
            keys.bindings["focus_left"].keys(),
            vec!["prefix+h"],
            "a flat action binding is unaffected",
        );
        assert_eq!(keys.bindings["paste_clipboard"].keys().len(), 2);

        let docker = keys.bindings["docker"]
            .component()
            .expect("a table under [keys] is a component layer");
        assert_eq!(docker.bindings["restart_selected"].keys(), vec!["r"]);
        assert_eq!(docker.bindings["stop_selected"].keys(), vec!["s"]);
        assert_eq!(docker.bind.len(), 1);
        assert_eq!(docker.bind[0].action, "spawn_command");
        assert_eq!(docker.bind[0].keys, "t");
        assert_eq!(docker.bind[0].args["command"], "lazydocker");
        assert!(docker.unbind["s"]);
        assert!(
            keys.bindings["docker"].keys().is_empty(),
            "a layer binds nothing at the top level; its own entries do",
        );
    }

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
