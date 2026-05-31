use std::collections::HashMap;
use heca_config::theme::AppConfig;
use winit::keyboard::NamedKey;

/// Target for resize actions.
// Variants are constructed in tests and will be used by the RPC parser in Phase 4.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeTarget {
    Column,
    Pane,
}

/// Window-manager action.
///
/// Unit variants are used for keybindings (no arguments).
/// Parameterized variants are used for RPC commands and direct invocation
/// (e.g. from the command palette or mouse handlers).
// Note: WmAction does NOT derive Hash because f64 fields in parameterized
// variants do not implement Hash. The registry uses discriminant-based
// dispatch, so Hash is unnecessary.
// Parameterized variants are currently only constructed in tests and via RPC
// (Phase 4). The `dead_code` lint fires on the binary; suppress it until
// the ActionRegistry wires them in (Phase 2).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum WmAction {
    // ── Navigation (unit) ──
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    NextPane,
    PrevPane,
    WorkspaceNext,
    WorkspacePrev,
    FocusToggleLocal,
    FocusToggleGlobal,

    // ── Navigation (parameterized) ──
    FocusPane { pane_id: u64 },
    FocusWorkspace { ws_idx: usize },

    // ── Layout (unit) ──
    SplitHorizontal,
    SplitVertical,
    ResizeIncrease,
    ResizeDecrease,
    PaneHeightIncrease,
    PaneHeightDecrease,
    SwapLeft,
    SwapRight,
    SwapUp,
    SwapDown,
    MovePaneLeft,
    MovePaneRight,

    // ── Layout (parameterized) ──
    Swap { a_id: u64, b_id: u64 },
    Move { pane_id: u64, target_col: usize },
    Resize { target: ResizeTarget, delta: i32 },
    ResizeTo { target: ResizeTarget, width: f64, height: f64 },

    // ── Pane (unit) ──
    Float,
    ClosePane,
    PaneSelect,
    SwapSelect,
    SwapAndFocus,
    RenamePane,

    // ── Pane (parameterized) ──
    FloatAt { pane_id: u64, x: f64, y: f64, width: f64, height: f64 },
    ClosePaneById { pane_id: u64 },
    RenameTarget { pane_id: u64, name: String },

    // ── Workspace (unit) ──
    CreateWorkspace,
    RenameWorkspace,

    // ── Sidebar / Chrome (unit) ──
    SidebarLeft,
    SidebarRight,
    SidebarFocus,
    SidebarUp,
    SidebarDown,
    SidebarLeftNav,
    SidebarRightNav,
    SidebarExpandToggle,
    TabNext,
    TabPrev,

    // ── System ──
    CommandPalette,
}

/// Return the discriminant of a `WmAction`.
/// Two instances share a discriminant iff they are the same variant,
/// regardless of field values.
// Used by ActionRegistry (Phase 2) for handler dispatch.
#[allow(dead_code)]
pub fn action_discriminant(action: &WmAction) -> std::mem::Discriminant<WmAction> {
    std::mem::discriminant(action)
}

