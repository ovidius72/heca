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
//! Nothing is restored when a menu closes: a container's keyboard focus is not a mode, an overlay
//! never takes it away, and it is still there afterwards (F003/P086/T365).

use crate::app::interaction::InteractionSource;
use crate::app_state::AppState;
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
}

/// What the menu was opened on. The host fills it from the hit-test (mouse) or from the focused
/// container's cursor (keyboard); providers match the arm they handle and return `vec![]` for any
/// other.
///
/// **Two arms, and the second one is generic** (F003/P086/T365). A row is named the way its
/// component named it — the container it is in and the `nav_key` the row declared — and nothing
/// else. It used to carry three workspace-shaped variants filled with facts the *host* had
/// resolved (a pane's column, a workspace's custom name), which is why a Docker row could not be
/// right-clicked at all: there was no variant for it and no way to add one without the host
/// learning what Docker is. The component that wrote the key is the one that reads it back, against
/// its own model, so those facts are its own to look up.
#[derive(Clone, Debug)]
pub enum ContextTarget {
    /// A content pane. `hyperlink` is `Some(url)` only for the mouse path (the clicked cell);
    /// a keyboard-opened menu has `None` (no target cell) so no "Open link" entry is added.
    Pane {
        pane_id: PaneId,
        hyperlink: Option<String>,
    },
    /// A **row of a mounted container**: which placement it is in, and the key that row declared
    /// (`NavExt::nav_key`). Opaque to the host — it never parses one.
    ///
    /// `container` is the **mount id**, not the component type, so a component seated twice can
    /// tell which of its seatings was clicked and answer for that one only.
    Row { container: String, key: String },
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
    /// Registry seeded with the **one** built-in provider: the content pane, which is the app's own
    /// domain. A container's rows are its component's to describe, and the workspaces component
    /// registers its three menus through [`Provider::context_menus`] like any plugin
    /// (F003/P086/T365).
    pub fn with_builtins() -> Self {
        let mut r = Self::default();
        // Weight 0, so a component's or plugin's entries (weight 1 upwards) merge after it.
        r.register(ContextPath::PANE, vec![0], std::rc::Rc::new(build_pane_menu));
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
/// **No mode is restored when it closes** (F003/P086/T365): chrome focus is not a mode, an overlay
/// does not take it away, and it is simply still there afterwards — so there is nothing to record
/// and no `origin` parameter to pass.
pub(crate) fn open_context_menu_for(
    state: &mut AppState,
    path: &str,
    target: ContextTarget,
    anchor: Point,
    source: InteractionSource,
) {
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

/// Resolve the active context for a keyboard-opened context menu (`OpenContextMenu` /
/// `prefix+>`): the focused container's cursor row, or the focused content pane.
///
/// **A focused container decides the context**, not a mode (F003/P086/T365) — the menu follows the
/// keyboard, and the keyboard is in a container or it is not. The row is named by
/// `(container, nav_key)` and its **component** says which menu path describes it
/// ([`Provider::context_path`](crate::providers::Provider::context_path)), so the host resolves a
/// menu for a Docker row exactly as it does for a workspace row, knowing neither.
///
/// Returns `None` when there is nothing to target (no cursor in the focused container, or no
/// focused pane outside one).
pub(crate) fn resolve_active_context(state: &AppState) -> Option<(ContextPath, ContextTarget)> {
    let container = state.chrome_state.focused_container();
    let cursor = container
        .as_deref()
        .and_then(|mount| {
            use heca_grid_ui::reactive::SignalGet as _;
            state.chrome_state.container_cursor(mount).get()
        });
    resolve_context_for(
        container.as_deref(),
        cursor.as_deref(),
        crate::app::interaction::focused_pane_id(state),
        &|mount, key| {
            let ctx = crate::providers::ChromeCtx::new(App::new(&state.chrome_state));
            state
                .chrome_host
                .provider(mount)
                .and_then(|p| p.context_path(key, &ctx))
        },
    )
}

/// Pure mapping behind [`resolve_active_context`]: the focused container + its cursor row + the
/// focused pane → menu `(path, target)`. Split out from the `AppState` reads so the mapping is
/// unit-testable without a full app.
///
/// `path_for` is the mounted component's answer for one of **its own** row keys; a component that
/// names no menu for that row (or is not mounted) yields no menu rather than a guessed one.
fn resolve_context_for(
    container: Option<&str>,
    cursor: Option<&str>,
    focused_pane: Option<PaneId>,
    path_for: &dyn Fn(&str, &str) -> Option<String>,
) -> Option<(ContextPath, ContextTarget)> {
    match container {
        Some(container) => {
            let key = cursor?;
            Some((
                ContextPath(path_for(container, key)?),
                ContextTarget::Row {
                    container: container.to_string(),
                    key: key.to_string(),
                },
            ))
        }
        None => Some((
            ContextPath(ContextPath::PANE.to_string()),
            ContextTarget::Pane { pane_id: focused_pane?, hyperlink: None },
        )),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
//  Built-in providers
// ──────────────────────────────────────────────────────────────────────────────

/// An entry whose id, label and action name coincide (`zoom_column`, `float`, …).
///
/// `pub(crate)` because a **component builds its own menu**: the three sidebar builders are moving
/// into the workspaces provider (F003/P086/T365), while `build_pane_menu` stays here — the content
/// pane is the app's own domain. These three are the shared vocabulary both sides write entries
/// with, so they belong to neither.
pub(crate) fn item(id: &str, label: &str) -> DropdownItem {
    DropdownItem::new(id, label)
}

/// An entry whose visual identity (`id` → icon/label) differs from what it runs. The `action` is
/// the name [`build_action`](crate::input::build_action) resolves, and `args` are its arguments —
/// the same pair a `config.toml` binding or an RPC command would supply.
pub(crate) fn item_running(
    id: &str,
    label: &str,
    action: &str,
    args: &[(&str, PropValue)],
) -> DropdownItem {
    let mut intent = Intent::new(action);
    for (k, v) in args {
        intent = intent.arg(*k, v.clone());
    }
    DropdownItem::with_intent(id, label, intent)
}

/// A `usize` argument as the `PropValue` an [`Intent`] carries — the form every `ws_idx` /
/// `col_idx` menu argument takes.
pub(crate) fn usize_arg(v: usize) -> PropValue {
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

    /// `resolve_context_for` with a stub component: it names `row:*` keys `stub.row` and knows
    /// nothing else — which is all the host is allowed to know about a row (F003/P086/T365).
    fn resolve(
        container: Option<&str>,
        cursor: Option<&str>,
        focused: Option<PaneId>,
    ) -> Option<(ContextPath, ContextTarget)> {
        resolve_context_for(container, cursor, focused, &|_mount, key| {
            key.starts_with("row:").then(|| "stub.row".to_string())
        })
    }

    /// **A destructive entry uses the same verb as the action it names** (F003/P086/T370).
    ///
    /// A `DropdownItem`'s `id` is its catalog identity — the icon and the shortcut resolve from it —
    /// while its label is written by hand at the call site. Nothing compared the two, so the sidebar
    /// pane row said "Delete pane" while its own action, its palette entry, the content-pane menu
    /// and its confirm dialog all said close. The user found it by reading the menu.
    ///
    /// **Only the destructive three**, deliberately. A menu label is free to differ from the catalog
    /// in general — "Zoom / unzoom" is a better menu entry than "Toggle Column Zoom" — and holding
    /// every entry to the catalog's wording would flag those as drift. What may not differ is the
    /// verb on something irreversible: close and delete promise different consequences.
    ///
    /// The verb alone is compared, since a menu is sentence case ("Close pane") and the catalog is
    /// title case ("Close Pane") — two conventions, each applied consistently.
    #[test]
    fn a_destructive_entry_uses_the_same_verb_as_the_action_it_names() {
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let mut every_item = Vec::new();
        every_item.extend(pane_action_items(true));
        every_item.extend(pane_row_items(PaneId(1), Some((0, 0)), true));
        every_item.extend(column_row_items(0, 0));
        every_item.extend(workspace_row_items(0, true));

        let verb = |s: &str| s.split_whitespace().next().unwrap_or("").to_lowercase();
        let mut checked = 0;
        for item in &every_item {
            if !["close", "delete_column", "delete_workspace"].contains(&item.id.as_str()) {
                continue;
            }
            let meta = catalog
                .find(&item.id)
                .unwrap_or_else(|| panic!("`{}` is a built-in action", item.id));
            assert_eq!(
                verb(&item.label),
                verb(&meta.label),
                "menu entry '{}' (id `{}`) disagrees with its action's label '{}'",
                item.label,
                item.id,
                meta.label,
            );
            checked += 1;
        }
        assert_eq!(checked, 4, "two close entries + one column + one workspace");
    }

    /// **The host seeds the content pane and nothing else** (F003/P086/T365). A container's rows
    /// are described by the component that owns them: the three row menus arrive at open time
    /// through `Provider::context_menus`, merged by `plugin_providers_for`, exactly as a plugin's
    /// would. The host knowing three workspace-shaped paths was the last place it enumerated
    /// another component's row kinds.
    #[test]
    fn the_registry_seeds_only_the_content_pane() {
        let r = ContextMenuRegistry::with_builtins();
        assert!(r.providers.contains_key(ContextPath::PANE));
        assert_eq!(r.providers.get(ContextPath::PANE).unwrap().len(), 1);
        for owned_by_the_component in [
            crate::providers::workspaces::MENU_PANE,
            crate::providers::workspaces::MENU_COLUMN,
            crate::providers::workspaces::MENU_WORKSPACE,
        ] {
            assert!(
                !r.providers.contains_key(owned_by_the_component),
                "{owned_by_the_component} is the component's to declare, not the host's to seed",
            );
        }
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
            workspace_row_items(0, true)
                .iter()
                .any(|i| i.id == "reset_workspace_name")
        );
        assert!(
            !workspace_row_items(0, false)
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
    /// are separate. The sidebar "Close pane" entry shows the `close` icon but runs
    /// `close_pane_by_id` against *that row's* pane — the whole reason a plugin entry can exist at
    /// all, since the intent is a NAME, not a closed-enum variant.
    #[test]
    fn an_entry_identity_and_the_action_it_runs_are_separate() {
        let items = pane_row_items(PaneId(7), Some((2, 3)), false);
        let delete = items.iter().find(|i| i.id == "close").unwrap();
        assert_eq!(delete.label, "Close pane");
        assert!(delete.danger);
        // Identity is `close` (the icon), behaviour is the by-id action with the row's pane.
        assert_eq!(delete.intent.action, "close_pane_by_id");
        assert_eq!(delete.intent.args.get("pane_id"), Some(&PropValue::Int(7)));

        // "New pane" targets THIS pane's column — which the component looked up in its own model.
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
        all.extend(pane_row_items(PaneId(1), Some((0, 0)), true));
        all.extend(column_row_items(0, 0));
        all.extend(workspace_row_items(0, true));
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
            "docker.container",
            vec![1, 1],
            std::rc::Rc::new(|_c: &ChromeCtx, _t: &ContextTarget| {
                vec![DropdownItem::new("first", "First")]
            }),
        );
        r.register(
            "docker.container",
            vec![1, 2],
            std::rc::Rc::new(|_c: &ChromeCtx, _t: &ContextTarget| {
                vec![DropdownItem::new("last", "Last")]
            }),
        );
        // … and the plugin slots in at [1,1,1].
        let plugin = vec![ContextMenuProvider {
            weight: vec![1, 1, 1],
            build: std::rc::Rc::new(|_c: &ChromeCtx, t: &ContextTarget| {
                let ContextTarget::Row { key, .. } = t else {
                    return Vec::new();
                };
                vec![DropdownItem::with_intent(
                    "docker.restart",
                    "Restart",
                    // A name-keyed action of the PLUGIN's — there is no WmAction for this, and the
                    // row is named by the key the plugin itself wrote.
                    Intent::new("plugin.docker.restart")
                        .arg("container", PropValue::Text(key.clone())),
                )]
            }),
        }];

        let mut ordered = r.ordered_providers("docker.container");
        ordered.extend(plugin);
        ordered.sort_by(|a, b| a.weight.cmp(&b.weight));
        let order: Vec<Vec<i64>> = ordered.iter().map(|p| p.weight.clone()).collect();
        assert_eq!(
            order,
            vec![vec![1, 1], vec![1, 1, 1], vec![1, 2]],
            "the plugin sits BETWEEN the built-ins, not after them"
        );

        // And its entry dispatches the plugin's own action, with the target's data.
        let target = ContextTarget::Row {
            container: "docker".into(),
            key: "container:nginx".into(),
        };
        let entry = (ordered[1].build)(&test_ctx(), &target).remove(0);
        assert_eq!(entry.id, "docker.restart");
        assert_eq!(entry.intent.action, "plugin.docker.restart");
        assert_eq!(
            entry.intent.args.get("container"),
            Some(&PropValue::Text("container:nginx".into())),
        );
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

    // The three row menus moved into the component that owns those rows (F003/P086/T365); these
    // tests assert on their content, so they reach for them there.
    use crate::providers::workspaces::{column_row_items, pane_row_items, workspace_row_items};

    /// **The host names a row and asks its component what it is** (F003/P086/T365). It produces the
    /// container + the key the row declared, and takes the path back — knowing neither that panes
    /// exist nor that this component has three kinds of row.
    #[test]
    fn a_focused_container_targets_its_cursor_row() {
        let (path, target) = resolve(Some("dock.left"), Some("row:7"), None).unwrap();
        assert_eq!(path.0, "stub.row", "the component named the menu, not the host");
        let ContextTarget::Row { container, key } = target else {
            panic!("a container's row is a Row target");
        };
        assert_eq!((container.as_str(), key.as_str()), ("dock.left", "row:7"));
    }

    /// A key the component does not recognise — a stale cursor, another placement's row — yields no
    /// menu rather than a wrong one.
    #[test]
    fn a_row_its_component_does_not_name_opens_nothing() {
        assert!(resolve(Some("dock.left"), Some("who:knows"), Some(PaneId(4))).is_none());
    }

    #[test]
    fn outside_every_container_the_focused_pane_is_the_target() {
        let (path, target) = resolve(None, None, Some(PaneId(4))).unwrap();
        assert_eq!(path.0, ContextPath::PANE);
        assert!(matches!(target, ContextTarget::Pane { pane_id: PaneId(4), hyperlink: None }));
    }

    #[test]
    fn resolve_context_none_when_no_target() {
        // A focused container with no cursor, and no container with no focused pane: nothing to
        // describe either way.
        assert!(resolve(Some("dock.left"), None, Some(PaneId(1))).is_none());
        assert!(resolve(None, None, None).is_none());
    }

    #[test]
    fn context_menus_differ_by_where_opened() {
        // The content each context produces is distinct — proving the menu adapts to where
        // it is opened (pane vs sidebar column vs sidebar workspace).
        let pane = pane_action_items(true);
        let col = column_row_items(0, 0);
        let ws = workspace_row_items(0, true);
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