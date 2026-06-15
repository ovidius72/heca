//! Shared chrome/UI state — the single, signal-backed store the chrome reads
//! (via signals) and actions write (`read-via-signals / write-via-actions`).
//!
//! This consolidates the **derived UI state** that was previously scattered across
//! `SidebarState` (region visibility/width), `SidebarTree` (per-workspace collapse),
//! `input_mode` (targeting candidates), and the ad-hoc `ChromeSinks` `Rc<Cell>`
//! click/toggle stopgap. Canonical *session* state stays in `Session`; this layer
//! holds UI state + ids only.
//!
//! **Shape — namespace-ready.** Today this is one concrete struct (the `chrome`
//! namespace of the future plugin-facing namespaced signal store — see
//! `grid-ui-chrome-plan.md` §4). When more docks/plugins arrive, the store wraps
//! this rather than replacing it.
//!
//! ## Access contract (enforced by API shape)
//! - **Write only through the `set_*` / `toggle_*` methods.** Signal fields are
//!   `pub(crate)` (so the chrome-build code in this crate can bind them to widgets),
//!   never `pub` — external code cannot mutate them, and `.set(` on a raw field is a
//!   grep-able bug. This realizes the write-via-actions contract.
//! - **Read** via the `&self` accessors. Collection signals
//!   (`collapsed_ws`, `pick_candidates`) are read with `.with(...)` (borrow) — never
//!   `.get()`, which would clone the whole collection on every read. The accessors
//!   here do that for you.
//!
//! ## Threading
//! Signals are `Rc`/thread-local-runtime backed, so `SharedChromeState` is
//! **`!Send + !Sync`** — it must only be touched on the UI thread where `AppState`
//! lives. Do not move it into `tokio::spawn` / another thread.
//!
//! ## Clone
//! `Clone` produces a **shallow alias**: the clone's signals are the *same* backing
//! stores, so writing through either handle is observed by both. It is a second
//! handle, not an independent snapshot. (For the same reason these types are **not**
//! `Copy` — an implicit `let r = state.left;` alias would be a footgun.)
//!
//! Note: chrome "selection" here is the **active/hovered pane** for the chrome — it
//! is distinct from the terminal text `SelectionState` (`app/selection_model.rs`).
//!
//! TODO(chrome-migration): `SidebarState` / `SidebarTree.collapsed` / `input_mode`
//! candidates still hold their own copies; consumers migrate onto this store
//! incrementally (see PLAN.md P0). Until then the not-yet-read API is
//! `#[expect(dead_code)]` below.

// Foundation phase: the store is defined + constructed but consumers haven't moved
// onto it yet, so much of this API is not read. `expect` (not `allow`) makes this
// self-cleaning — once everything is consumed the unfulfilled expectation errors,
// prompting removal. Remove when the last consumer migrates.
#![expect(dead_code, reason = "chrome-state consumers migrate incrementally; see PLAN.md P0")]

use std::collections::HashSet;

use heca_core::layout::PaneId;
use heca_grid_ui::reactive::{signal, Signal, SignalGet, SignalUpdate, SignalWith};
use heca_grid_ui::widgets::RegionMode;

/// A chrome region's display mode + size (vertical sidebars / horizontal bars).
///
/// Not `Copy`: the `Signal` fields are aliasing handles, so a silent copy would
/// alias the shared store. `Clone` is an explicit second handle to the same signals.
#[derive(Clone, Debug)]
pub(crate) struct RegionState {
    /// Expanded / collapsed-rail / hidden.
    pub(crate) mode: Signal<RegionMode>,
    /// Expanded extent in logical px (width for sidebars, height for bars).
    pub(crate) size: Signal<f32>,
}

impl RegionState {
    fn new(mode: RegionMode, size: f32) -> Self {
        Self { mode: signal(mode), size: signal(size) }
    }
}

/// The chrome's active/hovered **pane** selection (NOT terminal text selection).
///
/// Not `Copy` (aliasing handles) — see [`RegionState`].
#[derive(Clone, Debug)]
pub(crate) struct ChromeSelection {
    /// The pane the chrome shows as active (mirrors WM focus for display).
    pub(crate) active_pane: Signal<Option<PaneId>>,
    /// The pane the cursor is hovering in the chrome.
    pub(crate) hovered_pane: Signal<Option<PaneId>>,
}

