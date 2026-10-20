//! **Keeping the retained chrome in step with the session**.
//!
//! Two directions, and the split between them is the point:
//!
//! - [`sync_chrome_state`] pushes the **session** into the reactive store — what is active, what is
//!   collapsed, what a pane is running. The store is what components and plugins read.
//! - [`sync_chrome_signals`] pushes the **store** into the retained widget tree, through the signals
//!   a component registered while building itself. This is what lets focus move without rebuilding
//!   anything: `chrome_signature` deliberately excludes focus, so a state read at build time would
//!   freeze for the session.
//!
//! [`ChromeSignals`] is the list a component fills in as it builds. **A state that changes with
//! focus must be published here**, a line after the value it was built with — the mistake that
//! produced three bugs in one day.
//!
//! Split out of a 5236-line `chrome/mod.rs` that held every kind of logic at once.

use crate::chrome::*;

/// Handles to the retained chrome tree's **value** signals — the state that changes
/// without a structural change (pane/column selection + status text). Collected
/// during [`build_chrome_root`] and pushed each frame by [`sync_chrome_signals`], so
/// these values are **not** in [`chrome_signature`] and focus changes no longer
/// rebuild the tree. Rebuilt with the tree on structural change.
#[derive(Default)]
pub(crate) struct ChromeSignals {
    /// Each pane card's `active` signal, keyed by pane id.
    pub(crate) pane_active: Vec<(PaneId, Signal<bool>)>,
    /// **"You were just here"** per pane — the mark back-and-forth would return to. Bound like
    /// [`pane_active`](Self::pane_active) because it changes on *focus*, which deliberately does
    /// not rebuild the tree (`chrome_signature` excludes it): read as a plain bool at build time it
    /// would freeze at whatever it was when the sidebar was last rebuilt.
    pub(crate) pane_previous: Vec<(PaneId, Signal<bool>)>,
    /// Each column [`MarkerGroup`](heca_grid_ui::widgets::MarkerGroup)'s `active` signal + the pane ids it
    /// holds (active iff it contains the active pane).
    pub(crate) col_active: Vec<(Vec<PaneId>, Signal<bool>)>,
    /// Each workspace [`DockFrame`](heca_grid_ui::widgets::DockFrame)'s `active` signal + the pane ids it holds (active
    /// iff it contains the active pane). Drives the active-workspace accent wash in
    /// place, mirroring [`col_active`](ChromeSignals::col_active).
    pub(crate) ws_active: Vec<(Vec<PaneId>, Signal<bool>)>,
    /// **The workspace `prefix+Shift+i` would return to**, by index. Bound like
    /// [`ws_active`](Self::ws_active) because it changes on focus, which does not rebuild the tree.
    pub(crate) ws_previous: Vec<(usize, Signal<bool>)>,
    /// Every navigable row's cursor-outline signal, as `(mount, key, signal)` — driven from
    /// that **mount's** cursor (`container_cursor`), which is why the mount is part of the key.
    ///
    /// Generic: this used to be two domain-keyed families (`pane_nav` by
    /// `PaneId`, `ws_nav` by `ws_idx`), which no component outside the workspace tree could join. A
    /// row now declares one identity (`Base::key`) and this is the highlight half of it; the
    /// cursor highlight stays distinct from `active_pane`, as it always was.
    pub(crate) row_nav: Vec<(String, String, Signal<bool>)>,
    /// The status-bar label's text signal.
    pub(crate) status: Option<Signal<String>>,
}

pub(crate) fn sync_pane_runtime_state(
    session: &heca_core::layout::Session,
    panes: &crate::chrome::PaneRuntimeStore,
) -> bool {
    use std::collections::HashSet;
    // Every frame, every pane is compared with the store. Only a real change sets a signal or
    // announces, so an idle frame repaints nothing; the cost is the scan itself.
    let mut live_panes = HashSet::new();
    let mut changed = false;
    for pane in session.workspaces.iter().flat_map(|ws| ws.panes()) {
        live_panes.insert(pane.id);
        changed |= panes.set_runtime(pane.id, &pane.runtime, pane.custom_name.as_deref());
    }
    panes.retain(&live_panes);
    changed
}

