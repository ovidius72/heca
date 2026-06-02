/// Target for resize actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeTarget {
    Column,
    Pane,
}

impl std::str::FromStr for ResizeTarget {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "column" | "col" => Ok(ResizeTarget::Column),
            "pane" => Ok(ResizeTarget::Pane),
            _ => Err(format!("unknown resize target: {}", s)),
        }
    }
}

/// Axis for resize actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeAxis {
    X,
    Y,
}

impl std::str::FromStr for ResizeAxis {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "x" | "horizontal" | "width" => Ok(ResizeAxis::X),
            "y" | "vertical" | "height" => Ok(ResizeAxis::Y),
            _ => Err(format!("unknown resize axis: {}", s)),
        }
    }
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
    MovePaneToWorkspace { pane_id: u64, ws_idx: usize },
    MovePaneToColumn { pane_id: u64, ws_idx: usize, col_idx: usize },
    Resize { target: ResizeTarget, axis: ResizeAxis, amount: f64 },
    ResizeTo { target: ResizeTarget, width: f64, height: f64 },

    // ── Pane (unit) ──
    Float,
    ClosePane,
    PaneSelect,
    SwapPane,
    SwapAndFocusPane,
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

    // ── System ──
    CommandPalette,

    // ── External commands ──
    SpawnCommand { command: String },

    // ── Mode management ──
    EnterMode { name: String },

    // ── Config ──
    ReloadConfig,
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
pub fn action_from_name(name: &str) -> Option<WmAction> {
    match name {
        "focus_left" => Some(WmAction::FocusLeft),
        "focus_right" => Some(WmAction::FocusRight),
        "focus_up" => Some(WmAction::FocusUp),
        "focus_down" => Some(WmAction::FocusDown),
        "split_horizontal" => Some(WmAction::SplitHorizontal),
        "split_vertical" => Some(WmAction::SplitVertical),
        "float" => Some(WmAction::Float),
        "close" => Some(WmAction::ClosePane),
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
        "swap_pane" => Some(WmAction::SwapPane),
        "swap_and_focus_pane" => Some(WmAction::SwapAndFocusPane),
        "swap_left" => Some(WmAction::SwapLeft),
        "swap_right" => Some(WmAction::SwapRight),
        "swap_up" => Some(WmAction::SwapUp),
        "swap_down" => Some(WmAction::SwapDown),
        "move_pane_left" => Some(WmAction::MovePaneLeft),
        "move_pane_right" => Some(WmAction::MovePaneRight),
        "move_pane_to_workspace" => Some(WmAction::MovePaneToWorkspace { pane_id: 0, ws_idx: 0 }),
        "move_pane_to_column" => Some(WmAction::MovePaneToColumn { pane_id: 0, ws_idx: 0, col_idx: 0 }),
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
        "reload_config" => Some(WmAction::ReloadConfig),
        _ => {
            // Dynamic: focus_workspace_1 → FocusWorkspace { ws_idx: 0 }
            if let Some(rest) = name.strip_prefix("focus_workspace_")
                && let Ok(n) = rest.parse::<usize>()
                && n >= 1
            {
                return Some(WmAction::FocusWorkspace { ws_idx: n - 1 });
            }
            None
        }
    }
}

// ── Parameterized action builders ──

fn get_u64(args: &std::collections::HashMap<String, String>, key: &str) -> Option<u64> {
    args.get(key)?.parse().ok()
}
fn get_usize(args: &std::collections::HashMap<String, String>, key: &str) -> Option<usize> {
    args.get(key)?.parse().ok()
}
fn get_f64(args: &std::collections::HashMap<String, String>, key: &str) -> Option<f64> {
    args.get(key)?.parse().ok()
}
fn get_string(args: &std::collections::HashMap<String, String>, key: &str) -> Option<String> {
    args.get(key).cloned()
}
fn get_enum<T: std::str::FromStr>(args: &std::collections::HashMap<String, String>, key: &str) -> Option<T> {
    args.get(key)?.parse().ok()
}

