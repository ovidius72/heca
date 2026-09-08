use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
//  BindingValue
// ═══════════════════════════════════════════════════════════════════════════════

/// A single binding value: one combo, or a list of combos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BindingValue {
    Single(String),
    Many(Vec<String>),
}

/// An empty binding — what a `#[derive(Default)]` config struct holding one starts at, and what
/// [`keys`](BindingValue::keys) answers as "no combos" rather than one blank combo.
impl Default for BindingValue {
    fn default() -> Self {
        Self::Single(String::new())
    }
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

// ═══════════════════════════════════════════════════════════════════════════════
//  SurfaceKeysConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// One **surface's** binding layer — `[[keys.surface]]`, spelled `[[keys.component]]` before
/// F003/P082/T416 and still accepted that way.
///
/// **A dock and an overlay are the same thing to the keyboard**: something that holds it for a
/// while and answers keys of its own. The exposé's `x` / `r` / `d` were Rust literals in
/// `chrome/expose.rs` while the workspaces dock's identical `x` came from this file — one rule
/// written twice, and only one of the two rebindable. So the layer is named for what it is.
///
/// The surface is named by the **`name` field**, and an optional **`id`** narrows the entry to one
/// placement. An entry with no `id` applies to every seating of that surface; an entry with one is
/// layered on top for that placement alone, so two mounts of the same component can differ.
///
/// ```toml
/// [[keys.surface]]
/// name         = "expose"       # a layer's name, a provider's kind — whatever holds the keyboard
/// dismiss      = "q,Escape"
/// delete_pane  = "x"
///
/// [[keys.surface]]
/// name      = "docker"          # a FIELD, never the table name
/// restart_selected = "r"        # the surface's own declared action
/// next_pane        = "n"        # …or any EXISTING action id, simply bound here
///
/// [[keys.surface]]
/// name = "docker"
/// id   = "docker.right"         # this placement only, layered over the entry above
/// restart_selected = "R"
///
/// [[keys.surface.bind]]         # the arg-carrying form; merged by `keys`, because arrays are
/// action = "spawn_command"      #   otherwise replaced wholesale and one entry would drop the rest
/// keys   = "t"
/// args   = { command = "lazydocker", float = "true" }
///
/// [keys.surface.unbind]         # explicit removal — never null/empty-string semantics
/// "s" = true
/// ```
///
/// **Why the name is a field.** F003/P085/T355 shipped `[keys.<kind>]`, where the component's name
/// *was* the table name. Nothing under `[keys]` could then be told apart by name — a table might be
/// a component or an action binding — so the parser guessed from the value's **shape**. That breaks
/// the day a component is called `unbind` or `widgets`. An array of tables removes the guess and
/// reads like `[[keys.command]]` and `[[keys.mode]]`, which were already this shape.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SurfaceKeysConfig {
    /// Which surface — matched against a visible layer's name, then a provider's `kind()`.
    pub name: String,
    /// Which placement, when the entry is meant for only one — matched against the mount id.
    /// Absent means every placement of `name`.
    #[serde(default)]
    pub id: Option<String>,
    /// `action = "key"` entries. Merged per key by the ordinary table rules.
    #[serde(flatten)]
    pub bindings: KeybindingMap,
    /// `[[keys.surface.bind]]` — bindings that carry `args`. Merged **by `keys`**: a user entry
    /// with the same combo replaces that default and leaves the others alone. Without that rule the
    /// array would be replaced wholesale and binding one key would silently drop every other.
    #[serde(default)]
    pub bind: Vec<ModeBindingConfig>,
    /// `[keys.surface.unbind]` — combos to remove from this layer, keyed by the **combo**, so it
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
    /// The key(s) for this binding — one combo, several comma-separated, or a list. Also accepts
    /// `key` (singular) for convenience.
    ///
    /// The **same** [`BindingValue`] every other binding table takes, so one action reached by two
    /// keys is one entry wherever it is written: `keys = ["q", "Ctrl+q"]` here reads exactly as it
    /// does in `[keys]` or `[[keys.surface]]`. It was a bare `String` until 2026-08-21, which meant
    /// a mode block needed a whole repeated `[[keys.mode.bindings]]` per key — the same capability
    /// spelled two ways in two tables (Antonio: *"why not `keys = ["q", "Ctrl+q"]`?"*).
    #[serde(alias = "key")]
    pub keys: BindingValue,
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
    /// Flat action bindings (any key not named "prefix", "bind", "command", "mode", "unbind",
    /// "widgets", or "component").
    #[serde(flatten)]
    pub bindings: KeybindingMap,
    /// `[[keys.bind]]` — normal-mode bindings that carry `args`, for the actions a flat
    /// `action = "keys"` line cannot express.
    ///
    /// The flat form is `action = keys`, so it has nowhere to put an argument; this is the same
    /// entry `[[keys.component.bind]]` and `[[keys.mode]]` already use, at the top level where a
    /// global binding lives. Both forms bind the same way and through the same seam — the array is
    /// not a second keymap, it is the flat table with room for `args`.
    ///
    /// ```toml
    /// [[keys.bind]]
    /// keys = "prefix+o"
    /// action = "command_palette"
    /// args = { mode = "pane" }
    /// ```
    #[serde(default)]
    pub bind: Vec<ModeBindingConfig>,
    /// Widget-internal keybindings (`[keys.widgets]`): the generic, cross-widget navigation +
    /// editing vocabulary (`item_next`/`item_previous`, `menu_up`/`menu_down`, `menu_history_up`/`menu_history_down`, `activate`,
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
    /// Per-surface binding layers — `[[keys.surface]]`, consulted only while that surface holds the
    /// keyboard (F003/P082/T416). An entry names its surface in `name` and may narrow itself to one
    /// placement with `id`.
    #[serde(default)]
    pub surface: Vec<SurfaceKeysConfig>,
    /// The former spelling of the field above, `[[keys.component]]` (F003/P086/T362) — accepted
    /// unchanged so existing configs keep working.
    ///
    /// A separate field rather than a serde `alias`, because an alias makes the two spellings
    /// *duplicates of one field*: a file using both — which the bundled defaults do while the
    /// rename settles — would fail to parse. Nothing downstream sees two lists; they are joined by
    /// [`surfaces`](KeysConfig::surfaces), which is the only way either is read.
    #[serde(default)]
    pub component: Vec<SurfaceKeysConfig>,
}

