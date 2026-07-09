//! Context-menu resolution: a dotted [`ContextPath`] + opaque [`ContextTarget`] resolve to a
//! merged dropdown via the [`ContextMenuRegistry`]. Built-in providers cover the content pane
//! and the sidebar surfaces; plugins attach via `Contribution::ContextMenu` (context-menu-5).
//!
//! Both the mouse right-click (explicit target from the hit-test) and the keyboard
//! `OpenContextMenu` (implicit target via `resolve_active_context`, context-menu-7) funnel
//! through [`open_context_menu_for`]. The host — not the handler — decides which menu applies
//! for a given context, so a plugin only declares *where* (`context_path`) and *what*
//! (`build`); it never decides *when* to open.
//!
//! Mode-restore: when a menu is opened from a non-Normal mode (e.g. `SidebarNav`), the host
//! records the origin mode in [`AppState::overlay_origin_mode`] and restores it when the last
//! overlay closes (see `chrome::overlay::resolve`). So opening a context menu from the sidebar
//! and dismissing/selecting returns the user to the sidebar, not to Normal.

use crate::app::interaction::InteractionSource;
use crate::app_state::{AppState, InputMode};
use crate::chrome::{open_dropdown, DropdownItem, DropdownSpec};
use crate::input::WmAction;
use heca_core::layout::{PaneId, Point};

// ──────────────────────────────────────────────────────────────────────────────
//  Context model
// ──────────────────────────────────────────────────────────────────────────────

/// Dotted context identifier — the registry key. Built-in paths are constants; plugins use
/// arbitrary dotted strings (e.g. `"docker.container"`). Compared as a plain string.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContextPath(pub String);

impl ContextPath {
    /// A content pane (right-click on a pane, or keyboard `OpenContextMenu` from Normal).
    pub const PANE: &'static str = "pane";
    /// A pane row in the sidebar tree.
    pub const SIDEBAR_PANE: &'static str = "sidebar.pane";
    /// A column row in the sidebar tree.
    pub const SIDEBAR_COLUMN: &'static str = "sidebar.column";
    /// A workspace row in the sidebar tree.
    pub const SIDEBAR_WORKSPACE: &'static str = "sidebar.workspace";
}

/// Opaque target data a provider's `build` receives. The host fills it from the hit-test
/// (mouse) or the active context (keyboard); providers match the arm they handle and return
/// `vec![]` for any other.
#[derive(Clone, Debug)]
pub enum ContextTarget {
    /// A content pane. `hyperlink` is `Some(url)` only for the mouse path (the clicked cell);
    /// a keyboard-opened menu has `None` (no target cell) so no "Open link" entry is added.
    /// `pane_id` is carried for the target contract — the built-in pane provider acts on the
    /// focused pane (via `ClosePane`), but a plugin provider for `pane` may want the explicit id.
    #[allow(dead_code)] // seam contract: carried for plugin pane providers, not read by built-in
    Pane {
        pane_id: PaneId,
        hyperlink: Option<String>,
    },
    /// A pane row in the sidebar tree (the pane id; the provider resolves ws/col from the
    /// session so the menu actions target the right column).
    SidebarPane {
        pane_id: PaneId,
    },
    /// A column row in the sidebar tree.
    SidebarColumn {
        ws_idx: usize,
        col_idx: usize,
    },
    /// A workspace row in the sidebar tree.
    SidebarWorkspace {
        ws_idx: usize,
    },
}

// ──────────────────────────────────────────────────────────────────────────────
//  Registry
// ──────────────────────────────────────────────────────────────────────────────

/// A registered context-menu provider: a weight (Dewey/fractional index, C2) for merge ordering
/// and a build function. Built-in providers use static function pointers (no captures); plugin
/// providers (context-menu-5) will wrap a marshalled closure.
#[derive(Clone)]
pub struct ContextMenuProvider {
    /// Merge order within a context path. Built-ins use a stable weight; a plugin inserts
    /// between two built-ins with a fractional `Vec<i64>` (e.g. `[1,1,1]` between `[1,1]` and
    /// `[1,2]`). Sorted ascending; ties keep insertion order (stable sort).
    pub weight: Vec<i64>,
    pub build: fn(&AppState, &ContextTarget) -> Vec<DropdownItem>,
}

/// Runtime registry of context-menu providers keyed by [`ContextPath`]. Seeded with the
/// built-in providers at startup; plugins add to it via `Contribution::ContextMenu`
/// (context-menu-5). `items_for` collects every provider matching a path, sorts by weight, and
/// concatenates their items — so a plugin's entries merge with the built-in's.
#[derive(Default)]
pub struct ContextMenuRegistry {
    providers: std::collections::HashMap<String, Vec<ContextMenuProvider>>,
}

