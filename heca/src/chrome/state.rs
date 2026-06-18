//! Shared chrome/UI state, split along the **shell vs container** boundary
//! (`read-via-signals / write-via-actions`).
//!
//! The sidebar is a content-agnostic **shell** hosting movable **containers**
//! (WorkspacesContainer today; Docker/agents/git/… later — see
//! `pluggable-chrome-plugin-plan.md` §2.1, `grid-ui-chrome-plan.md` §4). So state
//! splits accordingly — see memory `chrome-is-container-namespaced-everywhere`:
//!
//! - [`SharedChromeState`] = **container-agnostic shell state**: region
//!   visibility/mode/width (and, later, container placement/order + overlay stack +
//!   shell dock-list scroll). NOTHING workspace/pane-specific lives here.
//! - [`WorkspacesContainerState`] = the **WorkspacesContainer's own** namespaced
//!   state: per-workspace collapse, active/hovered pane, targeting candidates, and
//!   the container's **content scroll**. Workspace/pane specifics are fine *here* —
//!   that's this container's job. Held by `SharedChromeState.workspaces`; when a
//!   second container exists this generalizes to a container-id-keyed registry, and
//!   the state travels with the container when it moves between regions.
//!
//! Canonical *session* state stays in `Session`; this is derived UI state + ids.
//!
//! ## Access contract
//! - **Write** only through `set_*` / `toggle_*` methods. Signal fields are
//!   `pub(crate)` (the chrome-build code binds them to widgets), never `pub`.
//! - **Read** via the `&self` accessors; collection signals use `.with(...)`
//!   (borrow), never `.get()` (which clones the whole collection).
//!
//! ## Threading / Clone
//! `Rc`/thread-local-runtime backed → **`!Send + !Sync`** (UI thread only). `Clone`
//! is a **shallow alias** (same backing signals), which is why these types are not
//! `Copy`.

// Foundation phase: selection/targeting/scroll aren't read yet (collapse + regions
// are). `expect` self-cleans once all are consumed. Remove then.
use std::collections::HashSet;

use heca_core::layout::PaneId;
use heca_grid_ui::reactive::{signal, Signal, SignalGet, SignalUpdate, SignalWith};
use heca_grid_ui::widgets::RegionMode;

use super::{ChromeEvent, ChromeEventBus, ChromeRegion};

/// A chrome region's display mode + size (vertical sidebars / horizontal bars).
/// Container-agnostic shell state. Not `Copy` (aliasing signal handles).
#[derive(Clone, Debug)]
pub(crate) struct RegionState {
    pub(crate) mode: Signal<RegionMode>,
    pub(crate) size: Signal<f32>,
}

impl RegionState {
    fn new(mode: RegionMode, size: f32) -> Self {
        Self { mode: signal(mode), size: signal(size) }
    }
}

/// A container's active/hovered **pane** selection (NOT terminal text selection).
#[derive(Clone, Debug)]
pub(crate) struct ChromeSelection {
    pub(crate) active_pane: Signal<Option<PaneId>>,
    pub(crate) hovered_pane: Signal<Option<PaneId>>,
}

impl ChromeSelection {
    fn new() -> Self {
        Self { active_pane: signal(None), hovered_pane: signal(None) }
    }
}

/// The **WorkspacesContainer's** own namespaced UI state. Workspace/pane specifics
/// belong here (this is the workspaces container), never on the shell. Region-
/// independent, so it travels with the container if it moves between regions.
#[derive(Clone, Debug)]
pub struct WorkspacesContainerState {
    events: ChromeEventBus,
    /// Workspaces collapsed in this container (by ws index). Read via `with_collapsed_ws`.
    pub(crate) collapsed_ws: Signal<HashSet<usize>>,
    /// Active/hovered pane in this container.
    pub(crate) selection: ChromeSelection,
    /// Targeting pick candidates (letter → pane) for move/swap/take overlays, driving
    /// the universal `KeyHint`s. **Empty = no pick active.** Read via `with_pick_candidates`.
    pub(crate) pick_candidates: Signal<Vec<(char, PaneId)>>,
    /// This container's **content** scroll offset (logical px) — scrolls when the
    /// container has too many items. (The shell's dock-list scroll is separate.)
    #[allow(dead_code)]
    pub(crate) scroll: Signal<f32>,
}

impl WorkspacesContainerState {
    fn new(events: ChromeEventBus) -> Self {
        Self {
            events,
            collapsed_ws: signal(HashSet::new()),
            selection: ChromeSelection::new(),
            pick_candidates: signal(Vec::new()),
            scroll: signal(0.0),
        }
    }

    // ── Reads ──
    pub fn active_pane(&self) -> Option<PaneId> { self.selection.active_pane.get() }
    pub fn hovered_pane(&self) -> Option<PaneId> { self.selection.hovered_pane.get() }
    #[allow(dead_code)]
    pub fn scroll(&self) -> f32 { self.scroll.get() }

