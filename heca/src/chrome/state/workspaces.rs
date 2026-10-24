//! The workspaces sidebar's own state: its model, collapse, selection and settings mirrors.

use super::*;

/// A container's active/hovered **pane** selection (NOT terminal text selection).
#[derive(Clone, Debug)]
pub(crate) struct ChromeSelection {
    pub(crate) active_pane: Signal<Option<PaneId>>,
    /// **The pane a back-and-forth binding would return to, per workspace** (`prefix+i` /
    /// `prefix+Shift+i`). Signal-backed like [`active_pane`](Self::active_pane), so the mark moves
    /// without a rebuild. Per workspace because a cross-workspace jump leaves the target in the
    /// workspace you came from.
    pub(crate) previous_panes: Signal<Vec<Option<PaneId>>>,
    /// The workspace `prefix+Shift+i` would return to.
    pub(crate) previous_ws: Signal<Option<usize>>,
}

impl ChromeSelection {
    fn new() -> Self {
        Self {
            active_pane: signal(None),
            previous_panes: signal(Vec::new()),
            previous_ws: signal(None),
        }
    }
}

/// The **WorkspacesContainer's** own namespaced UI state. Workspace/pane specifics
/// belong here (this is the workspaces container), never on the shell. Region-
/// independent, so it travels with the container if it moves between regions.
#[derive(Clone, Debug)]
pub struct WorkspacesContainerState {
    events: ChromeEventBus,
    /// **The workspaces component's own model** — its rows, its cursor, its collapse state.
    ///
    /// It lives here rather than on `AppState` because it belongs to the component, not to the
    /// app. It is not on the provider struct either: a provider is a `Box<dyn Provider>`, so
    /// anything that wanted the model back would need `as_any` and a downcast — host code reaching
    /// into one named component, which is the thing the pluggable design exists to prevent. Here it is reachable through the ordinary
    /// [`StateView`](crate::host::StateView) every provider already reads from, and through
    /// `ProviderCx` inside `perform`.
    ///
    /// `RefCell` because the store is shared by clone (its signals alias), and the model is a plain
    /// projection rebuilt from `Session` rather than a set of signals. Borrow it for as short a span
    /// as possible — never across a call that might reach back into the store.
    tree: std::rc::Rc<std::cell::RefCell<crate::providers::workspaces::WorkspaceTree>>,
    /// Workspaces collapsed in this container (by ws index). Read via `with_collapsed_ws`.
    pub(crate) collapsed_ws: Signal<HashSet<usize>>,
    /// Active/hovered pane in this container.
    pub(crate) selection: ChromeSelection,
    /// The keyboard pick currently in progress (move/select/swap/take), if any. Exposed
    /// reactively (+ `PendingPickChanged`) so components/plugins can render their own UI
    /// for the pending action. `None` when idle.
    pub(crate) pending_pick: Signal<Option<PendingPick>>,

    /// Whether a renamed pane's sidebar card shows a small dimmed `(process)` suffix
    /// after its name. Mirrors `[settings] pane_renamed_add_process_name`; projected
    /// from `AppState` in `sync_chrome_state`, read by `pane_card` at tree-build time.
    /// Carried here (rather than threaded through every card signature) so the flag
    /// reaches the card via the `ws_state` it already receives.
    pub(crate) pane_renamed_add_process_name: Signal<bool>,
    /// The program catalog (`[program]`) — icons and display names for running processes.
    ///
    /// **This component's, not every component's**: the context every component receives stays
    /// component-agnostic, so a component that renders processes reads the catalog from its own
    /// state, like its model beside it.
    ///
    /// An `Rc` because a `Signal` read clones, and this is a whole parsed config; mirrored from
    /// `AppState` in `sync_chrome_state`, so a `prefix+Shift+r` reload reaches it.
    pub(crate) programs: Signal<std::rc::Rc<heca_config::programs::ProgramsConfig>>,
    /// The lines shown under a pane's name, in order — `[appearance.sidebar] pane_lines` resolved by
    /// name; mirrored in `sync_chrome_state`, read when a row is built. A change rebuilds the rows
    /// (`chrome_signature` hashes the names), so it is not a per-frame signal.
    pub(crate) pane_lines: Signal<crate::chrome::panes::pane_items::PaneLineSet>,
    /// Open status labels and glyphs, read by each agent line's own subscription.
    pub(crate) agent_statuses: Signal<std::rc::Rc<heca_config::settings::AgentStatuses>>,
}

impl WorkspacesContainerState {
    pub(crate) fn set_agent_statuses(
        &self,
        statuses: &heca_config::settings::AgentStatuses,
    ) {
        if self.agent_statuses.with(|current| current.as_ref() != statuses) {
            self.agent_statuses.set(std::rc::Rc::new(statuses.clone()));
        }
    }

    /// The lines to build under each pane's name. See [`pane_lines`](Self::pane_lines) field.
    pub(crate) fn pane_lines(&self) -> crate::chrome::panes::pane_items::PaneLineSet {
        use heca_grid_ui::reactive::SignalGet as _;
        self.pane_lines.get_untracked()
    }

    /// Mirror the lines that show. Written only when they differ, so an unchanged reload costs nothing.
    pub(crate) fn set_pane_lines(&self, lines: crate::chrome::panes::pane_items::PaneLineSet) {
        use heca_grid_ui::reactive::{SignalGet as _, SignalUpdate as _};
        if self.pane_lines.get_untracked() != lines {
            self.pane_lines.set(lines);
        }
    }

