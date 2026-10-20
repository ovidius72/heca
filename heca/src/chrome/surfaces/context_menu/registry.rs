//! **Who contributes entries to a context path, and in what order** — the built-in seed and the
//! mounted components' contributions, merged into one sorted list.

use heca_grid_ui::order::Order;

use super::pane_menu::build_pane_menu;
use super::{ContextPath, ContextTarget};
use crate::chrome::DropdownItem;
use crate::providers::ChromeCtx;

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

/// A registered context-menu provider: where its block sits (CSS `order`) and a build function.
#[derive(Clone)]
pub struct ContextMenuProvider {
    /// Where this block sits within a context path — the one [`Order`] every widget's `.order(..)`
    /// takes. Built-ins use a stable order; a plugin inserts between two of them with a list
    /// (`[1, 1, 1]` between `[1, 1]` and `[1, 2]`). Lower first; ties keep insertion order.
    pub order: Order,
    pub build: MenuBuild,
}

/// Runtime registry of context-menu providers keyed by [`ContextPath`]. Seeded with the
/// built-in providers at startup; plugins add to it via `Contribution::ContextMenu`.
/// `items_for` collects every provider matching a path, sorts by order, and
/// concatenates their items — so a plugin's entries merge with the built-in's.
#[derive(Default)]
pub struct ContextMenuRegistry {
    providers: std::collections::HashMap<String, Vec<ContextMenuProvider>>,
}

impl ContextMenuRegistry {
    /// Registry seeded with the **one** built-in provider: the content pane, which is the app's own
    /// domain. A container's rows are its component's to describe, and the workspaces component
    /// registers its three menus through
    /// [`Provider::context_menus`](crate::providers::Provider::context_menus) like any plugin.
    pub fn with_builtins() -> Self {
        let mut r = Self::default();
        // Order 0, so a component's or plugin's entries (1 upwards) merge after it.
        r.register(ContextPath::PANE, 0, std::rc::Rc::new(build_pane_menu));
        r
    }

    /// Register a provider for a context path — the built-in seed and a plugin's
    /// `Contribution::ContextMenu` go through this same call.
    pub fn register(&mut self, path: &str, order: impl Into<Order>, build: MenuBuild) {
        self.providers
            .entry(path.to_string())
            .or_default()
            .push(ContextMenuProvider {
                order: order.into(),
                build,
            });
    }

    /// Collect + merge items for a context path: every matching provider — **built-in and
    /// plugin-contributed alike** — sorted by order (stable), concatenated. `vec![]` when nothing
    /// matches (an empty menu — the caller may choose not to open it).
    ///
    /// `plugin` are the [`ContextMenuContribution`](crate::chrome::ContextMenuContribution)s the
    /// mounted providers declared for this path. They are merged **by order**, not appended, so a
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

        // **One sorted list, not blocks-then-items.** Every entry has an order: the one it declared
        // for itself, or its block's when it declared none. So a plugin can place its entries as a
        // block — which is what it usually wants — and still lift a single one to the top, without
        // anyone having to understand two orderings to predict where anything lands.
        //
        // Stable, so entries that end up with equal orders keep the order their source produced
        // them in — which is what makes a block where no entry says anything come out as written.
        let mut ordered: Vec<(Order, DropdownItem)> = Vec::new();
        for p in providers {
            for item in (p.build)(ctx, target) {
                let order = item.order.as_ref().map_or(p.order, order_of);
                ordered.push((order, item));
            }
        }
        ordered.sort_by_key(|(order, _)| *order);
        ordered.into_iter().map(|(_, item)| item).collect()
    }

    /// Providers for a path, cloned + sorted by order (stable sort keeps insertion order on
    /// ties). Exposed so the merge ordering is unit-testable without an `AppState`.
    pub(crate) fn ordered_providers(&self, path: &str) -> Vec<ContextMenuProvider> {
        let Some(provs) = self.providers.get(path) else {
            return Vec::new();
        };
        let mut ordered: Vec<ContextMenuProvider> = provs.clone();
        ordered.sort_by_key(|p| p.order);
        ordered
    }
}

/// **A described entry's order, as the library compares it** — the one place the plugin kit's
/// `ViewOrder` becomes an [`Order`], so a menu and a widget can never disagree about what `[0, 5]`
/// means. An order the library cannot hold (more than four places) is `0`, like any other unreadable
/// value from a description.
fn order_of(order: &heca_view::ViewOrder) -> Order {
    Order::from_places(&order.0).unwrap_or_default()
}

/// Every context-menu contribution the **mounted** providers declare for `path`, as providers ready
/// to be merged with the built-ins by order.
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
            order: c.order,
            build: c.build,
        })
        .collect()
}
