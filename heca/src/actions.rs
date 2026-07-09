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

/// Static descriptor for a window-manager action. The built-in set lives in
/// [`ActionRegistry::ALL`]; [`ActionCatalog`] loads them into the runtime, plugin-extensible
/// metadata surface every UI reads from.
#[derive(Debug, Clone, Copy)]
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
    // Preserved for the command palette + RPC introspection (read in tests only for now).
    #[cfg_attr(not(test), allow(dead_code))]
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

// ── Built-in action metadata seed ──
// The compile-time catalog of built-in actions. `ActionCatalog::with_builtins()` loads this into
// the runtime, plugin-extensible catalog owned by `AppState`; every UI surface resolves metadata
// through that catalog, not this const directly.
impl ActionRegistry {
    /// The built-in action descriptors, in a stable order. Seed for [`ActionCatalog`].
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
            name: "open_context_menu",
            label: "Open Context Menu",
            description: "Open the focused pane's context menu at the cursor.",
            category: ActionCategory::Layout,
            default_binding: ">",
            icon: Some(Glyph::DotsThreeVertical),
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

}

/// Runtime catalog of action metadata, owned by [`AppState`](crate::app_state::AppState).
///
/// Seeded from the built-in [`ActionRegistry::ALL`] descriptors at startup and (plugin action API)
/// extended by plugins. The single runtime home every UI surface resolves action metadata through —
/// replacing the old `ActionRegistry::find/icon/label/...` statics, so the set of actions (and their
/// icons/labels/confirmation) is a runtime, extensible surface rather than a compile-time constant.
///
/// Phase A holds built-ins as `&'static ActionDescriptor` (zero-copy). Owned plugin entries (with
/// `String` metadata) arrive with the plugin action API.
pub struct ActionCatalog {
    by_name: HashMap<&'static str, &'static ActionDescriptor>,
    order: Vec<&'static ActionDescriptor>,
    /// Declarative confirmation requirement per action **config name** (see [`ConfirmSpec`]). The
    /// central gate reads this to decide whether an action needs a confirm/response prompt — the
    /// guard lives on the action, not the call site. Seeded with the built-in destructive actions;
    /// plugins register their own with the plugin action API.
    confirm: HashMap<&'static str, ConfirmSpec>,
}

impl ActionCatalog {
    /// Build the catalog seeded from the built-in [`ActionRegistry::ALL`] descriptors.
    pub fn with_builtins() -> Self {
        let order: Vec<&'static ActionDescriptor> = ActionRegistry::ALL.iter().collect();
        let by_name = order.iter().map(|d| (d.name, *d)).collect();
        Self {
            by_name,
            order,
            confirm: builtin_confirm_specs(),
        }
    }

    /// The declarative confirmation spec for an action **config name**, if it needs confirmation.
    pub fn confirm_spec(&self, name: &str) -> Option<&ConfirmSpec> {
        self.confirm.get(name)
    }

    /// Look up an action descriptor by its config name.
    pub fn find(&self, name: &str) -> Option<&'static ActionDescriptor> {
        self.by_name.get(name).copied()
    }

    /// The icon [`Glyph`] for an action, by config name — the single source of action iconography
    /// (pane-action bar, context menu, command palette all resolve through here).
    pub fn icon(&self, name: &str) -> Option<Glyph> {
        self.find(name).and_then(|d| d.icon)
    }

    /// The human-readable label for an action, by config name — so a caller names the action
    /// rather than re-spelling the label.
    pub fn label(&self, name: &str) -> Option<&'static str> {
        self.find(name).map(|d| d.label)
    }

    /// All actions in a given category, in stable order.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "preserved for the command palette + RPC introspection")
    )]
    pub fn by_category(
        &self,
        category: ActionCategory,
    ) -> impl Iterator<Item = &'static ActionDescriptor> + '_ {
        self.order.iter().copied().filter(move |d| d.category == category)
    }

    /// Total number of catalogued actions.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "preserved for the command palette + RPC introspection")
    )]
    pub fn count(&self) -> usize {
        self.order.len()
    }
}