/// Map a config key name to its unit `WmAction` variant.
/// Parameterized variants are not reachable from config — they are
/// constructed programmatically (RPC, mouse handlers, command palette).
fn action_from_name(name: &str) -> Option<WmAction> {
    match name {
        "focus_left" => Some(WmAction::FocusLeft),
        "focus_right" => Some(WmAction::FocusRight),
        "focus_up" => Some(WmAction::FocusUp),
        "focus_down" => Some(WmAction::FocusDown),
        "split_horizontal" => Some(WmAction::SplitHorizontal),
        "split_vertical" => Some(WmAction::SplitVertical),
        "float" => Some(WmAction::Float),
        "close" => Some(WmAction::ClosePane),
        "tab_next" => Some(WmAction::TabNext),
        "tab_prev" => Some(WmAction::TabPrev),
        "resize_increase" => Some(WmAction::ResizeIncrease),
        "resize_decrease" => Some(WmAction::ResizeDecrease),
        "sidebar_left" => Some(WmAction::SidebarLeft),
        "sidebar_right" => Some(WmAction::SidebarRight),
        "sidebar_focus" => Some(WmAction::SidebarFocus),
        "sidebar_up" => Some(WmAction::SidebarUp),
        "sidebar_down" => Some(WmAction::SidebarDown),
        "sidebar_left_nav" => Some(WmAction::SidebarLeftNav),
        "sidebar_right_nav" => Some(WmAction::SidebarRightNav),
        "sidebar_expand_toggle" => Some(WmAction::SidebarExpandToggle),
        "next_pane" => Some(WmAction::NextPane),
        "prev_pane" => Some(WmAction::PrevPane),
        "pane_select" => Some(WmAction::PaneSelect),
        "swap_select" => Some(WmAction::SwapSelect),
        "swap_and_focus" => Some(WmAction::SwapAndFocus),
        "swap_left" => Some(WmAction::SwapLeft),
        "swap_right" => Some(WmAction::SwapRight),
        "swap_up" => Some(WmAction::SwapUp),
        "swap_down" => Some(WmAction::SwapDown),
        "move_pane_left" => Some(WmAction::MovePaneLeft),
        "move_pane_right" => Some(WmAction::MovePaneRight),
        "pane_height_increase" => Some(WmAction::PaneHeightIncrease),
        "pane_height_decrease" => Some(WmAction::PaneHeightDecrease),
        "workspace_next" => Some(WmAction::WorkspaceNext),
        "focus_toggle_local" => Some(WmAction::FocusToggleLocal),
        "focus_toggle_global" => Some(WmAction::FocusToggleGlobal),
        "workspace_prev" => Some(WmAction::WorkspacePrev),
        "create_workspace" => Some(WmAction::CreateWorkspace),
        "rename_workspace" => Some(WmAction::RenameWorkspace),
        "rename_pane" => Some(WmAction::RenamePane),
        "command_palette" => Some(WmAction::CommandPalette),
        _ => None,
    }
}

/// Binding priority: lower = checked first. Focus wins over resize on conflicts.
fn action_priority(action: &WmAction) -> u8 {
    match action {
        // Navigation (highest priority)
        WmAction::FocusLeft | WmAction::FocusRight |
        WmAction::FocusUp | WmAction::FocusDown |
        WmAction::NextPane | WmAction::PrevPane => 0,
        // Sidebar navigation (only used in sidebar mode via resolve_mode)
        // Low priority so they don't override focus bindings in normal/prefix mode.
        WmAction::SidebarFocus => 0,
        WmAction::SidebarUp | WmAction::SidebarDown |
        WmAction::SidebarLeftNav | WmAction::SidebarRightNav |
        WmAction::SidebarExpandToggle => 4,
        // Pane management
        WmAction::SplitHorizontal | WmAction::SplitVertical |
        WmAction::Float | WmAction::ClosePane |
        WmAction::PaneSelect | WmAction::SwapSelect | WmAction::SwapAndFocus |
        WmAction::FocusToggleLocal | WmAction::FocusToggleGlobal |
        WmAction::CreateWorkspace | WmAction::RenameWorkspace |
        WmAction::RenamePane | WmAction::WorkspaceNext |
        WmAction::WorkspacePrev => 1,
        // Swap
        WmAction::SwapLeft | WmAction::SwapRight |
        WmAction::SwapUp | WmAction::SwapDown |
        WmAction::MovePaneLeft | WmAction::MovePaneRight => 2,
        // Resize (lowest priority — checked last)
        WmAction::ResizeIncrease | WmAction::ResizeDecrease |
        WmAction::PaneHeightIncrease | WmAction::PaneHeightDecrease => 3,
        // Tabs and sidebars
        WmAction::TabNext | WmAction::TabPrev |
        WmAction::SidebarLeft | WmAction::SidebarRight => 4,
        // System
        WmAction::CommandPalette => 5,
        // Parameterized variants are not resolved from keybindings,
        // but we still match them explicitly to avoid catch-all.
        WmAction::FocusPane { .. }
        | WmAction::FocusWorkspace { .. }
        | WmAction::Swap { .. }
        | WmAction::Move { .. }
        | WmAction::Resize { .. }
        | WmAction::ResizeTo { .. }
        | WmAction::FloatAt { .. }
        | WmAction::ClosePaneById { .. }
        | WmAction::RenameTarget { .. } => 6,
    }
}

