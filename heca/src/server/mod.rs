//! **The server's half of the app's state** — what is shared by every window and needs none.
//!
//! heca is a server and a client: the server holds the content (workspaces, panes, the
//! terminal processes behind them, notifications) and a window — a client — draws it and moves
//! through it by itself. `AppState` used to hold both in one struct beside the window and the GPU.
//! This is the first piece taken out of it: the state that **cannot need a window**, so it can be
//! built and driven without one.
//!
//! What lives here is what the server owns: the terminal processes ([`BackendStore`]) and what
//! starting one takes — it starts and ends them itself ([`Terminals`]) — the notification store
//! ([`NotificationRuntime`]), the git facts it keeps for panes ([`GitRuntimeCache`]) and the program
//! catalog a terminal is started from. The session is still kept beside it in `AppState`, but an
//! action reaches it only as content ([`ServerLayout`]), never through a window's view.
//!
//! Standalone heca is this and one client in one process, so there is one way to reach any of it —
//! `state.server.<part>` — whether the caller is a window or, later, a connection.

mod action;
mod bootstrap;
mod change;
mod cx;
mod handlers;
mod layout;
mod poll;
pub(crate) mod feedback;
pub(crate) mod process;
/// The local socket clients and scripts reach the server through. Unix for now (Windows named
/// pipes later).
#[cfg(unix)]
pub(crate) mod socket;
pub(crate) mod run;
pub(crate) mod runtime;
mod terminals;
mod stream;
mod session;
mod connection;
pub(crate) mod sessions;
#[cfg(test)]
mod testing;
mod waker;
pub(crate) mod wire;

use std::rc::Rc;
use std::time::Instant;

use heca_config::programs::ProgramsConfig;

pub(crate) use action::{NotificationCommand, NotificationSettings, ServerAction};
pub(crate) use change::{Change, Refusal};
pub(crate) use cx::{Asker, ServerCx};
pub(crate) use layout::ServerLayout;
pub(crate) use poll::BackendPoll;
pub(crate) use terminals::Terminals;
pub(crate) use waker::ServerWaker;

use crate::app::backend_factory::{Launch, TerminalSettings, launch, terminal_palette_defaults};
use crate::app::backend_store::BackendStore;
use crate::app::git_monitor::GitRuntimeCache;
use crate::notification::NotificationRuntime;