impl ChromeSelection {
    fn new() -> Self {
        Self { active_pane: signal(None), hovered_pane: signal(None) }
    }
}

/// Shared, signal-backed chrome UI state. One per app, held in `AppState`.
///
/// See the module docs for the access contract, threading (`!Send + !Sync`), and
/// `Clone` (shallow-alias) semantics.
#[derive(Clone, Debug)]
pub struct SharedChromeState {
    pub(crate) left: RegionState,
    pub(crate) right: RegionState,
    pub(crate) selection: ChromeSelection,
    /// Workspaces collapsed in the chrome (by ws index). Read via `with_collapsed_ws`.
    pub(crate) collapsed_ws: Signal<HashSet<usize>>,
    /// Targeting pick candidates (letter → pane) for move/swap/take overlays,
    /// driving the universal `KeyHint`s. **Empty = no pick active.** Read via
    /// `with_pick_candidates` / `pick_active` (never `.get()` — clones the Vec).
    pub(crate) pick_candidates: Signal<Vec<(char, PaneId)>>,
    /// Sidebar scroll offset (logical px).
    pub(crate) scroll: Signal<f32>,
}

impl SharedChromeState {
    /// Construct the store with initial region modes + widths. Signals are created
    /// here (requires the reactive runtime — available on the UI thread at
    /// `AppState` construction). Initial modes mirror `SidebarState` defaults during
    /// the migration to avoid behavioral divergence.
    pub fn new(left_width: f32, left_visible: bool, right_width: f32, right_visible: bool) -> Self {
        let mode = |visible: bool| if visible { RegionMode::Expanded } else { RegionMode::Hidden };
        Self {
            left: RegionState::new(mode(left_visible), left_width),
            right: RegionState::new(mode(right_visible), right_width),
            selection: ChromeSelection::new(),
            collapsed_ws: signal(HashSet::new()),
            pick_candidates: signal(Vec::new()),
            scroll: signal(0.0),
        }
    }

    // ── Reads (RegionMode / Option<PaneId> / f32 are Copy → `.get()` is cheap) ──

    pub fn left_mode(&self) -> RegionMode { self.left.mode.get() }
    pub fn left_size(&self) -> f32 { self.left.size.get() }
    pub fn left_visible(&self) -> bool { !matches!(self.left.mode.get(), RegionMode::Hidden) }
    pub fn right_mode(&self) -> RegionMode { self.right.mode.get() }
    pub fn right_size(&self) -> f32 { self.right.size.get() }
    pub fn right_visible(&self) -> bool { !matches!(self.right.mode.get(), RegionMode::Hidden) }
    pub fn active_pane(&self) -> Option<PaneId> { self.selection.active_pane.get() }
    pub fn hovered_pane(&self) -> Option<PaneId> { self.selection.hovered_pane.get() }
    pub fn scroll(&self) -> f32 { self.scroll.get() }

    /// Is workspace `ws_idx` collapsed? (Borrows — no clone.)
    pub fn is_ws_collapsed(&self, ws_idx: usize) -> bool {
        self.collapsed_ws.with(|s| s.contains(&ws_idx))
    }

    /// Borrow the collapsed-ws set without cloning it.
    pub fn with_collapsed_ws<R>(&self, f: impl FnOnce(&HashSet<usize>) -> R) -> R {
        self.collapsed_ws.with(f)
    }

    /// Is a targeting pick currently active (any candidates)? (Borrows — no clone.)
    pub fn pick_active(&self) -> bool {
        self.pick_candidates.with(|c| !c.is_empty())
    }

    /// Borrow the pick candidates without cloning the Vec.
    pub fn with_pick_candidates<R>(&self, f: impl FnOnce(&[(char, PaneId)]) -> R) -> R {
        self.pick_candidates.with(|c| f(c))
    }

    // ── Writes (the only mutation path — write-via-actions) ──

