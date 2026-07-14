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
    /// Handlers for **name-keyed** actions registered at runtime by providers (and later WASM
    /// plugins) — the actions that have no [`WmAction`](crate::input::WmAction) variant because the
    /// enum is closed and a plugin cannot extend it. Their *metadata* lives in the one
    /// [`ActionCatalog`], next to the built-ins; only the handler lives here. See [`Dispatch`].
    dyn_handlers: HashMap<String, DynHandler>,
}

/// How an action is run — the three back ends behind the one dispatch door
/// (`dispatch_view_intent`). There is no parallel dispatch path.
///
/// This is a *description* of the registry's contents, used by
/// [`ActionRegistry::dispatch_of`] to answer "how would this name run?" for introspection and
/// tests; the dispatch itself is keyed by discriminant (built-ins) or by name (the rest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    /// A built-in: `fn(&mut AppState, &WmAction)` keyed by `Discriminant<WmAction>`. Parameterized
    /// variants share one handler (it destructures the action for its args).
    Native,
    /// Name-keyed **with** a native host handler — a first-party provider contributing an action of
    /// its own, before any WASM exists. Receives the `Intent`, so its args arrive as data.
    NativeDyn,
    /// Name-keyed with **no** host handler: the action is declared (it has metadata, a policy, a
    /// name) but the host cannot run it — it is forwarded to its owner across the plugin boundary
    /// (WASM, plugin-08). Dispatching one today is a no-op with a debug warning.
    Declarative,
}

/// The handler for a name-keyed action. Unlike [`ActionHandler`] (a bare `fn` pointer keyed by
/// discriminant), this is a closure — a provider closes over its own state — and it receives the
/// [`Intent`](crate::chrome::Intent), so its arguments arrive as serializable data rather than as
/// an enum variant's fields.
///
/// It takes `&mut AppState` deliberately: §2.3 forbids a provider from mutating app state from its
/// *build/observe* path, but an action handler **is** the sanctioned write path — dispatching an
/// action is exactly how a provider is supposed to change things.
pub type DynHandler = std::rc::Rc<dyn Fn(&mut crate::app_state::AppState, &crate::chrome::Intent)>;

/// RAII handle for a registered dynamic action.
///
/// Held by the provider that registered the action (in `ProviderHandles`, alongside its event
/// subscriptions) so that unmounting the provider retires its actions. The handle carries only the
/// id: the registry and the catalog are reached from `HecaApp`/`AppState`, not from `Drop`; the
/// host calls [`unregister_dynamic`] with this id when it drops the provider — the same lifetime,
/// without wrapping the registry in a `RefCell`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionHandle(pub String);

/// Register a **name-keyed** action: its metadata joins the one [`ActionCatalog`] (so it gets an
/// icon, a label, introspection and a declared policy exactly like a built-in) and its handler —
/// when it has one — joins the [`ActionRegistry`].
///
/// This takes both halves because they live in different places for a borrow reason, not a design
/// one: an action handler is `fn(&mut AppState, …)` and gets **no** registry, so action *metadata*
/// must be reachable from `AppState` (the catalog), while the handler table must be borrowable
/// alongside `&mut AppState` (the registry). Metadata is still stored exactly once.
///
/// `handler: None` registers a [`Dispatch::Declarative`] action (declared, host cannot run it —
/// plugin-08 forwards it to its owner). Re-registering the same id replaces the previous entry (a
/// provider remounting). Returns the [`ActionHandle`] the provider keeps and hands back to
/// [`unregister_dynamic`] on unmount.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "plugin-04 seam: the first registrant is T1 (chrome placement actions) / a provider; exercised by tests today"
    )
)]
pub fn register_dynamic(
    registry: &mut ActionRegistry,
    catalog: &mut ActionCatalog,
    meta: ActionMeta,
    handler: Option<DynHandler>,
) -> ActionHandle {
    let id = meta.name.clone();
    catalog.insert(meta);
    match handler {
        Some(h) => {
            registry.dyn_handlers.insert(id.clone(), h);
        }
        None => {
            registry.dyn_handlers.remove(&id);
        }
    }
    ActionHandle(id)
}

