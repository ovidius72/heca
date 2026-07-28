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

use crate::app_state::PendingPick;
use heca_core::layout::PaneId;
use heca_core::runtime::{ContentKind, GitInfo, PaneRuntime, ProcessStatus};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate, SignalWith, signal};
use heca_grid_ui::widgets::RegionMode;

use super::{ChromeEvent, ChromeEventBus, RegionId, SidebarSelection};

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
    /// User-set custom display name (from rename); `None` while tracking the process.
    /// Mirrored here so plugins observe renames via the store + `PaneCustomNameChanged`.
    pub(crate) custom_name: Signal<Option<String>>,
    /// Terminal viewport offset from the live bottom (rows above bottom). `0` = pinned to bottom.
    /// The backend owns this value; the store mirrors it for chrome/GUI reactivity.
    pub(crate) viewport_offset: Signal<usize>,
    /// Whether the viewport is pinned to the live bottom (`viewport_offset == 0`).
    pub(crate) at_bottom: Signal<bool>,
    /// Total retained terminal content rows (history + visible). Drives scrollbar thumb sizing.
    pub(crate) scrollback_rows: Signal<usize>,
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
            custom_name: signal(None),
            viewport_offset: signal(0),
            at_bottom: signal(true),
            scrollback_rows: signal(0),
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
        Self {
            mode: signal(mode),
            size: signal(size),
        }
    }
}

/// A container's active/hovered **pane** selection (NOT terminal text selection).
#[derive(Clone, Debug)]
pub(crate) struct ChromeSelection {
    pub(crate) active_pane: Signal<Option<PaneId>>,
}

impl ChromeSelection {
    fn new() -> Self {
        Self {
            active_pane: signal(None),
        }
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
    /// Targeting pick candidates (letter → `ws_idx`) for the "move column/pane to
    /// workspace" overlay, driving the universal `KeyHint`s over each workspace dock.
    /// **Empty = no pick active.** Read via `with_ws_pick_candidates`. Transient UI
    /// state, so (unlike pane picks) it emits no event bus notification.
    pub(crate) ws_pick_candidates: Signal<Vec<(char, usize)>>,
    /// Targeting pick candidates (letter → `(ws_idx, col_idx)`) for the "move pane to
    /// column" overlay over every workspace's columns. **Empty = no pick active.**
    /// Read via `with_col_pick_candidates`. Transient UI state (no event emitted).
    pub(crate) col_pick_candidates: Signal<Vec<(char, usize, usize)>>,
    /// The keyboard pick currently in progress (move/select/swap/take), if any. Exposed
    /// reactively (+ `PendingPickChanged`) so components/plugins can render their own UI
    /// for the pending action. `None` when idle.
    pub(crate) pending_pick: Signal<Option<PendingPick>>,
    /// Per-pane reactive mirror of canonical runtime metadata.
    pub(crate) panes: Signal<HashMap<PaneId, PaneRuntimeSignals>>,

    /// The sidebar-nav cursor selection while in `InputMode::SidebarNav`, projected
    /// from `AppState.sidebar_tree.current_item()`. `None` = not navigating. Drives
    /// the expanded sidebar's nav-cursor highlight, kept **distinct** from
    /// `active_pane` (the real session focus).
    pub(crate) nav_selection: Signal<Option<SidebarSelection>>,
    /// Whether a renamed pane's sidebar card shows a small dimmed `(process)` suffix
    /// after its name. Mirrors `[settings] pane_renamed_add_process_name`; projected
    /// from `AppState` in `sync_chrome_state`, read by `pane_card` at tree-build time.
    /// Carried here (rather than threaded through every card signature) so the flag
    /// reaches the card via the `ws_state` it already receives.
    pub(crate) pane_renamed_add_process_name: Signal<bool>,
    /// Whether the sidebar pane card shows a working-directory row. Mirrors
    /// `[settings] pane_show_cwd`; projected in `sync_chrome_state`, read by `pane_card`.
    /// Carried here for the same reason as `pane_renamed_add_process_name`.
    pub(crate) pane_show_cwd: Signal<bool>,
}

impl WorkspacesContainerState {
    fn new(events: ChromeEventBus) -> Self {
        Self {
            events,
            collapsed_ws: signal(HashSet::new()),
            selection: ChromeSelection::new(),
            pick_candidates: signal(Vec::new()),
            ws_pick_candidates: signal(Vec::new()),
            col_pick_candidates: signal(Vec::new()),
            pending_pick: signal(None),
            panes: signal(HashMap::new()),
            nav_selection: signal(None),
            // Matches the `[settings] pane_renamed_add_process_name` default (`true`);
            // `sync_chrome_state` sets the real value each sync.
            pane_renamed_add_process_name: signal(true),
            // Matches the `[settings] pane_show_cwd` default (`false`).
            pane_show_cwd: signal(false),
        }
    }

