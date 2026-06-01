//! Keymap registry — per-mode key binding resolution.
//!
//! Replaces the monolithic `KeyBindings` struct with a clean separation
//! between key representation (`KeyCombo`) and mode-specific maps.

use std::collections::HashMap;
use crate::input::WmAction;

/// Normalized representation of a key press.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_: bool,
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
                _ => key = part.to_lowercase(),
            }
        }
        Self { key, ctrl, shift, alt, super_ }
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
            && let Some(action) = map.remove(old) {
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
}
