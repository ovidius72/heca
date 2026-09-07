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
use heca_grid_ui::widgets::Menu;

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
/// component named it — the container it is in and the `key` the row declared — and nothing
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
    /// A **contribution to a named menu** — a row another component is adding to a menu it does
    /// not own ([`Menu::name`](heca_grid_ui::widgets::Menu::name)).
    ///
    /// It carries nothing, and that is the decision, not an omission: since a menu is declared on
    /// the widget it belongs to (F004/P084/T395), the host never resolves *which row* was clicked,
    /// so it has no row payload to hand on. A contributed entry acts on **app state** — the focused
    /// pane, the selected row — rather than on the row the menu was opened for. The menu's name is
    /// passed beside this, as the `path`.
    ///
    /// It used to be `Row { container, key }`, filled by the host from a hit-test. Both fields
    /// became unreadable the moment rows started declaring their own menus, and were being passed
    /// an empty string.
    Contribution,
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

        // **One sorted list, not blocks-then-items.** Every entry has a weight: the one it declared
        // for itself, or its block's when it declared none. So a plugin can place its entries as a
        // block — which is what it usually wants — and still lift a single one to the top, without
        // anyone having to understand two orderings to predict where anything lands
        //.
        //
        // Stable, so entries that end up with equal weights keep the order their source produced
        // them in — which is what makes a block with no weights at all come out exactly as written.
        let mut weighted: Vec<(Vec<i64>, DropdownItem)> = Vec::new();
        for p in providers {
            for item in (p.build)(ctx, target) {
                let weight = item.weight.clone().unwrap_or_else(|| p.weight.clone());
                weighted.push((weight, item));
            }
        }
        weighted.sort_by(|a, b| a.0.cmp(&b.0));
        weighted.into_iter().map(|(_, item)| item).collect()
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
pub(crate) fn plugin_providers_for(
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

/// Resolve the active context for a keyboard-opened context menu (`OpenContextMenu` / `prefix+>`):
/// **the focused content pane**, and nothing else.
///
/// It used to also resolve the focused container's cursor row, by asking the component which menu
/// path described that key. That whole route is gone: a row **declares its own menu**
/// (F004/P084/T395), so the keyboard trigger finds it by walking the tree from the container's
/// cursor (`chrome::open_declared_menu_for_focus`) and never asks the host to name a menu. What is
/// left here is the fallback for a content pane, which is not a widget and cannot carry a
/// declaration.
///
/// Returns `None` when no pane is focused.
pub(crate) fn resolve_active_context(state: &AppState) -> Option<(ContextPath, ContextTarget)> {
    Some((
        ContextPath(ContextPath::PANE.to_string()),
        ContextTarget::Pane {
            pane_id: crate::app::interaction::focused_pane_id(state)?,
            hyperlink: None,
        },
    ))
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

/// **Turn a list of entries into a declared [`Menu`]** — the bridge from the `DropdownItem`
/// vocabulary above to a menu a widget carries (F004/P084/T395).
///
/// This is what a component calls where it is drawing the row, so the menu's rows capture the row's
/// own ids and nothing has to resolve "what did you right-click" from a position afterwards.
///
/// Three things cross here, and each is deliberate:
///
/// - **The icon comes from the action**, via [`ChromeCtx::action_icon`], because `ActionMeta.icon`
///   is the single source of an action's `Glyph`. A component naming its own glyph would let the
///   sidebar's "Close pane" drift from the palette's `close`.
/// - **Behaviour crosses as an [`Intent`]**, wrapped in `InteractionIntent::View`, so a menu entry
///   is dispatched by name through the **central gate** — the same policy and destructive-confirm a
///   keypress or an RPC call gets. A menu row is not a back door.
/// - **`name` names the menu**, so other components may contribute rows to it
///   ([`Menu::name`](heca_grid_ui::widgets::Menu::name)). Pass `""` for a menu that is closed.
pub(crate) fn menu_from_items(
    title: &str,
    description: &str,
    name: &str,
    items: Vec<DropdownItem>,
    catalog: &crate::actions::ActionCatalog,
    emit: &crate::chrome::ChromeIntentEmitter,
) -> Menu {
    // **One conversion, two authoring paths** (F003/P097/T501). The body moved to
    // `heca_view_realize::menu_from_items` so a *described* node can declare a menu and get the
    // identical one — this is the app's half, supplying the two things only it knows: an entry's
    // icon, from the action catalog, and how a choice is dispatched.
    heca_view_realize::menu_from_items(
        title,
        description,
        name,
        items,
        &|id| catalog.icon(id),
        &|intent| {
            let emit = emit.clone();
            Box::new(move || {
                emit.fire(crate::app::interaction::InteractionIntent::View(intent.clone()))
            })
        },
    )
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

/// **Turn the identity a menu's declarer published into the target its providers read.**
///
/// A pane declares `pane:<id>` about itself already — the same declaration the keyboard cursor, the
/// right-click target and a drag all read — so a menu declared on a pane arrives here saying what
/// it is about, and nothing has to hit-test to find out.
///
/// The hyperlink is resolved here rather than by the pane, and that is the one thing this path
/// still owes to the terminal: whether the clicked cell holds a link is a question about the
/// terminal's own contents, and until the terminal is a component the only thing that can answer is
/// the host. A keyboard-opened menu passes no point and so never gets a link entry, which is right
/// — there is no cell under a keystroke.
pub(crate) fn target_from_subject(
    state: &AppState,
    subject: Option<&str>,
    at: Option<heca_core::layout::Point>,
) -> ContextTarget {
    let Some(pane_id) = subject.and_then(pane_id_from_key) else {
        return ContextTarget::Contribution;
    };
    let hyperlink = at.and_then(|p| {
        crate::app::terminal_host::hyperlink_uri_at_position(
            state,
            pane_id,
            (p.x as f32, p.y as f32),
        )
    });
    ContextTarget::Pane { pane_id, hyperlink }
}

/// `pane:<id>` → the pane. The identity a pane publishes about itself; anything else is not a pane.
fn pane_id_from_key(key: &str) -> Option<PaneId> {
    key.strip_prefix("pane:")?.parse().ok().map(PaneId)
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

    /// **An entry can sit somewhere other than where its block sits.** ⚠️ Ran red first.
    ///
    /// Block weight is the right granularity most of the time — a plugin thinks in "my entries" —
    /// but a block can only move whole, so a plugin with one entry belonging at the very top and
    /// the rest at the bottom was stuck. An entry that declares its own weight is placed by it;
    /// one that declares nothing takes its block's, so this changes nothing for anybody who says
    /// nothing.
    #[test]
    fn an_entry_can_outrank_its_own_block() {
        let mut r = ContextMenuRegistry::with_builtins();
        r.register(
            ContextPath::PANE,
            // A block that sorts LAST — and yet one of its entries belongs first.
            vec![9],
            std::rc::Rc::new(|_ctx: &ChromeCtx, _target: &ContextTarget| {
                vec![
                    DropdownItem::new("myplugin.pin", "Pin this").weight(vec![-1]),
                    DropdownItem::new("myplugin.rest", "Something else"),
                ]
            }),
        );

        let ctx = test_ctx();
        let items = r.items_for(
            &ctx,
            ContextPath::PANE,
            &ContextTarget::Pane {
                pane_id: PaneId(1),
                hyperlink: None,
            },
            vec![],
        );
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();

        assert_eq!(
            ids.first(),
            Some(&"myplugin.pin"),
            "the entry that named a weight is placed by it, above every built-in: {ids:?}",
        );
        assert_eq!(
            ids.last(),
            Some(&"myplugin.rest"),
            "and its silent sibling stays where its block sits, at the end: {ids:?}",
        );
    }

    /// **Saying nothing changes nothing.** The whole ordering is one sorted list now, so this pins
    /// that a block with no per-entry weights comes out exactly as its provider wrote it — the
    /// sort must be stable, and every built-in menu depends on it.
    #[test]
    fn entries_that_name_no_weight_keep_the_order_their_block_wrote_them_in() {
        let mut r = ContextMenuRegistry::with_builtins();
        r.register(
            ContextPath::PANE,
            vec![9],
            std::rc::Rc::new(|_ctx: &ChromeCtx, _target: &ContextTarget| {
                vec![
                    DropdownItem::new("one", "One"),
                    DropdownItem::new("two", "Two"),
                    DropdownItem::new("three", "Three"),
                ]
            }),
        );

        let ctx = test_ctx();
        let items = r.items_for(
            &ctx,
            ContextPath::PANE,
            &ContextTarget::Pane {
                pane_id: PaneId(1),
                hyperlink: None,
            },
            vec![],
        );
        let mine: Vec<&str> = items
            .iter()
            .map(|i| i.id.as_str())
            .filter(|id| matches!(*id, "one" | "two" | "three"))
            .collect();
        assert_eq!(mine, vec!["one", "two", "three"], "written order, kept");
    }

    /// **A menu says what it is about, and the host never hit-tests to find out.** ⚠️ Ran red first.
    ///
    /// A pane publishes `pane:<id>` about itself already — the same declaration the keyboard
    /// cursor, the right-click target and a drag all read — so a menu declared on a pane arrives
    /// carrying it, and this is the whole of turning that back into the thing it names. It is what
    /// let the pane's menu stop being hand-built by the mouse handler, and with it went the
    /// ordering that made right-clicking a pane never focus it.
    ///
    /// Only the reading is asked here: resolving the hyperlink needs a live terminal, and there is
    /// no headless `AppState` to give it one (see `heca/tests/by_id_actions.rs`, written as a
    /// source lint for exactly that reason).
    #[test]
    fn a_menu_declared_on_a_pane_says_which_pane_it_is_about() {
        assert_eq!(
            pane_id_from_key("pane:7"),
            Some(PaneId(7)),
            "read from the identity the pane already publishes, not searched for by position",
        );
    }

    /// **A declarer that publishes no identity, or one that is not a pane's, is a contribution** —
    /// entries that act on app state rather than on a particular thing.
    ///
    /// Guarded because the tempting fix for the test above is to require an identity, and `key` is
    /// optional everywhere in this codebase; and because a loose parse would read a sidebar row as
    /// a pane and build it the wrong menu.
    #[test]
    fn an_identity_that_is_not_a_panes_is_never_mistaken_for_one() {
        assert_eq!(pane_id_from_key("sidebar.row"), None);
        assert_eq!(
            pane_id_from_key("pane:"),
            None,
            "a prefix alone names nothing"
        );
        assert_eq!(pane_id_from_key("pane:abc"), None);
        assert_eq!(
            pane_id_from_key("workspace:7"),
            None,
            "a different kind of thing"
        );
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
                let ContextTarget::Contribution = t else {
                    return Vec::new();
                };
                vec![DropdownItem::with_intent(
                    "docker.restart",
                    "Restart",
                    // A name-keyed action of the PLUGIN's — there is no WmAction for this. It takes
                    // no row argument: a contributed row has no per-row payload, so it acts on app
                    // state (see `ContextTarget::Contribution`).
                    Intent::new("plugin.docker.restart"),
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
        let entry = (ordered[1].build)(&test_ctx(), &ContextTarget::Contribution).remove(0);
        assert_eq!(entry.id, "docker.restart");
        assert_eq!(entry.intent.action, "plugin.docker.restart");
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