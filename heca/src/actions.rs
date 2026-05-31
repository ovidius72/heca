//! Action registry — static catalog of all window-manager actions.
//!
//! This module provides the data source for the command palette and
//! documentation generators. Each action maps a config key to a human-readable
//! label, description, category, and default keybinding.
//!
//! Note: These types appear unused in the binary because the command palette UI
//! is not yet implemented. They are fully exercised in unit tests and will be
//! wired into the UI in a follow-up plan.
#![allow(dead_code)]

/// Category for grouping actions in the command palette and documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionCategory {
    /// Focus, sidebar navigation, workspace switching.
    Navigation,
    /// Splits, resizes, moves, swaps.
    Layout,
    /// Pane lifecycle: float, hide, close, rename, select.
    Pane,
    /// Workspace creation, renaming, switching.
    Workspace,
    /// Session-level: overview, save, load.
    Session,
    /// UI chrome: sidebar toggle, tab management.
    Chrome,
    /// System: command palette, quit.
    System,
}

impl ActionCategory {
    /// Human-readable category name for UI display.
    pub const fn label(self) -> &'static str {
        match self {
            ActionCategory::Navigation => "Navigation",
            ActionCategory::Layout => "Layout",
            ActionCategory::Pane => "Pane",
            ActionCategory::Workspace => "Workspace",
            ActionCategory::Session => "Session",
            ActionCategory::Chrome => "Chrome",
            ActionCategory::System => "System",
        }
    }
}

/// Static descriptor for a window-manager action.
#[derive(Debug, Clone)]
pub struct ActionDescriptor {
    /// Config key name (e.g. "focus_left").
    pub name: &'static str,
    /// Human-readable label for the command palette.
    pub label: &'static str,
    /// Short description of what the action does.
    pub description: &'static str,
    /// Category for grouping.
    pub category: ActionCategory,
    /// Default keybinding string (e.g. "h,ArrowLeft").
    pub default_binding: &'static str,
}

/// Static registry of all window-manager actions.
pub struct ActionRegistry;