impl ContextMenuRegistry {
    /// Registry seeded with the four built-in providers (pane + sidebar.pane/column/workspace).
    pub fn with_builtins() -> Self {
        let mut r = Self::default();
        // Built-ins: weight 0 (pane) / 1 (sidebar.*). Plugins insert fractional weights between.
        r.register(ContextPath::PANE, vec![0], build_pane_menu);
        r.register(ContextPath::SIDEBAR_PANE, vec![1], build_sidebar_pane_menu);
        r.register(ContextPath::SIDEBAR_COLUMN, vec![1], build_sidebar_column_menu);
        r.register(ContextPath::SIDEBAR_WORKSPACE, vec![1], build_sidebar_workspace_menu);
        r
    }

    /// Register a provider for a context path (used by the built-in seed and, later, plugins).
    pub fn register(
        &mut self,
        path: &str,
        weight: Vec<i64>,
        build: fn(&AppState, &ContextTarget) -> Vec<DropdownItem>,
    ) {
        self.providers
            .entry(path.to_string())
            .or_default()
            .push(ContextMenuProvider { weight, build });
    }

    /// Collect + merge items for a context path: every matching provider, sorted by weight
    /// (stable), concatenated. `vec![]` when no provider matches (an empty menu — the caller
    /// may choose not to open it).
    pub fn items_for(
        &self,
        state: &AppState,
        path: &str,
        target: &ContextTarget,
    ) -> Vec<DropdownItem> {
        let mut items = Vec::new();
        for p in self.ordered_providers(path) {
            items.extend((p.build)(state, target));
        }
        items
    }

    /// Providers for a path, cloned + sorted by weight (stable sort keeps insertion order on
    /// ties). Exposed so the merge ordering is unit-testable without an `AppState`.
    pub(crate) fn ordered_providers(&self, path: &str) -> Vec<ContextMenuProvider> {
        let Some(provs) = self.providers.get(path) else {
            return Vec::new();
        };
        let mut ordered: Vec<ContextMenuProvider> = provs.clone();
        ordered.sort_by(|a, b| a.weight.cmp(&b.weight));
        ordered
    }
}

// ──────────────────────────────────────────────────────────────────────────────
//  Unified open path
// ──────────────────────────────────────────────────────────────────────────────

/// Open the context menu for a resolved `(path, target)` pair: registry lookup → merged items
/// → [`open_dropdown`] (the host-owned overlay). Both the mouse right-click and the keyboard
/// `OpenContextMenu` route here.
///
/// `origin` is the input mode to restore when the last overlay closes (mode-restore). The mouse
/// path passes `None` (capture the current mode only if it is restorable, e.g. `SidebarNav`);
/// the keyboard-from-mode path passes it explicitly. The origin is recorded only when no overlay
/// is already open (a stacked overlay — e.g. a confirm prompt on top of the menu — keeps the
/// origin already recorded for the menu).
pub(crate) fn open_context_menu_for(
    state: &mut AppState,
    path: &str,
    target: ContextTarget,
    anchor: Point,
    source: InteractionSource,
    origin: Option<InputMode>,
) {
    if state.overlay_origin_mode.is_none() {
        state.overlay_origin_mode = origin.or_else(|| restorable_mode(state.input_mode.clone()));
    }
    let items = state.context_menu_registry.items_for(state, path, &target);
    // A keyboard/RPC-opened menu is centered on its anchor (window center); a mouse-opened menu
    // is placed down-right of the click point.
    let centered = matches!(source, InteractionSource::Keyboard);
    open_dropdown(
        state,
        DropdownSpec {
            anchor,
            items,
            source,
            centered,
        },
    );
}

