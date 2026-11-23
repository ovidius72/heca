//! **Saving the server's sessions and rebuilding them when it starts** — the loop's half of
//! [`persist`](crate::server::persist): what goes in the file, when it is written, and what is done
//! with what comes out.

use super::*;
use crate::notification::Notification;
use crate::server::persist::{Saved, SavedSession, SessionStore};
use heca_config::loader::AppConfig;

impl ServerLoop {
    /// Open this server's file, unless its sessions are not kept. Before the identity moves into
    /// the services, which is why it is not folded into [`restore_sessions`](Self::restore_sessions).
    pub(in crate::server) fn open_store(
        &mut self,
        identity: &crate::server::identity::ServerIdentity,
        config: &AppConfig,
    ) {
        self.store = SessionStore::open(identity, &config.config.settings);
    }

    /// **Rebuild every session the file holds**, each with its own key, name, folder, layout and
    /// whether it was running. A session that was running is started the way Restart starts one; a
    /// stopped one stays stopped until Restart. What could not be rebuilt is reported once.
    pub(in crate::server) fn restore_sessions(&mut self, config: &AppConfig) {
        let Some(store) = self.store.as_mut() else {
            return;
        };
        let crate::server::persist::Loaded {
            saved,
            mut problems,
        } = store.load();
        let Saved {
            next_session,
            recent,
            sessions,
        } = saved;
        self.next_session = self.next_session.max(next_session);
        let mut restored = Vec::new();
        for session in sessions {
            let (id, name) = (session.id, session.name.clone());
            match self.restore(session, config) {
                Ok(()) => restored.push(id),
                Err(why) => problems.push(format!("session '{name}' could not be rebuilt ({why})")),
            }
        }
        if let Some(highest) = restored.iter().map(|id| id.0).max() {
            self.next_session = self.next_session.max(highest.saturating_add(1));
        }
        // Use order, oldest first: what the file remembered, then any session it did not mention.
        let unmentioned: Vec<_> = restored
            .iter()
            .copied()
            .filter(|id| !recent.contains(id))
            .collect();
        self.recent_sessions = unmentioned
            .into_iter()
            .chain(recent.into_iter().filter(|id| restored.contains(id)))
            .fold(Vec::new(), |mut order, id| {
                order.retain(|seen| *seen != id);
                order.push(id);
                order
            });
        self.last_session = self.recent_sessions.last().copied();
        if !problems.is_empty() {
            eprintln!("[heca] saved sessions: {}", problems.join("; "));
            let (_, drafts) = crate::notification::collect_notifications(|| {
                Notification::warning("Some saved sessions could not be restored")
                    .body(problems.join("\n"))
                    .dedup_key("sessions.restore")
                    .send();
            });
            self.notices.extend(drafts);
        }
    }

    /// One saved session, rebuilt and, if it was running, started again.
    fn restore(&mut self, saved: SavedSession, config: &AppConfig) -> Result<(), String> {
        let wake = self
            .wake
            .clone()
            .ok_or("the server has no way to wake itself")?;
        let running = saved.status == SessionStatus::Active;
        let name = saved.name.clone();
        // The project's own settings apply when its folder is there; the user's when it is gone.
        let project = match saved.folder.as_deref() {
            Some(folder) if folder.is_dir() => (self.machine.project_config)(folder),
            _ => config.clone(),
        };
        let launch = self.machine.launch;
        let (runtime, drafts) = crate::notification::collect_notifications(|| {
            let mut runtime = SessionRuntime::restored(&project, wake, launch, saved)
                .map_err(|error| error.to_string())?;
            if running && let Err(why) = runtime.restart() {
                Notification::warning(format!("Session '{name}' was not started again"))
                    .body(why)
                    .dedup_key(format!("sessions.restart:{name}"))
                    .send();
            }
            Ok::<_, String>(runtime)
        });
        let mut runtime = runtime?;
        for draft in drafts {
            runtime.raise_lasting(draft);
        }
        self.install(runtime);
        Ok(())
    }

    /// Add a built session to this server, with what every session shares.
    pub(super) fn install(&mut self, mut runtime: SessionRuntime) {
        runtime.install_actions(self.registry.clone(), self.catalog.clone());
        runtime.install_event_fanout(self.events.clone());
        runtime.install_services(self.services.clone());
        self.sessions.push(runtime);
        self.sessions_changed();
    }

    /// Everything the file keeps, as the server is now.
    fn saved(&self) -> Saved {
        Saved {
            next_session: self.next_session,
            recent: self.recent_sessions.clone(),
            sessions: self.sessions.iter().map(SessionRuntime::saved).collect(),
        }
    }

    /// Write the file now, if sessions are kept.
    pub(super) fn save_now(&mut self) {
        if self.store.is_some() {
            let saved = self.saved();
            if let Some(store) = self.store.as_mut() {
                store.save(&saved);
            }
        }
    }

    /// **Write the file once things have settled.** Called at the end of each turn: a turn that ran
    /// says something may have changed, which makes a write due after the configured delay; the turn
    /// the delay ends on writes. A server nobody touches has no turns and so writes nothing.
    pub(super) fn autosave(&mut self, now: Instant) {
        let Some(store) = self.store.as_mut() else {
            return;
        };
        if store.take_due(now) {
            self.save_now();
        } else {
            store.arm(now);
        }
    }

    /// The final write, and no more after it: the sessions' processes end next, and what is left
    /// then must never replace what ran.
    pub(super) fn save_and_close_store(&mut self) {
        // `HECA_LOG_SESSIONS=1`: what is written down for each terminal on the way out — the agent
        // it reported (driver and conversation id), what it was launched with, and whether a
        // program was running. What a restore can and cannot bring back is decided by this.
        if std::env::var_os("HECA_LOG_SESSIONS").is_some() {
            for session in &self.sessions {
                for terminal in session.recorded_terminals() {
                    eprintln!(
                        "[sessions] saved {:?}: agent={:?} running={} launch={:?}",
                        terminal.owner, terminal.agent, terminal.running, terminal.launch
                    );
                }
            }
        }
        self.save_now();
        self.store = None;
    }

    /// Show the reports held for the first session there is.
    pub(super) fn flush_notices(&mut self) {
        if self.notices.is_empty() {
            return;
        }
        if let Some(index) = self.last_index() {
            for draft in std::mem::take(&mut self.notices) {
                self.sessions[index].raise_lasting(draft);
            }
        }
    }
}
