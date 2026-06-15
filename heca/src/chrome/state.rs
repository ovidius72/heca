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
//! `grid-ui-chrome-plan.md` §4 / `pluggable-chrome-plugin-plan.md` Phase 2). When
//! more docks/plugins arrive, the store wraps this rather than replacing it.
//!
//! **Reactivity.** Fields are [`Signal`]s: writers call `.set(...)`, the retained
//! chrome tree reads them so changes drive a redraw. Signals are created once at
//! `AppState` construction and live for the app's lifetime.
//!
//! Note: chrome "selection" here is the **active/hovered pane** for the chrome — it
//! is distinct from the terminal text `SelectionState` (`app/selection_model.rs`).

// Foundation commit: the store is defined + constructed but not read yet; consumers
// (region vis/width, collapse, selection, targeting, scroll) migrate onto it next.
// Remove this once the first consumers land.
#![allow(dead_code)]

use std::collections::HashSet;

use heca_core::layout::PaneId;
use heca_grid_ui::reactive::{signal, Signal, SignalGet, SignalUpdate, SignalWith};
use heca_grid_ui::widgets::RegionMode;

/// A chrome region's display mode + size (vertical sidebars / horizontal bars).
#[derive(Clone, Copy, Debug)]
pub struct RegionState {
    /// Expanded / collapsed-rail / hidden.
    pub mode: Signal<RegionMode>,
    /// Expanded extent in logical px (width for sidebars, height for bars).
    pub size: Signal<f32>,
}

impl RegionState {
    fn new(mode: RegionMode, size: f32) -> Self {
        Self { mode: signal(mode), size: signal(size) }
    }
}

/// The chrome's active/hovered **pane** selection (NOT terminal text selection).
#[derive(Clone, Copy, Debug)]
pub struct ChromeSelection {
    /// The pane the chrome shows as active (mirrors WM focus for display).
    pub active_pane: Signal<Option<PaneId>>,
    /// The pane the cursor is hovering in the chrome.
    pub hovered_pane: Signal<Option<PaneId>>,
}

impl ChromeSelection {
    fn new() -> Self {
        Self { active_pane: signal(None), hovered_pane: signal(None) }
    }
}

/// Shared, signal-backed chrome UI state. One per app, held in `AppState`.
#[derive(Clone)]
pub struct SharedChromeState {
    /// Left sidebar region (mode + width).
    pub left: RegionState,
    /// Right sidebar region (mode + width).
    pub right: RegionState,
    /// Active/hovered pane in the chrome.
    pub selection: ChromeSelection,
    /// Workspaces the user has collapsed in the chrome (by ws index).
    pub collapsed_ws: Signal<HashSet<usize>>,
    /// Targeting pick candidates (letter → pane) for move/swap/take overlays,
    /// driving the universal `KeyHint`s. `None` when no pick is active.
    pub pick_candidates: Signal<Option<Vec<(char, PaneId)>>>,
    /// Sidebar scroll offset (logical px).
    pub scroll: Signal<f32>,
}

impl SharedChromeState {
    /// Construct the store with initial values (signals created here — requires the
    /// reactive runtime, available on the UI thread at `AppState` construction).
    pub fn new(left_width: f32, right_width: f32) -> Self {
        Self {
            left: RegionState::new(RegionMode::Expanded, left_width),
            right: RegionState::new(RegionMode::Hidden, right_width),
            selection: ChromeSelection::new(),
            collapsed_ws: signal(HashSet::new()),
            pick_candidates: signal(None),
            scroll: signal(0.0),
        }
    }

    // ── Convenience read/write helpers (write-via-actions call these) ──────────

    /// Whether a region is visible (not `Hidden`).
    pub fn left_visible(&self) -> bool {
        !matches!(self.left.mode.get(), RegionMode::Hidden)
    }

    /// Is workspace `ws_idx` collapsed in the chrome?
    pub fn is_ws_collapsed(&self, ws_idx: usize) -> bool {
        self.collapsed_ws.with(|s| s.contains(&ws_idx))
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

    #[test]
    fn toggle_ws_collapsed_flips() {
        let s = SharedChromeState::new(280.0, 280.0);
        assert!(!s.is_ws_collapsed(2));
        s.toggle_ws_collapsed(2);
        assert!(s.is_ws_collapsed(2));
        s.toggle_ws_collapsed(2);
        assert!(!s.is_ws_collapsed(2));
    }

    #[test]
    fn region_defaults() {
        let s = SharedChromeState::new(300.0, 260.0);
        assert!(s.left_visible());
        assert_eq!(s.left.size.get(), 300.0);
        assert!(!matches!(s.right.mode.get(), RegionMode::Expanded));
    }
}