/// Mirror canonical app/runtime state into the shared chrome store before the
/// retained tree reads it. `InputMode` remains the source of truth for keyboard
/// pick flows; the store is the reactive UI mirror.
pub(crate) fn sync_chrome_state(state: &mut crate::app_state::AppState) -> bool {
    state
        .chrome_state
        .workspaces
        .set_active_pane(state.focused_pane);
    state
        .chrome_state
        .workspaces
        .set_pane_renamed_add_process_name(state.settings.pane_renamed_add_process_name);
    // Which lines show under a pane's name, by name — the user's list, plus what other crates added
    // while it is still the default.
    state
        .chrome_state
        .workspaces
        .set_pane_lines(state.pane_lines.shown(&state.appearance.sidebar.pane_lines));
    // The program catalog is this component's, not the shared context's. Mirrored
    // here like the display flags above, so `prefix+Shift+r` reaches it; guarded on `Rc` identity,
    // which is exactly what a reload replaces.
    state
        .chrome_state
        .workspaces
        .set_programs(state.server.programs.clone());
    // Mirror the in-progress pick (its kind + prompt) into the store so components and
    // plugins can react to the pending action (e.g. a custom prompt overlay).
    let pending_pick = state.input_mode.pending_pick(&state.action_catalog);
    state.chrome_state.workspaces.set_pending_pick(pending_pick);
    // Bridge backend-detected runtime → canonical `Pane.runtime` + emit
    // `pane.exited{code}` BEFORE mirroring `Pane.runtime` into the store.
    crate::app::process_monitor::sync_pane_runtime_from_backends(state);
    crate::app::git_monitor::sync_pane_git_from_cwds(state);
    sync_pane_runtime_state(&state.session, &state.chrome_state.panes)
}

