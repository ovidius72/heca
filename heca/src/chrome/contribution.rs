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

use super::{ChromeSignals, DragItemRegistry, HintTargetRegistry, RegionId};
use crate::providers::ChromeCtx;

/// Stable string identity of a mounted container (equals its provider's `id()`).
pub type ContainerId = String;

/// A built container body — a host-understood `heca-grid-ui` widget subtree. The
/// host owns render/focus/clip/overlays (§2.6); the provider only *builds* this
/// model on (re)mount.
pub type WidgetModel = Box<dyn heca_grid_ui::Component>;

/// The render seam itself: a container's body builder. Reads the frame's inputs from
/// [`ChromeCtx`](crate::providers::ChromeCtx) and registers its host ids in
/// [`BuildCx`]. See [`ContainerContribution::build`].
pub type BuildBody = Box<dyn Fn(&ChromeCtx<'_>, &mut BuildCx<'_>) -> WidgetModel>;

/// The **mutable half** of the render seam: the host's per-build registries, handed
/// to [`ContainerContribution::build`] alongside the read-only
/// [`ChromeCtx`](crate::providers::ChromeCtx).
///
/// A container body is not just widgets — building it *allocates host ids*: a drag
/// item id per draggable/droppable row, a hint target id per pickable row, and a
/// signal per value that changes without a structural rebuild. Those registries are
/// owned by the host (the hint registry is shared across every retained tree, per
/// `HintTargetRegistry`), so the build has to borrow them mutably.
///
/// They are passed as an explicit `&mut` parameter rather than hidden behind interior
/// mutability in `ChromeCtx`: it keeps the plugin-facing context a pure read/observe
/// facade, turns a double-borrow into a compile error instead of a runtime panic, and
/// matches how [`realize`](crate::chrome::realize) already threads the same registries.
pub struct BuildCx<'a> {
    /// Value signals the host pushes each frame without rebuilding the tree
    /// (selection, status, per-pane info).
    pub(crate) signals: &'a mut ChromeSignals,
    /// Drag sources / drop targets registered by this build.
    pub(crate) drag: &'a mut DragItemRegistry,
    /// KeyHint pick targets registered by this build (shared across all trees).
    pub(crate) hints: &'a mut HintTargetRegistry,
}

impl<'a> BuildCx<'a> {
    /// Borrow the host's per-build registries for one container build.
    pub(crate) fn new(
        signals: &'a mut ChromeSignals,
        drag: &'a mut DragItemRegistry,
        hints: &'a mut HintTargetRegistry,
    ) -> Self {
        Self {
            signals,
            drag,
            hints,
        }
    }
}

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

/// Entries a provider adds to a **context menu** (context-menu-5).
///
/// The provider declares *where* (`context_path`) and *what* (`build`); it never decides *when* —
/// the host opens the menu on right-click or `prefix+>`, resolves the context, and merges every
/// provider registered for that path in `weight` order, so a plugin's entries slot **between** the
/// built-ins rather than after them.
///
/// **Why this is not a [`Contribution`] variant.** `Provider::build_contribution` returns exactly
/// **one** `Contribution` — the provider's mounted body, which the render path projects each frame.
/// A context menu is not a mounted body, and a provider wants *both* (a Docker container in the
/// sidebar **and** a "Restart" entry on its rows). Making it a `Contribution` variant would force a
/// provider to choose one or the other. So it is its own hook,
/// [`Provider::context_menus`](crate::providers::Provider::context_menus), which returns as many as
/// the provider likes — across as many paths as it likes.
pub struct ContextMenuContribution {
    /// Where these entries appear — a dotted [`ContextPath`](crate::chrome::ContextPath):
    /// `"pane"`, `"sidebar.workspace"`, or a path the provider itself defines.
    pub context_path: String,
    /// Merge order among the providers of that path (Dewey / fractional index): `[1,1,1]` lands
    /// between built-ins weighted `[1,1]` and `[1,2]`. Sorted ascending; ties keep registration
    /// order.
    pub weight: Vec<i64>,
    /// Builds the entries for one opening of the menu. Reads the app through the
    /// [`ChromeCtx`](crate::providers::ChromeCtx) facade and *what was clicked* from the
    /// [`ContextTarget`](crate::chrome::ContextTarget) — never `AppState`. Each entry carries an
    /// `Intent`, so a provider dispatches **its own** registered actions, not just heca's.
    pub build: crate::chrome::MenuBuild,
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
    /// This container's share of its region, as a **flex grow factor** (F003/P011/T021).
    ///
    /// Fractional, never fixed: `1.0` is one share, `2.0` is twice as much as a `1.0`
    /// beside it, and `0.0` means "as big as my content" (no share of the leftover). The
    /// default is `1.0`, which gives the rule without a special case — one container takes
    /// the whole region, two take half each, `2.0` against `1.0` takes two thirds.
    ///
    /// **Share of the region's MAIN AXIS, not of its height.** `flex_grow` is main-axis
    /// relative, so this one number is the height in a sidebar (a column) and the width in
    /// a bar (a row), with nothing to add when bar regions arrive. Calling it `height_grow`
    /// would have baked "regions are vertical" into the contract.
    ///
    /// It is `flex_grow` because that is exactly what it is, and the widget library has had
    /// it all along — no new sizing language to learn or to parse.
    pub grow: f32,
    /// Builds the container body — **the render seam**. Called by the region host on
    /// (re)mount / invalidation, which for the retained chrome tree means once per
    /// structural change (`chrome_signature`), not once per frame.
    ///
    /// It reads its inputs from [`ChromeCtx`](crate::providers::ChromeCtx) (state
    /// selectors, the workspace projection, theme, intent emitter) and registers its
    /// drag/hint/signal ids in [`BuildCx`].
    pub build: BuildBody,
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