    // ── Reads ──
    pub fn active_pane(&self) -> Option<PaneId> {
        self.selection.active_pane.get()
    }
    /// The current sidebar-nav cursor selection (`None` when not navigating).
    pub fn nav_selection(&self) -> Option<SidebarSelection> {
        self.nav_selection.get()
    }
    /// Whether renamed panes show the `(process)` suffix in their sidebar card. Read
    /// untracked — consumed by `pane_card` while building the retained tree, not inside
    /// a reactive paint closure.
    pub fn pane_renamed_add_process_name(&self) -> bool {
        self.pane_renamed_add_process_name.get_untracked()
    }
    /// Whether the sidebar pane card shows a cwd row. Read untracked — consumed by
    /// `pane_card` at tree-build time.
    pub fn pane_show_cwd(&self) -> bool {
        self.pane_show_cwd.get_untracked()
    }
    /// Is workspace `ws_idx` collapsed? (Borrows — no clone.)
    pub fn is_ws_collapsed(&self, ws_idx: usize) -> bool {
        self.collapsed_ws.with(|s| s.contains(&ws_idx))
    }
    /// Borrow the collapsed-ws set without cloning it.
    pub fn with_collapsed_ws<R>(&self, f: impl FnOnce(&HashSet<usize>) -> R) -> R {
        self.collapsed_ws.with(f)
    }
    /// Is a targeting pick active? (Borrows — no clone.)
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "pane-pick widgets do not bind this helper yet; covered by state tests today"
        )
    )]
    pub fn pick_active(&self) -> bool {
        self.pick_candidates.with(|c| !c.is_empty())
    }
    /// Borrow the pick candidates without cloning the Vec.
    pub fn with_pick_candidates<R>(&self, f: impl FnOnce(&[(char, PaneId)]) -> R) -> R {
        self.pick_candidates.with(|c| f(c))
    }
    /// Borrow the workspace pick candidates without cloning the Vec.
    pub fn with_ws_pick_candidates<R>(&self, f: impl FnOnce(&[(char, usize)]) -> R) -> R {
        self.ws_pick_candidates.with(|c| f(c))
    }
    /// Borrow the column pick candidates without cloning the Vec.
    pub fn with_col_pick_candidates<R>(&self, f: impl FnOnce(&[(char, usize, usize)]) -> R) -> R {
        self.col_pick_candidates.with(|c| f(c))
    }
    /// The in-progress keyboard pick (move/select/swap/take), if any.
    pub fn pending_pick(&self) -> Option<PendingPick> {
        self.pending_pick.get_untracked()
    }
    pub(crate) fn with_pane_runtime<R>(
        &self,
        pane: PaneId,
        f: impl FnOnce(Option<&PaneRuntimeSignals>) -> R,
    ) -> R {
        self.panes.with(|panes| f(panes.get(&pane)))
    }

    /// Snapshot a pane's reactive runtime into a plain [`PaneRuntime`] (the public
    /// read selector behind the host API's `app.state.pane_runtime`). `None` if the
    /// pane has no mirrored runtime.
    pub fn pane_runtime(&self, pane: PaneId) -> Option<PaneRuntime> {
        self.with_pane_runtime(pane, |runtime| {
            runtime.map(|r| PaneRuntime {
                program: r.program.get_untracked(),
                status: r.status.get_untracked(),
                cwd: r.cwd.get_untracked(),
                exit_code: r.exit_code.get_untracked(),
                git: r.git.get_untracked(),
                kind: r.kind.get_untracked(),
            })
        })
    }

    /// A pane's user-set custom name (from rename), mirrored into the store; `None`
    /// while it tracks the process name or the pane has no mirrored entry.
    pub fn pane_custom_name(&self, pane: PaneId) -> Option<String> {
        self.with_pane_runtime(pane, |runtime| {
            runtime.and_then(|r| r.custom_name.get_untracked())
        })
    }

    /// Terminal viewport state for a pane. `None` if the pane has no mirrored entry
    /// (e.g. non-terminal panes or freshly created panes not yet synced).
    pub fn terminal_viewport(&self, pane: PaneId) -> Option<crate::host::TerminalViewport> {
        self.with_pane_runtime(pane, |runtime| {
            runtime.map(|r| crate::host::TerminalViewport {
                viewport_offset: r.viewport_offset.get_untracked(),
                at_bottom: r.at_bottom.get_untracked(),
                scrollback_rows: r.scrollback_rows.get_untracked(),
            })
        })
    }

    // ── Writes ──
    pub fn set_active_pane(&self, pane: Option<PaneId>) {
        if self.selection.active_pane.get_untracked() == pane {
            return;
        }
        self.selection.active_pane.set(pane);
        self.events.emit(ChromeEvent::PaneActiveChanged { pane });
    }
    /// Set (or clear, with `None`) the sidebar-nav cursor selection. Emits
    /// [`ChromeEvent::SidebarSelectionChanged`]. Cleared on leaving nav mode.
    pub fn set_nav_selection(&self, selection: Option<SidebarSelection>) {
        if self.nav_selection.get_untracked() == selection {
            return;
        }
        self.nav_selection.set(selection);
        self.events
            .emit(ChromeEvent::SidebarSelectionChanged { selection });
    }
    /// Project the `[settings] pane_renamed_add_process_name` flag into the store.
    /// Idempotent (no-op when unchanged). A pure display-config flag read at card build,
    /// so it emits no `ChromeEvent` — a config reload rebuilds the sidebar anyway.
    pub fn set_pane_renamed_add_process_name(&self, on: bool) {
        if self.pane_renamed_add_process_name.get_untracked() == on {
            return;
        }
        self.pane_renamed_add_process_name.set(on);
    }
    /// Project the `[settings] pane_show_cwd` flag into the store. Idempotent; emits no
    /// `ChromeEvent` (pure display-config flag read at card build — a reload rebuilds the
    /// sidebar anyway).
    pub fn set_pane_show_cwd(&self, on: bool) {
        if self.pane_show_cwd.get_untracked() == on {
            return;
        }
        self.pane_show_cwd.set(on);
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
    pub fn set_ws_pick_candidates(&self, candidates: Vec<(char, usize)>) {
        if self.ws_pick_candidates.get_untracked() == candidates {
            return;
        }
        self.ws_pick_candidates.set(candidates);
    }
    pub fn clear_ws_pick_candidates(&self) {
        if self.ws_pick_candidates.get_untracked().is_empty() {
            return;
        }
        self.ws_pick_candidates.update(|c| c.clear());
    }
    pub fn set_col_pick_candidates(&self, candidates: Vec<(char, usize, usize)>) {
        if self.col_pick_candidates.get_untracked() == candidates {
            return;
        }
        self.col_pick_candidates.set(candidates);
    }
    pub fn clear_col_pick_candidates(&self) {
        if self.col_pick_candidates.get_untracked().is_empty() {
            return;
        }
        self.col_pick_candidates.update(|c| c.clear());
    }
    /// Set the in-progress pick (`None` clears it). Guarded — emits
    /// [`ChromeEvent::PendingPickChanged`] only on a real change.
    pub fn set_pending_pick(&self, pick: Option<PendingPick>) {
        if self.pending_pick.get_untracked() == pick {
            return;
        }
        self.pending_pick.set(pick.clone());
        self.events.emit(ChromeEvent::PendingPickChanged { pick });
    }
    pub(crate) fn set_pane_runtime(
        &self,
        pane: PaneId,
        runtime: &PaneRuntime,
        custom_name: Option<&str>,
    ) -> bool {
        // Bulk path (per-frame from `sync_pane_runtime_state`): ONE `panes.update`
        // to ensure the entry + read the per-field signal handles (Copy) and the
        // current values into outer locals. The `.set()`s + emits run OUTSIDE the
        // borrow so a future effect reading `panes` during a set can't double-borrow
        // (reactive hazard — `SignalUpdate::update` borrows `panes` mutably; a
        // `.set()` inside it that fires a `panes`-reading effect would panic).
        let mut sigs: Option<PaneRuntimeSignals> = None;
        let mut cur: Option<PaneRuntime> = None;
        let mut cur_custom: Option<String> = None;
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
            cur_custom = entry.custom_name.get_untracked();
        });
        let sigs = sigs.expect("entry ensured above");
        let cur = cur.expect("entry ensured above");
        let mut changed = false;
        if cur.program != runtime.program {
            sigs.program.set(runtime.program.clone());
            self.events.emit(ChromeEvent::PaneProcessChanged { pane });
            changed = true;
        }
        if cur.status != runtime.status {
            sigs.status.set(runtime.status.clone());
            self.events.emit(ChromeEvent::PaneStatusChanged {
                pane,
                status: runtime.status.clone(),
            });
            changed = true;
        }
        if cur.cwd != runtime.cwd {
            sigs.cwd.set(runtime.cwd.clone());
            self.events.emit(ChromeEvent::PaneCwdChanged { pane });
            changed = true;
        }
        if cur.exit_code != runtime.exit_code {
            sigs.exit_code.set(runtime.exit_code);
            changed = true;
        }
        if cur.git != runtime.git {
            sigs.git.set(runtime.git.clone());
            self.events.emit(ChromeEvent::PaneGitChanged { pane });
            changed = true;
        }
        if cur.kind != runtime.kind {
            sigs.kind.set(runtime.kind.clone());
            changed = true;
        }
        let next_custom = custom_name.map(|s| s.to_string());
        if cur_custom != next_custom {
            sigs.custom_name.set(next_custom.clone());
            self.events.emit(ChromeEvent::PaneCustomNameChanged {
                pane,
                name: next_custom,
            });
            changed = true;
        }
        changed
    }
    // Per-field write API for Phase 2's process monitor. `set_pane_runtime` is
    // the bulk path used in prod today; these are exercised by tests and will
    // be called individually once detection lands (Phase 2+).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "per-field process runtime writes are still test-only until monitor wiring lands"
        )
    )]
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "per-field process runtime writes are still test-only until monitor wiring lands"
        )
    )]
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
            self.events
                .emit(ChromeEvent::PaneStatusChanged { pane, status });
        }
    }
    // Per-field write API for Phase 2's process monitor. `set_pane_runtime` is
    // the bulk path used in prod today; these are exercised by tests and will
    // be called individually once detection lands (Phase 2+).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "per-field process runtime writes are still test-only until monitor wiring lands"
        )
    )]
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
    #[expect(
        dead_code,
        reason = "per-field process runtime writes are still test-only until monitor wiring lands"
    )]
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "per-field process runtime writes are still test-only until monitor wiring lands"
        )
    )]
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
    #[expect(
        dead_code,
        reason = "per-field process runtime writes are still test-only until monitor wiring lands"
    )]
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

    /// Mirror terminal viewport state from a backend snapshot into the store.
    ///
    /// Idempotent: emits `TerminalViewportChanged` only when a value actually
    /// changes. Returns `true` if any field changed, `false` for a no-op.
    pub(crate) fn set_pane_viewport(
        &self,
        pane: PaneId,
        viewport_offset: usize,
        at_bottom: bool,
        scrollback_rows: usize,
    ) -> bool {
        let mut sigs: Option<PaneRuntimeSignals> = None;
        let mut cur_offset = None;
        let mut cur_at_bottom = None;
        let mut cur_rows = None;
        self.panes.update(|panes| {
            let entry = panes
                .entry(pane)
                .or_insert_with(|| PaneRuntimeSignals::new(&PaneRuntime::default()));
            sigs = Some(*entry);
            cur_offset = Some(entry.viewport_offset.get_untracked());
            cur_at_bottom = Some(entry.at_bottom.get_untracked());
            cur_rows = Some(entry.scrollback_rows.get_untracked());
        });
        let sigs = sigs.expect("entry ensured above");
        let mut changed = false;
        if cur_offset != Some(viewport_offset) {
            sigs.viewport_offset.set(viewport_offset);
            changed = true;
        }
        if cur_at_bottom != Some(at_bottom) {
            sigs.at_bottom.set(at_bottom);
            changed = true;
        }
        if cur_rows != Some(scrollback_rows) {
            sigs.scrollback_rows.set(scrollback_rows);
            changed = true;
        }
        if changed {
            self.events.emit(ChromeEvent::TerminalViewportChanged {
                pane,
                viewport_offset,
                at_bottom,
                scrollback_rows,
            });
        }
        changed
    }

    pub(crate) fn retain_panes(&self, keep: &HashSet<PaneId>) {
        self.panes
            .update(|panes| panes.retain(|pane, _| keep.contains(pane)));
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
        self.events
            .emit(ChromeEvent::WorkspaceCollapsedChanged { ws_idx, collapsed });
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
        self.events
            .emit(ChromeEvent::WorkspaceCollapsedChanged { ws_idx, collapsed });
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
    /// Scroll offsets for containers' **own** nested scroll areas, keyed by the **mount id**
    /// (F003/P011/T021).
    ///
    /// Per mount, not per container kind: the same container can be seated twice — two of them in
    /// one region, or one in each — and each mount scrolls its own content. Keying this by kind is
    /// what made two mounts scroll together, which is invisible until there are two.
    ///
    /// Created on first ask and kept here so an offset outlives the retained tree; a rebuild
    /// (triggered by something as small as a pane's git status changing) restores rather than
    /// resets. Shared, so a clone of this store aliases the same signals.
    container_scroll: std::rc::Rc<std::cell::RefCell<HashMap<String, Signal<f32>>>>,
}