impl Default for ActionCatalog {
    fn default() -> Self {
        Self::with_builtins()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Declarative action confirmation / response (action-interaction plan, Phase B)
// ══════════════════════════════════════════════════════════════════════════════

/// A **native** outcome callback (see [`Outcome::Callback`]). Runs once when its response button is
/// chosen, receiving the live [`AppState`](crate::app_state::AppState) + [`ActionRegistry`].
///
/// **NATIVE-ONLY — read before using.** A closure is not serializable, so it can never cross the
/// WASM or RPC boundary; the plugin-facing builder does **not** expose it, and when an action's
/// metadata is serialized (RPC/introspection) a `Callback` outcome is rendered **opaquely**
/// (e.g. `"native"`), never silently dropped. **Prefer [`Outcome::Dispatch`]** (portable, testable,
/// RPC-drivable): reach for `Callback` only when the logic genuinely cannot be a named action — and
/// first ask whether a small native action + `Dispatch` is cleaner. It runs *after* the user chose,
/// so it executes directly and does not re-enter interaction policy; do not use it to smuggle
/// un-gated destructive work (compose `Dispatch`/`Proceed` for that). Captured state must be
/// `'static` (the `Rc`), like every [`open_modal`](crate::chrome::open_modal) completion.
pub type ConfirmCallback = std::rc::Rc<dyn Fn(&mut crate::app_state::AppState, &ActionRegistry)>;

/// The role of a confirmation response button — drives initial focus (Default/Cancel), the danger
/// tint (Danger), and which button Esc / scrim maps to (Cancel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonRole {
    /// The safe default — takes initial focus so Enter activates it.
    Default,
    /// The cancel choice — Esc / scrim (when dismissible) resolve to this button's outcome.
    Cancel,
    /// A destructive choice — rendered with the danger tint.
    Danger,
}

/// What choosing a response button does. Declarative variants (`Proceed`/`Cancel`/`Dispatch`) are
/// serializable and plugin/RPC-safe; [`Callback`](Outcome::Callback) is a native-only escape hatch.
#[derive(Clone)]
pub enum Outcome {
    /// Run the **original gated action** (the "yes, do it"). Executed via `registry.execute`, which
    /// bypasses the dispatch gate that raised the prompt (no loop).
    Proceed,
    /// Do nothing.
    Cancel,
    /// Dispatch **another** action (declarative — plugin/RPC-safe); it is policy-routed normally.
    // Used by plugin-declared confirmations (Phase C) + tests; the built-in specs use Proceed/Cancel.
    #[cfg_attr(not(test), allow(dead_code))]
    Dispatch(crate::input::WmAction),
    /// Run a native closure. **Native-only** — see [`ConfirmCallback`].
    #[cfg_attr(not(test), allow(dead_code))]
    Callback(ConfirmCallback),
}

/// One response button of a [`ConfirmSpec`].
#[derive(Clone)]
pub struct ResponseButton {
    /// Comes back in [`ModalResult::Action`](crate::chrome::ModalResult); also the tooltip action id.
    pub id: String,
    pub label: String,
    pub role: ButtonRole,
    pub outcome: Outcome,
}

impl ResponseButton {
    /// A `Cancel`-role button that does nothing (the safe default choice).
    pub fn cancel(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role: ButtonRole::Cancel,
            outcome: Outcome::Cancel,
        }
    }

    /// A button that runs the original action ([`Outcome::Proceed`]); `danger` gives it the
    /// destructive tint + `Danger` role, otherwise the `Default` role.
    pub fn proceed(id: impl Into<String>, label: impl Into<String>, danger: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role: if danger { ButtonRole::Danger } else { ButtonRole::Default },
            outcome: Outcome::Proceed,
        }
    }

    /// A fully-specified button.
    // Used by plugin-declared confirmations (Phase C) + tests; the built-in specs use the
    // `cancel`/`proceed` constructors.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        role: ButtonRole,
        outcome: Outcome,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role,
            outcome,
        }
    }
}

/// Declarative confirmation / response requirement attached to an action. Pure data (the native
/// [`Outcome::Callback`] aside): the central gate converts it to a
/// [`ModalSpec`](crate::chrome::ModalSpec) at open time and runs the chosen button's [`Outcome`].
/// The action's **title** stays dynamic (computed by the gate from the concrete target); this spec
/// owns the reusable parts — the body message, the response buttons + outcomes, whether the choice
/// is forced, and the on/off config key.
#[derive(Clone)]
pub struct ConfirmSpec {
    /// Body message under the (dynamic) title — e.g. "This action cannot be undone."
    pub message: String,
    /// The response buttons (2 for yes/no, 3 for yes/no/cancel, N for anything).
    pub buttons: Vec<ResponseButton>,
    /// `false` = forced decision (Esc / scrim swallowed) — mirrors `Dialog::dismissible`.
    pub dismissible: bool,
    /// Config key under `[confirm]` that toggles this prompt on/off (Phase B maps the three
    /// built-ins to the existing `[settings] confirm_*` flags; the generic `[confirm]` table is
    /// Phase B2). Defaults to [`default_enabled`](ConfirmSpec::default_enabled) when unset.
    pub config_name: String,
    pub default_enabled: bool,
}

