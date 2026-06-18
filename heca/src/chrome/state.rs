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
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use heca_core::layout::PaneId;
use heca_core::runtime::{ContentKind, GitInfo, PaneRuntime, ProcessStatus};
use heca_grid_ui::reactive::{signal, Signal, SignalGet, SignalUpdate, SignalWith};
use heca_grid_ui::widgets::RegionMode;

use super::{ChromeEvent, ChromeEventBus, ChromeRegion};

/// Per-pane reactive mirror of canonical [`PaneRuntime`] fields.
///
/// Each field is a separate [`Signal`] so a change marks only that field's
/// subscribers dirty. The struct is `Copy` because every `Signal` handle is
/// `Copy` (a cheap ID) — callers copy a handle out of a `panes.update` closure
/// and call `.set()` *outside* the borrow, so a `.set()` that fires an effect
/// reading `panes` can't double-borrow the `panes` signal (which would panic).
#[derive(Clone, Copy, Debug)]
pub(crate) struct PaneRuntimeSignals {
    /// Raw foreground program name, verbatim (e.g. `"nvim"`); `None` until detected.
    pub(crate) program: Signal<Option<String>>,
    /// High-level foreground-process lifecycle status.
    pub(crate) status: Signal<ProcessStatus>,
    /// Current working directory of the pane's foreground process.
    pub(crate) cwd: Signal<Option<PathBuf>>,
    /// Exit code of the last exited process (set on `Exit`/`Success`/`Error`).
    pub(crate) exit_code: Signal<Option<i32>>,
    /// Git summary for `cwd` if it is inside a repo; `None` otherwise.
    pub(crate) git: Signal<Option<GitInfo>>,
    /// Source/content kind hosted by the pane (`Terminal` now; `App`/`Plugin` later).
    pub(crate) kind: Signal<ContentKind>,
}