/// Only `SidebarNav` is restorable today: opening a context menu from the sidebar returns to the
/// sidebar after close. `Normal` is not captured (restoring Normal is a no-op, so we leave the
/// field `None` to keep the existing behaviour unchanged). Add future restorable modes here.
fn restorable_mode(mode: InputMode) -> Option<InputMode> {
    match mode {
        InputMode::SidebarNav => Some(mode),
        _ => None,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
//  Built-in providers
// ──────────────────────────────────────────────────────────────────────────────

/// The common pane actions every content-pane context menu offers. The item id doubles as the
/// action name its icon resolves from (`ActionCatalog::icon`). Shared by the mouse-open and
/// keyboard-open paths so the two never drift.
fn pane_action_items() -> Vec<DropdownItem> {
    vec![
        DropdownItem::new("split_horizontal", "New column", WmAction::SplitHorizontal),
        DropdownItem::new("split_vertical", "Split down", WmAction::SplitVertical),
        DropdownItem::new("zoom_column", "Zoom / unzoom", WmAction::ZoomColumn),
        DropdownItem::new("float", "Float / unfloat", WmAction::Float),
        DropdownItem::new("close", "Close pane", WmAction::ClosePane).danger(true),
    ]
}

/// Provider for [`ContextPath::PANE`] — a content pane. "Open link" first only when the mouse
/// click carried a hyperlink (keyboard-opened menus have `None` → no link entry).
fn build_pane_menu(_state: &AppState, target: &ContextTarget) -> Vec<DropdownItem> {
    let ContextTarget::Pane { hyperlink, .. } = target else {
        return Vec::new();
    };
    let mut items = pane_action_items();
    if let Some(url) = hyperlink {
        items.insert(0, DropdownItem::new("open_link", "Open link", WmAction::OpenLink { url: url.clone() }));
    }
    items
}

/// Provider for [`ContextPath::SIDEBAR_PANE`] — a pane row in the sidebar tree. "New pane" (in
/// the pane's column) + "Delete pane" (by id, danger).
fn build_sidebar_pane_menu(state: &AppState, target: &ContextTarget) -> Vec<DropdownItem> {
    let ContextTarget::SidebarPane { pane_id } = target else {
        return Vec::new();
    };
    let mut items = Vec::new();
    if let Some((ws_idx, col_idx, _)) = crate::find_pane_location(&state.session, *pane_id) {
        items.push(DropdownItem::new(
            "split_vertical",
            "New pane",
            WmAction::AddPaneToColumn { ws_idx, col_idx },
        ));
    }
    items.push(
        DropdownItem::new("close", "Delete pane", WmAction::ClosePaneById { pane_id: *pane_id })
            .danger(true),
    );
    items
}

/// Provider for [`ContextPath::SIDEBAR_COLUMN`] — a column row. "New pane" (in the column) +
/// "New column" + "Delete column" (danger).
fn build_sidebar_column_menu(_state: &AppState, target: &ContextTarget) -> Vec<DropdownItem> {
    let ContextTarget::SidebarColumn { ws_idx, col_idx } = target else {
        return Vec::new();
    };
    let (ws_idx, col_idx) = (*ws_idx, *col_idx);
    vec![
        DropdownItem::new(
            "split_vertical",
            "New pane",
            WmAction::AddPaneToColumn { ws_idx, col_idx },
        ),
        DropdownItem::new(
            "split_horizontal",
            "New column",
            WmAction::AddColumnToWorkspace { ws_idx },
        ),
        DropdownItem::new(
            "close",
            "Delete column",
            WmAction::DeleteColumn { ws_idx, col_idx },
        )
        .danger(true),
    ]
}

/// Provider for [`ContextPath::SIDEBAR_WORKSPACE`] — a workspace row. "New column" + "New
/// workspace" + "Delete workspace" (danger).
fn build_sidebar_workspace_menu(_state: &AppState, target: &ContextTarget) -> Vec<DropdownItem> {
    let ContextTarget::SidebarWorkspace { ws_idx } = target else {
        return Vec::new();
    };
    let ws_idx = *ws_idx;
    vec![
        DropdownItem::new(
            "split_horizontal",
            "New column",
            WmAction::AddColumnToWorkspace { ws_idx },
        ),
        DropdownItem::new("create_workspace", "New workspace", WmAction::CreateWorkspace),
        DropdownItem::new(
            "close",
            "Delete workspace",
            WmAction::DeleteWorkspace { ws_idx },
        )
        .danger(true),
    ]
}

// ──────────────────────────────────────────────────────────────────────────────
//  Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_seeds_four_built_in_providers() {
        let r = ContextMenuRegistry::with_builtins();
        assert!(r.providers.contains_key(ContextPath::PANE));
        assert!(r.providers.contains_key(ContextPath::SIDEBAR_PANE));
        assert!(r.providers.contains_key(ContextPath::SIDEBAR_COLUMN));
        assert!(r.providers.contains_key(ContextPath::SIDEBAR_WORKSPACE));
        // One built-in per path in this phase.
        assert_eq!(r.providers.get(ContextPath::PANE).unwrap().len(), 1);
    }

    #[test]
    fn pane_action_items_has_five_entries() {
        let items = pane_action_items();
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].id, "split_horizontal");
        assert_eq!(items[4].id, "close");
        assert!(items[4].danger, "close is danger-styled");
    }

    #[test]
    fn ordered_providers_sorts_by_weight() {
        // A plugin inserts between two built-ins via fractional weights.
        let mut r = ContextMenuRegistry::with_builtins();
        // Register a second provider for PANE with a higher weight — it sorts after the built-in.
        r.register(ContextPath::PANE, vec![2], |_state, _target| {
            vec![DropdownItem::new("plugin_extra", "Plugin", WmAction::SplitHorizontal)]
        });
        let ordered = r.ordered_providers(ContextPath::PANE);
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].weight, vec![0], "built-in (weight 0) first");
        assert_eq!(ordered[1].weight, vec![2], "plugin (weight 2) second");
    }

    #[test]
    fn unknown_path_yields_no_providers() {
        let r = ContextMenuRegistry::with_builtins();
        assert!(r.ordered_providers("no.such.path").is_empty());
    }

    #[test]
    fn restorable_mode_only_sidebar_nav() {
        assert!(matches!(restorable_mode(InputMode::SidebarNav), Some(InputMode::SidebarNav)));
        assert!(restorable_mode(InputMode::Normal).is_none());
        assert!(restorable_mode(InputMode::Prefix).is_none());
    }
}