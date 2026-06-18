//! Per-wake monitor bridging backend-detected runtime → canonical `Pane.runtime`
//! (Phase 1) and emitting `pane.exited{code}` (Phase 2).
//!
//! Detection itself lives in `TerminalBackend::update` (event-driven on the
//! terminal-output / reader-EOF wake, debounced — **no periodic timer** per
//! `pane-runtime-state-plan.md` §0.2). This module only copies each backend's
//! cached `PaneBackend::runtime()` snapshot onto the canonical layout `Pane` and
//! drains one-shot exit codes via `take_exit_code`. The existing
//! `sync_pane_runtime_state` (in `chrome/mod.rs`) then mirrors `Pane.runtime` →
//! the chrome store and emits the per-field `pane.*.changed` events.

use crate::app::backend_store::BackendStore;
use crate::app_state::AppState;
use crate::chrome::{ChromeEvent, ChromeEventBus};
use heca_core::layout::{PaneId, Session};

/// Copy each live pane's backend `runtime()` → its canonical `Pane.runtime`, and
/// drain any one-shot exit codes → emit `pane.exited{code}`.
///
/// Called once per frame in `sync_chrome_state`, **before** `sync_pane_runtime_state`
/// so the store mirror sees fresh canonical values.
pub(crate) fn sync_pane_runtime_from_backends(state: &mut AppState) {
    let bus = state.chrome_state.events();
    sync_pane_runtime_from_backends_impl(&mut state.session, &mut state.backends, &bus);
}

/// Testable core of [`sync_pane_runtime_from_backends`] — takes the three pieces
/// the monitor touches directly (session, backends, event bus) so unit tests can
/// drive it without constructing a full GPU/window `AppState`.
fn sync_pane_runtime_from_backends_impl(
    session: &mut Session,
    backends: &mut BackendStore,
    bus: &ChromeEventBus,
) {
    let mut exits: Vec<(PaneId, Option<i32>)> = Vec::new();

    for ws in &mut session.workspaces {
        for col in &mut ws.scrolling.columns {
            for pane in &mut col.panes {
                if let Some(backend) = backends.get_mut(pane.id) {
                    pane.runtime = backend.runtime();
                    if let Some(code) = backend.take_exit_code() {
                        exits.push((pane.id, Some(code)));
                    }
                }
            }
        }
        for float in &mut ws.floating_panes {
            if let Some(backend) = backends.get_mut(float.pane.id) {
                float.pane.runtime = backend.runtime();
                if let Some(code) = backend.take_exit_code() {
                    exits.push((float.pane.id, Some(code)));
                }
            }
        }
    }

    // Emit `pane.exited{code}` for backends that exited this wake. Done after the
    // session/backends borrows end; the bus is `Rc`-shared, so emitting reaches the
    // same subscribers as the store setters.
    for (pane, code) in exits {
        bus.emit(ChromeEvent::PaneExited { pane, code });
    }
}

#[cfg(test)]
mod tests {
    use super::sync_pane_runtime_from_backends_impl;
    use crate::app::backend_store::BackendStore;
    use crate::chrome::{ChromeEvent, ChromeEventBus};
    use heca_core::backend::FakeBackend;
    use heca_core::layout::{
        workspace::FloatingPane, ColumnWidth, LayoutOptions, Pane, PaneId, Point, Session,
        SessionId, Size,
    };
    use heca_core::runtime::{ContentKind, PaneRuntime, ProcessStatus};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Witness type for captured `pane.exited{code}` events in tests — kept named so
    /// the test body stays readable without a `clippy::type_complexity` allow.
    type ExitWitness = Rc<RefCell<Vec<(PaneId, Option<i32>)>>>;

    fn session_with_tiled_pane(id: PaneId) -> Session {
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        let ws = session
            .active_workspace_mut()
            .expect("session should create an initial workspace");
        ws.add_pane(
            Pane::new(id, "editor"),
            None,
            true,
            ColumnWidth::Proportion(0.5),
        );
        session
    }

    fn runtime_nvim() -> PaneRuntime {
        PaneRuntime {
            program: Some("nvim".into()),
            status: ProcessStatus::Running,
            kind: ContentKind::Terminal,
            ..PaneRuntime::default()
        }
    }

