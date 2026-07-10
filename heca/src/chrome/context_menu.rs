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

impl ContextTarget {
    /// The pane this target refers to, if any (a content/sidebar pane). `None` for column
    /// and workspace targets. Used to retarget pane actions (rename/close) to the item the
    /// menu / sidebar cursor is on.
    pub(crate) fn pane_id(&self) -> Option<PaneId> {
        match self {
            ContextTarget::Pane { pane_id, .. } | ContextTarget::SidebarPane { pane_id } => {
                Some(*pane_id)
            }
            _ => None,
        }
    }

    /// The workspace index this target belongs to, if resolvable. Direct for workspace/column
    /// targets; for a pane target it is resolved from the pane's location in `state`. Used to
    /// retarget workspace actions (rename) to the item the sidebar cursor is on.
    pub(crate) fn ws_idx(&self, state: &AppState) -> Option<usize> {
        match self {
            ContextTarget::SidebarWorkspace { ws_idx }
            | ContextTarget::SidebarColumn { ws_idx, .. } => Some(*ws_idx),
            ContextTarget::Pane { pane_id, .. } | ContextTarget::SidebarPane { pane_id } => {
                crate::find_pane_location(&state.session, *pane_id).map(|(ws, _, _)| ws)
            }
        }
    }
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

/// Resolve the active context for a keyboard-opened context menu (`OpenContextMenu` /
/// `prefix+>`). Reads the current [`InputMode`] and the sidebar cursor / focused pane to
/// produce a `(path, target)` pair that [`open_context_menu_for`] can route to the right
/// provider.
///
/// - `SidebarNav`: maps [`crate::sidebar::SidebarItem`] at the cursor to its context path
///   (pane→`sidebar.pane`, column→`sidebar.column`, workspace→`sidebar.workspace`,
///   floating-pane→`pane`).
/// - `Normal` (and any other mode): resolves to the focused content pane (`"pane"`).
/// - Returns `None` when there is no active target (no focused pane, no sidebar cursor).
pub(crate) fn resolve_active_context(state: &AppState) -> Option<(ContextPath, ContextTarget)> {
    resolve_context_for(
        &state.input_mode,
        state.sidebar_tree.current_item(),
        crate::app::interaction::focused_pane_id(state),
    )
}

/// Pure mapping behind [`resolve_active_context`]: input mode + the active sidebar item +
/// the focused pane → menu `(path, target)`. Split out from the `AppState` reads so the
/// mapping is unit-testable without a full app. In `SidebarNav` the sidebar cursor decides
/// the context (pane→`sidebar.pane`, column→`sidebar.column`, workspace→`sidebar.workspace`,
/// floating-pane→`pane`); in any other mode the focused content pane does. Returns `None`
/// when there is nothing to target (no sidebar cursor / no focused pane).
fn resolve_context_for(
    input_mode: &InputMode,
    sidebar_item: Option<&crate::sidebar::SidebarItem>,
    focused_pane: Option<PaneId>,
) -> Option<(ContextPath, ContextTarget)> {
    match input_mode {
        InputMode::SidebarNav => Some(match sidebar_item? {
            crate::sidebar::SidebarItem::Pane { pane_id } => (
                ContextPath(ContextPath::SIDEBAR_PANE.to_string()),
                ContextTarget::SidebarPane { pane_id: *pane_id },
            ),
            crate::sidebar::SidebarItem::FloatingPane { pane_id, .. } => (
                ContextPath(ContextPath::PANE.to_string()),
                ContextTarget::Pane { pane_id: *pane_id, hyperlink: None },
            ),
            crate::sidebar::SidebarItem::Column { ws_idx, col_idx } => (
                ContextPath(ContextPath::SIDEBAR_COLUMN.to_string()),
                ContextTarget::SidebarColumn { ws_idx: *ws_idx, col_idx: *col_idx },
            ),
            crate::sidebar::SidebarItem::Workspace { ws_idx } => (
                ContextPath(ContextPath::SIDEBAR_WORKSPACE.to_string()),
                ContextTarget::SidebarWorkspace { ws_idx: *ws_idx },
            ),
        }),
        _ => Some((
            ContextPath(ContextPath::PANE.to_string()),
            ContextTarget::Pane { pane_id: focused_pane?, hyperlink: None },
        )),
    }
}

/// Carries a context-menu target through the `Prefix` → `Normal` dispatch transition.
///
/// When the user presses `prefix+>` in `SidebarNav`, the prefix arm in
/// `handle_sidebar_nav_mode` resolves the active context and stashes it here *before*
/// transitioning to `Prefix` mode. `handle_prefix_mode` then normalises the input mode to
/// `Normal` before dispatching `OpenContextMenu`, so the handler cannot read `SidebarNav`.
/// `PendingContext` bridges that gap: the handler consumes it and opens the correct menu.
#[derive(Clone, Debug)]
pub(crate) struct PendingContext {
    pub path: ContextPath,
    pub target: ContextTarget,
    /// The mode to restore after the menu closes (e.g. `Some(SidebarNav)`).
    pub origin: Option<InputMode>,
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
        DropdownItem::new("rename_pane", "Rename", WmAction::RenamePane),
        DropdownItem::new("rename_workspace", "Rename workspace", WmAction::RenameWorkspace),
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
    items.push(DropdownItem::new(
        "rename_pane",
        "Rename pane",
        WmAction::RenamePaneById { pane_id: *pane_id },
    ));
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
    sidebar_column_items(*ws_idx, *col_idx)
}

/// Menu items for a sidebar **column** row (state-free, so unit-testable): new pane in the
/// column, new column, delete column (danger).
fn sidebar_column_items(ws_idx: usize, col_idx: usize) -> Vec<DropdownItem> {
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
        // NB: no "Rename column" entry — a column's name is not displayed anywhere yet
        // (columns render as a MarkerGroup with no header/label), so renaming would have
        // no visible effect. The RenameColumn / RenameColumnByIdx action stays wired (RPC +
        // handler) for when columns surface a name; re-add the entry then. See `col_idx`
        // still threaded below for the delete action.
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
    sidebar_workspace_items(*ws_idx)
}

/// Menu items for a sidebar **workspace** row (state-free, so unit-testable): new column,
/// new workspace, delete workspace (danger).
fn sidebar_workspace_items(ws_idx: usize) -> Vec<DropdownItem> {
    vec![
        DropdownItem::new(
            "split_horizontal",
            "New column",
            WmAction::AddColumnToWorkspace { ws_idx },
        ),
        DropdownItem::new("create_workspace", "New workspace", WmAction::CreateWorkspace),
        DropdownItem::new(
            "rename_workspace",
            "Rename workspace",
            WmAction::RenameWorkspaceByIdx { ws_idx },
        ),
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
    fn pane_action_items_include_rename_and_end_in_close() {
        let items = pane_action_items();
        assert_eq!(items[0].id, "split_horizontal");
        assert!(items.iter().any(|i| i.id == "rename_pane"), "has Rename");
        assert!(
            items.iter().any(|i| i.id == "rename_workspace"),
            "has Rename workspace"
        );
        let last = items.last().unwrap();
        assert_eq!(last.id, "close");
        assert!(last.danger, "close is danger-styled");
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

    use crate::sidebar::SidebarItem;

    #[test]
    fn resolve_context_sidebar_items_map_to_distinct_paths() {
        // Each sidebar cursor item resolves to its own context path + target, so the menu
        // content differs by where it was opened.
        let pane = SidebarItem::Pane { pane_id: PaneId(7) };
        let (path, target) =
            resolve_context_for(&InputMode::SidebarNav, Some(&pane), None).unwrap();
        assert_eq!(path.0, ContextPath::SIDEBAR_PANE);
        assert!(matches!(target, ContextTarget::SidebarPane { pane_id: PaneId(7) }));

        let col = SidebarItem::Column { ws_idx: 1, col_idx: 2 };
        let (path, target) =
            resolve_context_for(&InputMode::SidebarNav, Some(&col), None).unwrap();
        assert_eq!(path.0, ContextPath::SIDEBAR_COLUMN);
        assert!(matches!(target, ContextTarget::SidebarColumn { ws_idx: 1, col_idx: 2 }));

        let ws = SidebarItem::Workspace { ws_idx: 3 };
        let (path, target) =
            resolve_context_for(&InputMode::SidebarNav, Some(&ws), None).unwrap();
        assert_eq!(path.0, ContextPath::SIDEBAR_WORKSPACE);
        assert!(matches!(target, ContextTarget::SidebarWorkspace { ws_idx: 3 }));

        // A floating pane in the sidebar resolves to the generic pane menu.
        let float = SidebarItem::FloatingPane { pane_id: PaneId(9), ws_idx: 0 };
        let (path, target) =
            resolve_context_for(&InputMode::SidebarNav, Some(&float), None).unwrap();
        assert_eq!(path.0, ContextPath::PANE);
        assert!(matches!(target, ContextTarget::Pane { pane_id: PaneId(9), hyperlink: None }));
    }

    #[test]
    fn resolve_context_non_sidebar_uses_focused_pane() {
        // Any non-sidebar mode → the focused content pane.
        let (path, target) =
            resolve_context_for(&InputMode::Normal, None, Some(PaneId(4))).unwrap();
        assert_eq!(path.0, ContextPath::PANE);
        assert!(matches!(target, ContextTarget::Pane { pane_id: PaneId(4), hyperlink: None }));
    }

    #[test]
    fn resolve_context_none_when_no_target() {
        // SidebarNav with no cursor item, and Normal with no focused pane, both resolve to None.
        assert!(resolve_context_for(&InputMode::SidebarNav, None, Some(PaneId(1))).is_none());
        assert!(resolve_context_for(&InputMode::Normal, None, None).is_none());
    }

    #[test]
    fn context_menus_differ_by_where_opened() {
        // The content each context produces is distinct — proving the menu adapts to where
        // it is opened (pane vs sidebar column vs sidebar workspace).
        let pane = pane_action_items();
        let col = sidebar_column_items(0, 0);
        let ws = sidebar_workspace_items(0);
        let labels = |v: &[DropdownItem]| v.iter().map(|i| i.label.clone()).collect::<Vec<_>>();
        let has = |v: &[DropdownItem], s: &str| v.iter().any(|i| i.label == s);

        // Distinct label sets — no two contexts produce the same menu.
        assert_ne!(labels(&pane), labels(&col));
        assert_ne!(labels(&col), labels(&ws));
        assert_ne!(labels(&pane), labels(&ws));

        // Signature entries unique to each context.
        assert!(has(&pane, "Zoom / unzoom") && has(&pane, "Float / unfloat"));
        assert!(has(&col, "Delete column"));
        assert!(has(&ws, "New workspace") && has(&ws, "Delete workspace"));
        // Each sidebar context still ends in a danger `close`-id action (delete) for its own scope.
        assert_eq!(col.last().map(|i| i.id.as_str()), Some("close"));
        assert!(col.last().map(|i| i.danger).unwrap_or(false));
        assert_eq!(ws.last().map(|i| i.id.as_str()), Some("close"));
    }
}