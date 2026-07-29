//! Keymap registry — per-mode key binding resolution.
//!
//! Replaces the monolithic `KeyBindings` struct with a clean separation
//! between key representation (`KeyCombo`) and mode-specific maps.

use crate::input::WmAction;
use std::collections::HashMap;

/// Normalized representation of a key press.
/// Equality and hashing are case-insensitive on the `key` field so that
/// "Enter" and "enter" match the same binding.
#[derive(Clone, Debug, Eq)]
pub struct KeyCombo {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_: bool,
}

impl PartialEq for KeyCombo {
    fn eq(&self, other: &Self) -> bool {
        self.ctrl == other.ctrl
            && self.shift == other.shift
            && self.alt == other.alt
            && self.super_ == other.super_
            && self.key.eq_ignore_ascii_case(&other.key)
    }
}

impl std::hash::Hash for KeyCombo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ctrl.hash(state);
        self.shift.hash(state);
        self.alt.hash(state);
        self.super_.hash(state);
        self.key.to_lowercase().hash(state);
    }
}

impl KeyCombo {
    /// Parse a key string like "h", "Ctrl+h", "Ctrl+Shift+l", "Space".
    /// Character keys are lowercased so "Q" and "q" match the same binding.
    pub fn parse(s: &str) -> Self {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut super_ = false;
        let mut key = String::new();
        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                "super" | "win" | "cmd" => super_ = true,
                _ => {
                    let k = part.to_lowercase();
                    // Map shifted symbols to their base key + set shift flag.
                    // This lets config write `ctrl+{` instead of `ctrl+shift+[`.
                    let (base, is_shifted) = match k.as_str() {
                        "{" => ("[", true),
                        "}" => ("]", true),
                        ":" => (";", true),
                        "\"" => ("'", true),
                        "<" => (",", true),
                        ">" => (".", true),
                        "?" => ("/", true),
                        "+" => ("=", true),
                        "_" => ("-", true),
                        "|" => ("\\", true),
                        "!" => ("1", true),
                        "@" => ("2", true),
                        "#" => ("3", true),
                        "$" => ("4", true),
                        "%" => ("5", true),
                        "^" => ("6", true),
                        "&" => ("7", true),
                        "*" => ("8", true),
                        "(" => ("9", true),
                        ")" => ("0", true),
                        "~" => ("`", true),
                        _ => (k.as_str(), false),
                    };
                    key = base.to_string();
                    if is_shifted {
                        shift = true;
                    }
                }
            }
        }
        Self {
            key,
            ctrl,
            shift,
            alt,
            super_,
        }
    }
}

/// **Every resolved keymap**, as one thing — the layers a keypress is matched against, in the order
/// the input path consults them.
///
/// They are grouped because they are one artefact with one lifetime: all four are built from the
/// same config at load and rebuilt together on `prefix+Shift+r`, and every consumer that needs one
/// needs several. Threading them as four parameters had already pushed the window-event entry point
/// past what one function should take.
pub struct Keymaps {
    /// Prefix (`normal`) and direct (`global`) bindings — `[keys]`.
    pub flat: KeymapRegistry,
    /// One per custom input mode — `[[keys.mode]]`, plus the built-in `resize` / `sidebar` /
    /// `selection` / `focus` layers.
    pub modes: HashMap<String, KeymapRegistry>,
    /// One per component or narrowed placement — `[[keys.component]]`, consulted only while that
    /// component holds chrome focus (F003/P086/T362).
    pub components: HashMap<String, KeymapRegistry>,
    /// Mode name → (trigger combo, sticky). A mode entered by focus rather than a key has none.
    pub triggers: HashMap<String, (KeyCombo, bool)>,
    /// **What key runs this action**, reversed out of the maps above (F003/P086/T366).
    pub by_action: BindingIndex,
}

/// One place a key is bound, named the way the user's file names it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundKey {
    /// Which layer — `[keys]`, `[[keys.mode]] sidebar`, `[[keys.component]] workspaces`, `plugin`.
    pub layer: String,
    /// The combo exactly as it is typed, `prefix+` and all.
    pub key: String,
}