/// The built-in confirmation specs, keyed by action config name. The three destructive actions —
/// close pane / delete column / delete workspace — each get a `[Cancel] [<verb>]` forced prompt.
fn builtin_confirm_specs() -> HashMap<&'static str, ConfirmSpec> {
    let mk = |config_name: &'static str, verb: &str| ConfirmSpec {
        message: "This action cannot be undone.".to_string(),
        buttons: vec![
            ResponseButton::cancel("cancel", "Cancel"),
            ResponseButton::proceed("confirm", verb, true),
        ],
        dismissible: false,
        config_name: config_name.to_string(),
        default_enabled: true,
    };
    HashMap::from([
        ("delete_pane", mk("delete_pane", "Close")),
        ("delete_column", mk("delete_column", "Delete")),
        ("delete_workspace", mk("delete_workspace", "Delete")),
    ])
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
        assert!(
            ActionCatalog::with_builtins().count() > 0,
            "catalog should not be empty"
        );
    }

    #[test]
    fn test_find_known_action() {
        let catalog = ActionCatalog::with_builtins();
        let desc = catalog.find("focus_left");
        assert!(desc.is_some(), "should find focus_left");
        let desc = desc.unwrap();
        assert_eq!(desc.label, "Focus Column Left");
        assert!(matches!(desc.category, ActionCategory::Navigation));
    }

    #[test]
    fn test_find_unknown_action() {
        assert!(ActionCatalog::with_builtins().find("nonexistent").is_none());
    }

    #[test]
    fn test_selection_action_descriptors_exist() {
        let catalog = ActionCatalog::with_builtins();
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
            let desc = catalog.find(name);
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
        let catalog = ActionCatalog::with_builtins();
        let nav_count = catalog.by_category(ActionCategory::Navigation).count();
        assert!(nav_count > 0, "should have navigation actions");

        let sys_count = catalog.by_category(ActionCategory::System).count();
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
    fn builtin_confirm_specs_are_declared_for_the_destructive_actions() {
        let catalog = ActionCatalog::with_builtins();
        // The three destructive actions each carry a forced [Cancel] [<danger Proceed>] prompt.
        for name in ["delete_pane", "delete_column", "delete_workspace"] {
            let spec = catalog
                .confirm_spec(name)
                .unwrap_or_else(|| panic!("missing confirm spec for {name}"));
            assert!(!spec.dismissible, "{name} is a forced decision");
            assert!(spec.default_enabled);
            assert_eq!(spec.config_name, name);
            assert_eq!(spec.buttons.len(), 2, "{name}: cancel + confirm");
            assert_eq!(spec.buttons[0].role, ButtonRole::Cancel);
            assert_eq!(spec.buttons[1].role, ButtonRole::Danger);
            assert!(matches!(spec.buttons[1].outcome, Outcome::Proceed));
        }
        // Non-destructive actions carry no confirm spec.
        assert!(catalog.confirm_spec("focus_left").is_none());
    }

    #[test]
    fn outcome_variants_compose() {
        // All four outcomes construct (the declarative three + the native Callback). This also
        // exercises the ResponseButton constructors + `new`.
        let fired = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let f = fired.clone();
        let buttons = [
            ResponseButton::cancel("cancel", "Cancel"),
            ResponseButton::proceed("ok", "OK", false),
            ResponseButton::new(
                "other",
                "Discard",
                ButtonRole::Default,
                Outcome::Dispatch(crate::input::WmAction::ReloadConfig),
            ),
            ResponseButton::new(
                "cb",
                "Run",
                ButtonRole::Default,
                Outcome::Callback(std::rc::Rc::new(move |_state, _reg| f.set(f.get() + 1))),
            ),
        ];
        assert_eq!(buttons.len(), 4);
        // The callback is a stored `Rc<dyn Fn>` — invoking it (as `run_outcome` would) runs once.
        if let Outcome::Callback(cb) = &buttons[3].outcome {
            // Can't build a full AppState/registry here, so just confirm the closure is wired;
            // end-to-end firing is covered by in-app verification + the gate's existing tests.
            let _ = cb; // callback is present and typed correctly
        } else {
            panic!("expected a Callback outcome");
        }
        assert_eq!(fired.get(), 0, "constructing does not fire the callback");
    }

    #[test]
    fn test_session_category_exists() {
        let count = ActionCatalog::with_builtins()
            .by_category(ActionCategory::Session)
            .count();
        assert_eq!(
            count, 0,
            "no actions in Session category yet, but variant is reserved"
        );
    }
}
