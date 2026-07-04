//! Action registry — dispatch table for all window-manager actions.
//!
//! This module provides the `ActionRegistry` which maps `WmAction` discriminants
//! to named handler functions. It also preserves the static metadata catalog
//! (labels, categories, default bindings) for the command palette and docs.

use heca_grid_ui::Glyph;
use std::collections::HashMap;

/// Handler signature for all window-manager actions.
///
/// The `WmAction` parameter carries the full variant (including any embedded
/// arguments), so the same handler can serve both unit and parameterized
/// variants that share a discriminant.
pub type ActionHandler = fn(&mut crate::app_state::AppState, &crate::input::WmAction);

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
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "no session-level actions exist yet; used when overview/save/load land")
    )]
    Session,
    /// UI chrome: sidebar toggle, tab management.
    Chrome,
    /// System: command palette, quit.
    System,
}

// ActionCategory and its label() are used by ActionDescriptor metadata.
// The metadata catalog is preserved for the command palette (not yet implemented).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "category labels are preserved for command-palette metadata")
)]
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
#[derive(Debug, Clone, Copy)]
// Preserved for the command palette and RPC introspection (not yet implemented).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "descriptor fields are preserved for command-palette and RPC metadata")
)]
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
    /// Centralized action icon. The single source of an action's [`Glyph`] —
    /// every surface that renders this action (pane-action bar, context menu,
    /// command palette) reads it from here instead of inventing its own. `None`
    /// for actions without an assigned icon yet.
    pub icon: Option<Glyph>,
}

/// Registry that maps action discriminants to handler functions.
///
/// Registry that maps action discriminants to handler functions.
///
/// Call `register()` during app initialization to wire up all actions,
/// then `execute()` at runtime to dispatch.
///
/// # Invariants
///
/// - Every `WmAction` variant must have a registered handler in
///   `build_registry()`. In debug builds, `execute()` panics if a handler
///   is missing. In release builds, missing handlers are silently skipped.
/// - Handlers are keyed by `Discriminant<WmAction>`, so all parameterized
///   variants of the same action share one handler (the handler destructures
///   the action to extract arguments).
pub struct ActionRegistry {
    handlers: HashMap<std::mem::Discriminant<crate::input::WmAction>, ActionHandler>,
}

impl ActionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for all variants that share `action`'s discriminant.
    pub fn register(&mut self, action: &crate::input::WmAction, handler: ActionHandler) {
        let disc = crate::input::action_discriminant(action);
        self.handlers.insert(disc, handler);
    }

    /// Execute the handler for `action`, if one is registered.
    ///
    /// In debug builds, panics if no handler is registered (this is a bug —
    /// every `WmAction` variant must have a handler in `build_registry()`).
    /// In release builds, silently does nothing.
    pub fn execute(&self, action: &crate::input::WmAction, state: &mut crate::app_state::AppState) {
        let disc = crate::input::action_discriminant(action);
        if let Some(handler) = self.handlers.get(&disc) {
            handler(state, action);
        } else {
            #[cfg(debug_assertions)]
            panic!("no handler registered for action: {:?}", action);
        }
    }

    /// Check whether a handler is registered for the given action.
    // Used in tests and debugging; kept for future RPC introspection.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by tests and reserved for future RPC introspection")
    )]
    pub fn has_handler(&self, action: &crate::input::WmAction) -> bool {
        let disc = crate::input::action_discriminant(action);
        self.handlers.contains_key(&disc)
    }
}

