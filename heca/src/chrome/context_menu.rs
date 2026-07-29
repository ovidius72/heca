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
use crate::chrome::{open_dropdown, DropdownItem, DropdownSpec, Intent, PropValue};
use crate::host::App;
use crate::providers::ChromeCtx;
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

/// Opaque target data a provider's `build` receives — **the facts about the thing the menu was
/// opened on**. The host fills it from the hit-test (mouse) or the active context (keyboard);
/// providers match the arm they handle and return `vec![]` for any other.
///
/// The target carries what only the host can resolve (a sidebar pane's column, a workspace's custom
/// name), precisely so a **plugin** builder does not need the session to write a useful menu: it
/// reads the target for *what was clicked* and `ChromeCtx` for *the app's state*. A builder never
/// sees `AppState`.
#[derive(Clone, Debug)]
pub enum ContextTarget {
    /// A content pane. `hyperlink` is `Some(url)` only for the mouse path (the clicked cell);
    /// a keyboard-opened menu has `None` (no target cell) so no "Open link" entry is added.
    Pane {
        pane_id: PaneId,
        hyperlink: Option<String>,
    },
    /// A pane row in the sidebar tree. `ws_idx`/`col_idx` are the pane's location, resolved by the
    /// host — so the menu's "New pane" lands in *this* pane's column without the builder touching
    /// the session.
    SidebarPane {
        pane_id: PaneId,
        ws_idx: usize,
        col_idx: usize,
    },
    /// A column row in the sidebar tree.
    SidebarColumn {
        ws_idx: usize,
        col_idx: usize,
    },
    /// A workspace row in the sidebar tree. `custom_name` is the user-set name, if any — the menu
    /// offers "Use default name" only when there is one to clear.
    SidebarWorkspace {
        ws_idx: usize,
        custom_name: Option<String>,
    },
}

impl ContextTarget {
    /// The pane this target refers to, if any (a content/sidebar pane). `None` for column
    /// and workspace targets. Used to retarget pane actions (rename/close) to the item the
    /// menu / sidebar cursor is on.
    pub(crate) fn pane_id(&self) -> Option<PaneId> {
        match self {
            ContextTarget::Pane { pane_id, .. }
            | ContextTarget::SidebarPane { pane_id, .. } => Some(*pane_id),
            _ => None,
        }
    }

