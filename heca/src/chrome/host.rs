// ChromeHost is a foundation **runtime** (plugin-02): it is exercised by tests
// today and consumed by first-party providers + the render path in plugin-03, so
// some methods read as dead code in the binary now. Same allow rationale as
// `providers/mod.rs` / `chrome/events.rs`.
#![allow(dead_code)]

//! [`ChromeHost`] — the host/runtime that owns all pluggable chrome regions
//! (contract §3.1.1). It keeps a registry of mounted containers **per
//! [`RegionId`]**, tracks their order and placement, and owns host-level
//! move/reorder between compatible regions (validated against each container's
//! `supported_regions`). Host-level container movement is distinct from
//! container-*internal* drag/drop, which stays inside the mounted container
//! (§2.9).
//!
//! **plugin-02 scope — runtime only, no render.** The app hand-paints its chrome
//! today and wires no provider yet, so `ChromeHost` here is the pure data/runtime
//! core: it seats containers from their [`Provider`] *metadata* (it never calls
//! [`Provider::build_contribution`], the render seam) and exposes move/reorder as
//! plain methods that the `WmAction` layer drives (input parity, §2.9). Rendering
//! and the first real provider land in plugin-03.

use std::collections::HashMap;

use super::contribution::ContainerId;
use super::{ChromeEvent, ChromeEventBus, RegionId};
use crate::providers::{ChromeCtx, Provider, ProviderHandles};

/// A provider seated in a region, with its activation handles once activated.
pub struct MountedContribution {
    provider: Box<dyn Provider>,
    /// `Some` once [`Provider::on_activate`] has run (plugin-03 wiring); the held
    /// subscriptions live as long as this stays mounted.
    handles: Option<ProviderHandles>,
}

impl MountedContribution {
    /// The container's stable id (== its provider id).
    pub fn id(&self) -> &str {
        self.provider.id()
    }

    /// Human title.
    pub fn title(&self) -> &str {
        self.provider.title()
    }

    /// Host-level move/reorder allowed?
    pub fn movable(&self) -> bool {
        self.provider.movable()
    }

    /// Does this container do anything with keyboard focus beyond scrolling?
    /// (`Provider::keyboard_navigable`, F003/P011/T020.)
    pub fn keyboard_navigable(&self) -> bool {
        self.provider.keyboard_navigable()
    }

    /// The underlying provider (for the render path to build its contribution).
    pub fn provider(&self) -> &dyn Provider {
        self.provider.as_ref()
    }

    /// Run the provider's activation, keeping its subscription handles alive.
    /// Called by the render/app wiring (plugin-03), not in plugin-02.
    pub fn activate(&mut self, ctx: &ChromeCtx) {
        if self.handles.is_none() {
            self.handles = Some(self.provider.on_activate(ctx));
        }
    }
}

/// One region's ordered list of mounted containers plus its host-level
/// visibility. (This is placement/ordering state — the *shell* mode/size still
/// lives on `SharedChromeState`'s `left`/`right` until top/bottom bars exist.)
pub struct RegionHost {
    contributions: Vec<MountedContribution>,
    visible: bool,
}

impl RegionHost {
    fn new() -> Self {
        Self {
            contributions: Vec::new(),
            visible: true,
        }
    }

    /// Insert `mc` at the position implied by its provider's `default_order`
    /// (stable: before the first entry with a strictly greater order).
    fn insert_ordered(&mut self, mc: MountedContribution) {
        let order = mc.provider.default_order();
        let pos = self
            .contributions
            .iter()
            .position(|m| m.provider.default_order() > order)
            .unwrap_or(self.contributions.len());
        self.contributions.insert(pos, mc);
    }
}

/// Why a host-level container move/reorder was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    /// No container is registered under that id.
    UnknownContainer,
    /// The target region is not in the container's `supported_regions`.
    UnsupportedRegion,
    /// A `reorder` named a `before` target that isn't registered in the region.
    TargetNotFound,
}

impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveError::UnknownContainer => f.write_str("no container registered under that id"),
            MoveError::UnsupportedRegion => {
                f.write_str("target region is not in the container's supported_regions")
            }
            MoveError::TargetNotFound => {
                f.write_str("reorder target container is not in the region")
            }
        }
    }
}

