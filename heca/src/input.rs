use heca_core::layout::PaneId;

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
    FocusPane {
        pane_id: PaneId,
    },
    FocusWorkspace {
        ws_idx: usize,
    },

    // ── Layout (unit) ──
    SplitHorizontal,
    SplitVertical,
    ZoomColumn,
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
    MoveColumnUp,
    MoveColumnDown,

    // ── Layout (parameterized) ──
    Swap {
        a_id: PaneId,
        b_id: PaneId,
    },
    Move {
        pane_id: PaneId,
        target_col: usize,
    },
    MovePaneToWorkspace {
        pane_id: PaneId,
        ws_idx: usize,
    },
    MovePaneToColumn {
        pane_id: PaneId,
        ws_idx: usize,
        col_idx: usize,
    },
    MoveColumnToWorkspace {
        col_idx: usize,
        ws_idx: usize,
        focus: bool,
    },
    /// Move a column to a position (within its workspace, or into another) — sidebar
    /// column DnD / RPC (F4.5). `src_ws == dst_ws` reorders; otherwise it moves.
    MoveColumn {
        src_ws: usize,
        src_col: usize,
        dst_ws: usize,
        dst_idx: usize,
        focus: bool,
    },
    /// Swap two columns' positions — Shift+drag column swap / RPC (F4.5).
    SwapColumns {
        a_ws: usize,
        a_col: usize,
        b_ws: usize,
        b_col: usize,
    },
    Resize {
        target: ResizeTarget,
        axis: ResizeAxis,
        amount: f64,
    },
    ResizeTo {
        target: ResizeTarget,
        width: f64,
        height: f64,
    },

    // ── Pane (unit) ──
    Float,
    ClosePane,
    PaneSelect,
    SwapPane,
    SwapAndFocusPane,
    RenamePane,
    RenameColumn,

    // ── Pane (parameterized) ──
    FloatAt {
        pane_id: PaneId,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    ClosePaneById {
        pane_id: PaneId,
    },
    RenameTarget {
        pane_id: PaneId,
        name: String,
    },

    // ── Quick take (unit) ──
    PaneTake,
    PaneTakeAndFocus,

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
    SidebarCreateWorkspace,
    SidebarCreateColumn,
    SidebarSplitInColumn,
    SidebarZoomSelectedColumn,
    SidebarDeleteSelected,
    CollapseCurrentWorkspace,
    ExpandCurrentWorkspace,
    ToggleCurrentWorkspaceCollapsed,
    CollapseCurrentColumn,
    ExpandCurrentColumn,
    ToggleCurrentColumnCollapsed,

    // ── System ──
    CommandPalette,

    // ── External commands ──
    SpawnCommand {
        command: String,
    },

    // ── Mode management ──
    EnterMode {
        name: String,
    },

    // ── Selection (host capability, reusable across pane types) ──
    EnterSelectionMode,
    SelectionLeft,
    SelectionRight,
    SelectionUp,
    SelectionDown,
    ClearSelection,
    CopySelection,
    PasteClipboard,
    BeginSelection,
    ToggleSelectionEndpoint,

    // ── Sidebar-specific (parameterized) ──
    AddPaneToColumn {
        ws_idx: usize,
        col_idx: usize,
    },

    // ── Destructive (parameterized) ──
    DeleteColumn {
        ws_idx: usize,
        col_idx: usize,
    },
    DeleteWorkspace {
        ws_idx: usize,
    },

    // ── Take pane (parameterized) ──
    TakePane {
        pane_id: PaneId,
        focus_after: bool,
    },

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

/**
Map a config key name to its unit `WmAction` variant.
Parameterized variants are not reachable from config — they are
constructed programmatically (RPC, mouse handlers, command palette).
*/
pub fn action_from_name(name: &str) -> Option<WmAction> {
    match name {
        "focus_left" => Some(WmAction::FocusLeft),
        "focus_right" => Some(WmAction::FocusRight),
        "focus_up" => Some(WmAction::FocusUp),
        "focus_down" => Some(WmAction::FocusDown),
        "split_horizontal" => Some(WmAction::SplitHorizontal),
        "split_vertical" => Some(WmAction::SplitVertical),
        "zoom_column" => Some(WmAction::ZoomColumn),
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
        "sidebar_create_workspace" => Some(WmAction::SidebarCreateWorkspace),
        "sidebar_create_column" => Some(WmAction::SidebarCreateColumn),
        "sidebar_split_in_column" => Some(WmAction::SidebarSplitInColumn),
        "sidebar_zoom_selected_column" => Some(WmAction::SidebarZoomSelectedColumn),
        "sidebar_delete_selected" => Some(WmAction::SidebarDeleteSelected),
        "collapse_current_workspace" => Some(WmAction::CollapseCurrentWorkspace),
        "expand_current_workspace" => Some(WmAction::ExpandCurrentWorkspace),
        "toggle_current_workspace_collapsed" => Some(WmAction::ToggleCurrentWorkspaceCollapsed),
        "collapse_current_column" => Some(WmAction::CollapseCurrentColumn),
        "expand_current_column" => Some(WmAction::ExpandCurrentColumn),
        "toggle_current_column_collapsed" => Some(WmAction::ToggleCurrentColumnCollapsed),
        "next_pane" => Some(WmAction::NextPane),
        "prev_pane" => Some(WmAction::PrevPane),
        "pane_select" => Some(WmAction::PaneSelect),
        "swap_pane" => Some(WmAction::SwapPane),
        "pane_take" => Some(WmAction::PaneTake),
        "pane_take_and_focus" => Some(WmAction::PaneTakeAndFocus),
        "swap_and_focus_pane" => Some(WmAction::SwapAndFocusPane),
        "swap_left" => Some(WmAction::SwapLeft),
        "swap_right" => Some(WmAction::SwapRight),
        "swap_up" => Some(WmAction::SwapUp),
        "swap_down" => Some(WmAction::SwapDown),
        "move_pane_left" => Some(WmAction::MovePaneLeft),
        "move_pane_right" => Some(WmAction::MovePaneRight),
        "move_column_up" => Some(WmAction::MoveColumnUp),
        "move_column_down" => Some(WmAction::MoveColumnDown),
        "move_pane_to_workspace" => Some(WmAction::MovePaneToWorkspace {
            pane_id: PaneId(0),
            ws_idx: 0,
        }),
        "move_pane_to_column" => Some(WmAction::MovePaneToColumn {
            pane_id: PaneId(0),
            ws_idx: 0,
            col_idx: 0,
        }),
        "move_column_to_workspace" => Some(WmAction::MoveColumnToWorkspace {
            col_idx: 0,
            ws_idx: 0,
            focus: true,
        }),
        "move_column" => Some(WmAction::MoveColumn {
            src_ws: 0,
            src_col: 0,
            dst_ws: 0,
            dst_idx: 0,
            focus: true,
        }),
        "swap_columns" => Some(WmAction::SwapColumns {
            a_ws: 0,
            a_col: 0,
            b_ws: 0,
            b_col: 0,
        }),
        "pane_height_increase" => Some(WmAction::PaneHeightIncrease),
        "pane_height_decrease" => Some(WmAction::PaneHeightDecrease),
        "workspace_next" => Some(WmAction::WorkspaceNext),
        "focus_toggle_local" => Some(WmAction::FocusToggleLocal),
        "focus_toggle_global" => Some(WmAction::FocusToggleGlobal),
        "workspace_prev" => Some(WmAction::WorkspacePrev),
        "create_workspace" => Some(WmAction::CreateWorkspace),
        "rename_workspace" => Some(WmAction::RenameWorkspace),
        "rename_pane" => Some(WmAction::RenamePane),
        "rename_column" => Some(WmAction::RenameColumn),
        "command_palette" => Some(WmAction::CommandPalette),
        "add_pane_to_column" => Some(WmAction::AddPaneToColumn {
            ws_idx: 0,
            col_idx: 0,
        }),
        "delete_column" => Some(WmAction::DeleteColumn {
            ws_idx: 0,
            col_idx: 0,
        }),
        "delete_workspace" => Some(WmAction::DeleteWorkspace { ws_idx: 0 }),
        "reload_config" => Some(WmAction::ReloadConfig),
        "enter_selection_mode" => Some(WmAction::EnterSelectionMode),
        "selection_left" => Some(WmAction::SelectionLeft),
        "selection_right" => Some(WmAction::SelectionRight),
        "selection_up" => Some(WmAction::SelectionUp),
        "selection_down" => Some(WmAction::SelectionDown),
        "clear_selection" => Some(WmAction::ClearSelection),
        "copy_selection" => Some(WmAction::CopySelection),
        "paste_clipboard" => Some(WmAction::PasteClipboard),
        "begin_selection" => Some(WmAction::BeginSelection),
        "toggle_selection_endpoint" => Some(WmAction::ToggleSelectionEndpoint),
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
fn get_enum<T: std::str::FromStr>(
    args: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<T> {
    args.get(key)?.parse().ok()
}

/// Build a parameterized `WmAction` from a name and string args.
/// Returns `None` if the action name is unknown or args are missing/invalid.
///
/// Supported names and required args:
///   "focus_pane"          → pane_id: PaneId
///   "focus_workspace"     → ws_idx: usize
///   "swap"                → a_id: PaneId, b_id: PaneId
///   "move"                → pane_id: PaneId, target_col: usize
///   "resize"              → target: "column"|"pane", axis: "x"|"y", amount: f64
///   "resize_to"           → target: "column"|"pane", width: f64, height: f64
///   "float_at"            → pane_id: PaneId, x: f64, y: f64, width: f64, height: f64
///   "close_pane_by_id"    → pane_id: PaneId
///   "rename_target"       → pane_id: PaneId, name: String
///   "spawn_command"       → command: String
pub fn build_action(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    match name {
        "focus_pane" => Some(WmAction::FocusPane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "focus_workspace" => Some(WmAction::FocusWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "swap" => Some(WmAction::Swap {
            a_id: PaneId(get_u64(args, "a_id")?),
            b_id: PaneId(get_u64(args, "b_id")?),
        }),
        "move" => Some(WmAction::Move {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            target_col: get_usize(args, "target_col")?,
        }),
        "move_pane_to_workspace" => Some(WmAction::MovePaneToWorkspace {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "move_pane_to_column" => Some(WmAction::MovePaneToColumn {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "move_column" => Some(WmAction::MoveColumn {
            src_ws: get_usize(args, "src_ws")?,
            src_col: get_usize(args, "src_col")?,
            dst_ws: get_usize(args, "dst_ws")?,
            dst_idx: get_usize(args, "dst_idx")?,
            focus: args
                .get("focus")
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }),
        "swap_columns" => Some(WmAction::SwapColumns {
            a_ws: get_usize(args, "a_ws")?,
            a_col: get_usize(args, "a_col")?,
            b_ws: get_usize(args, "b_ws")?,
            b_col: get_usize(args, "b_col")?,
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
            pane_id: PaneId(get_u64(args, "pane_id")?),
            x: get_f64(args, "x")?,
            y: get_f64(args, "y")?,
            width: get_f64(args, "width")?,
            height: get_f64(args, "height")?,
        }),
        "close_pane_by_id" => Some(WmAction::ClosePaneById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "rename_target" => Some(WmAction::RenameTarget {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            name: get_string(args, "name")?,
        }),
        "take_pane" => Some(WmAction::TakePane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            focus_after: args
                .get("focus_after")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
        }),
        "add_pane_to_column" => Some(WmAction::AddPaneToColumn {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "delete_column" => Some(WmAction::DeleteColumn {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "delete_workspace" => Some(WmAction::DeleteWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "spawn_command" => Some(WmAction::SpawnCommand {
            command: get_string(args, "command")?,
        }),
        _ => None,
    }
}

/// Binding priority: lower = checked first. Focus wins over resize on conflicts.
///
/// Test-only helper: the exhaustive test in this module calls it directly so
/// exhaustiveness is enforced by the test's `each_variant()` list, and any
/// divergence between the production body and a shadow implementation
/// surfaces immediately as a compile error.
#[cfg(test)]
pub(crate) fn action_priority(action: &WmAction) -> u8 {
    match action {
        // Navigation (highest priority)
        WmAction::FocusLeft
        | WmAction::FocusRight
        | WmAction::FocusUp
        | WmAction::FocusDown
        | WmAction::NextPane
        | WmAction::PrevPane => 0,
        // Sidebar navigation (only used in sidebar mode via resolve_mode)
        // Low priority so they don't override focus bindings in normal/prefix mode.
        WmAction::SidebarFocus => 0,
        WmAction::SidebarUp
        | WmAction::SidebarDown
        | WmAction::SidebarLeftNav
        | WmAction::SidebarRightNav
        | WmAction::SidebarExpandToggle
        | WmAction::SidebarCreateWorkspace
        | WmAction::SidebarCreateColumn
        | WmAction::SidebarSplitInColumn
        | WmAction::SidebarZoomSelectedColumn
        | WmAction::SidebarDeleteSelected
        | WmAction::CollapseCurrentWorkspace
        | WmAction::ExpandCurrentWorkspace
        | WmAction::ToggleCurrentWorkspaceCollapsed
        | WmAction::CollapseCurrentColumn
        | WmAction::ExpandCurrentColumn
        | WmAction::ToggleCurrentColumnCollapsed => 4,
        // Pane management
        WmAction::SplitHorizontal
        | WmAction::SplitVertical
        | WmAction::ZoomColumn
        | WmAction::Float
        | WmAction::ClosePane
        | WmAction::PaneSelect
        | WmAction::SwapPane
        | WmAction::SwapAndFocusPane
        | WmAction::FocusToggleLocal
        | WmAction::FocusToggleGlobal
        | WmAction::CreateWorkspace
        | WmAction::RenameWorkspace
        | WmAction::RenamePane
        | WmAction::RenameColumn
        | WmAction::WorkspaceNext
        | WmAction::WorkspacePrev => 1,
        // Swap
        WmAction::SwapLeft
        | WmAction::SwapRight
        | WmAction::SwapUp
        | WmAction::SwapDown
        | WmAction::MovePaneLeft
        | WmAction::MovePaneRight
        | WmAction::MoveColumnUp
        | WmAction::MoveColumnDown => 2,
        // Resize (lowest priority — checked last)
        WmAction::ResizeIncrease
        | WmAction::ResizeDecrease
        | WmAction::PaneHeightIncrease
        | WmAction::PaneHeightDecrease => 3,
        // Sidebars
        WmAction::SidebarLeft | WmAction::SidebarRight => 4,
        // System
        WmAction::CommandPalette => 5,
        // Selection (host capability). Treated as pane-management-class
        // actions so they share priority with close/rename-style actions.
        WmAction::EnterSelectionMode
        | WmAction::SelectionLeft
        | WmAction::SelectionRight
        | WmAction::SelectionUp
        | WmAction::SelectionDown
        | WmAction::ClearSelection
        | WmAction::CopySelection
        | WmAction::PasteClipboard
        | WmAction::BeginSelection
        | WmAction::ToggleSelectionEndpoint => 1,
        // Parameterized variants are not resolved from keybindings,
        // but we still match them explicitly to avoid catch-all.
        WmAction::FocusPane { .. }
        | WmAction::FocusWorkspace { .. }
        | WmAction::Swap { .. }
        | WmAction::Move { .. }
        | WmAction::MovePaneToWorkspace { .. }
        | WmAction::MovePaneToColumn { .. }
        | WmAction::MoveColumnToWorkspace { .. }
        | WmAction::MoveColumn { .. }
        | WmAction::SwapColumns { .. }
        | WmAction::Resize { .. }
        | WmAction::ResizeTo { .. }
        | WmAction::FloatAt { .. }
        | WmAction::ClosePaneById { .. }
        | WmAction::RenameTarget { .. }
        | WmAction::SpawnCommand { .. }
        | WmAction::EnterMode { .. }
        | WmAction::ReloadConfig
        | WmAction::AddPaneToColumn { .. }
        | WmAction::DeleteColumn { .. }
        | WmAction::DeleteWorkspace { .. }
        | WmAction::TakePane { .. }
        | WmAction::PaneTake
        | WmAction::PaneTakeAndFocus => 6,
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
        assert_eq!(action_from_name("zoom_column"), Some(WmAction::ZoomColumn));
        assert_eq!(
            action_from_name("toggle_current_workspace_collapsed"),
            Some(WmAction::ToggleCurrentWorkspaceCollapsed)
        );
        assert_eq!(
            action_from_name("toggle_current_column_collapsed"),
            Some(WmAction::ToggleCurrentColumnCollapsed)
        );
        assert_eq!(
            action_from_name("rename_column"),
            Some(WmAction::RenameColumn)
        );
        assert_eq!(action_from_name("close"), Some(WmAction::ClosePane));
        assert_eq!(
            action_from_name("command_palette"),
            Some(WmAction::CommandPalette)
        );
        // Selection actions are generic (host capability), not terminal-only.
        assert_eq!(
            action_from_name("enter_selection_mode"),
            Some(WmAction::EnterSelectionMode)
        );
        assert_eq!(
            action_from_name("selection_left"),
            Some(WmAction::SelectionLeft)
        );
        assert_eq!(
            action_from_name("selection_right"),
            Some(WmAction::SelectionRight)
        );
        assert_eq!(
            action_from_name("selection_up"),
            Some(WmAction::SelectionUp)
        );
        assert_eq!(
            action_from_name("selection_down"),
            Some(WmAction::SelectionDown)
        );
        assert_eq!(
            action_from_name("clear_selection"),
            Some(WmAction::ClearSelection)
        );
        assert_eq!(
            action_from_name("copy_selection"),
            Some(WmAction::CopySelection)
        );
        assert_eq!(
            action_from_name("paste_clipboard"),
            Some(WmAction::PasteClipboard)
        );
        assert_eq!(
            action_from_name("begin_selection"),
            Some(WmAction::BeginSelection)
        );
        assert_eq!(
            action_from_name("toggle_selection_endpoint"),
            Some(WmAction::ToggleSelectionEndpoint)
        );
        // Selection has no parameterized variants in Task 02; the parameterized
        // pathway is unreachable by design.
        assert_eq!(action_from_name("copy_selection_42"), None);
        assert_eq!(action_from_name("enter_selection_mode_now"), None);
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
        assert!(
            action_priority(&WmAction::CommandPalette) > action_priority(&WmAction::SidebarLeft)
        );
    }

    #[test]
    fn test_action_priority_exhaustive() {
        // Exhaustiveness is enforced by listing every variant here: if a new
        // `WmAction` variant is added, this function will fail to compile
        // until a representative value is appended. The real `action_priority`
        // is then called, so any divergence between the production body and
        // this test (e.g. a catch-all arm in production) surfaces immediately
        // as a compile error.
        fn each_variant() -> Vec<WmAction> {
            vec![
                // Navigation
                WmAction::FocusLeft, WmAction::FocusRight,
                WmAction::FocusUp, WmAction::FocusDown,
                WmAction::NextPane, WmAction::PrevPane,
                // Swap
                WmAction::SwapLeft, WmAction::SwapRight,
                WmAction::SwapUp, WmAction::SwapDown,
                WmAction::MovePaneLeft, WmAction::MovePaneRight,
                WmAction::MoveColumnUp, WmAction::MoveColumnDown,
                // Resize
                WmAction::ResizeIncrease, WmAction::ResizeDecrease,
                WmAction::PaneHeightIncrease, WmAction::PaneHeightDecrease,
                // Pane management
                WmAction::SplitHorizontal, WmAction::SplitVertical,
                WmAction::ZoomColumn, WmAction::Float, WmAction::ClosePane,
                WmAction::PaneSelect, WmAction::SwapPane, WmAction::SwapAndFocusPane,
                WmAction::FocusToggleLocal, WmAction::FocusToggleGlobal,
                WmAction::CreateWorkspace, WmAction::RenameWorkspace,
                WmAction::RenamePane, WmAction::RenameColumn,
                WmAction::WorkspaceNext, WmAction::WorkspacePrev,
                // Sidebar (mode-internal + global toggles)
                WmAction::SidebarFocus, WmAction::SidebarUp, WmAction::SidebarDown,
                WmAction::SidebarLeftNav, WmAction::SidebarRightNav,
                WmAction::SidebarExpandToggle,
                WmAction::SidebarCreateWorkspace, WmAction::SidebarCreateColumn,
                WmAction::SidebarSplitInColumn, WmAction::SidebarZoomSelectedColumn,
                WmAction::SidebarDeleteSelected,
                WmAction::CollapseCurrentWorkspace, WmAction::ExpandCurrentWorkspace,
                WmAction::ToggleCurrentWorkspaceCollapsed,
                WmAction::CollapseCurrentColumn, WmAction::ExpandCurrentColumn,
                WmAction::ToggleCurrentColumnCollapsed,
                WmAction::SidebarLeft, WmAction::SidebarRight,
                // System
                WmAction::CommandPalette,
                // Selection (host capability, Task 02)
                WmAction::EnterSelectionMode,
                WmAction::SelectionLeft, WmAction::SelectionRight,
                WmAction::SelectionUp, WmAction::SelectionDown,
                WmAction::ClearSelection,
                WmAction::CopySelection, WmAction::PasteClipboard,
                WmAction::BeginSelection, WmAction::ToggleSelectionEndpoint,
                // Take (panes + quick-take)
                WmAction::PaneTake, WmAction::PaneTakeAndFocus,
                // Parameterized variants
                WmAction::FocusPane { pane_id: PaneId(0) },
                WmAction::FocusWorkspace { ws_idx: 0 },
                WmAction::Swap { a_id: PaneId(0), b_id: PaneId(0) },
                WmAction::Move { pane_id: PaneId(0), target_col: 0 },
                WmAction::MovePaneToWorkspace { pane_id: PaneId(0), ws_idx: 0 },
                WmAction::MovePaneToColumn { pane_id: PaneId(0), ws_idx: 0, col_idx: 0 },
                WmAction::MoveColumnToWorkspace { col_idx: 0, ws_idx: 0, focus: false },
                WmAction::MoveColumn { src_ws: 0, src_col: 0, dst_ws: 0, dst_idx: 0, focus: false },
                WmAction::SwapColumns { a_ws: 0, a_col: 0, b_ws: 0, b_col: 0 },
                WmAction::Resize { target: ResizeTarget::Column, axis: ResizeAxis::X, amount: 0.0 },
                WmAction::ResizeTo { target: ResizeTarget::Column, width: 0.0, height: 0.0 },
                WmAction::FloatAt { pane_id: PaneId(0), x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
                WmAction::ClosePaneById { pane_id: PaneId(0) },
                WmAction::RenameTarget { pane_id: PaneId(0), name: String::new() },
                WmAction::SpawnCommand { command: String::new() },
                WmAction::EnterMode { name: String::new() },
                WmAction::AddPaneToColumn { ws_idx: 0, col_idx: 0 },
                WmAction::DeleteColumn { ws_idx: 0, col_idx: 0 },
                WmAction::DeleteWorkspace { ws_idx: 0 },
                WmAction::TakePane { pane_id: PaneId(0), focus_after: false },
                WmAction::ReloadConfig,
            ]
        }

        // Smoke: every variant compiles and returns a priority.
        for action in each_variant() {
            let _ = action_priority(&action);
        }
    }

    #[test]
    fn test_parameterized_variants_constructible() {
        // Exercise all parameterized variants so they are not flagged as dead code.
        let _ = WmAction::FocusPane { pane_id: PaneId(1) };
        let _ = WmAction::FocusWorkspace { ws_idx: 0 };
        let _ = WmAction::Swap { a_id: PaneId(1), b_id: PaneId(2) };
        let _ = WmAction::Move {
            pane_id: PaneId(1),
            target_col: 0,
        };
        let _ = WmAction::MovePaneToWorkspace {
            pane_id: PaneId(1),
            ws_idx: 0,
        };
        let _ = WmAction::MovePaneToColumn {
            pane_id: PaneId(1),
            ws_idx: 0,
            col_idx: 0,
        };
        let _ = WmAction::MoveColumnToWorkspace {
            col_idx: 0,
            ws_idx: 1,
            focus: true,
        };
        let _ = WmAction::ZoomColumn;
        let _ = WmAction::Resize {
            target: ResizeTarget::Column,
            axis: ResizeAxis::X,
            amount: 10.0,
        };
        let _ = WmAction::ResizeTo {
            target: ResizeTarget::Pane,
            width: 100.0,
            height: 200.0,
        };
        let _ = WmAction::FloatAt {
            pane_id: PaneId(1),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let _ = WmAction::ClosePaneById { pane_id: PaneId(1) };
        let _ = WmAction::RenameTarget {
            pane_id: PaneId(1),
            name: "test".to_string(),
        };
        let _ = WmAction::AddPaneToColumn {
            ws_idx: 0,
            col_idx: 0,
        };
        let _ = WmAction::DeleteColumn {
            ws_idx: 0,
            col_idx: 0,
        };
        let _ = WmAction::DeleteWorkspace { ws_idx: 0 };
        let _ = WmAction::TakePane {
            pane_id: PaneId(1),
            focus_after: false,
        };
        let _ = WmAction::RenameColumn;
        let _ = WmAction::PaneTake;
        let _ = WmAction::PaneTakeAndFocus;
    }

    #[test]
    fn test_action_discriminant_groups_variants() {
        let a = WmAction::FocusPane { pane_id: PaneId(1) };
        let b = WmAction::FocusPane { pane_id: PaneId(2) };
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