/// Retire a name-keyed action — drops both its handler and its metadata. `true` if it was
/// registered. Built-ins cannot be retired (their names are not removable from the catalog).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "plugin-04 seam: providers retire their actions on unmount once T1 registers the first one; exercised by tests today"
    )
)]
pub fn unregister_dynamic(
    registry: &mut ActionRegistry,
    catalog: &mut ActionCatalog,
    id: &str,
) -> bool {
    let had_handler = registry.dyn_handlers.remove(id).is_some();
    let had_meta = catalog.remove_dynamic(id);
    had_handler || had_meta
}

impl ActionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            dyn_handlers: HashMap::new(),
        }
    }

    /// How the action named `name` would run, if at all — see [`Dispatch`]. `None` when the name is
    /// unknown to both back ends. `catalog` supplies the name→built-in resolution and the set of
    /// declared name-keyed actions.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "plugin-04 seam: RPC/palette introspection of how an action runs; exercised by tests today"
        )
    )]
    pub fn dispatch_of(&self, catalog: &ActionCatalog, name: &str) -> Option<Dispatch> {
        if crate::input::action_from_name(name).is_some() || catalog.is_builtin(name) {
            return Some(Dispatch::Native);
        }
        if self.dyn_handlers.contains_key(name) {
            return Some(Dispatch::NativeDyn);
        }
        catalog.find(name).map(|_| Dispatch::Declarative)
    }

    /// Run a name-keyed action, passing the intent through so the handler reads its own args.
    /// `false` when no *handler* is registered under `id` — either the id is unknown, or the action
    /// is [`Dispatch::Declarative`] (declared but host-unrunnable). Neither is a crash: a binding or
    /// a menu item may legitimately name an action whose provider is not mounted.
    pub fn execute_dynamic(
        &self,
        id: &str,
        state: &mut crate::app_state::AppState,
        intent: &crate::chrome::Intent,
    ) -> bool {
        let Some(handler) = self.dyn_handlers.get(id) else {
            return false;
        };
        // Clone the `Rc` so the borrow of `self` ends before the handler runs (it takes
        // `&mut AppState`).
        let handler = handler.clone();
        handler(state, intent);
        true
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
            name: "sidebar_peek",
            label: "Sidebar Peek",
            description: "Focus the selected pane or workspace without leaving sidebar mode.",
            category: ActionCategory::Navigation,
            default_binding: "Space",
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
            // New column opens to the right (see the description); ColumnsPlusLeft stays
            // in the Glyph set for a future "add column to the left" action.
            icon: Some(Glyph::ColumnsPlusRight),
        },
        ActionDescriptor {
            name: "split_vertical",
            label: "New Pane in Column (Vertical Split)",
            description: "Add a new pane below the current one in the same column.",
            category: ActionCategory::Layout,
            default_binding: "v",
            icon: Some(Glyph::SquareHalfBottom),
        },
        ActionDescriptor {
            // Add-pane-to-a-specific-column (the pane-header "+" button and the sidebar
            // column "New Pane" entry, which target a column by index — distinct from
            // `split_vertical` which splits the active column). Menu/button-only, so no
            // binding; the "+" button still shows the `v` hint via `pane_action_name`.
            name: "add_pane_to_column",
            label: "New Pane in Column",
            description: "Add a new pane to this column.",
            category: ActionCategory::Layout,
            default_binding: "unbound",
            icon: Some(Glyph::FolderSimplePlus),
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
            icon: Some(Glyph::Trash),
        },
        // ── Pane ──
        ActionDescriptor {
            name: "close",
            label: "Close Pane",
            description: "Close the active pane.",
            category: ActionCategory::Pane,
            default_binding: "x",
            // Remove/close pane; pairs with add-pane's FolderSimplePlus (both act on a
            // pane "slot" in a column).
            icon: Some(Glyph::FolderSimpleMinus),
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
            icon: Some(Glyph::NotePencil),
        },
        ActionDescriptor {
            name: "reset_pane_name",
            label: "Use Process Name",
            description: "Clear the pane's custom name, reverting to the program name.",
            category: ActionCategory::Pane,
            default_binding: "unbound",
            icon: Some(Glyph::Backspace),
        },
        ActionDescriptor {
            name: "rename_column",
            label: "Rename Column",
            description: "Rename the active column.",
            category: ActionCategory::Layout,
            default_binding: "Shift+c",
            icon: Some(Glyph::NotePencil),
        },
        // ── Workspace ──
        ActionDescriptor {
            name: "create_workspace",
            label: "Create Workspace",
            description: "Create a new workspace and switch to it.",
            category: ActionCategory::Workspace,
            default_binding: "w",
            icon: Some(Glyph::StackPlus),
        },
        ActionDescriptor {
            name: "rename_workspace",
            label: "Rename Workspace",
            description: "Rename the current workspace.",
            category: ActionCategory::Workspace,
            default_binding: "Shift+w",
            icon: Some(Glyph::NotePencil),
        },
        ActionDescriptor {
            name: "reset_workspace_name",
            label: "Use Default Name",
            description: "Clear the workspace's custom name, reverting to \"Workspace N\".",
            category: ActionCategory::Workspace,
            default_binding: "unbound",
            icon: Some(Glyph::Backspace),
        },
        ActionDescriptor {
            name: "delete_workspace",
            label: "Delete Workspace",
            description: "Delete a workspace and all its panes (not the last workspace).",
            category: ActionCategory::Workspace,
            default_binding: "unbound",
            icon: Some(Glyph::StackMinus),
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

/// Runtime metadata for **one action** — a built-in or a name-keyed one contributed by a provider
/// or plugin. The single entry type of the [`ActionCatalog`]: built-in and plugin actions have the
/// *same* shape, so every surface (icons, tooltips, command palette, RPC introspection) treats them
/// identically.
///
/// Owned (`String`, not `&'static str`) precisely so a runtime-registered action can join the
/// catalog. The zero-copy `&'static ActionDescriptor` form was the deliberate Phase-A shortcut
/// (action-interaction plan §8b) that deferred this ripple; this is that ripple.
///
/// `policy` is **required, no default**. A built-in gets it from [`action_policy`]'s exhaustive
/// `match` — forgetting to classify a new variant there is a *compile error*, and this field is
/// **computed** from that match at registration, never hand-written, so the match stays the single
/// authority. A name-keyed action has no variant, so nothing else would force the question; a
/// permissive default would mean "the author forgot" silently resolves to the most permissive
/// setting in the system. It is a plain field so an action cannot be registered without answering
/// it.
///
/// [`action_policy`]: crate::app::interaction::action_policy
#[derive(Debug, Clone)]
pub struct ActionMeta {
    /// Stable id — the identity used by config bindings, RPC, menus, and `Intent.action`.
    pub name: String,
    pub label: String,
    pub description: String,
    pub category: ActionCategory,
    /// Default keybinding string (e.g. "h,ArrowLeft"); empty / "unbound" when it has none.
    // Preserved for the command palette + RPC introspection (read in tests only for now).
    #[cfg_attr(not(test), allow(dead_code))]
    pub default_binding: String,
    /// Centralized action icon — the single source of an action's [`Glyph`]. Every surface that
    /// renders this action reads it from here instead of inventing its own.
    pub icon: Option<Glyph>,
    /// Allow/Block classification for the interaction router. Required — see the type docs.
    pub policy: crate::app::interaction::ActionPolicy,
}

/// Runtime catalog of action metadata, owned by [`AppState`](crate::app_state::AppState).
///
/// **The single metadata home for every action**, built-in or plugin. Seeded from the built-in
/// [`ActionRegistry::ALL`] descriptors at startup and extended at runtime through
/// [`register_dynamic`]. Every UI surface resolves action metadata through here — icons, labels,
/// the pick prompts, the command palette, RPC introspection.
///
/// It lives on `AppState` (rather than inside `ActionRegistry`) for a borrow reason: an action
/// handler is `fn(&mut AppState, &WmAction)` and receives no registry, so metadata must be
/// reachable from the state. The registry holds the *handlers*; this holds the *metadata*. One of
/// each — never two of either.
pub struct ActionCatalog {
    by_name: HashMap<String, ActionMeta>,
    /// Names in stable order (built-ins in `ALL` order, then registration order).
    order: Vec<String>,
    /// The built-in names, so a dynamic action can never shadow or retire one.
    builtins: std::collections::HashSet<&'static str>,
    /// Declarative confirmation requirement per **confirm config name** (see [`ConfirmSpec`]). The
    /// central gate reads this to decide whether an action needs a confirm/response prompt — the
    /// guard lives on the action, not the call site.
    ///
    /// Keyed by the confirm name, which is **not always the action's own name**: pane close
    /// (`ClosePane` + `ClosePaneById`, descriptor name `close`) maps to the confirm key
    /// `delete_pane`, so either dispatch confirms identically (action-interaction plan §5.1). That
    /// is why this is a separate index rather than a field on [`ActionMeta`] — moving it onto the
    /// meta is `action-task-C`'s graft point, together with `register(ActionSpec)`.
    confirm: HashMap<String, ConfirmSpec>,
}

impl ActionCatalog {
    /// Build the catalog seeded from the built-in [`ActionRegistry::ALL`] descriptors.
    ///
    /// Each built-in's `policy` is **computed** from [`action_policy`](crate::app::interaction::action_policy)
    /// via [`builtin_policy`] — never hand-written here — so the exhaustive `match` in
    /// `interaction.rs` remains the only authority on built-in policy and the two cannot drift.
    pub fn with_builtins() -> Self {
        let mut catalog = Self {
            by_name: HashMap::new(),
            order: Vec::new(),
            builtins: ActionRegistry::ALL.iter().map(|d| d.name).collect(),
            confirm: builtin_confirm_specs(),
        };
        for d in ActionRegistry::ALL {
            catalog.insert(ActionMeta {
                name: d.name.to_string(),
                label: d.label.to_string(),
                description: d.description.to_string(),
                category: d.category,
                default_binding: d.default_binding.to_string(),
                icon: d.icon,
                policy: builtin_policy(d.name),
            });
        }
        catalog
    }

    /// Add or replace an action's metadata. Re-registering an id replaces it (a provider
    /// remounting) while keeping its position in the stable order.
    pub(crate) fn insert(&mut self, meta: ActionMeta) {
        if !self.by_name.contains_key(&meta.name) {
            self.order.push(meta.name.clone());
        }
        self.by_name.insert(meta.name.clone(), meta);
    }

    /// Remove a **dynamic** action's metadata. Built-ins are never removable, so a provider
    /// unmounting can't retire `close`. `true` if a dynamic entry was removed.
    pub(crate) fn remove_dynamic(&mut self, name: &str) -> bool {
        if self.is_builtin(name) || !self.by_name.contains_key(name) {
            return false;
        }
        self.by_name.remove(name);
        self.order.retain(|n| n != name);
        true
    }

    /// Whether `name` is one of the compiled-in built-in actions.
    pub fn is_builtin(&self, name: &str) -> bool {
        self.builtins.contains(name)
    }

    /// The declarative confirmation spec for a **confirm config name**, if it needs confirmation.
    pub fn confirm_spec(&self, name: &str) -> Option<&ConfirmSpec> {
        self.confirm.get(name)
    }

    /// Look up an action's metadata by its name.
    pub fn find(&self, name: &str) -> Option<&ActionMeta> {
        self.by_name.get(name)
    }

    /// The declared interaction policy for a name-keyed action — how the router judges a plugin
    /// action by exactly the same rules as a built-in.
    pub fn policy(&self, name: &str) -> Option<crate::app::interaction::ActionPolicy> {
        self.find(name).map(|m| m.policy)
    }

    /// The icon [`Glyph`] for an action, by name — the single source of action iconography
    /// (pane-action bar, context menu, command palette all resolve through here).
    pub fn icon(&self, name: &str) -> Option<Glyph> {
        self.find(name).and_then(|m| m.icon)
    }

    /// The human-readable label for an action, by name — so a caller names the action
    /// rather than re-spelling the label.
    pub fn label(&self, name: &str) -> Option<&str> {
        self.find(name).map(|m| m.label.as_str())
    }

    /// All actions in a given category, in stable order.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "preserved for the command palette + RPC introspection")
    )]
    pub fn by_category(&self, category: ActionCategory) -> impl Iterator<Item = &ActionMeta> + '_ {
        self.order
            .iter()
            .filter_map(move |n| self.by_name.get(n))
            .filter(move |m| m.category == category)
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