/// The state every window of a session shares. Built without a window.
pub struct ServerState {
    pub(crate) feedback: feedback::Feedback,
    /// The terminal processes — the one owner of them (`BackendStore::ensure` is the one door in).
    pub backends: BackendStore,
    /// What the configuration says a terminal process starts with.
    terminal_settings: TerminalSettings,
    /// How a terminal process wakes the loop when its output changes. `None` where there is no loop
    /// to wake, as in a test.
    waker: Option<ServerWaker>,
    /// How a terminal process is built.
    launch: Launch,
    /// The notification store: queueing, lifecycle, dedup, timers, and the list a window shows.
    notifications: NotificationRuntime,
    /// Git facts for panes' folders, kept by repo root so a repo shared by panes is read once.
    git_runtime_cache: GitRuntimeCache,
    /// The program catalog (`[program]`), behind an `Rc` so mirroring it into the chrome store is
    /// a pointer clone and the store can tell "unchanged" from "reloaded" by identity.
    /// Refreshed on reload.
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
                // `mode = "system"` has no OS backend yet: fall back to the in-app
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
            ServerAction::ConfigureTerminals(settings) => {
                self.configure_terminals(*settings);
                return Vec::new();
            }
        };
        changed
            .then_some(Change::NotificationsChanged)
            .into_iter()
            .collect()
    }

    /// Whether the server runs this kind of action — it has been moved here from the window.
    pub(crate) fn runs(kind: crate::input::WmActionKind) -> bool {
        handlers::handler_for(kind).is_some() || sessions::owns(kind)
    }

    /// **Run a built-in action on the server** for `asker`, on `session`, and say what changed.
    /// Only for a kind the server [`runs`](Self::runs); anything else is a routing mistake, said out
    /// loud and ignored.
    pub(crate) fn run(
        &mut self,
        session: &mut heca_core::layout::Session,
        asker: Asker,
        action: &crate::input::WmAction,
    ) -> Vec<Change> {
        let mut cx = ServerCx {
            layout: ServerLayout::new(session),
            terminals: self.terminals(),
            asker,
        };
        match handlers::handler_for(action.kind()) {
            Some(handler) => handler(&mut cx, action),
            None => {
                debug_assert!(false, "{action:?} is not a server action");
                Vec::new()
            }
        }
    }

    /// The terminal processes and what starting one takes, for one request.
    pub(crate) fn terminals(&mut self) -> Terminals<'_> {
        Terminals {
            backends: &mut self.backends,
            settings: &self.terminal_settings,
            waker: self.waker.as_ref(),
            launch: self.launch,
        }
    }

    /// This server, building terminal processes with `launch` — a test's, which starts nothing.
    #[cfg(test)]
    pub(crate) fn launching_with(mut self, launch: Launch) -> Self {
        self.launch = launch;
        self
    }

    /// New terminal settings: every running process takes them, and every one started from now on
    /// starts with them.
    fn configure_terminals(&mut self, settings: TerminalSettings) {
        let palette = terminal_palette_defaults(&settings.theme);
        for backend in self.backends.values_mut() {
            if let Err(error) = backend.reload_agent_config(settings.agents.clone()) {
                crate::chrome::warn_author(format!("[heca] cannot update terminal agent reporting: {error}"));
            }
            backend.set_scroll_animations_enabled(settings.scroll_animations);
            backend.set_link_detection(settings.link_detection);
            backend.reload_terminal_config(palette, settings.scrollback);
        }
        self.terminal_settings = settings;
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

    /// **Bring every pane's git facts up to date** from its working directory, re-asking git only
    /// when a repository's cached answer is old. The session is passed in because the panes' runtime
    /// still lives in it (the step that splits the session's shared layout from each window's view
    /// moves it here).
    pub(crate) fn refresh_git(&mut self, session: &mut heca_core::layout::Session, now: Instant) {
        crate::app::git_monitor::refresh(session, &mut self.git_runtime_cache, now);
    }

    /// **When the server next has something to do** — the next notification deadline — or `None`
    /// while the stack is held still, because a frozen deadline would only be found not due and
    /// re-armed at the same instant.
    pub(crate) fn next_wake(&self) -> Option<Instant> {
        let notification = match self.notifications.is_hovered() {
            true => None,
            false => self.notifications.next_expiry(),
        };
        [notification, self.backends.next_wake()].into_iter().flatten().min()
    }

    /// **The notifications on show**, as plain serializable data — the projection the server sends
    /// a window, which builds its own toasts from it
    /// ([`toast_spec`](crate::notification::toast_spec)). Sent on attach and whenever
    /// [`Change::NotificationsChanged`] is produced.
    pub(crate) fn visible_notifications(&self) -> Vec<crate::notification::NotificationView> {
        self.notifications.visible_views()
    }

    pub(crate) fn new(
        backends: BackendStore,
        notifications: NotificationRuntime,
        programs: ProgramsConfig,
        terminal_settings: TerminalSettings,
        waker: Option<ServerWaker>,
    ) -> Self {
        Self {
            feedback: Default::default(),
            backends,
            terminal_settings,
            waker,
            launch,
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
    use std::time::Duration;

    fn server(mode: heca_config::settings::NotificationSystem) -> ServerState {
        ServerState::for_tests(mode)
    }

    fn on_screen(server: &ServerState) -> usize {
        server.visible_notifications().len()
    }

    /// **The server's state needs no window.** It is built from plain data, so a test — and later a
    /// server with no window at all — can hold and drive it.
    #[test]
    fn the_server_state_is_built_with_no_window() {
        let server = server(Default::default());
        assert!(
            server.backends.get(heca_core::layout::PaneId(1)).is_none(),
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

        assert_eq!(
            server.tick(deadline + Duration::from_millis(1)),
            [Change::NotificationsChanged]
        );
        assert_eq!(on_screen(&server), 0);
    }
}