impl KeysConfig {
    /// **Every surface binding layer, whichever way it was spelled.** The one reader of `surface`
    /// and `component`, so a second spelling can never become a second code path — the failure that
    /// AGENTS' ⭐⭐ Rule Zero calls out, and the one two spellings invite.
    ///
    /// `[[keys.surface]]` entries come first, so a `[[keys.component]]` entry for the same name
    /// layers on top of them under the existing "later wins" merge.
    pub fn surfaces(&self) -> impl Iterator<Item = &SurfaceKeysConfig> {
        self.surface.iter().chain(self.component.iter())
    }
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

    /// `[[keys.component]]` carries the component's name as a **field**, so nothing under `[keys]`
    /// is interpreted by its table name any more (F003/P086/T362).
    #[test]
    fn a_component_layer_names_itself_in_a_field() {
        let src = r#"
prefix = "ctrl+b"
focus_left = "prefix+h"
paste_clipboard = ["Super+v", "Ctrl+Shift+v"]

[[keys.component]]
name = "docker"
restart_selected = "r"
stop_selected = "s"

[[keys.component.bind]]
action = "spawn_command"
keys = "t"
args = { command = "lazydocker" }

[keys.component.unbind]
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
        assert!(
            !keys.bindings.contains_key("docker"),
            "a component no longer occupies a name in the flat binding map",
        );

        let docker = &keys.component[0];
        assert_eq!(docker.name, "docker");
        assert_eq!(docker.id, None, "no id ⇒ every placement");
        assert_eq!(docker.bindings["restart_selected"].keys(), vec!["r"]);
        assert_eq!(docker.bindings["stop_selected"].keys(), vec!["s"]);
        assert_eq!(docker.bind.len(), 1);
        assert_eq!(docker.bind[0].action, "spawn_command");
        assert_eq!(docker.bind[0].keys.keys(), vec!["t"]);
        assert_eq!(docker.bind[0].args["command"], "lazydocker");
        assert!(docker.unbind["s"]);
    }

