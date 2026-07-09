//! The first built-in provider — [`WorkspacesContainerProvider`] — the
//! workspace-tree sidebar container. plugin-03 `t004` lands the skeleton +
//! `ChromeHost` registration; the real render seam (`build_contribution`
//! producing the workspace-tree `Flex`, moved from
//! `chrome::build_workspaces_container`) is filled in `t005`, and the bespoke
//! sidebar render path is cut over to read this provider in `t006`.
//!
//! Today this provider is registered in `ChromeHost` at startup but its
//! `build_contribution` is **not** consumed by the render path — the sidebar is
//! still built bespoke in `chrome::build_sidebar_shell`. Registration is
//! wiring-only: it seats the container in `LeftSidebar` at order 0 so
//! `ChromeHost::contribions(LeftSidebar)` reports it, exercising the host
//! runtime with a real first-party provider (replacing the `TestProvider`
//! stand-in) **without changing any visible behavior**.

use crate::chrome::{Contribution, ContainerContribution, RegionId, RegionSet, WidgetModel};
use crate::providers::{ChromeCtx, Provider};
use heca_grid_ui::widgets::Flex;

/// The built-in workspace-tree sidebar container.
///
/// Mounts in the left sidebar by default and is movable to the right sidebar
/// (`supported_regions = sidebars`). It is the first real [`Provider`] registered
/// in [`ChromeHost`](crate::chrome::ChromeHost) (plugin-03), proving the
/// pluggable-chrome runtime hosts a domain container — not just the
/// `TestProvider` stand-in in `host.rs` tests.
pub struct WorkspacesContainerProvider;

impl WorkspacesContainerProvider {
    /// A default instance ready to register.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WorkspacesContainerProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for WorkspacesContainerProvider {
    fn id(&self) -> &str {
        "workspaces"
    }

    fn supported_regions(&self) -> RegionSet {
        RegionSet::sidebars()
    }

    fn default_region(&self) -> RegionId {
        RegionId::LeftSidebar
    }

    fn default_order(&self) -> i32 {
        0
    }

    fn title(&self) -> &str {
        "Workspaces"
    }

    fn movable(&self) -> bool {
        true
    }

    fn collapsible(&self) -> bool {
        true
    }

    fn build_contribution(&self, _ctx: &ChromeCtx) -> Contribution {
        // plugin-03 t004 (skeleton): placeholder body. The real workspace-tree
        // `Flex` (moved from `chrome::build_workspaces_container`) lands in t005;
        // until then the render path does not consume this (the sidebar is still
        // built bespoke in `build_sidebar_shell`). Returning a minimal
        // `ContainerContribution` with an empty-`Flex` build closure keeps the
        // contribution valid if the host ever calls `build` early.
        Contribution::Container(ContainerContribution {
            id: self.id().to_string(),
            title: self.title().to_string(),
            supported_regions: self.supported_regions(),
            default_region: self.default_region(),
            default_order: self.default_order(),
            movable: self.movable(),
            collapsible: self.collapsible(),
            build: Box::new(|_ctx| Box::new(Flex::column()) as WidgetModel),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::{ChromeEventBus, ChromeHost};

    #[test]
    fn provider_metadata_is_self_consistent() {
        let p = WorkspacesContainerProvider::new();
        assert_eq!(p.id(), "workspaces");
        assert_eq!(p.title(), "Workspaces");
        assert_eq!(p.default_region(), RegionId::LeftSidebar);
        assert!(p.supported_regions().contains(RegionId::LeftSidebar));
        assert!(p.supported_regions().contains(RegionId::RightSidebar));
        assert_eq!(p.default_order(), 0);
        assert!(p.movable());
        assert!(p.collapsible());
    }

    #[test]
    fn register_seats_in_left_sidebar_at_order_zero() {
        let mut host = ChromeHost::new(ChromeEventBus::default());
        host.register(Box::new(WorkspacesContainerProvider::new()));
        // Seated in the left sidebar (its default_region), not the right.
        let left = host.contributions(RegionId::LeftSidebar);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id(), "workspaces");
        assert_eq!(left[0].title(), "Workspaces");
        assert!(host.contributions(RegionId::RightSidebar).is_empty());
        assert_eq!(host.placement("workspaces"), Some(RegionId::LeftSidebar));
    }

    #[test]
    fn build_contribution_returns_container_with_placeholder_body() {
        // The contribution is a Container carrying the provider metadata; the
        // build closure is a placeholder (empty Flex) until t005 fills the real
        // workspace-tree widget. It must still be callable without panicking.
        let p = WorkspacesContainerProvider::new();
        let ctx = ChromeCtx::new(crate::host::App::new(&crate::chrome::SharedChromeState::new(
            300.0,
            true,
            300.0,
            false,
        )));
        let contribution = p.build_contribution(&ctx);
        match contribution {
            Contribution::Container(c) => {
                assert_eq!(c.id, "workspaces");
                assert_eq!(c.default_region, RegionId::LeftSidebar);
                assert!(c.supported_regions.contains(RegionId::LeftSidebar));
                assert!(c.supported_regions.contains(RegionId::RightSidebar));
                // Placeholder build closure is callable and yields a (zero-child) widget.
                let _widget = (c.build)(&ctx);
            }
            _ => panic!("expected Contribution::Container, got {:?}", "other"),
        }
    }
}