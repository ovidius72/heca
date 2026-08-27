//! [`ChromeRegion`] — the generic **chrome region shell** that hosts Docks.
//!
//! One widget covers all four chrome regions: the left/right **sidebars**
//! (vertical) and the top/bottom **bars** (horizontal). It is a *dumb shell* —
//! an oriented, collapsible, mode-aware container that stacks
//! [`DockFrame`](super::DockFrame)s. It owns **no** workspace/tree/drag
//! semantics; those belong to the app-side Docks it hosts.
//!
//! **Mode-aware (P2 — input parity).** The region exposes a
//! [`RegionMode`] signal ([`mode_signal`](ChromeRegion::mode_signal)) the host
//! reads to drive collapse/expand. Per the chrome plan's *read-via-signals,
//! write-via-actions* model, the region reacts to the signal (sizing itself to
//! the rail when collapsed, folding out of layout when hidden); the app
//! dispatches the named toggle action that sets it, so mouse, keyboard, and RPC
//! all reach the same path. [`toggle`](ChromeRegion::toggle) is the convenience
//! intent a host binds.
//!
//! Buildable now: orientation + mode + collapse sizing + hosting Docks. The
//! **icon-rail** rendering is built on the Dock side: a [`DockFrame`](super::DockFrame)
//! given the region's [`mode_signal`](ChromeRegion::mode_signal) via
//! [`DockFrame::rail`](super::DockFrame::rail) draws icon-only while the region is
//! [`RegionMode::CollapsedRail`]. Left as documented seams: **scrolling** an
//! overflowing region needs the renderer's `PushClip`/`PopClip` (shipped with `ScrollRegion`,
//! P019(F004));
//! **Dock-level drop targets** wire onto the shipped `drag/` framework — P079(F004).

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::style::{Direction, Length};
use crate::widgets::Orientation;

/// Default expanded extent along the collapsing (cross) axis — sidebar width /
/// bar height (logical px). Configurable via [`ChromeRegion::expanded_size`].
const DEFAULT_EXPANDED: f32 = 240.0;
/// Default collapsed icon-rail extent (logical px). Configurable via
/// [`ChromeRegion::rail_size`].
const DEFAULT_RAIL: f32 = 48.0;

/// Display mode of a [`ChromeRegion`]. Read via a signal so children/hosts can
/// adapt (e.g. a Dock renders its [`DockFrame`](super::DockFrame) icon-only in
/// the rail); written by the host's toggle action (input parity, P2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum RegionMode {
    /// Full extent — Docks shown normally.
    #[default]
    Expanded,
    /// Thin icon rail — Docks collapse to icon-only via
    /// [`DockFrame::rail`](super::DockFrame::rail).
    CollapsedRail,
    /// Removed from layout entirely (`display: none`).
    Hidden,
}

/// The oriented, collapsible shell that hosts Docks across all four regions.
pub struct ChromeRegion {
    base: Base,
    orientation: Orientation,
    mode: Signal<RegionMode>,
    /// Extent along the collapsing (cross) axis when [`RegionMode::Expanded`].
    expanded_px: f32,
    /// Extent along the collapsing (cross) axis when [`RegionMode::CollapsedRail`].
    rail_px: f32,
}

#[heca_grid_ui_macros::props]
impl ChromeRegion {
    fn with(orientation: Orientation) -> Self {
        let mut base = Base::new();
        // Stack Docks along the region's long axis: a vertical sidebar stacks in
        // a column, a horizontal bar in a row.
        base.style.layout.direction = match orientation {
            Orientation::Vertical => Direction::Column,
            Orientation::Horizontal => Direction::Row,
        };
        Self {
            base,
            orientation,
            mode: signal(RegionMode::Expanded),
            expanded_px: DEFAULT_EXPANDED,
            rail_px: DEFAULT_RAIL,
        }
    }

    /// A vertical region — a left/right **sidebar**. Stacks Docks top-to-bottom;
    /// collapse shrinks its **width**.
    pub fn vertical() -> Self {
        Self::with(Orientation::Vertical)
    }

    /// A horizontal region — a top/bottom **bar**. Stacks Docks left-to-right;
    /// collapse shrinks its **height**.
    pub fn horizontal() -> Self {
        Self::with(Orientation::Horizontal)
    }

    /// Set the initial display mode.
    #[heca_grid_ui_macros::prop]
    pub fn mode(self, mode: RegionMode) -> Self {
        self.mode.set(mode);
        self
    }

    /// Bind the region to a **caller-owned** [`RegionMode`] signal instead of its
    /// internal one, so a host can drive collapse/expand from a central layout
    /// store and share the *same* signal with the region's rail-aware Docks via
    /// [`DockFrame::rail`](super::DockFrame::rail).
    #[heca_grid_ui_macros::host_only("bound to a live host signal, which static data cannot drive")]
    pub fn with_mode_signal(mut self, mode: Signal<RegionMode>) -> Self {
        self.mode = mode;
        self
    }

    /// Expanded extent along the collapsing axis (sidebar width / bar height, px).
    #[heca_grid_ui_macros::prop]
    pub fn expanded_size(mut self, px: f32) -> Self {
        self.expanded_px = px;
        self
    }

    /// Collapsed icon-rail extent along the collapsing axis (px).
    #[heca_grid_ui_macros::prop]
    pub fn rail_size(mut self, px: f32) -> Self {
        self.rail_px = px;
        self
    }

    /// Host a Dock (typically a [`DockFrame`](super::DockFrame)).
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn dock(self, c: impl Component + 'static) -> Self {
        self.child(c)
    }

    /// The display-mode signal — the binding point for host/Dock adaptation and
    /// for the app's toggle action (write here to collapse/expand the region).
    pub fn mode_signal(&self) -> Signal<RegionMode> {
        self.mode
    }

    /// Convenience intent: flip between [`RegionMode::Expanded`] and
    /// [`RegionMode::CollapsedRail`]. A host binds this to a key/RPC so collapse
    /// is reachable without the mouse (P2). [`RegionMode::Hidden`] is a distinct
    /// state set explicitly via [`mode_signal`](Self::mode_signal).
    pub fn toggle(&self) {
        let next = match self.mode.get_untracked() {
            RegionMode::CollapsedRail => RegionMode::Expanded,
            _ => RegionMode::CollapsedRail,
        };
        self.mode.set(next);
    }

    /// Apply the current mode to the region's own layout: size the collapsing
    /// axis to the rail when collapsed, fold out of layout when hidden.
    fn sync(&mut self) {
        let mode = self.mode.get_untracked();
        self.base.set_hidden(mode == RegionMode::Hidden);
        let extent = match mode {
            RegionMode::Expanded => self.expanded_px,
            // The rail extent also stands in while hidden (size is then moot).
            RegionMode::CollapsedRail | RegionMode::Hidden => self.rail_px,
        };
        // Only the collapsing (cross) axis is pinned; the long axis stretches to
        // fill the region's slot in the chrome.
        match self.orientation {
            Orientation::Vertical => self.base.style.layout.width = Length::Px(extent),
            Orientation::Horizontal => self.base.style.layout.height = Length::Px(extent),
        }
    }
}

impl Component for ChromeRegion {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Re-apply mode-driven sizing every layout pass (covers the initial
    /// `.mode(...)` and external signal changes).
    fn remeasure(&mut self) {
        self.sync();
    }
}

impl LayoutExt for ChromeRegion {}
impl StyleExt for ChromeRegion {}
impl Parent for ChromeRegion {}