#[derive(Clone, Debug)]
struct Binding {
    action: WmAction,
    key: String,
    ctrl: bool,
    shift: bool,
}

pub struct KeyBindings {
    bindings: Vec<Binding>,
    /// Mode-specific bindings: mode_name -> Vec<Binding>
    /// Each mode has its own keybinding set, resolved by resolve_mode().
    mode_bindings: HashMap<String, Vec<Binding>>,
}

impl KeyBindings {
    pub fn load(app_config: &AppConfig) -> Self {
        let mut bindings = Vec::new();
        for (name, key_str) in &app_config.config.keybindings {
            if let Some(action) = action_from_name(name) {
                for part in key_str.split(',') {
                    let part = part.trim();
                    if part.is_empty() { continue; }
                    let (ctrl, shift, key) = Self::parse_key(part);
                    bindings.push(Binding { action: action.clone(), key, ctrl, shift });
                }
            }
        }
        // Sort by priority so focus is checked before resize on conflicts
        bindings.sort_by_key(|b| action_priority(&b.action));

        // ── Load mode-specific bindings ──
        let mut mode_bindings = HashMap::new();
        // Sidebar mode: single-key bindings (no prefix required within the mode)
        let sidebar_entries: Vec<(&str, &str)> = vec![
            ("sidebar_down", "j"),
            ("sidebar_up", "k"),
            ("sidebar_left_nav", "h"),
            ("sidebar_right_nav", "l,Enter"),
            ("sidebar_expand_toggle", "Tab,Space"),
            ("sidebar_left", "b"),
        ];
        let mut sidebar_bindings = Vec::new();
        for (name, key_str) in sidebar_entries {
            if let Some(action) = action_from_name(name) {
                for part in key_str.split(',') {
                    let part = part.trim();
                    if part.is_empty() { continue; }
                    let (ctrl, shift, key) = Self::parse_key(part);
                    sidebar_bindings.push(Binding { action: action.clone(), key, ctrl, shift });
                }
            }
        }
        mode_bindings.insert("sidebar".to_string(), sidebar_bindings);

        Self { bindings, mode_bindings }
    }