impl ActionRegistry {
    /// All registered actions in a stable order.
    pub const ALL: &[ActionDescriptor] = &[
        // ── Navigation ──
        ActionDescriptor {
            name: "focus_left",
            label: "Focus Column Left",
            description: "Move focus to the column on the left.",
            category: ActionCategory::Navigation,
            default_binding: "h",
        },
        ActionDescriptor {
            name: "focus_right",
            label: "Focus Column Right",
            description: "Move focus to the column on the right.",
            category: ActionCategory::Navigation,
            default_binding: "l",
        },
        ActionDescriptor {
            name: "focus_up",
            label: "Focus Pane Up",
            description: "Move focus to the pane above in the current column.",
            category: ActionCategory::Navigation,
            default_binding: "k",
        },
        ActionDescriptor {
            name: "focus_down",
            label: "Focus Pane Down",
            description: "Move focus to the pane below in the current column.",
            category: ActionCategory::Navigation,
            default_binding: "j",
        },
        ActionDescriptor {
            name: "next_pane",
            label: "Next Pane in Column",
            description: "Cycle focus forward through panes in the active column.",
            category: ActionCategory::Navigation,
            default_binding: "n",
        },
        ActionDescriptor {
            name: "prev_pane",
            label: "Previous Pane in Column",
            description: "Cycle focus backward through panes in the active column.",
            category: ActionCategory::Navigation,
            default_binding: "p",
        },
        ActionDescriptor {
            name: "sidebar_focus",
            label: "Focus Sidebar",
            description: "Enter sidebar navigation mode.",
            category: ActionCategory::Navigation,
            default_binding: "e",
        },
        ActionDescriptor {
            name: "sidebar_up",
            label: "Sidebar Cursor Up",
            description: "Move the sidebar selection up.",
            category: ActionCategory::Navigation,
            default_binding: "k",
        },
        ActionDescriptor {
            name: "sidebar_down",
            label: "Sidebar Cursor Down",
            description: "Move the sidebar selection down.",
            category: ActionCategory::Navigation,
            default_binding: "j",
        },
        ActionDescriptor {
            name: "sidebar_left_nav",
            label: "Sidebar Collapse / Out",
            description: "Collapse the current tree node or move to parent.",
            category: ActionCategory::Navigation,
            default_binding: "h",
        },
        ActionDescriptor {
            name: "sidebar_right_nav",
            label: "Sidebar Expand / Enter",
            description: "Expand the current tree node or activate the selected item.",
            category: ActionCategory::Navigation,
            default_binding: "l",
        },
        ActionDescriptor {
            name: "workspace_next",
            label: "Next Workspace",
            description: "Switch to the next workspace.",
            category: ActionCategory::Navigation,
            default_binding: "d",
        },
        ActionDescriptor {
            name: "workspace_prev",
            label: "Previous Workspace",
            description: "Switch to the previous workspace.",
            category: ActionCategory::Navigation,
            default_binding: "u",
        },
        ActionDescriptor {
            name: "focus_toggle_local",
            label: "Toggle Focus Local",
            description: "Toggle between current and last-focused pane in the same workspace.",
            category: ActionCategory::Navigation,
            default_binding: "i",
        },
        ActionDescriptor {
            name: "focus_toggle_global",
            label: "Toggle Focus Global",
            description: "Toggle between current and last-visited workspace.",
            category: ActionCategory::Navigation,
            default_binding: "Shift+l",
        },
        // ── Layout ──
        ActionDescriptor {
            name: "split_horizontal",
            label: "New Column (Horizontal Split)",
            description: "Create a new column to the right.",
            category: ActionCategory::Layout,
            default_binding: "Enter",
        },
        ActionDescriptor {
            name: "split_vertical",
            label: "New Pane in Column (Vertical Split)",
            description: "Add a new pane below the current one in the same column.",
            category: ActionCategory::Layout,
            default_binding: "v",
        },
        ActionDescriptor {
            name: "resize_increase",
            label: "Increase Column Width",
            description: "Widen the active column.",
            category: ActionCategory::Layout,
            default_binding: "=",
        },
        ActionDescriptor {
            name: "resize_decrease",
            label: "Decrease Column Width",
            description: "Narrow the active column.",
            category: ActionCategory::Layout,
            default_binding: "-",
        },
        ActionDescriptor {
            name: "pane_height_increase",
            label: "Increase Pane Height",
            description: "Tallens the active pane within its column.",
            category: ActionCategory::Layout,
            default_binding: "Shift+=",
        },
        ActionDescriptor {
            name: "pane_height_decrease",
            label: "Decrease Pane Height",
            description: "Shortens the active pane within its column.",
            category: ActionCategory::Layout,
            default_binding: "Shift+-",
        },
        ActionDescriptor {
            name: "swap_left",
            label: "Swap Column Left",
            description: "Swap the active column with the one to its left.",
            category: ActionCategory::Layout,
            default_binding: "Ctrl+h",
        },
        ActionDescriptor {
            name: "swap_right",
            label: "Swap Column Right",
            description: "Swap the active column with the one to its right.",
            category: ActionCategory::Layout,
            default_binding: "Ctrl+l",
        },
        ActionDescriptor {
            name: "swap_up",
            label: "Swap Pane Up",
            description: "Swap the active pane with the one above.",
            category: ActionCategory::Layout,
            default_binding: "Ctrl+k",
        },
        ActionDescriptor {
            name: "swap_down",
            label: "Swap Pane Down",
            description: "Swap the active pane with the one below.",
            category: ActionCategory::Layout,
            default_binding: "Ctrl+j",
        },
        ActionDescriptor {
            name: "move_pane_left",
            label: "Move Pane to Column Left",
            description: "Move the active pane into the column on the left.",
            category: ActionCategory::Layout,
            default_binding: "[",
        },
        ActionDescriptor {
            name: "move_pane_right",
            label: "Move Pane to Column Right",
            description: "Move the active pane into the column on the right.",
            category: ActionCategory::Layout,
            default_binding: "]",
        },
        // ── Pane ──
        ActionDescriptor {
            name: "close",
            label: "Close Pane",
            description: "Close the active pane.",
            category: ActionCategory::Pane,
            default_binding: "x",
        },
        ActionDescriptor {
            name: "float",
            label: "Toggle Float",
            description: "Toggle the active pane between tiling and floating.",
            category: ActionCategory::Pane,
            default_binding: "f",
        },
        ActionDescriptor {
            name: "pane_select",
            label: "Quick-Select Pane",
            description: "Show letter labels on all panes; press a letter to focus it.",
            category: ActionCategory::Pane,
            default_binding: "q",
        },
        ActionDescriptor {
            name: "swap_select",
            label: "Quick-Swap Pane",
            description: "Show letter labels on all columns; press a letter to swap with it.",
            category: ActionCategory::Pane,
            default_binding: "Shift+q",
        },
        ActionDescriptor {
            name: "swap_and_focus",
            label: "Swap and Focus",
            description: "Like Quick-Swap, but focus the target after swapping.",
            category: ActionCategory::Pane,
            default_binding: "m",
        },
        ActionDescriptor {
            name: "rename_pane",
            label: "Rename Pane",
            description: "Rename the active pane.",
            category: ActionCategory::Pane,
            default_binding: "Shift+p",
        },
        // ── Workspace ──
        ActionDescriptor {
            name: "create_workspace",
            label: "Create Workspace",
            description: "Create a new workspace and switch to it.",
            category: ActionCategory::Workspace,
            default_binding: "w",
        },
        ActionDescriptor {
            name: "rename_workspace",
            label: "Rename Workspace",
            description: "Rename the current workspace.",
            category: ActionCategory::Workspace,
            default_binding: "Shift+w",
        },
        // ── Chrome ──
        ActionDescriptor {
            name: "sidebar_left",
            label: "Toggle Left Sidebar",
            description: "Show or hide the left sidebar.",
            category: ActionCategory::Chrome,
            default_binding: "b",
        },
        ActionDescriptor {
            name: "sidebar_right",
            label: "Toggle Right Sidebar",
            description: "Show or hide the right sidebar.",
            category: ActionCategory::Chrome,
            default_binding: ".",
        },
        ActionDescriptor {
            name: "sidebar_expand_toggle",
            label: "Toggle Sidebar Expand",
            description: "Expand or collapse the selected sidebar node.",
            category: ActionCategory::Chrome,
            default_binding: "Tab",
        },
        ActionDescriptor {
            name: "tab_next",
            label: "Next Tab",
            description: "Switch to the next tab.",
            category: ActionCategory::Chrome,
            default_binding: "Ctrl+]",
        },
        ActionDescriptor {
            name: "tab_prev",
            label: "Previous Tab",
            description: "Switch to the previous tab.",
            category: ActionCategory::Chrome,
            default_binding: "Ctrl+[",
        },
        // ── System ──
        ActionDescriptor {
            name: "command_palette",
            label: "Command Palette",
            description: "Open the command palette (not yet implemented).",
            category: ActionCategory::System,
            default_binding: "p",
        },
    ];

