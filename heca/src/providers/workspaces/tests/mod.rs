use super::*;
use super::dock_view::DockView;
use crate::chrome::BuildCx;
use crate::app_state::SidebarItemState;
use crate::chrome::{
    ChromeEventBus, ChromeHost, ChromeIntentEmitter, ChromeSignals, DragItemRegistry,
    SharedChromeState,
};
use crate::providers::workspaces::{ColumnEntry, PaneEntry, WorkspaceEntry, WorkspaceTree};
use heca_core::layout::PaneId;
use heca_grid_ui::theme::Theme as GuiTheme;
use std::rc::Rc;


mod build;
mod cursor;

/// One workspace, one column, one (active) pane — the smallest tree that still
/// exercises every level of the projection.
pub(super) fn tree() -> WorkspaceTree {
    let mut tree = WorkspaceTree::new();
    tree.workspaces.push(WorkspaceEntry {
        ws_idx: 0,
        ws_id: heca_core::layout::WorkspaceId(0),
        name: "ws1".into(),
        custom_name: None,
        collapsed: false,
        state: SidebarItemState::Active,
        columns: vec![ColumnEntry {
            col_idx: 0,
            col_id: heca_core::layout::ColumnId(0),
            collapsed: false,
            panes: vec![PaneEntry {
                pane_id: PaneId(1),
                name: "pane1".into(),
                custom_name: None,
                state: SidebarItemState::Active,
            }],
        }],
        floating_panes: Vec::new(),
    });
    tree
}

pub(super) fn store() -> SharedChromeState {
    let store = SharedChromeState::new(300.0, 300.0);
    store.workspaces.set_active_pane(Some(PaneId(1)));
    store
}

pub(super) fn container(p: &WorkspacesContainerProvider, ctx: &ChromeCtx<'_>) -> ContainerContribution {
    match p.build_contribution(ctx) {
        Contribution::Container(c) => c,
        _ => panic!("the workspaces provider contributes a Container"),
    }
}

/// The same tree with a **floating** pane beside the tiled one, and a renamed workspace — the
/// two facts the row menus have to read off this component's own model.
pub(super) fn tree_with_a_floating_pane() -> WorkspaceTree {
    let mut tree = tree();
    let ws = &mut tree.workspaces[0];
    ws.name = "My WS".into();
    ws.custom_name = Some("My WS".into());
    ws.floating_panes.push(PaneEntry {
        pane_id: PaneId(9),
        name: "float".into(),
        custom_name: None,
        state: SidebarItemState::None,
    });
    tree.sync_flat_items();
    tree
}

/// A store whose model is the little tree above.
pub(super) fn store_with_tree() -> SharedChromeState {
    let store = store();
    *store.workspaces.tree_mut() = tree();
    store.workspaces.tree_mut().sync_flat_items();
    store
}

// ── What the component declares ──


/// A facade over a store — what a menu builder and `context_path` receive.
pub(super) fn ctx_over(store: &SharedChromeState) -> ChromeCtx<'_> {
    ChromeCtx::new(crate::host::App::new(store))
}


#[test]
fn provider_metadata_is_self_consistent() {
    let p = WorkspacesContainerProvider::new("workspaces");
    assert_eq!(p.id(), "workspaces");
    assert_eq!(p.title(), "Workspaces");
    assert_eq!(p.default_region(), RegionId::LeftSidebar);
    assert!(p.supported_regions().contains(RegionId::LeftSidebar));
    assert!(p.supported_regions().contains(RegionId::RightSidebar));
    assert!(p.movable());
    assert!(p.collapsible());
}

#[test]
fn register_seats_in_the_left_sidebar() {
    let mut host = ChromeHost::new(ChromeEventBus::default());
    host.register(Box::new(WorkspacesContainerProvider::new("workspaces")));
    // Seated in the left sidebar (its default_region), not the right.
    let left = host.contributions(RegionId::LeftSidebar);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id(), "workspaces");
    assert_eq!(left[0].title(), "Workspaces");
    assert!(host.contributions(RegionId::RightSidebar).is_empty());
    assert_eq!(host.placement("workspaces"), Some(RegionId::LeftSidebar));
}

#[test]
fn contribution_carries_the_provider_metadata() {
    let p = WorkspacesContainerProvider::new("workspaces");
    let ctx = ChromeCtx::new(crate::host::App::new(&store()));
    let c = container(&p, &ctx);
    assert_eq!(c.id, "workspaces");
    assert_eq!(c.title, "Workspaces");
    assert_eq!(c.default_region, RegionId::LeftSidebar);
    assert!(c.supported_regions.contains(RegionId::LeftSidebar));
    assert!(c.supported_regions.contains(RegionId::RightSidebar));
    assert!(c.movable);
    assert!(c.collapsible);
}