impl SharedChromeState {
    /// This mount's own scroll offset signal, created on first ask.
    ///
    /// Keyed by **mount id**, so two mounts of the same container each get their own and scroll
    /// independently. The signal outlives the retained tree, so a rebuild restores the position
    /// instead of snapping to the top.
    pub fn container_scroll(&self, container: &str) -> Signal<f32> {
        if let Some(existing) = self.container_scroll.borrow().get(container) {
            return *existing;
        }
        let created = signal(0.0);
        self.container_scroll
            .borrow_mut()
            .insert(container.to_string(), created);
        created
    }

    /// Record a mount's scroll offset, emitting [`ChromeEvent::ContainerScrollChanged`] when it
    /// really moved. The epsilon guard is what stops a restore, or a wheel notch that changed
    /// nothing, from emitting.
    pub fn set_container_scroll(&self, container: &str, offset: f32) {
        let sig = self.container_scroll(container);
        if (sig.get_untracked() - offset).abs() <= f32::EPSILON {
            return;
        }
        sig.set(offset);
        self.events.emit(ChromeEvent::ContainerScrollChanged {
            container: container.to_string(),
            offset,
        });
    }

    /// Construct the store with initial region modes + widths (mirroring the
    /// `SidebarState` defaults during migration). Signals are created here — requires
    /// the reactive runtime, available on the UI thread at `AppState` construction.
    pub fn new(left_width: f32, left_visible: bool, right_width: f32, right_visible: bool) -> Self {
        let mode = |visible: bool| {
            if visible {
                RegionMode::Expanded
            } else {
                RegionMode::Hidden
            }
        };
        let events = ChromeEventBus::default();
        Self {
            events: events.clone(),
            left: RegionState::new(mode(left_visible), left_width),
            right: RegionState::new(mode(right_visible), right_width),
            workspaces: WorkspacesContainerState::new(events),
            container_scroll: std::rc::Rc::new(std::cell::RefCell::new(HashMap::new())),
        }
    }