    /// Look up an action descriptor by its config name.
    pub fn find(name: &str) -> Option<&'static ActionDescriptor> {
        Self::ALL.iter().find(|d| d.name == name)
    }

    /// Return all actions in a given category.
    pub fn by_category(category: ActionCategory) -> impl Iterator<Item = &'static ActionDescriptor> {
        Self::ALL.iter().filter(move |d| d.category == category)
    }

    /// Total number of registered actions.
    pub fn count() -> usize {
        Self::ALL.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_actions() {
        assert!(ActionRegistry::count() > 0, "registry should not be empty");
    }

    #[test]
    fn test_find_known_action() {
        let desc = ActionRegistry::find("focus_left");
        assert!(desc.is_some(), "should find focus_left");
        let desc = desc.unwrap();
        assert_eq!(desc.label, "Focus Column Left");
        assert!(matches!(desc.category, ActionCategory::Navigation));
    }

    #[test]
    fn test_find_unknown_action() {
        assert!(ActionRegistry::find("nonexistent").is_none());
    }

    #[test]
    fn test_by_category() {
        let nav_count = ActionRegistry::by_category(ActionCategory::Navigation).count();
        assert!(nav_count > 0, "should have navigation actions");

        let sys_count = ActionRegistry::by_category(ActionCategory::System).count();
        assert!(sys_count > 0, "should have system actions");
    }

    #[test]
    fn test_all_names_unique() {
        let mut names: Vec<&str> = ActionRegistry::ALL.iter().map(|d| d.name).collect();
        let original_len = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            original_len,
            "all action names must be unique"
        );
    }

    #[test]
    fn test_descriptors_are_populated() {
        for desc in ActionRegistry::ALL {
            assert!(!desc.name.is_empty(), "name must not be empty");
            assert!(!desc.label.is_empty(), "label must not be empty");
            assert!(!desc.description.is_empty(), "description must not be empty");
            assert!(!desc.default_binding.is_empty(), "default_binding must not be empty");
            // Ensure category label is non-empty (exercises ActionCategory::label)
            assert!(!desc.category.label().is_empty(), "category label must not be empty");
        }
    }

    #[test]
    fn test_session_category_exists() {
        // Exercises the Session variant so it is not flagged as dead code.
        let count = ActionRegistry::by_category(ActionCategory::Session).count();
        assert_eq!(count, 0, "no actions in Session category yet, but variant is reserved");
    }
}