    /// The workspace index this target belongs to, if resolvable. Direct for workspace/column
    /// targets; for a pane target it is resolved from the pane's location in `state`. Used to
    /// retarget workspace actions (rename) to the item the sidebar cursor is on.
    pub(crate) fn ws_idx(&self, state: &AppState) -> Option<usize> {
        match self {
            ContextTarget::SidebarWorkspace { ws_idx, .. }
            | ContextTarget::SidebarColumn { ws_idx, .. }
            | ContextTarget::SidebarPane { ws_idx, .. } => Some(*ws_idx),
            ContextTarget::Pane { pane_id, .. } => {
                crate::find_pane_location(&state.session, *pane_id).map(|(ws, _, _)| ws)
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
//  Registry
// ──────────────────────────────────────────────────────────────────────────────

/// How a provider builds its entries: read the app through the **`ChromeCtx` facade**, read *what
/// was clicked* from the [`ContextTarget`], return entries.
///
/// **One signature for built-ins and plugins alike.** A provider — first-party or plugin — never
/// receives `&AppState`; it reads state through `ctx.app().state()` selectors and event
/// subscriptions, the same facade a plugin has. That is the point of unifying: a gap in the facade
/// is hit by the built-in menus first, in the open, instead of staying invisible until the first
/// real plugin trips over it.
///
/// An `Rc<dyn Fn>` rather than a bare `fn` pointer because a plugin's builder **captures** (its own
/// state, its container id); a built-in is a plain fn coerced into it, capturing nothing.
pub type MenuBuild = std::rc::Rc<dyn Fn(&ChromeCtx, &ContextTarget) -> Vec<DropdownItem>>;

/// A registered context-menu provider: a weight (Dewey/fractional index, C2) for merge ordering
/// and a build function.
#[derive(Clone)]
pub struct ContextMenuProvider {
    /// Merge order within a context path. Built-ins use a stable weight; a plugin inserts
    /// between two built-ins with a fractional `Vec<i64>` (e.g. `[1,1,1]` between `[1,1]` and
    /// `[1,2]`). Sorted ascending; ties keep insertion order (stable sort).
    pub weight: Vec<i64>,
    pub build: MenuBuild,
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
        r.register(ContextPath::PANE, vec![0], std::rc::Rc::new(build_pane_menu));
        r.register(
            ContextPath::SIDEBAR_PANE,
            vec![1],
            std::rc::Rc::new(build_sidebar_pane_menu),
        );
        r.register(
            ContextPath::SIDEBAR_COLUMN,
            vec![1],
            std::rc::Rc::new(build_sidebar_column_menu),
        );
        r.register(
            ContextPath::SIDEBAR_WORKSPACE,
            vec![1],
            std::rc::Rc::new(build_sidebar_workspace_menu),
        );
        r
    }

    /// Register a provider for a context path — the built-in seed and a plugin's
    /// `Contribution::ContextMenu` go through this same call.
    pub fn register(&mut self, path: &str, weight: Vec<i64>, build: MenuBuild) {
        self.providers
            .entry(path.to_string())
            .or_default()
            .push(ContextMenuProvider { weight, build });
    }

    /// Collect + merge items for a context path: every matching provider — **built-in and
    /// plugin-contributed alike** — sorted by weight (stable), concatenated. `vec![]` when nothing
    /// matches (an empty menu — the caller may choose not to open it).
    ///
    /// `plugin` are the [`ContextMenuContribution`](crate::chrome::ContextMenuContribution)s the
    /// mounted providers declared for this path. They are merged **by weight**, not appended, so a
    /// plugin's entries land *between* the built-ins (`[1,1,1]` between `[1,1]` and `[1,2]`) rather
    /// than always at the end.
    ///
    /// Every provider reads the app through `ctx` — never `AppState` — so a plugin's builder and a
    /// built-in's are called through exactly the same contract.
    pub fn items_for(
        &self,
        ctx: &ChromeCtx,
        path: &str,
        target: &ContextTarget,
        plugin: Vec<ContextMenuProvider>,
    ) -> Vec<DropdownItem> {
        let mut providers = self.ordered_providers(path);
        providers.extend(plugin);
        // Re-sort: the plugin entries must interleave with the built-ins by weight.
        providers.sort_by(|a, b| a.weight.cmp(&b.weight));

        let mut items = Vec::new();
        for p in providers {
            items.extend((p.build)(ctx, target));
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
    // Providers — built-in and plugin alike — build against the host facade, never `AppState`. An
    // observe-only context is right here: building a menu reads state, it never renders.
    let ctx = ChromeCtx::new(App::new(&state.chrome_state));

    // Plugin entries are collected from the MOUNTED providers at open time, rather than
    // pre-registered at activation: a menu is then always built from what is mounted *now*, so an
    // unmounted provider's entries cannot linger and there is no registration to keep in sync.
    let plugin = plugin_providers_for(&state.chrome_host, &ctx, path);
    let items = state
        .context_menu_registry
        .items_for(&ctx, path, &target, plugin);
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

/// Every context-menu contribution the **mounted** providers declare for `path`, as providers ready
/// to be merged with the built-ins by weight.
///
/// A provider declares *where* and *what*; the host decides *when*. This is the "when": one pass
/// over what is mounted, at the moment the menu opens.
fn plugin_providers_for(
    host: &crate::chrome::ChromeHost,
    ctx: &ChromeCtx,
    path: &str,
) -> Vec<ContextMenuProvider> {
    host.mounted_providers()
        .flat_map(|p| p.context_menus(ctx))
        .filter(|c| c.context_path == path)
        .map(|c| ContextMenuProvider {
            weight: c.weight,
            build: c.build,
        })
        .collect()
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
/// - `SidebarNav`: maps [`crate::providers::workspaces::WorkspaceRow`] at the cursor to its context path
///   (pane→`sidebar.pane`, column→`sidebar.column`, workspace→`sidebar.workspace`,
///   floating-pane→`pane`).
/// - `Normal` (and any other mode): resolves to the focused content pane (`"pane"`).
/// - Returns `None` when there is no active target (no focused pane, no sidebar cursor).
pub(crate) fn resolve_active_context(state: &AppState) -> Option<(ContextPath, ContextTarget)> {
    resolve_context_for(
        &state.input_mode,
        state.chrome_state.workspaces.tree().current_item(),
        crate::app::interaction::focused_pane_id(state),
        &|pane_id| {
            crate::find_pane_location(&state.session, pane_id).map(|(ws, col, _)| (ws, col))
        },
        &|ws_idx| {
            state
                .session
                .workspaces
                .get(ws_idx)
                .and_then(|ws| ws.name.clone())
        },
    )
}

/// Pure mapping behind [`resolve_active_context`]: input mode + the active sidebar item +
/// the focused pane → menu `(path, target)`. Split out from the `AppState` reads so the
/// mapping is unit-testable without a full app. In `SidebarNav` the sidebar cursor decides
/// the context (pane→`sidebar.pane`, column→`sidebar.column`, workspace→`sidebar.workspace`,
/// floating-pane→`pane`); in any other mode the focused content pane does. Returns `None`
/// when there is nothing to target (no sidebar cursor / no focused pane).
/// `locate` resolves a pane to its `(ws_idx, col_idx)` and `ws_name` a workspace to its custom
/// name — the two facts a builder cannot get from the facade, so the **host** puts them on the
/// target. Passed as closures (rather than `&AppState`) so this mapping stays unit-testable.
fn resolve_context_for(
    input_mode: &InputMode,
    sidebar_item: Option<&crate::providers::workspaces::WorkspaceRow>,
    focused_pane: Option<PaneId>,
    locate: &dyn Fn(PaneId) -> Option<(usize, usize)>,
    ws_name: &dyn Fn(usize) -> Option<String>,
) -> Option<(ContextPath, ContextTarget)> {
    match input_mode {
        InputMode::SidebarNav => Some(match sidebar_item? {
            crate::providers::workspaces::WorkspaceRow::Pane { pane_id } => {
                // A sidebar pane row whose column can't be resolved is not a valid target.
                let (ws_idx, col_idx) = locate(*pane_id)?;
                (
                    ContextPath(ContextPath::SIDEBAR_PANE.to_string()),
                    ContextTarget::SidebarPane { pane_id: *pane_id, ws_idx, col_idx },
                )
            }
            crate::providers::workspaces::WorkspaceRow::FloatingPane { pane_id, .. } => (
                ContextPath(ContextPath::PANE.to_string()),
                ContextTarget::Pane { pane_id: *pane_id, hyperlink: None },
            ),
            crate::providers::workspaces::WorkspaceRow::Column { ws_idx, col_idx } => (
                ContextPath(ContextPath::SIDEBAR_COLUMN.to_string()),
                ContextTarget::SidebarColumn { ws_idx: *ws_idx, col_idx: *col_idx },
            ),
            crate::providers::workspaces::WorkspaceRow::Workspace { ws_idx } => (
                ContextPath(ContextPath::SIDEBAR_WORKSPACE.to_string()),
                ContextTarget::SidebarWorkspace {
                    ws_idx: *ws_idx,
                    custom_name: ws_name(*ws_idx),
                },
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

/// An entry whose id, label and action name coincide (`zoom_column`, `float`, …).
fn item(id: &str, label: &str) -> DropdownItem {
    DropdownItem::new(id, label)
}

/// An entry whose visual identity (`id` → icon/label) differs from what it runs. The `action` is
/// the name [`build_action`](crate::input::build_action) resolves, and `args` are its arguments —
/// the same pair a `config.toml` binding or an RPC command would supply.
fn item_running(id: &str, label: &str, action: &str, args: &[(&str, PropValue)]) -> DropdownItem {
    let mut intent = Intent::new(action);
    for (k, v) in args {
        intent = intent.arg(*k, v.clone());
    }
    DropdownItem::with_intent(id, label, intent)
}

fn usize_arg(v: usize) -> PropValue {
    PropValue::Int(v as i64)
}

/// Pane context-menu items. `has_custom_name` gates the "Use process name" reset entry — it only
/// appears when there is a custom name to clear. This is a **pane** menu, so it carries no
/// workspace-reset action.
fn pane_action_items(has_custom_name: bool) -> Vec<DropdownItem> {
    let mut items = vec![
        item("split_horizontal", "New column"),
        // "Add a pane" reads consistently everywhere: same label + FolderSimplePlus icon (via the
        // `add_pane_to_column` id) as the sidebar "New pane" and the pane-header "+" button. It
        // still RUNS `split_vertical` (add a pane to the active column) — identity vs behaviour.
        item_running("add_pane_to_column", "New pane", "split_vertical", &[]),
        item("zoom_column", "Zoom / unzoom"),
        item("float", "Float / unfloat"),
        item_running("rename_pane", "Rename", "rename_pane", &[]),
    ];
    if has_custom_name {
        items.push(item("reset_pane_name", "Use process name"));
    }
    items.push(item("rename_workspace", "Rename workspace"));
    items.push(item_running("close", "Close pane", "close", &[]).danger(true));
    items
}

/// Provider for [`ContextPath::PANE`] — a content pane. "Open link" first only when the mouse
/// click carried a hyperlink (keyboard-opened menus have `None` → no link entry).
fn build_pane_menu(ctx: &ChromeCtx, target: &ContextTarget) -> Vec<DropdownItem> {
    let ContextTarget::Pane { hyperlink, pane_id } = target else {
        return Vec::new();
    };
    // Read through the facade — the same selector a plugin would use.
    let has_custom_name = ctx.state().pane_custom_name(*pane_id).is_some();
    let mut items = pane_action_items(has_custom_name);
    if let Some(url) = hyperlink {
        items.insert(
            0,
            item_running(
                "open_link",
                "Open link",
                "open_link",
                &[("url", PropValue::Text(url.clone()))],
            ),
        );
    }
    items
}

/// Provider for [`ContextPath::SIDEBAR_PANE`] — a pane row in the sidebar tree. Its actions target
/// **that row's** pane (by id) and column, both carried on the target.
fn build_sidebar_pane_menu(ctx: &ChromeCtx, target: &ContextTarget) -> Vec<DropdownItem> {
    let ContextTarget::SidebarPane {
        pane_id,
        ws_idx,
        col_idx,
    } = target
    else {
        return Vec::new();
    };
    // Read through the facade — the same selector a plugin would use.
    let has_custom_name = ctx.state().pane_custom_name(*pane_id).is_some();
    sidebar_pane_items(*pane_id, *ws_idx, *col_idx, has_custom_name)
}

/// Menu items for a sidebar **pane** row (target-only, so unit-testable without a ctx). Every entry
/// acts on *that* row: the pane by id, "New pane" in the pane's own column.
fn sidebar_pane_items(
    pane_id: PaneId,
    ws_idx: usize,
    col_idx: usize,
    has_custom_name: bool,
) -> Vec<DropdownItem> {
    let pane = PropValue::Int(pane_id.0 as i64);
    let mut items = vec![
        item_running(
            "add_pane_to_column",
            "New pane",
            "add_pane_to_column",
            &[("ws_idx", usize_arg(ws_idx)), ("col_idx", usize_arg(col_idx))],
        ),
        item_running(
            "rename_pane",
            "Rename pane",
            "rename_pane_by_id",
            &[("pane_id", pane.clone())],
        ),
    ];
    if has_custom_name {
        items.push(item_running(
            "reset_pane_name",
            "Use process name",
            "reset_pane_name_by_id",
            &[("pane_id", pane.clone())],
        ));
    }
    items.push(
        item_running("close", "Delete pane", "close_pane_by_id", &[("pane_id", pane)]).danger(true),
    );
    items
}

/// Provider for [`ContextPath::SIDEBAR_COLUMN`] — a column row. "New pane" (in the column) +
/// "New column" + "Delete column" (danger).
fn build_sidebar_column_menu(_ctx: &ChromeCtx, target: &ContextTarget) -> Vec<DropdownItem> {
    let ContextTarget::SidebarColumn { ws_idx, col_idx } = target else {
        return Vec::new();
    };
    sidebar_column_items(*ws_idx, *col_idx)
}

/// Menu items for a sidebar **column** row (target-only, so unit-testable without a ctx).
fn sidebar_column_items(ws_idx: usize, col_idx: usize) -> Vec<DropdownItem> {
    vec![
        item_running(
            "add_pane_to_column",
            "New pane",
            "add_pane_to_column",
            &[("ws_idx", usize_arg(ws_idx)), ("col_idx", usize_arg(col_idx))],
        ),
        item_running(
            "split_horizontal",
            "New column",
            "add_column_to_workspace",
            &[("ws_idx", usize_arg(ws_idx))],
        ),
        // NB: no "Rename column" entry — a column's name is not displayed anywhere yet (columns
        // render as a MarkerGroup with no header/label), so renaming would have no visible effect.
        // The rename action stays wired (RPC + handler) for when columns surface a name.
        item_running(
            "delete_column",
            "Delete column",
            "delete_column",
            &[("ws_idx", usize_arg(ws_idx)), ("col_idx", usize_arg(col_idx))],
        )
        .danger(true),
    ]
}

/// Provider for [`ContextPath::SIDEBAR_WORKSPACE`] — a workspace row.
fn build_sidebar_workspace_menu(_ctx: &ChromeCtx, target: &ContextTarget) -> Vec<DropdownItem> {
    let ContextTarget::SidebarWorkspace {
        ws_idx,
        custom_name,
    } = target
    else {
        return Vec::new();
    };
    sidebar_workspace_items(*ws_idx, custom_name.is_some())
}

/// Menu items for a sidebar **workspace** row. `has_custom_name` gates the "Use default name"
/// reset entry — it only appears when there is a custom name to clear.
fn sidebar_workspace_items(ws_idx: usize, has_custom_name: bool) -> Vec<DropdownItem> {
    let mut items = vec![
        item_running(
            "split_horizontal",
            "New column",
            "add_column_to_workspace",
            &[("ws_idx", usize_arg(ws_idx))],
        ),
        item("create_workspace", "New workspace"),
        item_running(
            "rename_workspace",
            "Rename workspace",
            "rename_workspace_by_idx",
            &[("ws_idx", usize_arg(ws_idx))],
        ),
    ];
    if has_custom_name {
        items.push(item_running(
            "reset_workspace_name",
            "Use default name",
            "reset_workspace_name_by_idx",
            &[("ws_idx", usize_arg(ws_idx))],
        ));
    }
    items.push(
        item_running(
            "delete_workspace",
            "Delete workspace",
            "delete_workspace",
            &[("ws_idx", usize_arg(ws_idx))],
        )
        .danger(true),
    );
    items
}

// ──────────────────────────────────────────────────────────────────────────────
//  Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// An observe-only facade over an empty chrome store — what a provider's `build` receives.
    fn test_ctx() -> ChromeCtx<'static> {
        // Leaked deliberately: a test-only store that must outlive the borrow in `App`.
        let store: &'static crate::chrome::SharedChromeState =
            Box::leak(Box::new(crate::chrome::SharedChromeState::new(
                300.0, true, 300.0, false,
            )));
        ChromeCtx::new(App::new(store))
    }

    /// `resolve_context_for` with stub lookups: pane 7 lives at ws 1 / col 2; workspace 3 is named.
    fn resolve(
        mode: &InputMode,
        item: Option<&crate::providers::workspaces::WorkspaceRow>,
        focused: Option<PaneId>,
    ) -> Option<(ContextPath, ContextTarget)> {
        resolve_context_for(
            mode,
            item,
            focused,
            &|_pane| Some((1, 2)),
            &|ws| (ws == 3).then(|| "My WS".to_string()),
        )
    }

    fn build_sidebar_pane_menu_items(
        pane_id: PaneId,
        ws_idx: usize,
        col_idx: usize,
        has_custom_name: bool,
    ) -> Vec<DropdownItem> {
        sidebar_pane_items(pane_id, ws_idx, col_idx, has_custom_name)
    }

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
        let items = pane_action_items(true);
        assert_eq!(items[0].id, "split_horizontal");
        assert!(items.iter().any(|i| i.id == "rename_pane"), "has Rename");
        assert!(
            items.iter().any(|i| i.id == "rename_workspace"),
            "has Rename workspace"
        );
        let last = items.last().unwrap();
        assert_eq!(last.id, "close");
        assert!(last.danger, "close is danger-styled");
        // Pane menu carries no workspace-reset action (that lives in the workspace menu).
        assert!(
            !items.iter().any(|i| i.id == "reset_workspace_name"),
            "pane menu must not reset the workspace"
        );
    }

    #[test]
    fn reset_name_entries_are_conditional_on_a_custom_name() {
        // "Use process name" only when the pane has a custom name.
        assert!(
            pane_action_items(true).iter().any(|i| i.id == "reset_pane_name"),
            "reset shows with a custom name"
        );
        assert!(
            !pane_action_items(false).iter().any(|i| i.id == "reset_pane_name"),
            "reset hidden without a custom name"
        );
        // "Use default name" only when the workspace has a custom name.
        assert!(
            sidebar_workspace_items(0, true)
                .iter()
                .any(|i| i.id == "reset_workspace_name")
        );
        assert!(
            !sidebar_workspace_items(0, false)
                .iter()
                .any(|i| i.id == "reset_workspace_name")
        );
    }

    #[test]
    fn ordered_providers_sorts_by_weight() {
        // A plugin inserts between two built-ins via fractional weights.
        let mut r = ContextMenuRegistry::with_builtins();
        // Register a second provider for PANE with a higher weight — it sorts after the built-in.
        r.register(
            ContextPath::PANE,
            vec![2],
            std::rc::Rc::new(|_ctx: &ChromeCtx, _target: &ContextTarget| {
                vec![DropdownItem::new("plugin_extra", "Plugin")]
            }),
        );
        let ordered = r.ordered_providers(ContextPath::PANE);
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].weight, vec![0], "built-in (weight 0) first");
        assert_eq!(ordered[1].weight, vec![2], "plugin (weight 2) second");
    }

    // ── context-menu-5: a plugin's entries ──

    /// A menu entry's **identity** (`id` → icon/label) and its **behaviour** (`intent` → what runs)
    /// are separate. The sidebar "Delete pane" entry shows the `close` icon but runs
    /// `close_pane_by_id` against *that row's* pane — the whole reason a plugin entry can exist at
    /// all, since the intent is a NAME, not a closed-enum variant.
    #[test]
    fn an_entry_identity_and_the_action_it_runs_are_separate() {
        let items = build_sidebar_pane_menu_items(PaneId(7), 2, 3, false);
        let delete = items.iter().find(|i| i.id == "close").unwrap();
        assert_eq!(delete.label, "Delete pane");
        assert!(delete.danger);
        // Identity is `close` (the icon), behaviour is the by-id action with the row's pane.
        assert_eq!(delete.intent.action, "close_pane_by_id");
        assert_eq!(delete.intent.args.get("pane_id"), Some(&PropValue::Int(7)));

        // "New pane" targets THIS pane's column — the location the host put on the target.
        let new_pane = items.iter().find(|i| i.id == "add_pane_to_column").unwrap();
        assert_eq!(new_pane.intent.action, "add_pane_to_column");
        assert_eq!(new_pane.intent.args.get("ws_idx"), Some(&PropValue::Int(2)));
        assert_eq!(new_pane.intent.args.get("col_idx"), Some(&PropValue::Int(3)));
    }

    /// Every built-in entry's intent must actually RESOLVE — a name + args that `build_action` (or
    /// `action_from_name`) can turn into a real `WmAction`. Without this, converting the menus from
    /// `WmAction` to `Intent` could silently produce entries that do nothing when clicked.
    #[test]
    fn every_builtin_menu_entry_resolves_to_a_real_action() {
        let mut all = pane_action_items(true);
        all.extend(build_sidebar_pane_menu_items(PaneId(1), 0, 0, true));
        all.extend(sidebar_column_items(0, 0));
        all.extend(sidebar_workspace_items(0, true));
        all.push(item_running(
            "open_link",
            "Open link",
            "open_link",
            &[("url", PropValue::Text("https://x".into()))],
        ));

        for i in &all {
            let args: std::collections::HashMap<String, String> = i
                .intent
                .args
                .iter()
                .map(|(k, v)| {
                    let s = match v {
                        PropValue::Int(n) => n.to_string(),
                        PropValue::Text(t) => t.clone(),
                        other => format!("{other:?}"),
                    };
                    (k.clone(), s)
                })
                .collect();
            let resolved = crate::input::build_action(&i.intent.action, &args).is_some()
                || crate::input::action_from_name(&i.intent.action).is_some();
            assert!(
                resolved,
                "menu entry {:?} dispatches {:?}, which resolves to no action",
                i.id, i.intent.action
            );
        }
    }

    /// THE POINT OF THE TASK: a provider's entries merge **between** the built-ins by weight (C2),
    /// not appended after them — and the entry carries the provider's **own** action id, which no
    /// `WmAction` variant exists for.
    #[test]
    fn a_plugin_entry_merges_between_the_builtins_and_runs_its_own_action() {
        let mut r = ContextMenuRegistry::default();
        // Two "built-ins" at [1,1] and [1,2] …
        r.register(
            "sidebar.workspace",
            vec![1, 1],
            std::rc::Rc::new(|_c: &ChromeCtx, _t: &ContextTarget| {
                vec![DropdownItem::new("first", "First")]
            }),
        );
        r.register(
            "sidebar.workspace",
            vec![1, 2],
            std::rc::Rc::new(|_c: &ChromeCtx, _t: &ContextTarget| {
                vec![DropdownItem::new("last", "Last")]
            }),
        );
        // … and the plugin slots in at [1,1,1].
        let plugin = vec![ContextMenuProvider {
            weight: vec![1, 1, 1],
            build: std::rc::Rc::new(|_c: &ChromeCtx, t: &ContextTarget| {
                let ContextTarget::SidebarWorkspace { ws_idx, .. } = t else {
                    return Vec::new();
                };
                vec![DropdownItem::with_intent(
                    "docker.restart",
                    "Restart",
                    // A name-keyed action of the PLUGIN's — there is no WmAction for this.
                    Intent::new("plugin.docker.restart")
                        .arg("ws_idx", PropValue::Int(*ws_idx as i64)),
                )]
            }),
        }];

        let mut ordered = r.ordered_providers("sidebar.workspace");
        ordered.extend(plugin);
        ordered.sort_by(|a, b| a.weight.cmp(&b.weight));
        let order: Vec<Vec<i64>> = ordered.iter().map(|p| p.weight.clone()).collect();
        assert_eq!(
            order,
            vec![vec![1, 1], vec![1, 1, 1], vec![1, 2]],
            "the plugin sits BETWEEN the built-ins, not after them"
        );

        // And its entry dispatches the plugin's own action, with the target's data.
        let target = ContextTarget::SidebarWorkspace { ws_idx: 4, custom_name: None };
        let entry = (ordered[1].build)(&test_ctx(), &target).remove(0);
        assert_eq!(entry.id, "docker.restart");
        assert_eq!(entry.intent.action, "plugin.docker.restart");
        assert_eq!(entry.intent.args.get("ws_idx"), Some(&PropValue::Int(4)));
        assert!(
            crate::input::action_from_name(&entry.intent.action).is_none(),
            "the plugin's action has no WmAction variant — which is exactly why an \
             Intent (a name) is what a menu entry carries"
        );
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

    use crate::providers::workspaces::WorkspaceRow;

    #[test]
    fn resolve_context_sidebar_items_map_to_distinct_paths() {
        // Each sidebar cursor item resolves to its own context path + target, so the menu
        // content differs by where it was opened.
        let pane = WorkspaceRow::Pane { pane_id: PaneId(7) };
        let (path, target) =
            resolve(&InputMode::SidebarNav, Some(&pane), None).unwrap();
        assert_eq!(path.0, ContextPath::SIDEBAR_PANE);
        assert!(matches!(target, ContextTarget::SidebarPane { pane_id: PaneId(7), ws_idx: 1, col_idx: 2 }));

        let col = WorkspaceRow::Column { ws_idx: 1, col_idx: 2 };
        let (path, target) =
            resolve(&InputMode::SidebarNav, Some(&col), None).unwrap();
        assert_eq!(path.0, ContextPath::SIDEBAR_COLUMN);
        assert!(matches!(target, ContextTarget::SidebarColumn { ws_idx: 1, col_idx: 2 }));

        let ws = WorkspaceRow::Workspace { ws_idx: 3 };
        let (path, target) =
            resolve(&InputMode::SidebarNav, Some(&ws), None).unwrap();
        assert_eq!(path.0, ContextPath::SIDEBAR_WORKSPACE);
        assert!(matches!(target, ContextTarget::SidebarWorkspace { ws_idx: 3, .. }));

        // A floating pane in the sidebar resolves to the generic pane menu.
        let float = WorkspaceRow::FloatingPane { pane_id: PaneId(9), ws_idx: 0 };
        let (path, target) =
            resolve(&InputMode::SidebarNav, Some(&float), None).unwrap();
        assert_eq!(path.0, ContextPath::PANE);
        assert!(matches!(target, ContextTarget::Pane { pane_id: PaneId(9), hyperlink: None }));
    }

    #[test]
    fn resolve_context_non_sidebar_uses_focused_pane() {
        // Any non-sidebar mode → the focused content pane.
        let (path, target) =
            resolve(&InputMode::Normal, None, Some(PaneId(4))).unwrap();
        assert_eq!(path.0, ContextPath::PANE);
        assert!(matches!(target, ContextTarget::Pane { pane_id: PaneId(4), hyperlink: None }));
    }

    #[test]
    fn resolve_context_none_when_no_target() {
        // SidebarNav with no cursor item, and Normal with no focused pane, both resolve to None.
        assert!(resolve(&InputMode::SidebarNav, None, Some(PaneId(1))).is_none());
        assert!(resolve(&InputMode::Normal, None, None).is_none());
    }

    #[test]
    fn context_menus_differ_by_where_opened() {
        // The content each context produces is distinct — proving the menu adapts to where
        // it is opened (pane vs sidebar column vs sidebar workspace).
        let pane = pane_action_items(true);
        let col = sidebar_column_items(0, 0);
        let ws = sidebar_workspace_items(0, true);
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
        // Each sidebar context ends in a danger delete action, keyed by its own action
        // name so it shows that action's icon (Trash / StackMinus), not the pane `close` one.
        assert_eq!(col.last().map(|i| i.id.as_str()), Some("delete_column"));
        assert!(col.last().map(|i| i.danger).unwrap_or(false));
        assert_eq!(ws.last().map(|i| i.id.as_str()), Some("delete_workspace"));
        assert!(ws.last().map(|i| i.danger).unwrap_or(false));
    }
}