/// Action id → every key bound to it, across **every** layer (F003/P086/T366).
///
/// **Why this and not the config file.** Reading `[keys]` answers for the flat map alone: it cannot
/// see a `[[keys.mode]]` binding, cannot see a `[[keys.component]]` one, and can never see a key a
/// plugin registered at runtime — which is exactly what the shortcut lookup used to do. This index
/// is filled while each layer is built, so it sees all of them uniformly, and is rebuilt with them.
///
/// It replaced `ActionMeta::default_binding`, which held a *second* copy of every key already in
/// `keybindings.default.toml` — one nothing compared against, so the two could drift silently.
pub type BindingIndex = std::collections::BTreeMap<String, Vec<BoundKey>>;

/// Record that `action` answers to `key` in `layer`, keeping the list free of duplicates.
pub fn index_binding(index: &mut BindingIndex, action: &str, layer: &str, key: &str) {
    let bound = BoundKey {
        layer: layer.to_string(),
        key: key.to_string(),
    };
    let entry = index.entry(action.to_string()).or_default();
    if !entry.contains(&bound) {
        entry.push(bound);
    }
}

/// What a key is bound to — a built-in action, or a **name-keyed** one resolved at press time.
///
/// **The constraint that shapes this** (plugin-04 G3): config is loaded *before* providers and
/// plugins register their actions. So a binding to `plugin.docker.restart` is **not resolvable at
/// load** — the action does not exist yet. An unknown name at load is therefore **not a config
/// error**; it becomes a [`Dynamic`](ActionRef::Dynamic) reference that resolves when the key is
/// actually pressed.
#[derive(Clone, Debug, PartialEq)]
pub enum ActionRef {
    /// Resolved at **load** (`action_from_name` / `build_action`) — today's behaviour exactly: the
    /// same arg-parsing errors surface at load, and a press costs nothing.
    Builtin(WmAction),
    /// Resolved at **press** — an `Intent { action, args }` naming an action that may not be
    /// registered yet (or ever). Still unknown when pressed ⇒ a debug warning, never a crash.
    Dynamic(crate::chrome::Intent),
}

/// Registry that maps key combinations to actions, organized by input mode.
pub struct KeymapRegistry {
    modes: HashMap<String, HashMap<KeyCombo, ActionRef>>,
}

