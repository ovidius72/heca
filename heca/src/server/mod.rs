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

mod action;
mod change;

use std::rc::Rc;
use std::time::Instant;

use heca_config::programs::ProgramsConfig;

pub(crate) use action::{NotificationSettings, ServerAction};
pub(crate) use change::Change;

use crate::app::backend_store::BackendStore;
use crate::app::git_monitor::GitRuntimeCache;
use crate::notification::NotificationRuntime;

/// The state every window of a session shares. Built without a window.
pub struct ServerState {
    /// The terminal processes — the one owner of them (`BackendStore::ensure` is the one door in).
    pub backends: BackendStore,
    /// The notification store: queueing, lifecycle, dedup, timers, and the list a window shows.
    notifications: NotificationRuntime,
    /// Git facts for panes' folders, kept by repo root so a repo shared by panes is read once.
    pub git_runtime_cache: GitRuntimeCache,
    /// The program catalog (`[program]`), behind an `Rc` so mirroring it into the chrome store is
    /// a pointer clone and the store can tell "unchanged" from "reloaded" by identity
    /// (F003/P086/T367). Refreshed on reload.
    pub programs: Rc<ProgramsConfig>,
}

impl ServerState {
    /// Do what a window asked, and say what changed. The one door a command to the server goes
    /// through; `now` is the server's own clock reading, never the asker's.
    pub(crate) fn execute(&mut self, action: ServerAction, now: Instant) -> Vec<Change> {
        let changed = match action {
            ServerAction::Raise(draft) => {
                // `mode = "none"` drops it before it reaches the store: nothing queues, no timer.
                if self.notifications.suppressed() {
                    return Vec::new();
                }
                // `mode = "system"` has no OS backend yet (F009/T222): fall back to the in-app
                // stack and say so once.
                if let Some(notice) = self.notifications.system_fallback_notice() {
                    let _ = self.notifications.push(notice, now);
                }
                let _ = self.notifications.push(draft, now);
                true
            }
            ServerAction::DismissOne(id) => {
                self.notifications.dismiss_one(id, now);
                true
            }
            ServerAction::DismissLast => {
                self.notifications.dismiss_last(now);
                true
            }
            ServerAction::DismissAll => {
                self.notifications.dismiss_all(now);
                true
            }
            ServerAction::Hover(hovered) => self.notifications.set_hovered(hovered, now),
            ServerAction::Configure(settings) => {
                self.notifications.set_auto_dismiss(settings.auto_dismiss);
                self.notifications.set_mode(settings.mode);
                // Whether the cards on show moved is the only part a window has to redraw for.
                self.notifications
                    .set_max_visible(settings.max_visible, now)
            }
        };
        changed
            .then_some(Change::NotificationsChanged)
            .into_iter()
            .collect()
    }

    /// Let time pass: notifications past their deadline are dismissed. Called every turn of the
    /// loop; says nothing unless something changed.
    pub(crate) fn tick(&mut self, now: Instant) -> Vec<Change> {
        self.notifications
            .expire_due(now)
            .then_some(Change::NotificationsChanged)
            .into_iter()
            .collect()
    }

    /// **When the server next has something to do** — the next notification deadline — or `None`
    /// while the stack is held still, because a frozen deadline would only be found not due and
    /// re-armed at the same instant.
    pub(crate) fn next_wake(&self) -> Option<Instant> {
        match self.notifications.is_hovered() {
            true => None,
            false => self.notifications.next_expiry(),
        }
    }

    /// **The notifications on show**, as the list a window draws. A signal: it is the window's
    /// projection of the server's store, rewritten when the store changes.
    pub(crate) fn toasts(
        &self,
    ) -> heca_grid_ui::reactive::Signal<Vec<heca_grid_ui::widgets::ToastSpec>> {
        self.notifications.visible_toasts
    }

    /// What pressing `key` on the notification `id` does, and whether it dismisses after — or
    /// `None` when that card is not on show.
    pub(crate) fn toast_action(
        &self,
        id: crate::notification::NotificationId,
        key: &str,
    ) -> Option<(crate::chrome::Intent, bool)> {
        self.notifications
            .action_and_dismiss_after_for_visible(id, key)
    }

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
    use crate::notification::NotificationDraft;
    use heca_grid_ui::reactive::SignalGet;
    use std::time::Duration;

    fn server(mode: heca_config::settings::NotificationSystem) -> ServerState {
        ServerState::new(
            BackendStore::new(),
            NotificationRuntime::with_capacity(50, Duration::from_millis(4000), mode, 5),
            ProgramsConfig::default(),
        )
    }

    fn on_screen(server: &ServerState) -> usize {
        server.toasts().get_untracked().len()
    }

    /// **The server's state needs no window.** It is built from plain data, so a test — and later a
    /// server with no window at all — can hold and drive it.
    #[test]
    fn the_server_state_is_built_with_no_window() {
        let server = server(Default::default());
        assert!(
            server
                .backends
                .terminal_of(heca_core::layout::PaneId(1))
                .is_none(),
            "no terminal process yet"
        );
        assert_eq!(Rc::strong_count(&server.programs), 1);
        assert_eq!(on_screen(&server), 0);
    }

    /// **A command in, a change out, no window**: raising puts a card on show and says so; dismissing
    /// takes it off and says so; doing nothing says nothing.
    #[test]
    fn raising_and_dismissing_a_notification_reports_what_changed() {
        let mut server = server(Default::default());
        let now = Instant::now();
        let raised = server.execute(ServerAction::Raise(NotificationDraft::new("Saved")), now);
        assert_eq!(raised, [Change::NotificationsChanged]);
        assert_eq!(on_screen(&server), 1);

        let dismissed = server.execute(ServerAction::DismissAll, now);
        assert_eq!(dismissed, [Change::NotificationsChanged]);
        assert_eq!(on_screen(&server), 0);
        assert!(server.tick(now).is_empty(), "nothing due, nothing said");
    }

    /// `mode = "none"` is the server's rule: the draft is dropped before it reaches the store.
    #[test]
    fn a_notification_is_dropped_when_the_mode_says_nowhere() {
        let mut server = server(heca_config::settings::NotificationSystem::None);
        let changes = server.execute(
            ServerAction::Raise(NotificationDraft::new("Saved")),
            Instant::now(),
        );
        assert!(changes.is_empty());
        assert_eq!(on_screen(&server), 0);
    }

    /// **Time is the server's too**: a card past its deadline is dismissed by `tick`, and while the
    /// pointer rests on the stack nothing is due and no wake is asked for.
    #[test]
    fn a_notification_expires_with_time_and_a_held_stack_waits() {
        let mut server = server(Default::default());
        let start = Instant::now();
        let _ = server.execute(ServerAction::Raise(NotificationDraft::new("Saved")), start);
        let deadline = server.next_wake().expect("it will expire");

        let _ = server.execute(ServerAction::Hover(true), start);
        assert_eq!(server.next_wake(), None, "held still: no wake");
        let _ = server.execute(ServerAction::Hover(false), start);

        assert_eq!(server.tick(deadline + Duration::from_millis(1)), [Change::NotificationsChanged]);
        assert_eq!(on_screen(&server), 0);
    }
}
