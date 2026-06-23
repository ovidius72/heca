// The host API is a foundation **seam** (Phase 8): it is exercised by tests and
// will be consumed by first-party providers + the WASM bridge in later phases, so
// its methods read as dead code in the binary today. Mirrors `chrome/events.rs`
// (the Phase 0 event bus) which carries the same allow for the same reason.
#![allow(dead_code)]

//! First-party **host API** facade — the seam first-party providers (and the
//! future WASM plugin bridge) use to observe and read app state, mirroring §3.5 of
//! `pluggable-chrome-plugin-plan.md`:
//!
//! - [`App::on`] — `app.on(event, handler)`: subscribe to a typed [`ChromeEvent`]
//!   by string name (or `"*"` for all), getting an RAII subscription back.
//! - [`App::state`] — `app.state.*`: read-only selectors over the reactive chrome
//!   store.
//!
//! Today this wraps the in-process [`SharedChromeState`] (the Phase 0 event bus +
//! reactive store). The same surface is what the WASM bridge will marshal across
//! the host boundary later, so plugins never touch the internal `floem_reactive`
//! signals directly — they react via events and read via these selectors. The
//! remaining host-API namespaces from §3.5 (`app.actions`, `app.overlay`,
//! `app.regions`) are later phases and intentionally not implemented here.

use crate::chrome::{ChromeEvent, ChromeSubscription, SharedChromeState};
use heca_core::layout::PaneId;
use heca_core::runtime::{PaneRuntime, ProcessStatus};

/// A first-party handle to the host API. Cheap to clone — the underlying store is
/// `Rc`/signal-backed — so providers can each hold their own `App`.
#[derive(Clone)]
pub struct App {
    state: SharedChromeState,
}

impl App {
    /// Build a host handle over the app's shared chrome state.
    pub fn new(state: &SharedChromeState) -> Self {
        Self {
            state: state.clone(),
        }
    }

    /// Subscribe to a typed event by **name** (e.g. `"pane.status.changed"`) or
    /// `"*"` for every event. `handler` runs on the UI thread each time a matching
    /// event is emitted through the store's mutation chokepoint. The returned
    /// [`ChromeSubscription`] unsubscribes on drop — keep it alive for as long as
    /// the provider should listen.
    ///
    /// Event names are the stable strings from [`ChromeEvent::name`].
    #[must_use = "dropping the subscription immediately unsubscribes"]
    pub fn on(
        &self,
        event: impl Into<String>,
        handler: impl FnMut(&ChromeEvent) + 'static,
    ) -> ChromeSubscription {
        self.state.events().subscribe(event, handler)
    }

    /// Read-only state selectors (`app.state.*`).
    pub fn state(&self) -> StateView<'_> {
        StateView { state: &self.state }
    }
}

/// Read-only view over the chrome store — the `app.state.*` selectors. Each read
/// is a cheap signal/`get` (or a snapshot for the per-pane runtime). Providers read
/// *after* an event tells them something changed; they never mutate through here.
pub struct StateView<'a> {
    state: &'a SharedChromeState,
}

impl StateView<'_> {
    /// The currently active (focused) pane, if any.
    pub fn active_pane(&self) -> Option<PaneId> {
        self.state.workspaces.active_pane()
    }

    /// A snapshot of a pane's runtime (program, status, cwd, git, kind, exit code).
    /// `None` if the pane has no mirrored runtime.
    pub fn pane_runtime(&self, pane: PaneId) -> Option<PaneRuntime> {
        self.state.workspaces.pane_runtime(pane)
    }

    /// Convenience: a pane's current [`ProcessStatus`] (`None` if unknown).
    pub fn pane_status(&self, pane: PaneId) -> Option<ProcessStatus> {
        self.state.workspaces.pane_runtime(pane).map(|r| r.status)
    }

    /// A pane's user-set custom name (from rename), if any. `None` means the pane's
    /// name tracks its running process. Observe changes via `PaneCustomNameChanged`.
    pub fn pane_custom_name(&self, pane: PaneId) -> Option<String> {
        self.state.workspaces.pane_custom_name(pane)
    }

    /// The keyboard pick currently in progress (move/select/swap/take), if any — its
    /// kind + human prompt. Observe changes via `PendingPickChanged` to render a custom
    /// prompt UI for the pending action.
    pub fn pending_pick(&self) -> Option<crate::app_state::PendingPick> {
        self.state.workspaces.pending_pick()
    }

    /// Is workspace `ws_idx` collapsed in the sidebar?
    pub fn is_workspace_collapsed(&self, ws_idx: usize) -> bool {
        self.state.workspaces.is_ws_collapsed(ws_idx)
    }

    /// Is the left sidebar region visible (not hidden)?
    pub fn left_sidebar_visible(&self) -> bool {
        self.state.left_visible()
    }

    /// Is the right sidebar region visible (not hidden)?
    pub fn right_sidebar_visible(&self) -> bool {
        self.state.right_visible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::SharedChromeState;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn store() -> SharedChromeState {
        SharedChromeState::new(300.0, true, 300.0, false)
    }

    #[test]
    fn on_delivers_typed_and_catch_all_events() {
        let app = App::new(&store());
        let typed: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let all: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        let t = typed.clone();
        let _typed = app.on("pane.active.changed", move |e| {
            t.borrow_mut().push(e.name().into())
        });
        let a = all.clone();
        let _all = app.on("*", move |e| a.borrow_mut().push(e.name().into()));

        // Drive a store mutation through the public write path — it emits the event.
        app.state.workspaces.set_active_pane(Some(PaneId(7)));

        assert_eq!(typed.borrow().as_slice(), ["pane.active.changed"]);
        assert_eq!(all.borrow().as_slice(), ["pane.active.changed"]);
    }

    #[test]
    fn dropping_the_subscription_stops_delivery() {
        let app = App::new(&store());
        let seen: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
        let s = seen.clone();
        let sub = app.on("*", move |_| *s.borrow_mut() += 1);
        app.state.workspaces.set_active_pane(Some(PaneId(1)));
        assert_eq!(*seen.borrow(), 1);
        drop(sub);
        app.state.workspaces.set_active_pane(Some(PaneId(2)));
        assert_eq!(*seen.borrow(), 1, "no delivery after unsubscribe");
    }

    #[test]
    fn provider_subscribes_then_reads_state_end_to_end() {
        // A trivial first-party "provider": on a status change, record the pane's
        // status read back through the state selector (the event→read round-trip).
        type Observed = Rc<RefCell<Vec<(PaneId, Option<ProcessStatus>)>>>;
        let app = App::new(&store());
        let observed: Observed = Rc::new(RefCell::new(Vec::new()));

        let app_for_handler = app.clone();
        let log = observed.clone();
        let _sub = app.on("pane.status.changed", move |event| {
            if let ChromeEvent::PaneStatusChanged { pane, .. } = event {
                let status = app_for_handler.state().pane_status(*pane);
                log.borrow_mut().push((*pane, status));
            }
        });

        // Seed the pane runtime, then change its status via the store write path.
        let pane = PaneId(3);
        app.state
            .workspaces
            .set_pane_status(pane, ProcessStatus::Running);

        assert_eq!(
            observed.borrow().as_slice(),
            [(pane, Some(ProcessStatus::Running))],
        );
        // The selector also reads the same value directly.
        assert_eq!(app.state().pane_status(pane), Some(ProcessStatus::Running));
        assert_eq!(app.state().active_pane(), None);
    }
}