    #[test]
    fn monitor_copies_backend_runtime_into_canonical_pane() {
        let pid = PaneId(11);
        let mut session = session_with_tiled_pane(pid);
        let mut backends = BackendStore::new();
        let mut fake = FakeBackend::new(20, 6);
        fake.set_runtime(runtime_nvim());
        backends.insert_for_pane(pid, Box::new(fake));
        let bus = ChromeEventBus::default();

        sync_pane_runtime_from_backends_impl(&mut session, &mut backends, &bus);

        let pane_runtime = session
            .active_workspace()
            .and_then(|ws| ws.find_pane(pid))
            .map(|p| p.runtime.clone())
            .expect("pane should exist");
        assert_eq!(pane_runtime.program.as_deref(), Some("nvim"));
        assert_eq!(pane_runtime.status, ProcessStatus::Running);
        assert_eq!(pane_runtime.kind, ContentKind::Terminal);
    }

    #[test]
    fn monitor_emits_pane_exited_once_when_exit_code_queued() {
        let pid = PaneId(12);
        let mut session = session_with_tiled_pane(pid);
        let mut backends = BackendStore::new();
        let mut fake = FakeBackend::new(20, 6);
        fake.queue_exit(42);
        backends.insert_for_pane(pid, Box::new(fake));
        let bus = ChromeEventBus::default();

        let seen: ExitWitness = Rc::new(RefCell::new(Vec::new()));
        let seen_c = seen.clone();
        let _sub = bus.subscribe("pane.exited", move |event| {
            if let ChromeEvent::PaneExited { pane, code } = event {
                seen_c.borrow_mut().push((*pane, *code));
            }
        });

        sync_pane_runtime_from_backends_impl(&mut session, &mut backends, &bus);
        // A second wake must NOT re-emit: take_exit_code drains once.
        sync_pane_runtime_from_backends_impl(&mut session, &mut backends, &bus);

        assert_eq!(seen.borrow().as_slice(), &[(pid, Some(42))]);
    }

    #[test]
    fn monitor_skips_panes_without_a_backend() {
        let pid = PaneId(13);
        let mut session = session_with_tiled_pane(pid);
        let mut backends = BackendStore::new();
        // No backend inserted for this pane.
        let bus = ChromeEventBus::default();

        // Must not panic and must leave the pane's runtime at its default.
        sync_pane_runtime_from_backends_impl(&mut session, &mut backends, &bus);
        let rt = session
            .active_workspace()
            .and_then(|ws| ws.find_pane(pid))
            .map(|p| p.runtime.clone())
            .expect("pane should exist");
        assert_eq!(rt, PaneRuntime::default());
    }

    #[test]
    fn monitor_copies_floating_pane_runtime() {
        let pid = PaneId(14);
        let mut session = session_with_tiled_pane(PaneId(99));
        {
            let ws = session
                .active_workspace_mut()
                .expect("workspace exists");
            ws.floating_panes.push(FloatingPane {
                pane: Pane::new(pid, "float"),
                position: Point::new(10.0, 10.0),
                size: Size::new(300.0, 200.0),
                is_active: false,
                original_column_idx: None,
                original_pane_idx: None,
            });
        }
        let mut backends = BackendStore::new();
        let mut fake = FakeBackend::new(20, 6);
        fake.set_runtime(PaneRuntime {
            program: Some("lazygit".into()),
            status: ProcessStatus::Running,
            kind: ContentKind::Terminal,
            ..PaneRuntime::default()
        });
        backends.insert_for_pane(pid, Box::new(fake));
        let bus = ChromeEventBus::default();

        sync_pane_runtime_from_backends_impl(&mut session, &mut backends, &bus);

        let rt = session
            .active_workspace()
            .and_then(|ws| ws.floating_panes.iter().find(|f| f.pane.id == pid))
            .map(|f| f.pane.runtime.clone())
            .expect("floating pane should exist");
        assert_eq!(rt.program.as_deref(), Some("lazygit"));
        assert_eq!(rt.status, ProcessStatus::Running);
    }
}