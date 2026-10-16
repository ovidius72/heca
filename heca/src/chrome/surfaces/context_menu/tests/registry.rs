//! Who contributes to a path, and the one sorted list their entries merge into.

use super::*;
use crate::chrome::surfaces::context_menu::registry::*;
use crate::chrome::surfaces::context_menu::*;
use crate::chrome::{DropdownItem, Intent};
use heca_grid_ui::order::Order;

/// **The host seeds the content pane and nothing else**. A container's rows
/// are described by the component that owns them: the three row menus arrive at open time
/// through `Provider::context_menus`, merged by `plugin_providers_for`, exactly as a plugin's
/// would. The host knowing three workspace-shaped paths was the last place it enumerated
/// another component's row kinds.
#[test]
fn the_registry_seeds_only_the_content_pane() {
    let r = ContextMenuRegistry::with_builtins();
    assert!(!r.ordered_providers(ContextPath::PANE).is_empty());
    assert_eq!(r.ordered_providers(ContextPath::PANE).len(), 1);
    for owned_by_the_component in [
        crate::providers::workspaces::MENU_PANE,
        crate::providers::workspaces::MENU_COLUMN,
        crate::providers::workspaces::MENU_WORKSPACE,
    ] {
        assert!(
            r.ordered_providers(owned_by_the_component).is_empty(),
            "{owned_by_the_component} is the component's to declare, not the host's to seed",
        );
    }
}

#[test]
fn ordered_providers_sorts_by_order() {
    // A plugin inserts between two built-ins with a list order.
    let mut r = ContextMenuRegistry::with_builtins();
    // Register a second provider for PANE with a higher order — it sorts after the built-in.
    r.register(
        ContextPath::PANE,
        2,
        std::rc::Rc::new(|_ctx: &ChromeCtx, _target: &ContextTarget| {
            vec![DropdownItem::new("plugin_extra", "Plugin")]
        }),
    );
    let ordered = r.ordered_providers(ContextPath::PANE);
    assert_eq!(ordered.len(), 2);
    assert_eq!(ordered[0].order, Order::from(0), "built-in (order 0) first");
    assert_eq!(ordered[1].order, Order::from(2), "plugin (order 2) second");
}

/// **An entry can sit somewhere other than where its block sits.** ⚠️ Ran red first.
///
/// Block order is the right granularity most of the time — a plugin thinks in "my entries" —
/// but a block can only move whole, so a plugin with one entry belonging at the very top and
/// the rest at the bottom was stuck. An entry that declares its own order is placed by it;
/// one that declares nothing takes its block's, so this changes nothing for anybody who says
/// nothing.
#[test]
fn an_entry_can_outrank_its_own_block() {
    let mut r = ContextMenuRegistry::with_builtins();
    r.register(
        ContextPath::PANE,
        // A block that sorts LAST — and yet one of its entries belongs first.
        9,
        std::rc::Rc::new(|_ctx: &ChromeCtx, _target: &ContextTarget| {
            vec![
                DropdownItem::new("myplugin.pin", "Pin this").order(-1),
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
        "the entry that named an order is placed by it, above every built-in: {ids:?}",
    );
    assert_eq!(
        ids.last(),
        Some(&"myplugin.rest"),
        "and its silent sibling stays where its block sits, at the end: {ids:?}",
    );
}

/// **Saying nothing changes nothing.** The whole ordering is one sorted list now, so this pins
/// that a block with no per-entry orders comes out exactly as its provider wrote it — the
/// sort must be stable, and every built-in menu depends on it.
#[test]
fn entries_that_name_no_order_keep_the_order_their_block_wrote_them_in() {
    let mut r = ContextMenuRegistry::with_builtins();
    r.register(
        ContextPath::PANE,
        9,
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

/// **A provider's entries merge **between** the built-ins by order (C2),
/// not appended after them** — and the entry carries the provider's **own** action id, which no
/// `WmAction` variant exists for.
#[test]
fn a_plugin_entry_merges_between_the_builtins_and_runs_its_own_action() {
    let mut r = ContextMenuRegistry::default();
    // Two "built-ins" at [1,1] and [1,2] …
    r.register(
        "docker.container",
        [1, 1],
        std::rc::Rc::new(|_c: &ChromeCtx, _t: &ContextTarget| {
            vec![DropdownItem::new("first", "First")]
        }),
    );
    r.register(
        "docker.container",
        [1, 2],
        std::rc::Rc::new(|_c: &ChromeCtx, _t: &ContextTarget| {
            vec![DropdownItem::new("last", "Last")]
        }),
    );
    // … and the plugin slots in at [1,1,1].
    let plugin = vec![ContextMenuProvider {
        order: Order::from([1, 1, 1]),
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
    ordered.sort_by_key(|p| p.order);
    let order: Vec<Order> = ordered.iter().map(|p| p.order).collect();
    assert_eq!(
        order,
        vec![
            Order::from([1, 1]),
            Order::from([1, 1, 1]),
            Order::from([1, 2])
        ],
        "the plugin sits BETWEEN the built-ins, not after them"
    );

    // And its entry dispatches the plugin's own action, with the target's data.
    let entry = (ordered[1].build)(&test_ctx(), &ContextTarget::Contribution).remove(0);
    assert_eq!(entry.id, "docker.restart");
    assert_eq!(entry.intent.action, "plugin.docker.restart");
    assert!(
        crate::input::resolve_action(&entry.intent.action, &std::collections::HashMap::new())
            .is_none(),
        "the plugin's action has no WmAction variant — which is exactly why an \
         Intent (a name) is what a menu entry carries"
    );
}

#[test]
fn unknown_path_yields_no_providers() {
    let r = ContextMenuRegistry::with_builtins();
    assert!(r.ordered_providers("no.such.path").is_empty());
}