/// Build a parameterized `WmAction` from a name and string args.
/// Returns `None` if the action name is unknown or args are missing/invalid.
///
/// Supported names and required args:
///   "focus_pane"          → pane_id: u64
///   "focus_workspace"     → ws_idx: usize
///   "swap"                → a_id: u64, b_id: u64
///   "move"                → pane_id: u64, target_col: usize
///   "resize"              → target: "column"|"pane", axis: "x"|"y", amount: f64
///   "resize_to"           → target: "column"|"pane", width: f64, height: f64
///   "float_at"            → pane_id: u64, x: f64, y: f64, width: f64, height: f64
///   "close_pane_by_id"    → pane_id: u64
///   "rename_target"       → pane_id: u64, name: String
///   "spawn_command"       → command: String
pub fn build_action(name: &str, args: &std::collections::HashMap<String, String>) -> Option<WmAction> {
    match name {
        "focus_pane" => Some(WmAction::FocusPane {
            pane_id: get_u64(args, "pane_id")?,
        }),
        "focus_workspace" => Some(WmAction::FocusWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "swap" => Some(WmAction::Swap {
            a_id: get_u64(args, "a_id")?,
            b_id: get_u64(args, "b_id")?,
        }),
        "move" => Some(WmAction::Move {
            pane_id: get_u64(args, "pane_id")?,
            target_col: get_usize(args, "target_col")?,
        }),
        "move_pane_to_workspace" => Some(WmAction::MovePaneToWorkspace {
            pane_id: get_u64(args, "pane_id")?,
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "move_pane_to_column" => Some(WmAction::MovePaneToColumn {
            pane_id: get_u64(args, "pane_id")?,
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "resize" => Some(WmAction::Resize {
            target: get_enum(args, "target")?,
            axis: get_enum(args, "axis")?,
            amount: get_f64(args, "amount")?,
        }),
        "resize_to" => Some(WmAction::ResizeTo {
            target: get_enum(args, "target")?,
            width: get_f64(args, "width")?,
            height: get_f64(args, "height")?,
        }),
        "float_at" => Some(WmAction::FloatAt {
            pane_id: get_u64(args, "pane_id")?,
            x: get_f64(args, "x")?,
            y: get_f64(args, "y")?,
            width: get_f64(args, "width")?,
            height: get_f64(args, "height")?,
        }),
        "close_pane_by_id" => Some(WmAction::ClosePaneById {
            pane_id: get_u64(args, "pane_id")?,
        }),
        "rename_target" => Some(WmAction::RenameTarget {
            pane_id: get_u64(args, "pane_id")?,
            name: get_string(args, "name")?,
        }),
        "spawn_command" => Some(WmAction::SpawnCommand {
            command: get_string(args, "command")?,
        }),
        _ => None,
    }
}

/// Binding priority: lower = checked first. Focus wins over resize on conflicts.
#[cfg(test)]
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
        WmAction::PaneSelect | WmAction::SwapPane | WmAction::SwapAndFocusPane |
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
        // Sidebars
        WmAction::SidebarLeft | WmAction::SidebarRight => 4,
        // System
        WmAction::CommandPalette => 5,
        // Parameterized variants are not resolved from keybindings,
        // but we still match them explicitly to avoid catch-all.
        WmAction::FocusPane { .. }
        | WmAction::FocusWorkspace { .. }
        | WmAction::Swap { .. }
        | WmAction::Move { .. }
        | WmAction::MovePaneToWorkspace { .. }
        | WmAction::MovePaneToColumn { .. }
        | WmAction::Resize { .. }
        | WmAction::ResizeTo { .. }
        | WmAction::FloatAt { .. }
        | WmAction::ClosePaneById { .. }
        | WmAction::RenameTarget { .. }
        | WmAction::SpawnCommand { .. }
        | WmAction::EnterMode { .. }
        | WmAction::ReloadConfig => 6,
    }
}

/// Parse a key string like "h", "H", "Ctrl+h", "Ctrl+Shift+l", "Space".
/// Preserves key case exactly; shift ONLY from explicit "Shift+" modifier.
#[cfg(test)]
fn parse_key(s: &str) -> (bool, bool, bool, bool, String) {
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
            _ => key = part.to_string(),
        }
    }
    (ctrl, shift, alt, super_, key)
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
        let (ctrl, shift, alt, super_, key) = parse_key("h");
        assert!(!ctrl);
        assert!(!shift);
        assert!(!alt);
        assert!(!super_);
        assert_eq!(key, "h");
    }

    #[test]
    fn test_parse_key_with_modifiers() {
        let (ctrl, shift, alt, super_, key) = parse_key("Ctrl+Shift+l");
        assert!(ctrl);
        assert!(shift);
        assert!(!alt);
        assert!(!super_);
        assert_eq!(key, "l");
    }

    #[test]
    fn test_parse_key_shift_only() {
        let (ctrl, shift, alt, super_, key) = parse_key("Shift+w");
        assert!(!ctrl);
        assert!(shift);
        assert!(!alt);
        assert!(!super_);
        assert_eq!(key, "w");
    }

    #[test]
    fn test_parse_key_alt_super() {
        let (ctrl, shift, alt, super_, key) = parse_key("Alt+Super+x");
        assert!(!ctrl);
        assert!(!shift);
        assert!(alt);
        assert!(super_);
        assert_eq!(key, "x");
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
        let _ = WmAction::MovePaneToWorkspace { pane_id: 1, ws_idx: 0 };
        let _ = WmAction::MovePaneToColumn { pane_id: 1, ws_idx: 0, col_idx: 0 };
        let _ = WmAction::Resize { target: ResizeTarget::Column, axis: ResizeAxis::X, amount: 10.0 };
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

    #[test]
    fn test_focus_workspace_dynamic_parsing() {
        assert_eq!(
            action_from_name("focus_workspace_1"),
            Some(WmAction::FocusWorkspace { ws_idx: 0 })
        );
        assert_eq!(
            action_from_name("focus_workspace_9"),
            Some(WmAction::FocusWorkspace { ws_idx: 8 })
        );
        assert_eq!(action_from_name("focus_workspace_0"), None);
        assert_eq!(action_from_name("focus_workspace_"), None);
        assert_eq!(action_from_name("focus_workspace"), None);
    }
}