    /// The reason for the array form: a second entry with an `id` speaks for one placement only.
    #[test]
    fn an_id_narrows_an_entry_to_one_placement() {
        let src = r#"
[[keys.component]]
name = "workspaces"
next_item = "j"

[[keys.component]]
name = "workspaces"
id = "workspaces.right"
next_item = "n"
"#;
        let wrapper: HashMap<String, KeysConfig> =
            toml::from_str(&format!("[keys]\n{src}")).expect("parses");
        let component = &wrapper["keys"].component;

        assert_eq!(component.len(), 2, "both entries survive — an array, not a table");
        assert_eq!(component[0].id, None);
        assert_eq!(component[1].id.as_deref(), Some("workspaces.right"));
        assert_eq!(
            component[0].name, component[1].name,
            "the same component named twice is exactly what the id is for",
        );
    }

    /// A component may be called `unbind` or `widgets` — the names that would have collided with a
    /// config keyword under the old `[keys.<kind>]` shape.
    #[test]
    fn a_component_may_take_a_name_that_is_also_a_config_keyword() {
        let src = r#"
[keys.unbind]
"prefix+w" = true

[[keys.component]]
name = "unbind"
do_thing = "u"
"#;
        let wrapper: HashMap<String, KeysConfig> =
            toml::from_str(&format!("[keys]\n{src}")).expect("parses");
        let keys = &wrapper["keys"];

        assert!(keys.unbind["prefix+w"], "the real [keys.unbind] is untouched");
        assert_eq!(keys.component[0].name, "unbind");
        assert_eq!(keys.component[0].bindings["do_thing"].keys(), vec!["u"]);
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

    /// The workspaces dock's keys are a **surface layer** now, not a mode. The `sidebar` mode and
    /// its twelve `sidebar_*` actions are gone — a container is driven because it has focus, not
    /// because the app entered a state.
    ///
    /// Read through [`surfaces`](KeysConfig::surfaces), never one of the two fields behind it:
    /// asking `component` alone made this test an assertion about which *spelling* the bundled file
    /// happened to use, and it went red the day the defaults moved to `[[keys.surface]]` — with
    /// nothing actually broken.
    #[test]
    fn the_workspaces_dock_ships_a_surface_layer_not_a_mode() {
        let cfg = KeysConfig::default();
        assert!(
            !cfg.mode.iter().any(|m| m.name == "sidebar"),
            "the sidebar mode went with the built-ins it drove",
        );
        let ws = cfg
            .surfaces()
            .find(|c| c.name == "workspaces")
            .expect("the workspaces dock ships its keys here");
        // One spelling per key: `Down` and `ArrowDown` are the same physical key (both parse to
        // `GridKey::ArrowDown`), so binding both bound it twice and every surface that renders the
        // shortcut drew the same cap twice (2026-07-30).
        assert_eq!(ws.bindings["cursor_down"].keys(), vec!["j", "ArrowDown"]);
        assert_eq!(ws.bindings["peek_selected"].keys(), vec!["Space"]);
        assert_eq!(
            ws.bindings["global_focus"].keys(),
            vec!["prefix+e"],
            "the keystroke `sidebar_focus` used to own, now a plain container key",
        );
    }
}