    pub fn events(&self) -> ChromeEventBus {
        self.events.clone()
    }

    // ── Region (shell) reads/writes — RegionMode/f32 are Copy → `.get()` is cheap ──
    #[expect(
        dead_code,
        reason = "region mode accessors are part of the shell state API; only visibility is consumed today"
    )]
    pub fn left_mode(&self) -> RegionMode {
        self.left.mode.get()
    }
    pub fn left_size(&self) -> f32 {
        self.left.size.get()
    }
    pub fn left_visible(&self) -> bool {
        !matches!(self.left.mode.get(), RegionMode::Hidden)
    }
    #[expect(
        dead_code,
        reason = "region mode accessors are part of the shell state API; only visibility is consumed today"
    )]
    pub fn right_mode(&self) -> RegionMode {
        self.right.mode.get()
    }
    pub fn right_size(&self) -> f32 {
        self.right.size.get()
    }
    pub fn right_visible(&self) -> bool {
        !matches!(self.right.mode.get(), RegionMode::Hidden)
    }

    pub fn set_left_mode(&self, mode: RegionMode) {
        if self.left.mode.get_untracked() == mode {
            return;
        }
        self.left.mode.set(mode);
        self.events.emit(ChromeEvent::RegionModeChanged {
            region: RegionId::LeftSidebar,
            mode,
        });
    }
    pub fn set_left_size(&self, size: f32) {
        if (self.left.size.get_untracked() - size).abs() <= f32::EPSILON {
            return;
        }
        self.left.size.set(size);
        self.events.emit(ChromeEvent::RegionSizeChanged {
            region: RegionId::LeftSidebar,
            size,
        });
    }
    pub fn set_right_mode(&self, mode: RegionMode) {
        if self.right.mode.get_untracked() == mode {
            return;
        }
        self.right.mode.set(mode);
        self.events.emit(ChromeEvent::RegionModeChanged {
            region: RegionId::RightSidebar,
            mode,
        });
    }
    pub fn set_right_size(&self, size: f32) {
        if (self.right.size.get_untracked() - size).abs() <= f32::EPSILON {
            return;
        }
        self.right.size.set(size);
        self.events.emit(ChromeEvent::RegionSizeChanged {
            region: RegionId::RightSidebar,
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
        s.workspaces
            .set_pick_candidates(vec![('a', PaneId(1)), ('b', PaneId(2))]);
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
    fn pane_renamed_add_process_name_defaults_on_then_projects() {
        let s = state();
        // Defaults to the `[settings]` default so the first frame matches config.
        assert!(s.workspaces.pane_renamed_add_process_name());
        s.workspaces.set_pane_renamed_add_process_name(false);
        assert!(!s.workspaces.pane_renamed_add_process_name());
        // Idempotent re-set is a no-op (guarded setter).
        s.workspaces.set_pane_renamed_add_process_name(false);
        assert!(!s.workspaces.pane_renamed_add_process_name());
    }

    #[test]
    fn pane_show_cwd_defaults_off_then_projects() {
        let s = state();
        // Defaults to the `[settings]` default (`false`).
        assert!(!s.workspaces.pane_show_cwd());
        s.workspaces.set_pane_show_cwd(true);
        assert!(s.workspaces.pane_show_cwd());
        s.workspaces.set_pane_show_cwd(true);
        assert!(s.workspaces.pane_show_cwd());
    }

    #[test]
    fn nav_selection_round_trips_and_emits_on_change() {
        let s = state();
        assert_eq!(s.workspaces.nav_selection(), None);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_ev = seen.clone();
        let _sub = s.events().subscribe("sidebar.selection.changed", move |e| {
            seen_ev.borrow_mut().push(e.name().to_string());
        });

        let sel = SidebarSelection::Pane { pane_id: PaneId(4) };
        s.workspaces.set_nav_selection(Some(sel));
        assert_eq!(s.workspaces.nav_selection(), Some(sel));
        // Idempotent: setting the same value again emits nothing.
        s.workspaces.set_nav_selection(Some(sel));
        // Clearing on nav exit.
        s.workspaces.set_nav_selection(None);
        assert_eq!(s.workspaces.nav_selection(), None);

        assert_eq!(
            seen.borrow().as_slice(),
            ["sidebar.selection.changed", "sidebar.selection.changed"],
            "one event for the set, one for the clear — none for the duplicate"
        );
    }

    /// A mount's scroll offset round-trips and emits, keyed by its mount id.
    #[test]
    fn a_mounts_scroll_round_trips() {
        let s = state();
        assert_eq!(s.container_scroll("workspaces").get_untracked(), 0.0);
        s.set_container_scroll("workspaces", 42.5);
        assert_eq!(s.container_scroll("workspaces").get_untracked(), 42.5);
        // Asking again is the same signal, not a fresh one — otherwise a rebuild would reset it.
        assert_eq!(s.container_scroll("workspaces").get_untracked(), 42.5);
    }

    /// Every **placement** owns its own scroll offset.
    ///
    /// Scrolling is per container: each nests its own scroll area, and the shell does not scroll at
    /// all — it gives containers bounds and they take their shares of them (F003/P011/T021). So the
    /// offsets that exist are per placement, and placing one container twice must not make the two
    /// move together, which is invisible until there are two of something.
    ///
    /// Two placements show the same content, because that comes from the shared store. Their scroll
    /// positions are their own, because those belong to the placement.
    #[test]
    fn every_placement_owns_its_own_scroll_offset() {
        let s = state();

        s.set_container_scroll("dock.a", 30.0);
        assert_eq!(
            s.container_scroll("dock.b").get_untracked(),
            0.0,
            "two placements of one container scroll independently",
        );
        assert_eq!(
            s.container_scroll("workspaces").get_untracked(),
            0.0,
            "and a third placement elsewhere is untouched",
        );
        assert_eq!(s.container_scroll("dock.a").get_untracked(), 30.0);
    }

    #[test]
    fn clone_is_a_shallow_alias_not_a_snapshot() {
        let a = state();
        let b = a.clone();
        a.set_container_scroll("workspaces", 99.0);
        assert_eq!(
            b.container_scroll("workspaces").get_untracked(),
            99.0,
            "clone must alias the same signal store — including the per-mount offsets"
        );
        b.workspaces.toggle_ws_collapsed(3);
        assert!(
            a.workspaces.is_ws_collapsed(3),
            "collapse via clone must be seen by original"
        );
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

        s.workspaces
            .set_pane_runtime(pane, &runtime, Some("my pane"));

        let mirrored = s
            .workspaces
            .with_pane_runtime(pane, |runtime| runtime.expect("pane runtime").snapshot());
        assert_eq!(mirrored, runtime);
        assert_eq!(
            s.workspaces.pane_custom_name(pane),
            Some("my pane".to_string())
        );
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
        s.workspaces
            .set_pane_cwd(pane, Some(PathBuf::from("/repo")));
        s.workspaces
            .set_pane_cwd(pane, Some(PathBuf::from("/repo")));
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

    #[test]
    fn terminal_viewport_defaults_are_zeros() {
        let s = state();
        let pane = PaneId(10);
        // A pane that was never written via set_pane_viewport returns None.
        assert_eq!(s.workspaces.terminal_viewport(pane), None);

        // After a viewport write, it returns the expected values.
        s.workspaces
            .set_pane_viewport(pane, 42, false, 200);
        let vp = s.workspaces.terminal_viewport(pane).unwrap();
        assert_eq!(
            vp,
            crate::host::TerminalViewport {
                viewport_offset: 42,
                at_bottom: false,
                scrollback_rows: 200,
            }
        );
    }

    #[test]
    fn terminal_viewport_emits_only_on_change() {
        let s = state();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_events = seen.clone();
        let _sub = s.events().subscribe("*", move |event| {
            seen_events.borrow_mut().push(event.name().to_string());
        });
        let pane = PaneId(11);

        // First write -> terminal.viewport.changed emitted.
        s.workspaces
            .set_pane_viewport(pane, 10, false, 50);
        // Identical write -> no event.
        s.workspaces
            .set_pane_viewport(pane, 10, false, 50);
        // Single field change -> event fired.
        s.workspaces
            .set_pane_viewport(pane, 11, false, 50);
        // All fields changed -> single event.
        s.workspaces
            .set_pane_viewport(pane, 0, true, 100);
        // Back to the last values -> event fired (still a change from current).
        s.workspaces
            .set_pane_viewport(pane, 11, false, 50);

        assert_eq!(
            seen.borrow().as_slice(),
            [
                "terminal.viewport.changed",
                "terminal.viewport.changed",
                "terminal.viewport.changed",
                "terminal.viewport.changed",
            ],
            "expected only 4 events: first write, offset change, all-field change, revert"
        );
    }

    #[test]
    fn terminal_viewport_does_not_require_runtime_preinit() {
        let s = state();
        let pane = PaneId(12);
        // set_pane_viewport on a fresh pane without set_pane_runtime should work.
        s.workspaces
            .set_pane_viewport(pane, 5, true, 30);
        let vp = s.workspaces.terminal_viewport(pane).unwrap();
        assert_eq!(vp.viewport_offset, 5);
        assert!(vp.at_bottom);
        assert_eq!(vp.scrollback_rows, 30);
    }
}