impl KeymapRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            modes: HashMap::new(),
        }
    }

    /// Bind a key combination to an action in a given mode.
    pub fn bind(&mut self, mode: &str, combo: KeyCombo, action: ActionRef) {
        self.modes
            .entry(mode.to_string())
            .or_default()
            .insert(combo, action);
    }

    /// Remove a binding from a mode. Keyed by the **combo**, so `[keys.unbind]` retires a binding
    /// whatever it points at — a built-in or a dynamic action id alike.
    pub fn unbind(&mut self, mode: &str, combo: &KeyCombo) -> Option<ActionRef> {
        self.modes.get_mut(mode)?.remove(combo)
    }

    /// Rebind an existing key to a new key within the same mode.
    // Transitional: will be used for config reload / RPC in Phase 5.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by tests and reserved for config reload and RPC workflows")
    )]
    pub fn rebind(&mut self, mode: &str, old: &KeyCombo, new: KeyCombo) {
        if let Some(map) = self.modes.get_mut(mode)
            && let Some(action) = map.remove(old)
        {
            map.insert(new, action);
        }
    }

    /// Look up what `combo` is bound to in `mode`.
    pub fn resolve(&self, mode: &str, combo: &KeyCombo) -> Option<&ActionRef> {
        self.modes.get(mode)?.get(combo)
    }

    /// Look up `combo` and return it only if it is bound to a **built-in** — i.e. the binding was
    /// resolved at config load. `None` when unbound *or* when it points at a name-keyed action that
    /// resolves at press time.
    ///
    /// This is the introspection question "which `WmAction` does this key run?", which only has an
    /// answer for built-ins; a dynamic binding's answer depends on what is registered at the moment
    /// it is pressed.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by tests and reserved for config reload and RPC workflows")
    )]
    pub fn resolve_builtin(&self, mode: &str, combo: &KeyCombo) -> Option<&WmAction> {
        match self.resolve(mode, combo)? {
            ActionRef::Builtin(a) => Some(a),
            ActionRef::Dynamic(_) => None,
        }
    }

    /// Return all bindings for a mode.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn bindings_in_mode(&self, mode: &str) -> Option<&HashMap<KeyCombo, ActionRef>> {
        self.modes.get(mode)
    }

    /// Check whether a mode exists (has any bindings).
    // Transitional: will be used for config reload / RPC in Phase 5.
    #[expect(dead_code, reason = "used by tests and reserved for config reload and RPC workflows")]
    pub fn has_mode(&self, mode: &str) -> bool {
        self.modes.contains_key(mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_combo_parse() {
        let c = KeyCombo::parse("h");
        assert_eq!(c.key, "h");
        assert!(!c.ctrl);
        assert!(!c.shift);

        let c = KeyCombo::parse("Ctrl+Shift+l");
        assert_eq!(c.key, "l");
        assert!(c.ctrl);
        assert!(c.shift);
        assert!(!c.alt);
    }

    #[test]
    fn test_bind_and_resolve() {
        let mut reg = KeymapRegistry::new();
        let combo = KeyCombo::parse("h");
        let action = WmAction::FocusLeft;
        reg.bind("normal", combo.clone(), ActionRef::Builtin(action.clone()));
        assert_eq!(reg.resolve_builtin("normal", &combo), Some(&action));
        assert_eq!(reg.resolve_builtin("sidebar", &combo), None);
    }

    #[test]
    fn test_unbind() {
        let mut reg = KeymapRegistry::new();
        let combo = KeyCombo::parse("x");
        reg.bind("normal", combo.clone(), ActionRef::Builtin(WmAction::ClosePane));
        assert!(reg.resolve_builtin("normal", &combo).is_some());
        reg.unbind("normal", &combo);
        assert!(reg.resolve_builtin("normal", &combo).is_none());
    }

    #[test]
    fn test_rebind() {
        let mut reg = KeymapRegistry::new();
        let old = KeyCombo::parse("h");
        let new = KeyCombo::parse("Left");
        reg.bind("normal", old.clone(), ActionRef::Builtin(WmAction::FocusLeft));
        reg.rebind("normal", &old, new.clone());
        assert!(reg.resolve_builtin("normal", &old).is_none());
        assert_eq!(reg.resolve_builtin("normal", &new), Some(&WmAction::FocusLeft));
    }

    #[test]
    fn test_multiple_modes() {
        let mut reg = KeymapRegistry::new();
        let combo = KeyCombo::parse("j");
        reg.bind("normal", combo.clone(), ActionRef::Builtin(WmAction::FocusDown));
        reg.bind("selection", combo.clone(), ActionRef::Builtin(WmAction::FocusUp));
        assert_eq!(reg.resolve_builtin("normal", &combo), Some(&WmAction::FocusDown));
        assert_eq!(reg.resolve_builtin("selection", &combo), Some(&WmAction::FocusUp));
    }

    #[test]
    fn test_case_insensitive_named_key() {
        // Config stores "enter" (lowercased by parse), event sends "Enter".
        let mut reg = KeymapRegistry::new();
        let config_combo = KeyCombo::parse("Enter");
        let event_combo = KeyCombo {
            key: "Enter".to_string(),
            ctrl: false,
            shift: false,
            alt: false,
            super_: false,
        };
        reg.bind("normal", config_combo, ActionRef::Builtin(WmAction::SplitHorizontal));
        assert_eq!(
            reg.resolve_builtin("normal", &event_combo),
            Some(&WmAction::SplitHorizontal)
        );
    }

    #[test]
    fn test_shift_q_matches_lowercase_q() {
        // Shift+Q produces "Q" but config stores "q".
        let mut reg = KeymapRegistry::new();
        let config_combo = KeyCombo::parse("Shift+q");
        let event_combo = KeyCombo {
            key: "Q".to_string(),
            ctrl: false,
            shift: true,
            alt: false,
            super_: false,
        };
        reg.bind("normal", config_combo, ActionRef::Builtin(WmAction::SwapPane));
        assert_eq!(
            reg.resolve_builtin("normal", &event_combo),
            Some(&WmAction::SwapPane)
        );
    }
}