/// Push the chrome's value-state (selection + status text) into the retained tree's
/// bound signals. Guarded — writes only on change, so unchanged frames cause no
/// signal churn. Called each frame before paint; this is what lets focus changes
/// update the highlight + status **without** rebuilding the tree.
pub(crate) fn sync_chrome_signals(state: &crate::app_state::AppState) {
    let Some(retained) = state.chrome_tree.as_ref() else {
        return;
    };
    let active = state.chrome_state.workspaces.active_pane();
    for (pid, sig) in &retained.signals.pane_active {
        let v = active == Some(*pid);
        if sig.get_untracked() != v {
            sig.set(v);
        }
    }
    // The same sync for the back-and-forth mark, from the field `prefix+i` itself reads.
    for (pid, sig) in &retained.signals.pane_previous {
        let v = state.chrome_state.workspaces.is_previous_pane(*pid) && active != Some(*pid);
        if sig.get_untracked() != v {
            sig.set(v);
        }
    }
    // The workspace back-and-forth would take you to — the frame says *which* workspace, the pane
    // mark inside it says which pane. Two levels of one answer.
    let previous_ws = state.chrome_state.workspaces.previous_ws();
    for (idx, sig) in &retained.signals.ws_previous {
        let v = Some(*idx) == previous_ws;
        if sig.get_untracked() != v {
            sig.set(v);
        }
    }
    for (pids, sig) in &retained.signals.col_active {
        let v = active.is_some_and(|a| pids.contains(&a));
        if sig.get_untracked() != v {
            sig.set(v);
        }
    }
    for (pids, sig) in &retained.signals.ws_active {
        let v = active.is_some_and(|a| pids.contains(&a));
        if sig.get_untracked() != v {
            sig.set(v);
        }
    }
    // Project each mount's cursor onto its rows' cursor signals — distinct from `active` above, so
    // the expanded sidebar shows both the real focus and the cursor. Keyed by `(mount, key)`,
    // so two placements of one container light **different** rows.
    for (mount, key, sig) in &retained.signals.row_nav {
        let v = state
            .chrome_state
            .container_cursor(mount)
            .get_untracked()
            .as_deref()
            == Some(key.as_str());
        if sig.get_untracked() != v {
            sig.set(v);
        }
    }
    // **The letters are not projected from here any more**.
    //
    // They were: four per-widget signal lists written every frame, which wrote `None` over any
    // letter the universal picker had offered — so `prefix+/` lit nothing in the sidebar while
    // working perfectly over a layer, which is not synced from here. `chrome::hint` owns them now,
    // by the one rule that does not need a list of modes: **whoever offers a letter owns it until
    // they withdraw it.** A plugin's own picker is safe for the same reason heca's is.
    crate::chrome::hint::sync_offered_letters(state);
    if let Some(status) = &retained.signals.status {
        let next = chrome_status(state);
        if status.get_untracked() != next {
            status.set(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::testing::Windowed;
    use heca_core::runtime::{ContentKind, GitInfo, PaneRuntime, ProcessStatus};
    use std::path::PathBuf;

    #[test]
    fn sync_pane_runtime_state_projects_session_runtime_into_store() {
        let chrome = SharedChromeState::new(280.0, 260.0);
        let mut window = Windowed::new(Size::new(1280.0, 800.0), 1.0);
        {
            window.m().add_pane(
                heca_core::layout::Pane::new(PaneId(10), "editor"),
                None,
                true,
            );
            let ws = window
                .session
                .workspaces
                .first_mut()
                .expect("session should create an initial workspace");
            ws.add_floating_pane(
                heca_core::layout::Pane::new(PaneId(20), "git"),
                Rectangle::new(Point::new(50.0, 50.0), Size::new(400.0, 300.0)),
                None,
            );
            ws.find_pane_mut(PaneId(10)).expect("tiled pane").runtime = PaneRuntime {
                program: Some("nvim".into()),
                status: ProcessStatus::Running,
                cwd: Some(PathBuf::from("/tmp/project")),
                exit_code: Some(0),
                git: Some(GitInfo {
                    branch: Some("main".into()),
                    ahead: 1,
                    behind: 0,
                    added: 2,
                    modified: 3,
                    deleted: 4,
                    dirty: true,
                }),
                kind: ContentKind::Terminal,
            };
            ws.find_pane_mut(PaneId(20)).expect("floating pane").runtime = PaneRuntime {
                program: Some("lazygit".into()),
                status: ProcessStatus::Idle,
                cwd: Some(PathBuf::from("/tmp/project")),
                exit_code: None,
                git: None,
                kind: ContentKind::Terminal,
            };
        }

        super::sync_pane_runtime_state(&window.session, &chrome.panes);

        let tiled = chrome.panes.with(PaneId(10), |runtime| {
            runtime.expect("tiled runtime").snapshot()
        });
        let floating = chrome.panes.with(PaneId(20), |runtime| {
            runtime.expect("floating runtime").snapshot()
        });
        assert_eq!(tiled.program.as_deref(), Some("nvim"));
        assert_eq!(tiled.status, ProcessStatus::Running);
        assert_eq!(tiled.cwd, Some(PathBuf::from("/tmp/project")));
        assert_eq!(
            tiled.git,
            Some(GitInfo {
                branch: Some("main".into()),
                ahead: 1,
                behind: 0,
                added: 2,
                modified: 3,
                deleted: 4,
                dirty: true,
            })
        );
        assert_eq!(floating.program.as_deref(), Some("lazygit"));
        assert_eq!(floating.status, ProcessStatus::Idle);
    }

    #[test]
    fn sync_pane_runtime_state_prunes_removed_panes() {
        let chrome = SharedChromeState::new(280.0, 260.0);
        let mut window = Windowed::new(Size::new(1280.0, 800.0), 1.0);
        window.m().add_pane(
            heca_core::layout::Pane::new(PaneId(10), "editor"),
            None,
            true,
        );

        super::sync_pane_runtime_state(&window.session, &chrome.panes);
        assert!(chrome.panes.with(PaneId(10), |runtime| runtime.is_some()));

        window
            .session
            .workspaces
            .first_mut()
            .expect("session should keep its workspace")
            .scrolling
            .columns
            .clear();
        super::sync_pane_runtime_state(&window.session, &chrome.panes);

        assert!(!chrome.panes.with(PaneId(10), |runtime| runtime.is_some()));
    }
}
