// The contribution model is a **foundation seam** (plugin-02): the types are the
// vocabulary providers and (later) the WASM bridge speak, so several fields and
// placeholder variants read as dead code until the render/provider phases
// (plugin-03+) consume them. Mirrors `host.rs` / `events.rs`, which carry the
// same allow for the same reason.
#![allow(dead_code)]

//! The **contribution model** — the semantic units a provider hands the
//! [`ChromeHost`](super::host::ChromeHost), per contract §3.1.1/§3.2. A
//! contribution is never raw pixels; it is one of five typed units. For the
//! sidebars the important one is the **mounted container**; the bars take
//! toolbar groups and status segments; overlay requests are region-agnostic and
//! forwarded to the (future) overlay host.
//!
//! All geometry here uses the canonical `heca-core::layout` types
//! (`Rectangle`/`Point`/`Size`) per §5.7 — never the (already-removed) legacy
//! `Rect`.

use heca_core::layout::Rectangle;

use super::RegionId;
use crate::providers::ChromeCtx;

/// Stable string identity of a mounted container (equals its provider's `id()`).
pub type ContainerId = String;

/// A built container body — a host-understood `heca-grid-ui` widget subtree. The
/// host owns render/focus/clip/overlays (§2.6); the provider only *builds* this
/// model on (re)mount. Not exercised in plugin-02 (no render path yet).
pub type WidgetModel = Box<dyn heca_grid_ui::Component>;

/// A `Copy` set of [`RegionId`]s (bitmask over the four regions) — a container's
/// `supported_regions`.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct RegionSet(u8);

impl RegionSet {
    /// The empty set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// A set from a slice of regions.
    pub fn of(regions: &[RegionId]) -> Self {
        let mut mask = 0u8;
        for region in regions {
            mask |= 1 << region.index();
        }
        Self(mask)
    }

    /// Builder: add a region.
    pub fn with(mut self, region: RegionId) -> Self {
        self.0 |= 1 << region.index();
        self
    }

    /// Both vertical sidebars — the common movable-container case.
    pub fn sidebars() -> Self {
        Self::of(&[RegionId::LeftSidebar, RegionId::RightSidebar])
    }

    /// Is `region` in the set?
    pub fn contains(self, region: RegionId) -> bool {
        self.0 & (1 << region.index()) != 0
    }

    /// Is the set empty?
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Debug for RegionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set()
            .entries(RegionId::ALL.iter().filter(|r| self.contains(**r)))
            .finish()
    }
}

/// One semantic unit a provider contributes to a region (§3.2). The per-region
/// allow-list (§3.1.1) is enforced by the host, not the type system: sidebars
/// take `Container`/`Panel`; bars take `ToolbarGroup`/`StatusSegment`;
/// `OverlayRequest` is region-agnostic.
pub enum Contribution {
    /// A mounted, movable domain container (sidebars). The primary unit.
    Container(ContainerContribution),
    /// A clustered group of action buttons (bars). Placeholder until plugin-07.
    ToolbarGroup(ToolbarGroup),
    /// A text/badge status segment (bars). Placeholder until plugin-07.
    StatusSegment(StatusSegment),
    /// A fixed, non-movable panel (sidebars). Placeholder until a consumer exists.
    Panel(PanelContribution),
    /// A request to open an overlay — forwarded to the overlay host (§2.7.1).
    OverlayRequest(OverlaySpec),
}

/// A mounted container contribution: all host-level placement metadata plus the
/// build hook that produces its body.
pub struct ContainerContribution {
    pub id: ContainerId,
    pub title: String,
    /// Which regions this container may live in.
    pub supported_regions: RegionSet,
    /// Region it mounts in on first run.
    pub default_region: RegionId,
    /// Stacking order within a region (lower = earlier).
    pub default_order: i32,
    /// Host-level move/reorder allowed?
    pub movable: bool,
    /// Collapsible within its region shell?
    pub collapsible: bool,
    /// Builds the container body. Called by the region host on (re)mount /
    /// invalidation — **not** in plugin-02 (no render path yet).
    pub build: Box<dyn Fn(&ChromeCtx) -> WidgetModel>,
}

/// Placeholder: a clustered action-button group for a bar region. Fleshed out
/// when bar providers land.
#[derive(Clone, Debug, Default)]
pub struct ToolbarGroup {
    pub id: String,
    pub items: Vec<String>,
}

/// Placeholder: a text/badge status segment for a bar region.
#[derive(Clone, Debug, Default)]
pub struct StatusSegment {
    pub id: String,
    pub text: String,
}

/// Placeholder: a fixed, non-movable panel for a sidebar region.
#[derive(Clone, Debug, Default)]
pub struct PanelContribution {
    pub id: String,
    pub title: String,
}

/// Placeholder overlay request. The real `ModalSpec`/`DropdownSpec` + async
/// result contract land with the `OverlayHost` (contract §2.7.1, Phase 8); this
/// stub shows the geometry rule (§5.7) — anchors are `heca-core` `Rectangle`.
#[derive(Clone, Debug)]
pub enum OverlaySpec {
    Modal { title: String, message: String },
    Dropdown { anchor: Rectangle },
}