impl PaneRuntimeSignals {
    fn new(runtime: &PaneRuntime) -> Self {
        Self {
            program: signal(runtime.program.clone()),
            status: signal(runtime.status.clone()),
            cwd: signal(runtime.cwd.clone()),
            exit_code: signal(runtime.exit_code),
            git: signal(runtime.git.clone()),
            kind: signal(runtime.kind.clone()),
        }
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> PaneRuntime {
        PaneRuntime {
            program: self.program.get_untracked(),
            status: self.status.get_untracked(),
            cwd: self.cwd.get_untracked(),
            exit_code: self.exit_code.get_untracked(),
            git: self.git.get_untracked(),
            kind: self.kind.get_untracked(),
        }
    }
}

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
    /// Per-pane reactive mirror of canonical runtime metadata.
    pub(crate) panes: Signal<HashMap<PaneId, PaneRuntimeSignals>>,
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
            panes: signal(HashMap::new()),
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
    // Consumed by Phase 7 pane-info widgets; exercised by tests today, hence
    // `#[allow(dead_code)]` until a widget binds it.
    #[allow(dead_code)]
    pub(crate) fn with_pane_runtime<R>(&self, pane: PaneId, f: impl FnOnce(Option<&PaneRuntimeSignals>) -> R) -> R {
        self.panes.with(|panes| f(panes.get(&pane)))
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
    pub(crate) fn set_pane_runtime(&self, pane: PaneId, runtime: &PaneRuntime) {
        // Bulk path (per-frame from `sync_pane_runtime_state`): ONE `panes.update`
        // to ensure the entry + read the per-field signal handles (Copy) and the
        // current values into outer locals. The `.set()`s + emits run OUTSIDE the
        // borrow so a future effect reading `panes` during a set can't double-borrow
        // (reactive hazard — `SignalUpdate::update` borrows `panes` mutably; a
        // `.set()` inside it that fires a `panes`-reading effect would panic).
        let mut sigs: Option<PaneRuntimeSignals> = None;
        let mut cur: Option<PaneRuntime> = None;
        self.panes.update(|panes| {
            let entry = panes
                .entry(pane)
                .or_insert_with(|| PaneRuntimeSignals::new(&PaneRuntime::default()));
            sigs = Some(*entry); // PaneRuntimeSignals is Copy (all-handles-Copy)
            cur = Some(PaneRuntime {
                program: entry.program.get_untracked(),
                status: entry.status.get_untracked(),
                cwd: entry.cwd.get_untracked(),
                exit_code: entry.exit_code.get_untracked(),
                git: entry.git.get_untracked(),
                kind: entry.kind.get_untracked(),
            });
        });
        let sigs = sigs.expect("entry ensured above");
        let cur = cur.expect("entry ensured above");
        if cur.program != runtime.program {
            sigs.program.set(runtime.program.clone());
            self.events.emit(ChromeEvent::PaneProcessChanged { pane });
        }
        if cur.status != runtime.status {
            sigs.status.set(runtime.status.clone());
            self.events
                .emit(ChromeEvent::PaneStatusChanged { pane, status: runtime.status.clone() });
        }
        if cur.cwd != runtime.cwd {
            sigs.cwd.set(runtime.cwd.clone());
            self.events.emit(ChromeEvent::PaneCwdChanged { pane });
        }
        if cur.exit_code != runtime.exit_code {
            sigs.exit_code.set(runtime.exit_code);
        }
        if cur.git != runtime.git {
            sigs.git.set(runtime.git.clone());
            self.events.emit(ChromeEvent::PaneGitChanged { pane });
        }
        if cur.kind != runtime.kind {
            sigs.kind.set(runtime.kind.clone());
        }
    }
    // Per-field write API for Phase 2's process monitor. `set_pane_runtime` is
    // the bulk path used in prod today; these are exercised by tests and will
    // be called individually once detection lands (Phase 2+).
    #[allow(dead_code)]
    pub(crate) fn set_pane_program(&self, pane: PaneId, program: Option<String>) {
        // Decide inside the borrow; `.set()` + emit OUTSIDE it (reactive hazard fix).
        let mut sig: Option<Signal<Option<String>>> = None;
        self.panes.update(|panes| {
            let entry = panes
                .entry(pane)
                .or_insert_with(|| PaneRuntimeSignals::new(&PaneRuntime::default()));
            if entry.program.get_untracked() != program {
                sig = Some(entry.program); // Copy the handle out
            }
        });
        if let Some(sig) = sig {
            sig.set(program);
            self.events.emit(ChromeEvent::PaneProcessChanged { pane });
        }
    }
    // Per-field write API for Phase 2's process monitor. `set_pane_runtime` is
    // the bulk path used in prod today; these are exercised by tests and will
    // be called individually once detection lands (Phase 2+).
    #[allow(dead_code)]
    pub(crate) fn set_pane_status(&self, pane: PaneId, status: ProcessStatus) {
        let mut sig: Option<Signal<ProcessStatus>> = None;
        self.panes.update(|panes| {
            let entry = panes
                .entry(pane)
                .or_insert_with(|| PaneRuntimeSignals::new(&PaneRuntime::default()));
            if entry.status.get_untracked() != status {
                sig = Some(entry.status);
            }
        });
        if let Some(sig) = sig {
            sig.set(status.clone());
            self.events.emit(ChromeEvent::PaneStatusChanged { pane, status });
        }
    }
    // Per-field write API for Phase 2's process monitor. `set_pane_runtime` is
    // the bulk path used in prod today; these are exercised by tests and will
    // be called individually once detection lands (Phase 2+).
    #[allow(dead_code)]
    pub(crate) fn set_pane_cwd(&self, pane: PaneId, cwd: Option<PathBuf>) {
        let mut sig: Option<Signal<Option<PathBuf>>> = None;
        self.panes.update(|panes| {
            let entry = panes
                .entry(pane)
                .or_insert_with(|| PaneRuntimeSignals::new(&PaneRuntime::default()));
            if entry.cwd.get_untracked() != cwd {
                sig = Some(entry.cwd);
            }
        });
        if let Some(sig) = sig {
            sig.set(cwd);
            self.events.emit(ChromeEvent::PaneCwdChanged { pane });
        }
    }
    // Per-field write API for Phase 2's process monitor. `set_pane_runtime` is
    // the bulk path used in prod today; these are exercised by tests and will
    // be called individually once detection lands (Phase 2+).
    #[allow(dead_code)]
    pub(crate) fn set_pane_exit_code(&self, pane: PaneId, exit_code: Option<i32>) {
        let mut sig: Option<Signal<Option<i32>>> = None;
        self.panes.update(|panes| {
            let entry = panes
                .entry(pane)
                .or_insert_with(|| PaneRuntimeSignals::new(&PaneRuntime::default()));
            if entry.exit_code.get_untracked() != exit_code {
                sig = Some(entry.exit_code);
            }
        });
        if let Some(sig) = sig {
            sig.set(exit_code);
        }
    }
    // Per-field write API for Phase 2's process monitor. `set_pane_runtime` is
    // the bulk path used in prod today; these are exercised by tests and will
    // be called individually once detection lands (Phase 2+).
    #[allow(dead_code)]
    pub(crate) fn set_pane_git(&self, pane: PaneId, git: Option<GitInfo>) {
        let mut sig: Option<Signal<Option<GitInfo>>> = None;
        self.panes.update(|panes| {
            let entry = panes
                .entry(pane)
                .or_insert_with(|| PaneRuntimeSignals::new(&PaneRuntime::default()));
            if entry.git.get_untracked() != git {
                sig = Some(entry.git);
            }
        });
        if let Some(sig) = sig {
            sig.set(git);
            self.events.emit(ChromeEvent::PaneGitChanged { pane });
        }
    }
    // Per-field write API for Phase 2's process monitor. `set_pane_runtime` is
    // the bulk path used in prod today; these are exercised by tests and will
    // be called individually once detection lands (Phase 2+).
    #[allow(dead_code)]
    pub(crate) fn set_pane_kind(&self, pane: PaneId, kind: ContentKind) {
        let mut sig: Option<Signal<ContentKind>> = None;
        self.panes.update(|panes| {
            let entry = panes
                .entry(pane)
                .or_insert_with(|| PaneRuntimeSignals::new(&PaneRuntime::default()));
            if entry.kind.get_untracked() != kind {
                sig = Some(entry.kind);
            }
        });
        if let Some(sig) = sig {
            sig.set(kind);
        }
    }
    pub(crate) fn retain_panes(&self, keep: &HashSet<PaneId>) {
        self.panes.update(|panes| panes.retain(|pane, _| keep.contains(pane)));
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
    use heca_core::runtime::{ContentKind, GitInfo, PaneRuntime, ProcessStatus};
    use std::cell::RefCell;
    use std::path::PathBuf;
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

    #[test]
    fn pane_runtime_round_trips_through_signals() {
        let s = state();
        let pane = PaneId(42);
        let runtime = PaneRuntime {
            program: Some("nvim".into()),
            status: ProcessStatus::Running,
            cwd: Some(PathBuf::from("/tmp/project")),
            exit_code: Some(7),
            git: Some(GitInfo {
                branch: Some("main".into()),
                ahead: 1,
                behind: 2,
                added: 3,
                modified: 4,
                deleted: 5,
                dirty: true,
            }),
            kind: ContentKind::Terminal,
        };

        s.workspaces.set_pane_runtime(pane, &runtime);

        let mirrored = s
            .workspaces
            .with_pane_runtime(pane, |runtime| runtime.expect("pane runtime").snapshot());
        assert_eq!(mirrored, runtime);
    }

    #[test]
    fn pane_runtime_events_emit_only_on_change() {
        let s = state();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_events = seen.clone();
        let _sub = s.events().subscribe("*", move |event| {
            seen_events.borrow_mut().push(event.name().to_string());
        });
        let pane = PaneId(9);

        s.workspaces.set_pane_program(pane, Some("bash".into()));
        s.workspaces.set_pane_program(pane, Some("bash".into()));
        s.workspaces.set_pane_status(pane, ProcessStatus::Running);
        s.workspaces.set_pane_status(pane, ProcessStatus::Running);
        s.workspaces.set_pane_cwd(pane, Some(PathBuf::from("/repo")));
        s.workspaces.set_pane_cwd(pane, Some(PathBuf::from("/repo")));
        s.workspaces.set_pane_git(
            pane,
            Some(GitInfo {
                branch: Some("feat".into()),
                ahead: 0,
                behind: 0,
                added: 1,
                modified: 0,
                deleted: 0,
                dirty: true,
            }),
        );
        s.workspaces.set_pane_git(
            pane,
            Some(GitInfo {
                branch: Some("feat".into()),
                ahead: 0,
                behind: 0,
                added: 1,
                modified: 0,
                deleted: 0,
                dirty: true,
            }),
        );

        assert_eq!(
            seen.borrow().as_slice(),
            [
                "pane.process.changed",
                "pane.status.changed",
                "pane.cwd.changed",
                "pane.git.changed",
            ],
        );
    }
}