    /// Is workspace `ws_idx` collapsed? (Borrows — no clone.)
    pub fn is_ws_collapsed(&self, ws_idx: usize) -> bool {
        self.collapsed_ws.with(|s| s.contains(&ws_idx))
    }
    /// Borrow the collapsed-ws set without cloning it.
    pub fn with_collapsed_ws<R>(&self, f: impl FnOnce(&HashSet<usize>) -> R) -> R {
        self.collapsed_ws.with(f)
    }
    /// Is a targeting pick active? (Borrows — no clone.)
    #[allow(dead_code)]
    pub fn pick_active(&self) -> bool {
        self.pick_candidates.with(|c| !c.is_empty())
    }
    /// Borrow the pick candidates without cloning the Vec.
    pub fn with_pick_candidates<R>(&self, f: impl FnOnce(&[(char, PaneId)]) -> R) -> R {
        self.pick_candidates.with(|c| f(c))
    }

    // ── Writes ──
    pub fn set_active_pane(&self, pane: Option<PaneId>) {
        if self.selection.active_pane.get_untracked() == pane {
            return;
        }
        self.selection.active_pane.set(pane);
        self.events.emit(ChromeEvent::PaneActiveChanged { pane });
    }
    pub fn set_hovered_pane(&self, pane: Option<PaneId>) {
        if self.selection.hovered_pane.get_untracked() == pane {
            return;
        }
        self.selection.hovered_pane.set(pane);
        self.events.emit(ChromeEvent::PaneHoveredChanged { pane });
    }
    #[allow(dead_code)]
    pub fn set_scroll(&self, offset: f32) {
        if (self.scroll.get_untracked() - offset).abs() <= f32::EPSILON {
            return;
        }
        self.scroll.set(offset);
        self.events.emit(ChromeEvent::WorkspacesScrollChanged { offset });
    }
    pub fn set_pick_candidates(&self, candidates: Vec<(char, PaneId)>) {
        if self.pick_candidates.get_untracked() == candidates {
            return;
        }
        self.pick_candidates.set(candidates);
        self.events.emit(ChromeEvent::PanePickCandidatesChanged {
            candidates: self.pick_candidates.get_untracked(),
        });
    }
    pub fn clear_pick_candidates(&self) {
        if self.pick_candidates.get_untracked().is_empty() {
            return;
        }
        self.pick_candidates.update(|c| c.clear());
        self.events.emit(ChromeEvent::PanePickCandidatesChanged {
            candidates: Vec::new(),
        });
    }
    /// Set a workspace's collapsed state explicitly.
    pub fn set_ws_collapsed(&self, ws_idx: usize, collapsed: bool) {
        let changed = self.is_ws_collapsed(ws_idx) != collapsed;
        if !changed {
            return;
        }
        self.collapsed_ws.update(|s| {
            if collapsed {
                s.insert(ws_idx);
            } else {
                s.remove(&ws_idx);
            }
        });
        self.events.emit(ChromeEvent::WorkspaceCollapsedChanged { ws_idx, collapsed });
    }
    /// Toggle a workspace's collapsed state.
    pub fn toggle_ws_collapsed(&self, ws_idx: usize) {
        let mut collapsed = false;
        self.collapsed_ws.update(|s| {
            if !s.insert(ws_idx) {
                s.remove(&ws_idx);
                collapsed = false;
            } else {
                collapsed = true;
            }
        });
        self.events.emit(ChromeEvent::WorkspaceCollapsedChanged { ws_idx, collapsed });
    }
}

/// Container-agnostic **shell** chrome state + the mounted containers' states.
/// One per app, held in `AppState`. See module docs (shell vs container split).
#[derive(Clone, Debug)]
pub struct SharedChromeState {
    events: ChromeEventBus,
    pub(crate) left: RegionState,
    pub(crate) right: RegionState,
    /// The (currently sole) mounted container's state. Becomes a container-id-keyed
    /// registry when a second container (Docker/agents/…) is added.
    pub workspaces: WorkspacesContainerState,
}

impl SharedChromeState {
    /// Construct the store with initial region modes + widths (mirroring the
    /// `SidebarState` defaults during migration). Signals are created here — requires
    /// the reactive runtime, available on the UI thread at `AppState` construction.
    pub fn new(left_width: f32, left_visible: bool, right_width: f32, right_visible: bool) -> Self {
        let mode = |visible: bool| if visible { RegionMode::Expanded } else { RegionMode::Hidden };
        let events = ChromeEventBus::default();
        Self {
            events: events.clone(),
            left: RegionState::new(mode(left_visible), left_width),
            right: RegionState::new(mode(right_visible), right_width),
            workspaces: WorkspacesContainerState::new(events),
        }
    }

    #[allow(dead_code)]
    pub fn events(&self) -> ChromeEventBus { self.events.clone() }

