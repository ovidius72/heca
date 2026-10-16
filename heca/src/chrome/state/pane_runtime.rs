//! Each pane's live facts (program, status, folder, git, scroll), mirrored as signals.

use super::*;

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

    /// Read every field of this pane's runtime at once.
    ///
    /// **Tracked reads, one per field.** Outside a reactive scope this is the same as an untracked
    /// read; inside one it is the whole point — a subscription depends on this pane's six fields
    /// and on nothing else, so a neighbour's directory changing does not wake this row.
    pub(crate) fn snapshot(&self) -> PaneRuntime {
        PaneRuntime {
            program: self.program.get(),
            status: self.status.get(),
            cwd: self.cwd.get(),
            exit_code: self.exit_code.get(),
            git: self.git.get(),
            kind: self.kind.get(),
        }
    }

    /// Read every field of this pane's runtime **without subscribing** — what a write compares
    /// against, and what a one-off query answers.
    pub(crate) fn current(&self) -> PaneRuntime {
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

/// **Every pane's live facts, for every component** — the sidebar rows, the exposé, the command
/// palette and a plugin all read the same store. Held by [`SharedChromeState`] rather than by one
/// component, so no component has to reach into another to learn what a pane is running.
///
/// `Clone` is a shallow alias: clones share the same signals.
#[derive(Clone, Debug)]
pub struct PaneRuntimeStore {
    events: ChromeEventBus,
    panes: Signal<HashMap<PaneId, PaneRuntimeSignals>>,
}

impl PaneRuntimeStore {
    pub(super) fn new(events: ChromeEventBus) -> Self {
        Self {
            events,
            panes: signal(HashMap::new()),
        }
    }

    /// Look at one pane's signals, if the store has met it.
    pub(crate) fn with<R>(
        &self,
        pane: PaneId,
        f: impl FnOnce(Option<&PaneRuntimeSignals>) -> R,
    ) -> R {
        self.panes.with(|panes| f(panes.get(&pane)))
    }

    /// **This pane's signal handles, created if the store has not met it yet.**
    ///
    /// The handles are `Copy`, so a component takes them once while it builds and reads them from
    /// inside its own subscription — depending on that pane's fields rather than on the map that
    /// holds them. Subscribing to the map instead would wake every row on every frame, because the
    /// per-frame mirror touches it whether anything changed or not.
    ///
    /// It **creates** the entry rather than answering `None`, and that is the point: a handle a
    /// caller does not get is a subscription that can never form, so a row built in the same frame
    /// its pane appeared would be deaf for the life of the tree — the same "read once at build time
    /// and frozen" failure as a line that was never attached. The mirror fills the entry in on its
    /// next pass, and `retain` prunes it when the pane goes.
    pub(crate) fn signals(&self, pane: PaneId) -> PaneRuntimeSignals {
        if let Some(existing) = self.panes.with(|panes| panes.get(&pane).copied()) {
            return existing;
        }
        let created = PaneRuntimeSignals::new(&PaneRuntime::default());
        self.panes.update(|panes| {
            panes.insert(pane, created);
        });
        created
    }

    /// Snapshot a pane's reactive runtime into a plain [`PaneRuntime`] (the public
    /// read selector behind the host API's `app.state.pane_runtime`). `None` if the
    /// pane has no mirrored runtime.
    pub fn runtime(&self, pane: PaneId) -> Option<PaneRuntime> {
        self.with(pane, |runtime| runtime.map(PaneRuntimeSignals::current))
    }

    /// A pane's user-set custom name (from rename), mirrored into the store; `None`
    /// while it tracks the process name or the pane has no mirrored entry.
    pub fn custom_name(&self, pane: PaneId) -> Option<String> {
        self.with(pane, |runtime| {
            runtime.and_then(|r| r.custom_name.get_untracked())
        })
    }

    /// Terminal viewport state for a pane. `None` if the pane has no mirrored entry
    /// (e.g. non-terminal panes or freshly created panes not yet synced).
    pub fn terminal_viewport(&self, pane: PaneId) -> Option<crate::host::TerminalViewport> {
        self.with(pane, |runtime| {
            runtime.map(|r| crate::host::TerminalViewport {
                viewport_offset: r.viewport_offset.get_untracked(),
                at_bottom: r.at_bottom.get_untracked(),
                scrollback_rows: r.scrollback_rows.get_untracked(),
            })
        })
    }

    /// **Mirror a pane's runtime into the store** — the one write path, run every frame by the
    /// chrome sync. Each field that changed is set and announced; one that did not costs nothing.
    /// Returns whether anything changed.
    ///
    /// The handles are copied out before any `.set()`, so an effect a set wakes can read the pane
    /// map without finding it borrowed.
    pub(crate) fn set_runtime(
        &self,
        pane: PaneId,
        runtime: &PaneRuntime,
        custom_name: Option<&str>,
    ) -> bool {
        let sigs = self.signals(pane);
        let cur = sigs.current();
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
        let next_custom = custom_name.map(str::to_string);
        if sigs.custom_name.get_untracked() != next_custom {
            sigs.custom_name.set(next_custom.clone());
            self.events.emit(ChromeEvent::PaneCustomNameChanged {
                pane,
                name: next_custom,
            });
            changed = true;
        }
        changed
    }

    /// Mirror terminal viewport state from a backend snapshot into the store.
    ///
    /// Idempotent: emits `TerminalViewportChanged` only when a value actually
    /// changes. Returns `true` if any field changed, `false` for a no-op.
    pub(crate) fn set_viewport(
        &self,
        pane: PaneId,
        viewport_offset: usize,
        at_bottom: bool,
        scrollback_rows: usize,
    ) -> bool {
        let sigs = self.signals(pane);
        let mut changed = false;
        if sigs.viewport_offset.get_untracked() != viewport_offset {
            sigs.viewport_offset.set(viewport_offset);
            changed = true;
        }
        if sigs.at_bottom.get_untracked() != at_bottom {
            sigs.at_bottom.set(at_bottom);
            changed = true;
        }
        if sigs.scrollback_rows.get_untracked() != scrollback_rows {
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

    pub(crate) fn retain(&self, keep: &HashSet<PaneId>) {
        self.panes
            .update(|panes| panes.retain(|pane, _| keep.contains(pane)));
    }
}