    /// Read the component's model — its rows, cursor and collapse state.
    ///
    /// Keep the borrow short: it is a `RefCell`, so holding one across a call that reaches back
    /// into the store is a runtime panic rather than a compile error.
    pub fn tree(&self) -> std::cell::Ref<'_, crate::providers::workspaces::WorkspaceTree> {
        self.tree.borrow()
    }

    /// Mutate the component's model.
    pub fn tree_mut(&self) -> std::cell::RefMut<'_, crate::providers::workspaces::WorkspaceTree> {
        self.tree.borrow_mut()
    }

    pub(super) fn new(events: ChromeEventBus) -> Self {
        Self {
            events,
            tree: std::rc::Rc::new(std::cell::RefCell::new(
                crate::providers::workspaces::WorkspaceTree::new(),
            )),
            collapsed_ws: signal(HashSet::new()),
            selection: ChromeSelection::new(),
            pending_pick: signal(None),
            // The config's own defaults until `sync_chrome_state` mirrors the loaded values.
            pane_renamed_add_process_name: signal(
                heca_config::settings::SettingsConfig::default().pane_renamed_add_process_name,
            ),
            // The embedded defaults until `sync_chrome_state` mirrors the loaded config.
            programs: signal(std::rc::Rc::new(
                heca_config::programs::ProgramsConfig::default(),
            )),
            // heca's own lines until `sync_chrome_state` mirrors what the user's list resolves to.
            pane_lines: signal(
                crate::chrome::PaneRowLines::default()
                    .shown(&heca_config::appearance::default_sidebar_pane_lines()),
            ),
            agent_statuses: signal(std::rc::Rc::new(
                heca_config::settings::AgentsConfig::default().statuses,
            )),
        }
    }

    // ── Reads ──
    pub fn active_pane(&self) -> Option<PaneId> {
        self.selection.active_pane.get()
    }
    /// Is this pane the one a back-and-forth binding would return to, in **any** workspace?
    pub fn is_previous_pane(&self, pane: PaneId) -> bool {
        self.selection.previous_panes.get().contains(&Some(pane))
    }
    /// The workspace a back-and-forth binding would return to.
    pub fn previous_ws(&self) -> Option<usize> {
        self.selection.previous_ws.get()
    }
    /// Publish it, from `AppState::last_visited_ws_idx`.
    pub fn set_previous_ws(&self, ws: Option<usize>) {
        if self.selection.previous_ws.get_untracked() != ws {
            self.selection.previous_ws.set(ws);
        }
    }
    /// Publish the set; the host reads `AppState::last_visited_pane_per_ws`, the one source the
    /// bindings themselves use.
    pub fn set_previous_panes(&self, panes: Vec<Option<PaneId>>) {
        if self.selection.previous_panes.get_untracked() != panes {
            self.selection.previous_panes.set(panes);
        }
    }
    /// Whether renamed panes show the `(process)` suffix in their sidebar card. Read
    /// untracked — consumed by `pane_card` while building the retained tree, not inside
    /// a reactive paint closure.
    pub fn pane_renamed_add_process_name(&self) -> bool {
        self.pane_renamed_add_process_name.get_untracked()
    }
    /// The setting as a signal, for a row that follows it rather than reading it once while its tree
    /// is built — so a reload reaches every row without a rebuild.
    pub(crate) fn pane_renamed_add_process_name_signal(&self) -> Signal<bool> {
        self.pane_renamed_add_process_name
    }

    /// The program catalog this component resolves process icons and names through. Read untracked —
    /// consumed while building the retained tree, not inside a reactive paint closure.
    pub fn programs(&self) -> std::rc::Rc<heca_config::programs::ProgramsConfig> {
        self.programs.get_untracked()
    }
    /// Is workspace `ws_idx` collapsed? (Borrows — no clone.)
    pub fn is_ws_collapsed(&self, ws_idx: usize) -> bool {
        self.collapsed_ws.with(|s| s.contains(&ws_idx))
    }
    /// Borrow the collapsed-ws set without cloning it.
    pub fn with_collapsed_ws<R>(&self, f: impl FnOnce(&HashSet<usize>) -> R) -> R {
        self.collapsed_ws.with(f)
    }
    /// The in-progress keyboard pick (move/select/swap/take), if any.
    pub fn pending_pick(&self) -> Option<PendingPick> {
        self.pending_pick.get_untracked()
    }

    // ── Writes ──
    pub fn set_active_pane(&self, pane: Option<PaneId>) {
        if self.selection.active_pane.get_untracked() == pane {
            return;
        }
        self.selection.active_pane.set(pane);
        self.events.emit(ChromeEvent::PaneActiveChanged { pane });
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
    /// Mirror the program catalog into the store. Called from `sync_chrome_state`, so a config
    /// reload reaches the component; guarded on the `Rc` identity, which is what changes when a
    /// reload replaces the catalog, so an unchanged one costs a pointer comparison.
    pub fn set_programs(&self, programs: std::rc::Rc<heca_config::programs::ProgramsConfig>) {
        if std::rc::Rc::ptr_eq(&self.programs.get_untracked(), &programs) {
            return;
        }
        self.programs.set(programs);
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