    // ── Region (shell) reads/writes — RegionMode/f32 are Copy → `.get()` is cheap ──
    #[expect(dead_code, reason = "region mode accessors are part of the shell state API; only visibility is consumed today")]
    pub fn left_mode(&self) -> RegionMode { self.left.mode.get() }
    pub fn left_size(&self) -> f32 { self.left.size.get() }
    pub fn left_visible(&self) -> bool { !matches!(self.left.mode.get(), RegionMode::Hidden) }
    #[expect(dead_code, reason = "region mode accessors are part of the shell state API; only visibility is consumed today")]
    pub fn right_mode(&self) -> RegionMode { self.right.mode.get() }
    pub fn right_size(&self) -> f32 { self.right.size.get() }
    pub fn right_visible(&self) -> bool { !matches!(self.right.mode.get(), RegionMode::Hidden) }

    pub fn set_left_mode(&self, mode: RegionMode) {
        if self.left.mode.get_untracked() == mode {
            return;
        }
        self.left.mode.set(mode);
        self.events.emit(ChromeEvent::RegionModeChanged {
            region: ChromeRegion::Left,
            mode,
        });
    }
    pub fn set_left_size(&self, size: f32) {
        if (self.left.size.get_untracked() - size).abs() <= f32::EPSILON {
            return;
        }
        self.left.size.set(size);
        self.events.emit(ChromeEvent::RegionSizeChanged {
            region: ChromeRegion::Left,
            size,
        });
    }
    pub fn set_right_mode(&self, mode: RegionMode) {
        if self.right.mode.get_untracked() == mode {
            return;
        }
        self.right.mode.set(mode);
        self.events.emit(ChromeEvent::RegionModeChanged {
            region: ChromeRegion::Right,
            mode,
        });
    }
    #[expect(dead_code, reason = "right-region resizing is not yet wired through the current shell interactions")]
    pub fn set_right_size(&self, size: f32) {
        if (self.right.size.get_untracked() - size).abs() <= f32::EPSILON {
            return;
        }
        self.right.size.set(size);
        self.events.emit(ChromeEvent::RegionSizeChanged {
            region: ChromeRegion::Right,
            size,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

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
        assert!(!s.workspaces.is_ws_collapsed(2));
        s.workspaces.toggle_ws_collapsed(2);
        assert!(s.workspaces.is_ws_collapsed(2));
        s.workspaces.toggle_ws_collapsed(2);
        assert!(!s.workspaces.is_ws_collapsed(2));
    }

    #[test]
    fn set_ws_collapsed_is_idempotent() {
        let s = state();
        s.workspaces.set_ws_collapsed(1, true);
        s.workspaces.set_ws_collapsed(1, true);
        assert!(s.workspaces.is_ws_collapsed(1));
        s.workspaces.set_ws_collapsed(1, false);
        assert!(!s.workspaces.is_ws_collapsed(1));
    }

    #[test]
    fn pick_candidates_active_and_clear() {
        let s = state();
        assert!(!s.workspaces.pick_active());
        s.workspaces.set_pick_candidates(vec![('a', PaneId(1)), ('b', PaneId(2))]);
        assert!(s.workspaces.pick_active());
        assert_eq!(s.workspaces.with_pick_candidates(|c| c.len()), 2);
        s.workspaces.clear_pick_candidates();
        assert!(!s.workspaces.pick_active());
    }

    #[test]
    fn selection_defaults_none_then_set() {
        let s = state();
        assert_eq!(s.workspaces.active_pane(), None);
        s.workspaces.set_active_pane(Some(PaneId(7)));
        assert_eq!(s.workspaces.active_pane(), Some(PaneId(7)));
    }

    #[test]
    fn scroll_round_trips() {
        let s = state();
        assert_eq!(s.workspaces.scroll(), 0.0);
        s.workspaces.set_scroll(42.5);
        assert_eq!(s.workspaces.scroll(), 42.5);
    }

    #[test]
    fn clone_is_a_shallow_alias_not_a_snapshot() {
        let a = state();
        let b = a.clone();
        a.workspaces.set_scroll(99.0);
        assert_eq!(b.workspaces.scroll(), 99.0, "clone must alias the same signal store");
        b.workspaces.toggle_ws_collapsed(3);
        assert!(a.workspaces.is_ws_collapsed(3), "collapse via clone must be seen by original");
    }

    #[test]
    fn setters_emit_only_on_real_change() {
        let s = state();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_events = seen.clone();
        let _sub = s.events().subscribe("*", move |event| {
            seen_events.borrow_mut().push(event.name().to_string());
        });

        s.workspaces.set_active_pane(Some(PaneId(7)));
        s.workspaces.set_active_pane(Some(PaneId(7)));
        s.set_left_mode(RegionMode::CollapsedRail);
        s.set_left_mode(RegionMode::CollapsedRail);
        s.workspaces.set_pick_candidates(vec![('a', PaneId(7))]);
        s.workspaces.set_pick_candidates(vec![('a', PaneId(7))]);
        s.workspaces.clear_pick_candidates();
        s.workspaces.clear_pick_candidates();

        assert_eq!(
            seen.borrow().as_slice(),
            [
                "pane.active.changed",
                "chrome.region.mode.changed",
                "pane.pick.changed",
                "pane.pick.changed",
            ],
        );
    }
}