impl std::error::Error for MoveError {}

/// Owns all four chrome regions, container placement, and host-level moves.
pub struct ChromeHost {
    /// Indexed by [`RegionId::index`].
    regions: [RegionHost; 4],
    /// Reverse index: which region each container currently lives in.
    placement: HashMap<ContainerId, RegionId>,
    events: ChromeEventBus,
}

impl ChromeHost {
    /// A new host with four empty regions, wired to the chrome event bus (from
    /// `SharedChromeState::events()`).
    pub fn new(events: ChromeEventBus) -> Self {
        Self {
            regions: [
                RegionHost::new(),
                RegionHost::new(),
                RegionHost::new(),
                RegionHost::new(),
            ],
            placement: HashMap::new(),
            events,
        }
    }

    /// Register a provider, seating its container at its `default_region` /
    /// `default_order`. Reads only provider metadata — the render seam
    /// (`build_contribution`) is not touched here.
    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let id = provider.id().to_string();
        let region = provider.default_region();
        debug_assert!(
            provider.supported_regions().contains(region),
            "ChromeHost::register: provider '{}' default_region {:?} not in supported_regions {:?}",
            provider.id(),
            region,
            provider.supported_regions(),
        );
        let mc = MountedContribution {
            provider,
            handles: None,
        };
        self.regions[region.index()].insert_ordered(mc);
        self.placement.insert(id, region);
    }

    /// The ordered containers currently mounted in `region`.
    pub fn contributions(&self, region: RegionId) -> &[MountedContribution] {
        &self.regions[region.index()].contributions
    }

    /// Which region a container currently lives in, if registered.
    pub fn placement(&self, id: &str) -> Option<RegionId> {
        self.placement.get(id).copied()
    }

    /// Move a container to another region. Validated against the container's
    /// `supported_regions`; a no-op (still `Ok`) if it's already there. Emits
    /// [`ChromeEvent::ContainerPlacementChanged`] on a real move.
    pub fn move_container(&mut self, id: &str, to: RegionId) -> Result<(), MoveError> {
        let from = self.placement.get(id).copied().ok_or(MoveError::UnknownContainer)?;
        let idx = self.regions[from.index()]
            .contributions
            .iter()
            .position(|m| m.id() == id)
            .ok_or(MoveError::UnknownContainer)?;

        if !self.regions[from.index()].contributions[idx]
            .provider
            .supported_regions()
            .contains(to)
        {
            return Err(MoveError::UnsupportedRegion);
        }
        if from == to {
            return Ok(());
        }

        let mc = self.regions[from.index()].contributions.remove(idx);
        self.regions[to.index()].insert_ordered(mc);
        self.placement.insert(id.to_string(), to);
        self.events.emit(ChromeEvent::ContainerPlacementChanged {
            container_id: id.to_string(),
            region: to,
        });
        Ok(())
    }

    /// Reorder a container within its region, inserting it immediately before
    /// `before` (or at the end when `before` is `None`). A `before` that names a
    /// container not in the region is an error ([`MoveError::TargetNotFound`]) —
    /// "put X before Y" fails loudly rather than silently appending. Emits
    /// [`ChromeEvent::ContainerPlacementChanged`].
    pub fn reorder(&mut self, id: &str, before: Option<&str>) -> Result<(), MoveError> {
        let region = self.placement.get(id).copied().ok_or(MoveError::UnknownContainer)?;
        let list = &mut self.regions[region.index()].contributions;
        let from = list
            .iter()
            .position(|m| m.id() == id)
            .ok_or(MoveError::UnknownContainer)?;
        // Resolve the target position (in the pre-removal list) up front so a
        // missing target errors without mutating anything.
        let target = match before {
            Some(b) => Some(
                list.iter()
                    .position(|m| m.id() == b)
                    .ok_or(MoveError::TargetNotFound)?,
            ),
            None => None,
        };
        let mc = list.remove(from);
        // Removing `from` shifts every later index left by one, so adjust a target
        // that sat after it.
        let insert_at = match target {
            Some(t) if t > from => t - 1,
            Some(t) => t,
            None => list.len(),
        };
        list.insert(insert_at, mc);
        self.events.emit(ChromeEvent::ContainerPlacementChanged {
            container_id: id.to_string(),
            region,
        });
        Ok(())
    }

    /// Reorder a container within its region, inserting it immediately **after** `after`.
    ///
    /// The mirror of [`reorder`](Self::reorder). "Put X after Y" is not expressible as "put X before
    /// Z" without knowing what follows Y, which only the host knows — a caller (a plugin, a drag
    /// landing on the *trailing* edge of an item) cannot compute it. An `after` that names a
    /// container not in the region is an error ([`MoveError::TargetNotFound`]); when `after` is the
    /// last container, X lands at the end. Emits [`ChromeEvent::ContainerPlacementChanged`].
    pub fn reorder_after(&mut self, id: &str, after: &str) -> Result<(), MoveError> {
        let region = self.placement.get(id).copied().ok_or(MoveError::UnknownContainer)?;
        let list = &self.regions[region.index()].contributions;
        // The container that follows `after` is the one to insert before; none ⇒ append (`None`).
        let after_idx = list
            .iter()
            .position(|m| m.id() == after)
            .ok_or(MoveError::TargetNotFound)?;
        let before = list.get(after_idx + 1).map(|m| m.id().to_string());
        self.reorder(id, before.as_deref())
    }

    /// Every mounted provider, across every region, in placement order.
    ///
    /// The host's "who is here right now" — used to collect what mounted providers contribute
    /// outside the render path (their context-menu entries, context-menu-5).
    pub fn mounted_providers(&self) -> impl Iterator<Item = &dyn Provider> + '_ {
        self.regions
            .iter()
            .flat_map(|r| r.contributions.iter())
            .map(|m| m.provider())
    }

    /// Set a region's host-level visibility.
    ///
    /// **plugin-02 caveat:** this flag has no visual effect yet. Region *shell*
    /// mode/size still lives on `SharedChromeState`'s `left`/`right` (see
    /// [`RegionHost`]); the render path that reads this host-level flag — and
    /// reconciles it with the shell state — arrives with the chrome render
    /// migration in plugin-03.
    pub fn set_region_visible(&mut self, region: RegionId, visible: bool) {
        self.regions[region.index()].visible = visible;
    }

    /// Is a region host-level visible?
    pub fn is_region_visible(&self, region: RegionId) -> bool {
        self.regions[region.index()].visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::{Contribution, RegionSet};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A minimal stand-in for the future WorkspacesContainerProvider. Carries its
    /// own metadata; `build_contribution` is never called in plugin-02, so its
    /// build closure is unreachable.
    struct TestProvider {
        id: String,
        supported: RegionSet,
        default_region: RegionId,
        order: i32,
    }

    impl TestProvider {
        fn new(id: &str, supported: RegionSet, default_region: RegionId, order: i32) -> Self {
            Self {
                id: id.to_string(),
                supported,
                default_region,
                order,
            }
        }
    }

    impl Provider for TestProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn supported_regions(&self) -> RegionSet {
            self.supported
        }
        fn default_region(&self) -> RegionId {
            self.default_region
        }
        fn default_order(&self) -> i32 {
            self.order
        }
        fn title(&self) -> &str {
            &self.id
        }
        fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
            // These tests exercise placement/ordering/moves, which read provider
            // *metadata* only — the host never calls the render seam here. (The real
            // seam is implemented and tested on `WorkspacesContainerProvider`.)
            unimplemented!("TestProvider contributes no body; see WorkspacesContainerProvider")
        }
    }

    fn host() -> ChromeHost {
        ChromeHost::new(ChromeEventBus::default())
    }

    fn ids(host: &ChromeHost, region: RegionId) -> Vec<String> {
        host.contributions(region)
            .iter()
            .map(|m| m.id().to_string())
            .collect()
    }

    #[test]
    fn register_seats_at_default_region_in_order() {
        let mut h = host();
        // Register out of order; default_order should sort them.
        h.register(Box::new(TestProvider::new(
            "b",
            RegionSet::sidebars(),
            RegionId::LeftSidebar,
            10,
        )));
        h.register(Box::new(TestProvider::new(
            "a",
            RegionSet::sidebars(),
            RegionId::LeftSidebar,
            5,
        )));
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "b"]);
        assert_eq!(h.placement("a"), Some(RegionId::LeftSidebar));
        assert!(h.contributions(RegionId::RightSidebar).is_empty());
    }

    #[test]
    fn move_to_supported_region_succeeds_and_emits() {
        let mut h = host();
        let seen: Rc<RefCell<Vec<(String, RegionId)>>> = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let _sub = h.events.subscribe("chrome.container.placement.changed", move |e| {
            if let ChromeEvent::ContainerPlacementChanged { container_id, region } = e {
                log.borrow_mut().push((container_id.clone(), *region));
            }
        });

        h.register(Box::new(TestProvider::new(
            "ws",
            RegionSet::sidebars(),
            RegionId::LeftSidebar,
            0,
        )));
        h.move_container("ws", RegionId::RightSidebar).unwrap();

        assert!(h.contributions(RegionId::LeftSidebar).is_empty());
        assert_eq!(ids(&h, RegionId::RightSidebar), ["ws"]);
        assert_eq!(h.placement("ws"), Some(RegionId::RightSidebar));
        assert_eq!(seen.borrow().as_slice(), [("ws".to_string(), RegionId::RightSidebar)]);
    }

    #[test]
    fn move_to_unsupported_region_is_rejected() {
        let mut h = host();
        h.register(Box::new(TestProvider::new(
            "ws",
            RegionSet::of(&[RegionId::LeftSidebar]), // left only
            RegionId::LeftSidebar,
            0,
        )));
        assert_eq!(
            h.move_container("ws", RegionId::BottomBar),
            Err(MoveError::UnsupportedRegion)
        );
        // Unchanged.
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["ws"]);
    }

    #[test]
    fn move_unknown_container_errors() {
        let mut h = host();
        assert_eq!(
            h.move_container("nope", RegionId::LeftSidebar),
            Err(MoveError::UnknownContainer)
        );
    }

    #[test]
    fn reorder_moves_before_target() {
        let mut h = host();
        for (id, order) in [("a", 0), ("b", 1), ("c", 2)] {
            h.register(Box::new(TestProvider::new(
                id,
                RegionSet::sidebars(),
                RegionId::LeftSidebar,
                order,
            )));
        }
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "b", "c"]);
        // Move "c" before "a".
        h.reorder("c", Some("a")).unwrap();
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["c", "a", "b"]);
        // Move "c" to the end.
        h.reorder("c", None).unwrap();
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "b", "c"]);
        // A missing `before` target errors and leaves the order untouched.
        assert_eq!(h.reorder("a", Some("zzz")), Err(MoveError::TargetNotFound));
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "b", "c"]);
    }

    /// `reorder_after` is the mirror of `reorder(before)` — "put X after Y" resolves to "before
    /// whatever follows Y", which only the host can compute.
    #[test]
    fn reorder_moves_after_target() {
        let mut h = host();
        for (id, order) in [("a", 0), ("b", 1), ("c", 2)] {
            h.register(Box::new(TestProvider::new(
                id,
                RegionSet::sidebars(),
                RegionId::LeftSidebar,
                order,
            )));
        }
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "b", "c"]);
        // Move "a" after "b" → it lands before "c".
        h.reorder_after("a", "b").unwrap();
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["b", "a", "c"]);
        // After the LAST container ⇒ the end.
        h.reorder_after("b", "c").unwrap();
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "c", "b"]);
        // Already immediately after the target ⇒ no-op, not a shuffle.
        h.reorder_after("c", "a").unwrap();
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "c", "b"]);
        // A missing target errors and leaves the order untouched.
        assert_eq!(
            h.reorder_after("a", "zzz"),
            Err(MoveError::TargetNotFound)
        );
        assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "c", "b"]);
    }

    #[test]
    fn region_visibility_toggles() {
        let mut h = host();
        assert!(h.is_region_visible(RegionId::TopBar));
        h.set_region_visible(RegionId::TopBar, false);
        assert!(!h.is_region_visible(RegionId::TopBar));
    }
}