// ── Static metadata catalog for command palette and RPC introspection ──
// Not yet consumed by runtime UI; preserved for planned features.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "metadata catalog is preserved for planned command-palette and RPC introspection")
)]
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
            icon: None,
        },
        ActionDescriptor {
            name: "focus_right",
            label: "Focus Column Right",
            description: "Move focus to the column on the right.",
            category: ActionCategory::Navigation,
            default_binding: "l",
            icon: None,
        },
        ActionDescriptor {
            name: "focus_up",
            label: "Focus Pane Up",
            description: "Move focus to the pane above in the current column.",
            category: ActionCategory::Navigation,
            default_binding: "k",
            icon: None,
        },
        ActionDescriptor {
            name: "focus_down",
            label: "Focus Pane Down",
            description: "Move focus to the pane below in the current column.",
            category: ActionCategory::Navigation,
            default_binding: "j",
            icon: None,
        },
        ActionDescriptor {
            name: "next_pane",
            label: "Next Pane in Column",
            description: "Cycle focus forward through panes in the active column.",
            category: ActionCategory::Navigation,
            default_binding: "]",
            icon: None,
        },
        ActionDescriptor {
            name: "prev_pane",
            label: "Previous Pane in Column",
            description: "Cycle focus backward through panes in the active column.",
            category: ActionCategory::Navigation,
            default_binding: "[",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_focus",
            label: "Focus Sidebar",
            description: "Enter sidebar navigation mode.",
            category: ActionCategory::Navigation,
            default_binding: "e",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_up",
            label: "Sidebar Cursor Up",
            description: "Move the sidebar selection up.",
            category: ActionCategory::Navigation,
            default_binding: "k",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_down",
            label: "Sidebar Cursor Down",
            description: "Move the sidebar selection down.",
            category: ActionCategory::Navigation,
            default_binding: "j",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_left_nav",
            label: "Sidebar Collapse / Out",
            description: "Collapse the current tree node or move to parent.",
            category: ActionCategory::Navigation,
            default_binding: "h",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_right_nav",
            label: "Sidebar Expand / Enter",
            description: "Expand the current tree node or activate the selected item.",
            category: ActionCategory::Navigation,
            default_binding: "l",
            icon: None,
        },
        ActionDescriptor {
            name: "workspace_next",
            label: "Next Workspace",
            description: "Switch to the next workspace.",
            category: ActionCategory::Navigation,
            default_binding: "d",
            icon: None,
        },
        ActionDescriptor {
            name: "workspace_prev",
            label: "Previous Workspace",
            description: "Switch to the previous workspace.",
            category: ActionCategory::Navigation,
            default_binding: "u",
            icon: None,
        },
        ActionDescriptor {
            name: "focus_toggle_local",
            label: "Toggle Focus Local",
            description: "Toggle between current and last-focused pane in the same workspace.",
            category: ActionCategory::Navigation,
            default_binding: "i",
            icon: None,
        },
        ActionDescriptor {
            name: "focus_toggle_global",
            label: "Toggle Focus Global",
            description: "Toggle between current and last-visited workspace.",
            category: ActionCategory::Navigation,
            default_binding: "Shift+l",
            icon: None,
        },
        // ── Layout ──
        ActionDescriptor {
            name: "split_horizontal",
            label: "New Column (Horizontal Split)",
            description: "Create a new column to the right.",
            category: ActionCategory::Layout,
            default_binding: "Enter",
            icon: Some(Glyph::Plus),
        },
        ActionDescriptor {
            name: "split_vertical",
            label: "New Pane in Column (Vertical Split)",
            description: "Add a new pane below the current one in the same column.",
            category: ActionCategory::Layout,
            default_binding: "v",
            icon: Some(Glyph::SquareSplitVertical),
        },
        ActionDescriptor {
            name: "zoom_column",
            label: "Toggle Column Zoom",
            description: "Toggle the active column between viewport-wide zoom and its previous width.",
            category: ActionCategory::Layout,
            default_binding: "z",
            icon: Some(Glyph::FrameCorners),
        },
        ActionDescriptor {
            name: "scroll_view_left",
            label: "Scroll View Left",
            description: "Pan the horizontal view left to reach off-screen / overflowing columns.",
            category: ActionCategory::Layout,
            default_binding: "Shift+ArrowLeft",
            icon: None,
        },
        ActionDescriptor {
            name: "scroll_view_right",
            label: "Scroll View Right",
            description: "Pan the horizontal view right to reach off-screen / overflowing columns.",
            category: ActionCategory::Layout,
            default_binding: "Shift+ArrowRight",
            icon: None,
        },
        ActionDescriptor {
            name: "resize_increase",
            label: "Increase Column Width",
            description: "Widen the active column.",
            category: ActionCategory::Layout,
            default_binding: "=",
            icon: None,
        },
        ActionDescriptor {
            name: "resize_decrease",
            label: "Decrease Column Width",
            description: "Narrow the active column.",
            category: ActionCategory::Layout,
            default_binding: "-",
            icon: None,
        },
        ActionDescriptor {
            name: "pane_height_increase",
            label: "Increase Pane Height",
            description: "Tallens the active pane within its column.",
            category: ActionCategory::Layout,
            default_binding: "Shift+=",
            icon: None,
        },
        ActionDescriptor {
            name: "pane_height_decrease",
            label: "Decrease Pane Height",
            description: "Shortens the active pane within its column.",
            category: ActionCategory::Layout,
            default_binding: "Shift+-",
            icon: None,
        },
        // ── Font zoom ──
        ActionDescriptor {
            name: "app_font_increase",
            label: "App Font Bigger (Everything)",
            description: "Increase the whole-app font: chrome/UI and every terminal pane.",
            category: ActionCategory::System,
            default_binding: "Ctrl+=",
            icon: None,
        },
        ActionDescriptor {
            name: "app_font_decrease",
            label: "App Font Smaller (Everything)",
            description: "Decrease the whole-app font: chrome/UI and every terminal pane.",
            category: ActionCategory::System,
            default_binding: "Ctrl+-",
            icon: None,
        },
        ActionDescriptor {
            name: "app_font_reset",
            label: "App Font Reset (Everything)",
            description: "Reset the whole-app font to the configured sizes.",
            category: ActionCategory::System,
            default_binding: "Ctrl+0",
            icon: None,
        },
        ActionDescriptor {
            name: "pane_terminal_font_increase",
            label: "Terminal Font Bigger (Pane)",
            description: "Increase the focused pane's terminal font size.",
            category: ActionCategory::Pane,
            default_binding: "Ctrl+Shift+=",
            icon: None,
        },
        ActionDescriptor {
            name: "pane_terminal_font_decrease",
            label: "Terminal Font Smaller (Pane)",
            description: "Decrease the focused pane's terminal font size.",
            category: ActionCategory::Pane,
            default_binding: "Ctrl+Shift+-",
            icon: None,
        },
        ActionDescriptor {
            name: "pane_terminal_font_reset",
            label: "Terminal Font Reset (Pane)",
            description: "Reset the focused pane to follow the app-wide font size.",
            category: ActionCategory::Pane,
            default_binding: "Ctrl+Shift+0",
            icon: None,
        },
        ActionDescriptor {
            name: "swap_left",
            label: "Swap Column Left",
            description: "Swap the active column with the one to its left.",
            category: ActionCategory::Layout,
            default_binding: "Ctrl+h",
            icon: None,
        },
        ActionDescriptor {
            name: "swap_right",
            label: "Swap Column Right",
            description: "Swap the active column with the one to its right.",
            category: ActionCategory::Layout,
            default_binding: "Ctrl+l",
            icon: None,
        },
        ActionDescriptor {
            name: "swap_up",
            label: "Swap Pane Up",
            description: "Swap the active pane with the one above.",
            category: ActionCategory::Layout,
            default_binding: "Ctrl+k",
            icon: None,
        },
        ActionDescriptor {
            name: "swap_down",
            label: "Swap Pane Down",
            description: "Swap the active pane with the one below.",
            category: ActionCategory::Layout,
            default_binding: "Ctrl+j",
            icon: None,
        },
        ActionDescriptor {
            name: "move_pane_left",
            label: "Move Pane to Column Left",
            description: "Move the active pane into the column on the left.",
            category: ActionCategory::Layout,
            default_binding: "[",
            icon: Some(Glyph::ArrowLineLeft),
        },
        ActionDescriptor {
            name: "move_pane_right",
            label: "Move Pane to Column Right",
            description: "Move the active pane into the column on the right.",
            category: ActionCategory::Layout,
            default_binding: "]",
            icon: Some(Glyph::ArrowLineRight),
        },
        ActionDescriptor {
            name: "delete_column",
            label: "Delete Column",
            description: "Delete the focused column and all its panes.",
            category: ActionCategory::Layout,
            default_binding: "unbound",
            icon: None,
        },
        // ── Pane ──
        ActionDescriptor {
            name: "close",
            label: "Close Pane",
            description: "Close the active pane.",
            category: ActionCategory::Pane,
            default_binding: "x",
            icon: Some(Glyph::XSquare),
        },
        ActionDescriptor {
            name: "float",
            label: "Toggle Float",
            description: "Toggle the active pane between tiling and floating.",
            category: ActionCategory::Pane,
            default_binding: "f",
            icon: Some(Glyph::Cards),
        },
        ActionDescriptor {
            name: "pane_select",
            label: "Quick-Select Pane",
            description: "Press a letter to focus it.",
            category: ActionCategory::Pane,
            default_binding: "q",
            icon: None,
        },
        ActionDescriptor {
            name: "open_link",
            label: "Open Link",
            description: "Open the hyperlink in the OS default handler.",
            category: ActionCategory::Pane,
            // Constructed with a URL (mouse/HintKey/selection/menu); no global key.
            default_binding: "",
            icon: Some(Glyph::ArrowRight),
        },
        ActionDescriptor {
            name: "follow_link",
            label: "Follow Link",
            description: "Press a letter to open the link.",
            category: ActionCategory::Pane,
            default_binding: "Shift+o",
            icon: None,
        },
        // Pick-mode prompts (`description`) double as the in-progress pick text shown
        // in `InputMode::pending_pick()` — single source of truth, not duplicated. The
        // "+ focus" variants make the focus-follow difference explicit.
        ActionDescriptor {
            name: "swap_pane",
            label: "Quick-Swap Pane",
            description: "Select a pane to swap with — focus stays where it is.",
            category: ActionCategory::Pane,
            default_binding: "Shift+m",
            icon: None,
        },
        ActionDescriptor {
            name: "swap_and_focus_pane",
            label: "Swap and Focus",
            description: "Select a pane to swap with, then follow focus to it.",
            category: ActionCategory::Pane,
            default_binding: "m",
            icon: None,
        },
        ActionDescriptor {
            name: "pane_take",
            label: "Take Pane",
            description: "Select a pane to pull into the active column — focus stays where it is.",
            category: ActionCategory::Pane,
            default_binding: "t",
            icon: None,
        },
        ActionDescriptor {
            name: "pane_take_and_focus",
            label: "Take and Focus",
            description: "Select a pane to pull into the active column, then focus it.",
            category: ActionCategory::Pane,
            default_binding: "Shift+t",
            icon: None,
        },
        ActionDescriptor {
            name: "move_pane_to_workspace_pick",
            label: "Move Pane to Workspace",
            description: "Select a workspace to move the active pane to.",
            category: ActionCategory::Layout,
            default_binding: "g",
            icon: None,
        },
        ActionDescriptor {
            name: "move_column_to_workspace_pick",
            label: "Move Column to Workspace",
            description: "Select a workspace to move the active column to.",
            category: ActionCategory::Layout,
            default_binding: "c",
            icon: None,
        },
        ActionDescriptor {
            name: "move_pane_to_column_pick",
            label: "Move Pane to Column",
            description: "Select a column to move the active pane into.",
            category: ActionCategory::Layout,
            default_binding: "Shift+c",
            icon: None,
        },
        ActionDescriptor {
            name: "rename_pane",
            label: "Rename Pane",
            description: "Rename the active pane/tab.",
            category: ActionCategory::Pane,
            default_binding: "$",
            icon: None,
        },
        ActionDescriptor {
            name: "rename_column",
            label: "Rename Column",
            description: "Rename the active column.",
            category: ActionCategory::Layout,
            default_binding: "Shift+c",
            icon: None,
        },
        // ── Workspace ──
        ActionDescriptor {
            name: "create_workspace",
            label: "Create Workspace",
            description: "Create a new workspace and switch to it.",
            category: ActionCategory::Workspace,
            default_binding: "w",
            icon: None,
        },
        ActionDescriptor {
            name: "rename_workspace",
            label: "Rename Workspace",
            description: "Rename the current workspace.",
            category: ActionCategory::Workspace,
            default_binding: "Shift+w",
            icon: None,
        },
        ActionDescriptor {
            name: "delete_workspace",
            label: "Delete Workspace",
            description: "Delete a workspace and all its panes (not the last workspace).",
            category: ActionCategory::Workspace,
            default_binding: "unbound",
            icon: None,
        },
        // ── Chrome ──
        ActionDescriptor {
            name: "sidebar_left",
            label: "Toggle Left Sidebar",
            description: "Show or hide the left sidebar.",
            category: ActionCategory::Chrome,
            default_binding: "b",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_right",
            label: "Toggle Right Sidebar",
            description: "Show or hide the right sidebar.",
            category: ActionCategory::Chrome,
            default_binding: ".",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_expand_toggle",
            label: "Toggle Sidebar Expand",
            description: "Expand or collapse the selected sidebar node.",
            category: ActionCategory::Chrome,
            default_binding: "Tab",
            icon: None,
        },
        // Chrome region show/hide mounted-gate (sidebar-fu-6). Unbound by default
        // (listed in the `UNBOUND` test allowlist); the user binds them in config.
        ActionDescriptor {
            name: "show_left_sidebar",
            label: "Show Left Sidebar",
            description: "Mount (show) the left sidebar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "hide_left_sidebar",
            label: "Hide Left Sidebar",
            description: "Unmount (hide) the left sidebar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "toggle_left_sidebar",
            label: "Toggle Left Sidebar (show/hide)",
            description: "Mount or unmount the left sidebar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "show_right_sidebar",
            label: "Show Right Sidebar",
            description: "Mount (show) the right sidebar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "hide_right_sidebar",
            label: "Hide Right Sidebar",
            description: "Unmount (hide) the right sidebar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "toggle_right_sidebar",
            label: "Toggle Right Sidebar (show/hide)",
            description: "Mount or unmount the right sidebar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "show_top_bar",
            label: "Show Top Bar",
            description: "Mount (show) the top bar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "hide_top_bar",
            label: "Hide Top Bar",
            description: "Unmount (hide) the top bar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "toggle_top_bar",
            label: "Toggle Top Bar (show/hide)",
            description: "Mount or unmount the top bar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "show_bottom_bar",
            label: "Show Bottom Bar",
            description: "Mount (show) the bottom bar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "hide_bottom_bar",
            label: "Hide Bottom Bar",
            description: "Unmount (hide) the bottom bar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "toggle_bottom_bar",
            label: "Toggle Bottom Bar (show/hide)",
            description: "Mount or unmount the bottom bar region.",
            category: ActionCategory::Chrome,
            default_binding: "",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_create_workspace",
            label: "Sidebar Create Workspace",
            description: "Create a new workspace from the current sidebar selection context.",
            category: ActionCategory::Workspace,
            default_binding: "w",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_create_column",
            label: "Sidebar Create Column",
            description: "Create a new column in the selected sidebar workspace context.",
            category: ActionCategory::Layout,
            default_binding: "c",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_split_in_column",
            label: "Sidebar Split in Column",
            description: "Add a new pane in the selected sidebar column context.",
            category: ActionCategory::Layout,
            default_binding: "v",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_zoom_selected_column",
            label: "Sidebar Zoom Selected Column",
            description: "Toggle zoom for the column implied by the current sidebar selection.",
            category: ActionCategory::Layout,
            default_binding: "z",
            icon: None,
        },
        ActionDescriptor {
            name: "sidebar_delete_selected",
            label: "Sidebar Delete Selected",
            description: "Delete the selected sidebar item with confirmation.",
            category: ActionCategory::Pane,
            default_binding: "d",
            icon: None,
        },
        ActionDescriptor {
            name: "collapse_current_workspace",
            label: "Collapse Current Workspace Row",
            description: "Collapse the active workspace row in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            default_binding: "unbound",
            icon: None,
        },
        ActionDescriptor {
            name: "expand_current_workspace",
            label: "Expand Current Workspace Row",
            description: "Expand the active workspace row in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            default_binding: "unbound",
            icon: None,
        },
        ActionDescriptor {
            name: "toggle_current_workspace_collapsed",
            label: "Toggle Current Workspace Row",
            description: "Toggle the active workspace row collapsed state in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            default_binding: "<",
            icon: None,
        },
        ActionDescriptor {
            name: "collapse_current_column",
            label: "Collapse Current Column Row",
            description: "Collapse the focused tiled column row in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            default_binding: "unbound",
            icon: None,
        },
        ActionDescriptor {
            name: "expand_current_column",
            label: "Expand Current Column Row",
            description: "Expand the focused tiled column row in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            default_binding: "unbound",
            icon: None,
        },
        ActionDescriptor {
            name: "toggle_current_column_collapsed",
            label: "Toggle Current Column Row",
            description: "Toggle the focused tiled column row collapsed state in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            default_binding: "(",
            icon: None,
        },
        // ── System ──
        ActionDescriptor {
            name: "command_palette",
            label: "Command Palette",
            description: "Open the command palette (not yet implemented).",
            category: ActionCategory::System,
            default_binding: "p",
            icon: None,
        },
        ActionDescriptor {
            name: "reload_config",
            label: "Reload Config",
            description: "Reload keymaps, theme, and settings from config.toml without restarting.",
            category: ActionCategory::System,
            default_binding: "Shift+r",
            icon: None,
        },
        // ── Scrollback (host terminal viewport) ──
        ActionDescriptor {
            name: "scrollback_page_up",
            label: "Scrollback Page Up",
            description: "Scroll the terminal viewport up by one page and enter selection mode.",
            category: ActionCategory::Pane,
            default_binding: "PageUp",
            icon: None,
        },
        ActionDescriptor {
            name: "scrollback_page_down",
            label: "Scrollback Page Down",
            description: "Scroll the terminal viewport down by one page and enter selection mode.",
            category: ActionCategory::Pane,
            default_binding: "PageDown",
            icon: None,
        },
        ActionDescriptor {
            name: "scrollback_line_up",
            label: "Scrollback Line Up",
            description: "Scroll the terminal viewport up by a configurable number of lines (selection mode).",
            category: ActionCategory::Pane,
            default_binding: "u",
            icon: None,
        },
        ActionDescriptor {
            name: "scrollback_line_down",
            label: "Scrollback Line Down",
            description: "Scroll the terminal viewport down by a configurable number of lines (selection mode).",
            category: ActionCategory::Pane,
            default_binding: "d",
            icon: None,
        },
        ActionDescriptor {
            name: "scrollback_to_top",
            label: "Scrollback to Top",
            description: "Jump the terminal viewport to the top of scrollback history.",
            category: ActionCategory::Pane,
            default_binding: "g,Home",
            icon: None,
        },
        ActionDescriptor {
            name: "scrollback_to_bottom",
            label: "Scrollback to Bottom",
            description: "Snap the terminal viewport to the live bottom (latest output).",
            category: ActionCategory::Pane,
            default_binding: "Shift+g,End",
            icon: None,
        },
        ActionDescriptor {
            name: "exit_scrollback",
            label: "Exit Scrollback",
            description: "Snap to the live bottom, clear selection, and exit selection mode.",
            category: ActionCategory::Pane,
            default_binding: "Escape",
            icon: None,
        },
        // ── Direct scroll (non-prefix, no selection mode entry) ──
        ActionDescriptor {
            name: "scroll_page_up",
            label: "Scroll Page Up",
            description: "Scroll the terminal viewport up by one page immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            default_binding: "Shift+PageUp",
            icon: None,
        },
        ActionDescriptor {
            name: "scroll_page_down",
            label: "Scroll Page Down",
            description: "Scroll the terminal viewport down by one page immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            default_binding: "Shift+PageDown",
            icon: None,
        },
        ActionDescriptor {
            name: "scroll_line_up",
            label: "Scroll Line Up",
            description: "Scroll the terminal viewport up by a configurable number of lines immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            default_binding: "Shift+Up",
            icon: None,
        },
        ActionDescriptor {
            name: "scroll_line_down",
            label: "Scroll Line Down",
            description: "Scroll the terminal viewport down by a configurable number of lines immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            default_binding: "Shift+Down",
            icon: None,
        },
        ActionDescriptor {
            name: "scroll_to_top",
            label: "Scroll to Top",
            description: "Jump the terminal viewport to the top of scrollback history immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            default_binding: "Shift+Home",
            icon: None,
        },
        ActionDescriptor {
            name: "scroll_to_bottom",
            label: "Scroll to Bottom",
            description: "Snap the terminal viewport to the live bottom immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            default_binding: "Shift+End",
            icon: None,
        },
        ActionDescriptor {
            name: "scroll_to_offset",
            label: "Scroll to Offset",
            description: "Jump the terminal viewport to an explicit offset in rows above the live bottom. Used by the GUI scrollbar and RPC; no default keybinding.",
            category: ActionCategory::Pane,
            default_binding: "unbound",
            icon: None,
        },
        // ── Selection (host capability) ──
        ActionDescriptor {
            name: "enter_selection_mode",
            label: "Enter Selection Mode",
            description: "Enter the host-owned selection input mode. Selection data is driven by surface adapters (mouse, keyboard, RPC).",
            category: ActionCategory::Pane,
            default_binding: "s",
            icon: None,
        },
        ActionDescriptor {
            name: "selection_left",
            label: "Selection Left",
            description: "Move the active selection focus one cell left in selection mode.",
            category: ActionCategory::Pane,
            default_binding: "h,ArrowLeft",
            icon: None,
        },
        ActionDescriptor {
            name: "selection_right",
            label: "Selection Right",
            description: "Move the active selection focus one cell right in selection mode.",
            category: ActionCategory::Pane,
            default_binding: "l,ArrowRight",
            icon: None,
        },
        ActionDescriptor {
            name: "selection_up",
            label: "Selection Up",
            description: "Move the active selection focus one row up in selection mode.",
            category: ActionCategory::Pane,
            default_binding: "k,ArrowUp",
            icon: None,
        },
        ActionDescriptor {
            name: "selection_down",
            label: "Selection Down",
            description: "Move the active selection focus one row down in selection mode.",
            category: ActionCategory::Pane,
            default_binding: "j,ArrowDown",
            icon: None,
        },
        ActionDescriptor {
            name: "clear_selection",
            label: "Clear Selection",
            description: "Clear the active selection and exit selection mode if active.",
            category: ActionCategory::Pane,
            default_binding: "Shift+s",
            icon: None,
        },
        ActionDescriptor {
            name: "copy_selection",
            label: "Copy Selection",
            description: "Copy the active selection text to the system clipboard.",
            category: ActionCategory::Pane,
            default_binding: "y",
            icon: None,
        },
        ActionDescriptor {
            name: "paste_clipboard",
            label: "Paste Clipboard",
            description: "Paste system clipboard content into the focused pane (bracketed-paste aware).",
            category: ActionCategory::Pane,
            default_binding: "unbound",
            icon: None,
        },
        ActionDescriptor {
            name: "begin_selection",
            label: "Begin Selection",
            description: "Start a selection from the caret position in selection mode. No-op if a selection already exists — clear first to restart.",
            category: ActionCategory::Pane,
            default_binding: "v",
            icon: None,
        },
        ActionDescriptor {
            name: "toggle_selection_endpoint",
            label: "Toggle Selection Endpoint",
            description: "Swap which end of the selection is active so movement grows from the other side.",
            category: ActionCategory::Pane,
            default_binding: "o",
            icon: None,
        },
        ActionDescriptor {
            name: "open_link_at_caret",
            label: "Open Link at Caret",
            description: "Open the hyperlink under the selection caret.",
            category: ActionCategory::Pane,
            default_binding: "Shift+o",
            icon: None,
        },
        ActionDescriptor {
            name: "search_scrollback",
            label: "Search Scrollback",
            description: "Type to search the scrollback; Enter keeps matches, Esc cancels.",
            category: ActionCategory::Pane,
            default_binding: "/",
            icon: Some(Glyph::Search),
        },
        ActionDescriptor {
            name: "search_next_match",
            label: "Next Search Match",
            description: "Jump to the next scrollback-search match.",
            category: ActionCategory::Pane,
            default_binding: "n",
            icon: None,
        },
        ActionDescriptor {
            name: "search_prev_match",
            label: "Previous Search Match",
            description: "Jump to the previous scrollback-search match.",
            category: ActionCategory::Pane,
            default_binding: "Shift+n",
            icon: None,
        },
    ];

    /// Look up an action descriptor by its config name.
    pub fn find(name: &str) -> Option<&'static ActionDescriptor> {
        Self::ALL.iter().find(|d| d.name == name)
    }

    /// The icon [`Glyph`] for an action, by config name — read straight from the
    /// action's [`ActionDescriptor::icon`]. The single registry-level source of
    /// action iconography: every surface (pane-action bar, context menu, command
    /// palette) resolves icons through here, so none invents its own.
    pub fn icon(name: &str) -> Option<Glyph> {
        Self::find(name).and_then(|d| d.icon)
    }

    /// Return all actions in a given category.
    pub fn by_category(
        category: ActionCategory,
    ) -> impl Iterator<Item = &'static ActionDescriptor> {
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
    use crate::input::WmAction;

    #[test]
    fn test_registry_dispatch() {
        let mut registry = ActionRegistry::new();
        fn dummy_handler(state: &mut crate::app_state::AppState, _action: &WmAction) {
            state.needs_redraw = true;
        }
        registry.register(&WmAction::FocusLeft, dummy_handler);
        assert!(registry.has_handler(&WmAction::FocusLeft));
        assert!(!registry.has_handler(&WmAction::FocusRight));
    }

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
    fn test_selection_action_descriptors_exist() {
        for name in [
            "enter_selection_mode",
            "selection_left",
            "selection_right",
            "selection_up",
            "selection_down",
            "clear_selection",
            "copy_selection",
            "paste_clipboard",
        ] {
            let desc = ActionRegistry::find(name);
            assert!(desc.is_some(), "missing descriptor for {name}");
            let desc = desc.unwrap();
            assert!(
                !desc.default_binding.is_empty(),
                "{name} default_binding must be set"
            );
            assert!(!desc.label.is_empty(), "{name} label must be set");
            assert!(
                !desc.description.is_empty(),
                "{name} description must be set"
            );
        }
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
        assert_eq!(names.len(), original_len, "all action names must be unique");
    }

    #[test]
    fn test_descriptors_are_populated() {
        // Actions invoked only programmatically / by mouse / by context menu have no
        // global keybinding, so their `default_binding` is intentionally empty.
        const UNBOUND: &[&str] = &[
            "open_link",
            // Chrome region show/hide (sidebar-fu-6): intentionally unbound — the
            // user binds the wanted ones in config.
            "show_left_sidebar",
            "hide_left_sidebar",
            "toggle_left_sidebar",
            "show_right_sidebar",
            "hide_right_sidebar",
            "toggle_right_sidebar",
            "show_top_bar",
            "hide_top_bar",
            "toggle_top_bar",
            "show_bottom_bar",
            "hide_bottom_bar",
            "toggle_bottom_bar",
        ];
        for desc in ActionRegistry::ALL {
            assert!(!desc.name.is_empty(), "name must not be empty");
            assert!(!desc.label.is_empty(), "label must not be empty");
            assert!(
                !desc.description.is_empty(),
                "description must not be empty"
            );
            if !UNBOUND.contains(&desc.name) {
                assert!(
                    !desc.default_binding.is_empty(),
                    "default_binding must not be empty for {}",
                    desc.name
                );
            }
            assert!(
                !desc.category.label().is_empty(),
                "category label must not be empty"
            );
        }
    }

    #[test]
    fn test_session_category_exists() {
        let count = ActionRegistry::by_category(ActionCategory::Session).count();
        assert_eq!(
            count, 0,
            "no actions in Session category yet, but variant is reserved"
        );
    }
}