    pub fn set_left_mode(&self, mode: RegionMode) { self.left.mode.set(mode); }
    pub fn set_left_size(&self, size: f32) { self.left.size.set(size); }
    pub fn set_right_mode(&self, mode: RegionMode) { self.right.mode.set(mode); }
    pub fn set_right_size(&self, size: f32) { self.right.size.set(size); }
    pub fn set_active_pane(&self, pane: Option<PaneId>) { self.selection.active_pane.set(pane); }
    pub fn set_hovered_pane(&self, pane: Option<PaneId>) { self.selection.hovered_pane.set(pane); }
    pub fn set_scroll(&self, offset: f32) { self.scroll.set(offset); }

    /// Set the targeting pick candidates (empty clears the pick).
    pub fn set_pick_candidates(&self, candidates: Vec<(char, PaneId)>) {
        self.pick_candidates.set(candidates);
    }
    /// Clear the targeting pick.
    pub fn clear_pick_candidates(&self) {
        self.pick_candidates.update(|c| c.clear());
    }

    /// Set a workspace's collapsed state explicitly.
    pub fn set_ws_collapsed(&self, ws_idx: usize, collapsed: bool) {
        self.collapsed_ws.update(|s| {
            if collapsed {
                s.insert(ws_idx);
            } else {
                s.remove(&ws_idx);
            }
        });
    }

    /// Toggle a workspace's collapsed state.
    pub fn toggle_ws_collapsed(&self, ws_idx: usize) {
        self.collapsed_ws.update(|s| {
            if !s.insert(ws_idx) {
                s.remove(&ws_idx);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> SharedChromeState {
        SharedChromeState::new(280.0, true, 260.0, false)
    }

    #[test]
    fn region_initial_modes_mirror_visibility() {
        let s = state();
        assert!(s.left_visible());
        assert!(!s.right_visible());
    }

    #[test]
    fn region_initial_sizes() {
        let s = state();
        assert_eq!(s.left_size(), 280.0);
        assert_eq!(s.right_size(), 260.0);
    }

    #[test]
    fn toggle_ws_collapsed_flips() {
        let s = state();
        assert!(!s.is_ws_collapsed(2));
        s.toggle_ws_collapsed(2);
        assert!(s.is_ws_collapsed(2));
        s.toggle_ws_collapsed(2);
        assert!(!s.is_ws_collapsed(2));
    }

    #[test]
    fn set_ws_collapsed_is_idempotent() {
        let s = state();
        s.set_ws_collapsed(1, true);
        s.set_ws_collapsed(1, true);
        assert!(s.is_ws_collapsed(1));
        s.set_ws_collapsed(1, false);
        assert!(!s.is_ws_collapsed(1));
    }

    #[test]
    fn pick_candidates_active_and_clear() {
        let s = state();
        assert!(!s.pick_active());
        s.set_pick_candidates(vec![('a', PaneId(1)), ('b', PaneId(2))]);
        assert!(s.pick_active());
        assert_eq!(s.with_pick_candidates(|c| c.len()), 2);
        s.clear_pick_candidates();
        assert!(!s.pick_active());
    }

    #[test]
    fn selection_defaults_none_then_set() {
        let s = state();
        assert_eq!(s.active_pane(), None);
        assert_eq!(s.hovered_pane(), None);
        s.set_active_pane(Some(PaneId(7)));
        assert_eq!(s.active_pane(), Some(PaneId(7)));
    }

    #[test]
    fn scroll_round_trips() {
        let s = state();
        assert_eq!(s.scroll(), 0.0);
        s.set_scroll(42.5);
        assert_eq!(s.scroll(), 42.5);
    }

    #[test]
    fn clone_is_a_shallow_alias_not_a_snapshot() {
        // Critical semantic: cloning yields a second handle to the SAME signals,
        // so a write through one handle is visible through the other.
        let a = state();
        let b = a.clone();
        a.set_scroll(99.0);
        assert_eq!(b.scroll(), 99.0, "clone must alias the same signal store");
        b.toggle_ws_collapsed(3);
        assert!(a.is_ws_collapsed(3), "collapse via clone must be seen by original");
    }
}
