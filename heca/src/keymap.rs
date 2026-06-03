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

/// Registry that maps key combinations to actions, organized by input mode.
pub struct KeymapRegistry {
    modes: HashMap<String, HashMap<KeyCombo, WmAction>>,
}

impl KeymapRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            modes: HashMap::new(),
        }
    }

    /// Bind a key combination to an action in a given mode.
    pub fn bind(&mut self, mode: &str, combo: KeyCombo, action: WmAction) {
        self.modes
            .entry(mode.to_string())
            .or_default()
            .insert(combo, action);
    }

    /// Remove a binding from a mode.
    // Transitional: will be used for config reload / RPC in Phase 5.
    #[allow(dead_code)]
    pub fn unbind(&mut self, mode: &str, combo: &KeyCombo) -> Option<WmAction> {
        self.modes.get_mut(mode)?.remove(combo)
    }

    /// Rebind an existing key to a new key within the same mode.
    // Transitional: will be used for config reload / RPC in Phase 5.
    #[allow(dead_code)]
    pub fn rebind(&mut self, mode: &str, old: &KeyCombo, new: KeyCombo) {
        if let Some(map) = self.modes.get_mut(mode)
            && let Some(action) = map.remove(old)
        {
            map.insert(new, action);
        }
    }

    /// Look up the action bound to `combo` in `mode`.
    pub fn resolve(&self, mode: &str, combo: &KeyCombo) -> Option<&WmAction> {
        self.modes.get(mode)?.get(combo)
    }

    /// Return all bindings for a mode.
    // Transitional: will be used for config reload / RPC in Phase 5.
    #[allow(dead_code)]
    pub fn bindings_in_mode(&self, mode: &str) -> Option<&HashMap<KeyCombo, WmAction>> {
        self.modes.get(mode)
    }

    /// Check whether a mode exists (has any bindings).
    // Transitional: will be used for config reload / RPC in Phase 5.
    #[allow(dead_code)]
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
        reg.bind("normal", combo.clone(), action.clone());
        assert_eq!(reg.resolve("normal", &combo), Some(&action));
        assert_eq!(reg.resolve("sidebar", &combo), None);
    }

    #[test]
    fn test_unbind() {
        let mut reg = KeymapRegistry::new();
        let combo = KeyCombo::parse("x");
        reg.bind("normal", combo.clone(), WmAction::ClosePane);
        assert!(reg.resolve("normal", &combo).is_some());
        reg.unbind("normal", &combo);
        assert!(reg.resolve("normal", &combo).is_none());
    }

    #[test]
    fn test_rebind() {
        let mut reg = KeymapRegistry::new();
        let old = KeyCombo::parse("h");
        let new = KeyCombo::parse("Left");
        reg.bind("normal", old.clone(), WmAction::FocusLeft);
        reg.rebind("normal", &old, new.clone());
        assert!(reg.resolve("normal", &old).is_none());
        assert_eq!(reg.resolve("normal", &new), Some(&WmAction::FocusLeft));
    }

    #[test]
    fn test_multiple_modes() {
        let mut reg = KeymapRegistry::new();
        let combo = KeyCombo::parse("j");
        reg.bind("normal", combo.clone(), WmAction::FocusDown);
        reg.bind("sidebar", combo.clone(), WmAction::SidebarDown);
        assert_eq!(reg.resolve("normal", &combo), Some(&WmAction::FocusDown));
        assert_eq!(reg.resolve("sidebar", &combo), Some(&WmAction::SidebarDown));
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
        reg.bind("normal", config_combo, WmAction::SplitHorizontal);
        assert_eq!(
            reg.resolve("normal", &event_combo),
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
        reg.bind("normal", config_combo, WmAction::SwapPane);
        assert_eq!(
            reg.resolve("normal", &event_combo),
            Some(&WmAction::SwapPane)
        );
    }
}