/// The interaction policy of a **built-in**, derived from the one authority:
/// [`action_policy`](crate::app::interaction::action_policy)'s exhaustive `match`.
///
/// Most names resolve straight through `action_from_name`. The two that don't are parameterized and
/// never reachable from a bare name — they are constructed programmatically (mouse / HintKey /
/// selection / context menu / RPC / the GUI scrollbar) — so a representative variant stands in for
/// them purely to read the policy off the same match. Never classify an action here: classify it in
/// `action_policy` and it lands here automatically.
fn builtin_policy(name: &str) -> crate::app::interaction::ActionPolicy {
    let action = crate::input::action_from_name(name).unwrap_or_else(|| match name {
        "open_link" => crate::input::WmAction::OpenLink { url: String::new() },
        "scroll_to_offset" => crate::input::WmAction::ScrollToOffset { rows: 0 },
        other => unreachable!(
            "built-in action {other:?} has no WmAction: add it to action_from_name, or map a \
             representative variant here"
        ),
    });
    crate::app::interaction::action_policy(&action)
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
fn builtin_confirm_specs() -> HashMap<String, ConfirmSpec> {
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
        ("delete_pane".to_string(), mk("delete_pane", "Delete")),
        ("delete_column".to_string(), mk("delete_column", "Delete")),
        (
            "delete_workspace".to_string(),
            mk("delete_workspace", "Delete"),
        ),
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

    // ── plugin-04 / T3: the one runtime registry ──

    /// The built-in metadata after seeding is IDENTICAL to the `const ALL` descriptors it came
    /// from — the owned-`String` ripple must not have changed a single value (the non-regression
    /// test the task asks for).
    #[test]
    fn builtin_metadata_survives_the_owned_ripple_unchanged() {
        let catalog = ActionCatalog::with_builtins();
        assert_eq!(catalog.count(), ActionRegistry::ALL.len());
        for d in ActionRegistry::ALL {
            let m = catalog
                .find(d.name)
                .unwrap_or_else(|| panic!("missing meta for {}", d.name));
            assert_eq!(m.name, d.name);
            assert_eq!(m.label, d.label);
            assert_eq!(m.description, d.description);
            assert_eq!(m.category, d.category);
            assert_eq!(m.default_binding, d.default_binding);
            assert_eq!(m.icon, d.icon);
            assert!(catalog.is_builtin(d.name));
        }
    }

    /// Every built-in's `policy` is COMPUTED from `action_policy`'s exhaustive match, never
    /// hand-written — so the match stays the single authority and the two cannot drift. This also
    /// proves `builtin_policy` resolves all 115 names (it would `unreachable!` otherwise).
    #[test]
    fn builtin_policy_is_derived_from_the_exhaustive_match() {
        use crate::app::interaction::{action_policy, ActionPolicy};
        let catalog = ActionCatalog::with_builtins();
        for d in ActionRegistry::ALL {
            let meta = catalog.find(d.name).unwrap();
            if let Some(action) = crate::input::action_from_name(d.name) {
                assert_eq!(
                    meta.policy,
                    action_policy(&action),
                    "{}: catalog policy diverged from action_policy()",
                    d.name
                );
            }
        }
        // Spot-check the two parameterized names that have no bare-name variant.
        assert_eq!(
            catalog.find("scroll_to_offset").unwrap().policy,
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            catalog.find("open_link").unwrap().policy,
            action_policy(&WmAction::OpenLink { url: String::new() })
        );
    }

    fn dyn_meta(name: &str, policy: crate::app::interaction::ActionPolicy) -> ActionMeta {
        ActionMeta {
            name: name.to_string(),
            label: "Restart Container".to_string(),
            description: "Restart the selected Docker container.".to_string(),
            category: ActionCategory::System,
            default_binding: String::new(),
            icon: Some(Glyph::Trash),
            policy,
        }
    }

    /// A name-keyed action joins the SAME catalog as the built-ins — so it gets a label, an icon and
    /// introspection exactly like `close` does. This is the whole point of the owned ripple: before
    /// it, a plugin action could only be dispatched, never rendered.
    #[test]
    fn a_dynamic_action_lives_in_the_same_catalog_as_the_builtins() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        let builtins = catalog.count();

        let handle = register_dynamic(
            &mut registry,
            &mut catalog,
            dyn_meta("plugin.docker.restart", ActionPolicy::Global),
            Some(std::rc::Rc::new(|_state, _intent| {})),
        );

        assert_eq!(handle, ActionHandle("plugin.docker.restart".to_string()));
        assert_eq!(catalog.count(), builtins + 1);
        assert_eq!(
            catalog.label("plugin.docker.restart"),
            Some("Restart Container")
        );
        assert_eq!(catalog.icon("plugin.docker.restart"), Some(Glyph::Trash));
        assert_eq!(
            catalog.policy("plugin.docker.restart"),
            Some(ActionPolicy::Global),
            "the router reads the DECLARED policy"
        );
        assert!(!catalog.is_builtin("plugin.docker.restart"));
        assert_eq!(
            registry.dispatch_of(&catalog, "plugin.docker.restart"),
            Some(Dispatch::NativeDyn)
        );
        // A built-in still reports as Native, through the same one door.
        assert_eq!(
            registry.dispatch_of(&catalog, "close"),
            Some(Dispatch::Native)
        );
        assert_eq!(registry.dispatch_of(&catalog, "nope.not.a.thing"), None);
    }

    /// `unregister` (the provider unmounting and dropping its handle) retires BOTH halves — the
    /// handler and the metadata — so the id is no longer dispatchable or renderable.
    #[test]
    fn unregister_retires_both_the_handler_and_the_metadata() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        register_dynamic(
            &mut registry,
            &mut catalog,
            dyn_meta("plugin.docker.restart", ActionPolicy::TiledOnly),
            Some(std::rc::Rc::new(|_state, _intent| {})),
        );

        assert!(unregister_dynamic(
            &mut registry,
            &mut catalog,
            "plugin.docker.restart"
        ));
        assert!(catalog.find("plugin.docker.restart").is_none());
        assert_eq!(registry.dispatch_of(&catalog, "plugin.docker.restart"), None);
        // Idempotent: retiring it twice is not an error.
        assert!(!unregister_dynamic(
            &mut registry,
            &mut catalog,
            "plugin.docker.restart"
        ));
    }

    /// Re-registering an id (a provider remounting) REPLACES the entry rather than duplicating it.
    #[test]
    fn re_registering_an_id_replaces_it() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        let before = catalog.count();
        register_dynamic(
            &mut registry,
            &mut catalog,
            dyn_meta("plugin.x", ActionPolicy::Global),
            None,
        );
        let mut second = dyn_meta("plugin.x", ActionPolicy::TiledOnly);
        second.label = "Second".to_string();
        register_dynamic(&mut registry, &mut catalog, second, None);

        assert_eq!(catalog.count(), before + 1, "replaced, not duplicated");
        assert_eq!(catalog.label("plugin.x"), Some("Second"));
        assert_eq!(catalog.policy("plugin.x"), Some(ActionPolicy::TiledOnly));
    }

    /// A DECLARATIVE action (no host handler — its owner is a WASM plugin) is declared and
    /// policy-classified, but the host cannot run it. Dispatching it is a no-op, never a crash.
    #[test]
    fn a_declarative_action_is_declared_but_has_no_host_handler() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        register_dynamic(
            &mut registry,
            &mut catalog,
            dyn_meta("plugin.wasm.thing", ActionPolicy::FocusedPaneLocal),
            None, // no host handler
        );
        assert_eq!(
            registry.dispatch_of(&catalog, "plugin.wasm.thing"),
            Some(Dispatch::Declarative)
        );
        assert_eq!(
            catalog.policy("plugin.wasm.thing"),
            Some(ActionPolicy::FocusedPaneLocal)
        );
    }

    /// A dynamic action can neither shadow nor retire a built-in.
    #[test]
    fn a_dynamic_action_cannot_retire_a_builtin() {
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        assert!(!unregister_dynamic(&mut registry, &mut catalog, "close"));
        assert!(catalog.find("close").is_some(), "built-in survives");
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