    /// Parse a key string like "h", "H", "Ctrl+h", "Ctrl+Shift+l", "Space".
    /// Preserves key case exactly; shift ONLY from explicit "Shift+" modifier.
    fn parse_key(s: &str) -> (bool, bool, String) {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        let mut ctrl = false;
        let mut shift = false;
        let mut key = String::new();
        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" => ctrl = true,
                "shift" => shift = true,
                _ => key = part.to_string(),
            }
        }
        (ctrl, shift, key)
    }

    pub fn resolve(
        &self,
        key_text: &str,
        ctrl: bool,
        shift: bool,
        named: &winit::keyboard::Key,
        phys: &winit::keyboard::PhysicalKey,
    ) -> Option<WmAction> {
        let key = key_text.to_string();
        let named_key = match named {
            winit::keyboard::Key::Named(NamedKey::Space) => "Space".to_string(),
            winit::keyboard::Key::Named(n) => format!("{:?}", n),
            _ => key.clone(),
        };

        let phys_name = match phys {
            winit::keyboard::PhysicalKey::Code(c) => {
                let s = format!("{:?}", c);
                s.strip_prefix("Key").unwrap_or(&s).to_string()
            }
            _ => String::new(),
        };

        // macOS winit often doesn't report shift in modifiers.
        // Infer shift from key_text being any uppercase ASCII character (A-Z).
        // Physical key names like "KeyH" -> "H" are ALWAYS uppercase — never infer shift from them.
        let key_implies_shift = key.len() == 1
            && key.chars().next().unwrap().is_ascii_uppercase();

        // On macOS, shifted symbols (e.g. +, _, {, }, |, :, ", <, >, ?, ~, !, @, #, $, %, ^, &, *, (, ))
        // may have empty key_text or produce the shifted char without shift in modifiers.
        // Infer shift from physical key + logical key text mismatch.
        let phys_implies_shift = matches!(
            (phys_name.as_str(), key.as_str()),
            ("Equal", "+" | "") | ("Minus", "_" | "") | ("BracketLeft", "{" | "")
                | ("BracketRight", "}" | "") | ("Backslash", "|" | "")
                | ("Semicolon", ":" | "") | ("Quote", "\"" | "")
                | ("Comma", "<" | "") | ("Period", ">" | "")
                | ("Slash", "?" | "") | ("Backquote", "~" | "")
                | ("Digit1", "!" | "") | ("Digit2", "@" | "")
                | ("Digit3", "#" | "") | ("Digit4", "$" | "")
                | ("Digit5", "%" | "") | ("Digit6", "^" | "")
                | ("Digit7", "&" | "") | ("Digit8", "*" | "")
                | ("Digit9", "(" | "") | ("Digit0", ")" | "")
        );
        let effective_shift = shift || key_implies_shift || phys_implies_shift;

        for b in &self.bindings {
            let key_match = if b.key.len() == 1 {
                // Single-char: case-insensitive match (handles Shift+Q vs q).
                // Map physical key names to their unshifted character for symbol keys
                // (e.g., "Equal" → '=' for when macOS doesn't report key_text for Shift+=).
                let phys_as_char = match phys_name.as_str() {
                    "Equal" => Some('='),
                    "Minus" => Some('-'),
                    "Comma" => Some(','),
                    "Period" => Some('.'),
                    "Slash" => Some('/'),
                    "Semicolon" => Some(';'),
                    "Quote" => Some('\''),
                    "BracketLeft" => Some('['),
                    "BracketRight" => Some(']'),
                    "Backslash" => Some('\\'),
                    "Backquote" => Some('`'),
                    "Space" => Some(' '),
                    _ => None,
                };
                b.key.eq_ignore_ascii_case(&key)
                    || b.key.eq_ignore_ascii_case(&named_key)
                    || phys_as_char.is_some_and(|c| b.key.eq_ignore_ascii_case(&c.to_string()))
                    || (key.is_empty() && b.key.eq_ignore_ascii_case(&phys_name))
            } else {
                b.key.eq_ignore_ascii_case(&key)
                    || b.key.eq_ignore_ascii_case(&named_key)
                    || b.key.eq_ignore_ascii_case(&phys_name)
            };

            // Exact modifier match using effective_shift (inferred from key text/phys).
            let mod_match = b.ctrl == ctrl && b.shift == effective_shift;

            if key_match && mod_match {
                return Some(b.action.clone());
            }
        }
        None
    }

    /// Resolve a keypress against mode-specific bindings.
    /// Returns None if the mode doesn't exist or no binding matches.
    // Each parameter is a distinct dimension of the key event; a struct would not improve clarity.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_mode(
        &self,
        mode: &str,
        key_text: &str,
        ctrl: bool,
        _alt: bool,
        shift: bool,
        named: &winit::keyboard::Key,
        phys: &winit::keyboard::PhysicalKey,
    ) -> Option<WmAction> {
        let bindings = self.mode_bindings.get(mode)?;
        let key = key_text.to_string();
        let named_key = match named {
            winit::keyboard::Key::Named(NamedKey::Space) => "Space".to_string(),
            winit::keyboard::Key::Named(n) => format!("{:?}", n),
            _ => key.clone(),
        };

        let phys_name = match phys {
            winit::keyboard::PhysicalKey::Code(c) => {
                let s = format!("{:?}", c);
                s.strip_prefix("Key").unwrap_or(&s).to_string()
            }
            _ => String::new(),
        };

        let key_implies_shift = key.len() == 1
            && key.chars().next().unwrap().is_ascii_uppercase();
        let effective_shift = shift || key_implies_shift;

        for b in bindings {
            let key_match = if b.key.len() == 1 {
                // Single-char: case-insensitive match
                let phys_as_char = match phys_name.as_str() {
                    "Equal" => Some('='),
                    "Minus" => Some('-'),
                    "Comma" => Some(','),
                    "Period" => Some('.'),
                    "Slash" => Some('/'),
                    "Semicolon" => Some(';'),
                    "Quote" => Some('\''),
                    "BracketLeft" => Some('['),
                    "BracketRight" => Some(']'),
                    "Backslash" => Some('\\'),
                    "Backquote" => Some('`'),
                    "Space" => Some(' '),
                    _ => None,
                };
                b.key.eq_ignore_ascii_case(&key)
                    || b.key.eq_ignore_ascii_case(&named_key)
                    || phys_as_char.is_some_and(|c| b.key.eq_ignore_ascii_case(&c.to_string()))
                    || (key.is_empty() && b.key.eq_ignore_ascii_case(&phys_name))
            } else {
                b.key.eq_ignore_ascii_case(&key)
                    || b.key.eq_ignore_ascii_case(&named_key)
                    || b.key.eq_ignore_ascii_case(&phys_name)
            };

            let mod_match = b.ctrl == ctrl && b.shift == effective_shift;
            if key_match && mod_match {
                return Some(b.action.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_from_name_known() {
        assert_eq!(action_from_name("focus_left"), Some(WmAction::FocusLeft));
        assert_eq!(action_from_name("focus_right"), Some(WmAction::FocusRight));
        assert_eq!(action_from_name("close"), Some(WmAction::ClosePane));
        assert_eq!(action_from_name("command_palette"), Some(WmAction::CommandPalette));
    }

    #[test]
    fn test_action_from_name_unknown() {
        assert_eq!(action_from_name("not_real"), None);
        assert_eq!(action_from_name(""), None);
    }

    #[test]
    fn test_parse_key_simple() {
        let (ctrl, shift, key) = KeyBindings::parse_key("h");
        assert!(!ctrl);
        assert!(!shift);
        assert_eq!(key, "h");
    }

    #[test]
    fn test_parse_key_with_modifiers() {
        let (ctrl, shift, key) = KeyBindings::parse_key("Ctrl+Shift+l");
        assert!(ctrl);
        assert!(shift);
        assert_eq!(key, "l");
    }

    #[test]
    fn test_parse_key_shift_only() {
        let (ctrl, shift, key) = KeyBindings::parse_key("Shift+w");
        assert!(!ctrl);
        assert!(shift);
        assert_eq!(key, "w");
    }

    #[test]
    fn test_action_priority_order() {
        // Navigation should have highest priority (lowest number)
        assert!(action_priority(&WmAction::FocusLeft) < action_priority(&WmAction::ResizeIncrease));
        // Resize should have lower priority than pane management
        assert!(action_priority(&WmAction::ResizeIncrease) > action_priority(&WmAction::ClosePane));
        // CommandPalette should have lowest priority
        assert!(action_priority(&WmAction::CommandPalette) > action_priority(&WmAction::SidebarLeft));
    }

    #[test]
    fn test_parameterized_variants_constructible() {
        // Exercise all parameterized variants so they are not flagged as dead code.
        let _ = WmAction::FocusPane { pane_id: 1 };
        let _ = WmAction::FocusWorkspace { ws_idx: 0 };
        let _ = WmAction::Swap { a_id: 1, b_id: 2 };
        let _ = WmAction::Move { pane_id: 1, target_col: 0 };
        let _ = WmAction::Resize { target: ResizeTarget::Column, delta: 10 };
        let _ = WmAction::ResizeTo { target: ResizeTarget::Pane, width: 100.0, height: 200.0 };
        let _ = WmAction::FloatAt { pane_id: 1, x: 0.0, y: 0.0, width: 100.0, height: 100.0 };
        let _ = WmAction::ClosePaneById { pane_id: 1 };
        let _ = WmAction::RenameTarget { pane_id: 1, name: "test".to_string() };
    }

    #[test]
    fn test_action_discriminant_groups_variants() {
        let a = WmAction::FocusPane { pane_id: 1 };
        let b = WmAction::FocusPane { pane_id: 2 };
        let c = WmAction::FocusLeft;
        assert_eq!(action_discriminant(&a), action_discriminant(&b));
        assert_ne!(action_discriminant(&a), action_discriminant(&c));
    }
}
