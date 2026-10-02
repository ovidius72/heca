//! **The server's half of the app's state** — what is shared by every window and needs none.
//!
//! heca is a server and a client (F012): the server holds the content (workspaces, panes, the
//! terminal processes behind them, notifications) and a window — a client — draws it and moves
//! through it by itself. `AppState` used to hold both in one struct beside the window and the GPU.
//! This is the first piece taken out of it: the state that **cannot need a window**, so it can be
//! built and driven without one.
//!
//! What lives here is what the server owns: the terminal processes ([`BackendStore`]), the
//! notification store ([`NotificationRuntime`]), the git facts it keeps for panes
//! ([`GitRuntimeCache`]) and the program catalog a terminal is started from. The session's
//! layout is still in `AppState`: it mixes shared content with each window's own view, which is a
//! decision of its own (see `P104(F012)/T525`).
//!
//! Standalone heca is this and one client in one process, so there is one way to reach any of it —
//! `state.server.<part>` — whether the caller is a window or, later, a connection.

use std::rc::Rc;

use heca_config::programs::ProgramsConfig;

use crate::app::backend_store::BackendStore;
use crate::app::git_monitor::GitRuntimeCache;
use crate::notification::NotificationRuntime;

/// The state every window of a session shares. Built without a window.
pub struct ServerState {
    /// The terminal processes — the one owner of them (`BackendStore::ensure` is the one door in).
    pub backends: BackendStore,
    /// The notification store: queueing, lifecycle, dedup, timers, and the list a window shows.
    pub notifications: NotificationRuntime,
    /// Git facts for panes' folders, kept by repo root so a repo shared by panes is read once.
    pub git_runtime_cache: GitRuntimeCache,
    /// The program catalog (`[program]`), behind an `Rc` so mirroring it into the chrome store is
    /// a pointer clone and the store can tell "unchanged" from "reloaded" by identity
    /// (F003/P086/T367). Refreshed on reload.
    pub programs: Rc<ProgramsConfig>,
}

impl ServerState {
    pub fn new(
        backends: BackendStore,
        notifications: NotificationRuntime,
        programs: ProgramsConfig,
    ) -> Self {
        Self {
            backends,
            notifications,
            git_runtime_cache: GitRuntimeCache::default(),
            programs: Rc::new(programs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The server's state needs no window.** It is built from plain data, so a test — and later a
    /// server with no window at all — can hold and drive it.
    #[test]
    fn the_server_state_is_built_with_no_window() {
        let server = ServerState::new(
            BackendStore::new(),
            NotificationRuntime::with_capacity(
                50,
                std::time::Duration::from_millis(4000),
                Default::default(),
                5,
            ),
            ProgramsConfig::default(),
        );
        assert!(
            server
                .backends
                .terminal_of(heca_core::layout::PaneId(1))
                .is_none(),
            "no terminal process yet"
        );
        assert_eq!(Rc::strong_count(&server.programs), 1);
    }
}
