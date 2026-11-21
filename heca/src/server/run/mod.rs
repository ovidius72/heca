//! The single physical receive point and owner of named session runtimes.

use super::{
    ServerState, ServerWaker,
    connection::Connection,
    session::SessionRuntime,
    sessions::{SessionCommand, SessionInfo, SessionReply, SessionStatus},
    services::Call,
    wire::{ClientMsg, ServerLink, ServerMsg},
};
#[cfg(test)]
use heca_core::layout::Session;
use heca_core::layout::SessionId;
use std::{rc::Rc, time::Instant};

mod requests;
mod restore;
mod rpc;
mod sessions;

/// **What a server loop reaches the machine through**: how it starts a terminal process, and how it
/// reads the settings of a folder. The app's own unless a test, which must start no process and read
/// none of the user's files, hands in others.
#[derive(Clone, Copy)]
pub(crate) struct Machine {
    pub(crate) launch: crate::app::backend_factory::Launch,
    pub(crate) project_config: fn(&std::path::Path) -> heca_config::loader::AppConfig,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            launch: crate::app::backend_factory::launch,
            project_config: heca_config::loader::AppConfig::load_for,
        }
    }
}

pub(crate) struct ServerLoop {
    sessions: Vec<SessionRuntime>,
    clients: Vec<Rc<Connection>>,
    last_session: Option<SessionId>,
    recent_sessions: Vec<SessionId>,
    next_session: u64,
    next_client: u64,
    config: Option<heca_config::loader::AppConfig>,
    wake: Option<ServerWaker>,
    registry: crate::actions::ActionRegistry,
    catalog: crate::actions::ActionCatalog,
    stopped: bool,
    directory: Vec<SessionInfo>,
    /// `heca rpc --events` subscribers, shared with every session so each one's broadcast fans its
    /// events out to the scripts listening.
    events: super::EventFanout,
    /// The extension services this server runs, shared with every session.
    services: super::services::SharedServices,
    /// What this server reaches the machine through.
    machine: Machine,
    /// The file this server's sessions are kept in. `None` when they are not kept, and after the
    /// server has stopped and written them for the last time.
    store: Option<super::persist::SessionStore>,
    /// Reports raised before any session could show them, for the first one that can.
    notices: Vec<crate::notification::NotificationDraft>,
    /// Which server this is. Held for as long as the server lives: the lock that keeps its folder
    /// its own goes when the server does. `None` for a test-built server that runs no services.
    _identity: Option<super::identity::ServerIdentity>,
}

impl ServerLoop {
    pub(super) fn empty() -> Self {
        Self {
            sessions: Vec::new(),
            clients: Vec::new(),
            last_session: None,
            recent_sessions: Vec::new(),
            next_session: 1,
            next_client: 1,
            config: None,
            wake: None,
            registry: crate::actions::ActionRegistry::new(),
            catalog: crate::actions::ActionCatalog::with_builtins(),
            stopped: false,
            directory: Vec::new(),
            events: super::EventFanout::default(),
            services: Default::default(),
            machine: Machine::default(),
            store: None,
            notices: Vec::new(),
            _identity: None,
        }
    }

    /// This server, reaching the machine through `machine`.
    pub(in crate::server) fn on(mut self, machine: Machine) -> Self {
        self.machine = machine;
        self
    }

    #[cfg(test)]
    pub(crate) fn new(state: ServerState, session: Session) -> Self {
        Self::empty().with_session(state, session)
    }

    /// This server with one test-built session in it, made the most recent.
    #[cfg(test)]
    pub(crate) fn with_session(mut self, state: ServerState, session: Session) -> Self {
        let id = session.id;
        let mut runtime = SessionRuntime::new(state, session);
        runtime.install_event_fanout(self.events.clone());
        runtime.install_services(self.services.clone());
        self.sessions.push(runtime);
        self.last_session = Some(id);
        self.recent_sessions.push(id);
        self.next_session = id.0.saturating_add(1);
        self.sessions_changed();
        self
    }

    pub(super) fn configure_factory(
        &mut self,
        config: heca_config::loader::AppConfig,
        wake: ServerWaker,
    ) {
        self.config = Some(config);
        self.wake = Some(wake);
    }

    /// Run every declared service for this server — once, as it starts — and keep `identity` for
    /// as long as the server lives.
    pub(super) fn start_services(
        &mut self,
        config: &heca_config::loader::AppConfig,
        identity: super::identity::ServerIdentity,
        wake: &ServerWaker,
    ) {
        let catalog = self.catalog.snapshot();
        *self.services.borrow_mut() = super::services::ServiceHost::start(config, &identity, wake, &catalog);
        self._identity = Some(identity);
    }

    pub(crate) fn wants_shutdown(&self) -> bool {
        self.stopped
    }

    pub(crate) fn install_actions(
        &mut self,
        registry: crate::actions::ActionRegistry,
        catalog: crate::actions::ActionCatalog,
    ) {
        self.registry = registry;
        self.catalog = catalog;
        for session in &mut self.sessions {
            session.install_actions(self.registry.clone(), self.catalog.clone());
        }
    }

    /// Register a `heca rpc --events` subscriber: every session fans its events to it (the fanout
    /// is shared), until the subscriber's socket side goes.
    pub(super) fn subscribe_events(&mut self, events: std::sync::mpsc::SyncSender<String>) {
        self.events.subscribe(events);
    }

    /// **Build this server's actions**: heca's own, then what extensions and services added.
    /// Done once, as the server starts and before its services do, so a service's factory sees the
    /// complete catalog. Returns what collided while adding them.
    pub(super) fn register_actions(&mut self) -> crate::app::conflicts::Conflicts {
        let mut conflicts = crate::app::conflicts::Conflicts::default();
        crate::entry::register_queued_actions(&mut self.registry, &mut self.catalog, &mut conflicts);
        conflicts
    }

    /// **Stop the server**: what `kill_server` does, whoever asked. The loop ends on its next turn.
    pub(super) fn stop(&mut self) {
        self.shutdown();
        self.stopped = true;
    }

    pub(crate) fn shutdown(&mut self) {
        // Before the sessions end their processes: what ran in them is what is written down.
        self.save_and_close_store();
        self.services.borrow_mut().shutdown();
        for client in &self.clients {
            client.close();
        }
        for session in &mut self.sessions {
            session.shutdown();
        }
        self.clients.clear();
    }

    /// Tell the window that asked why its request was refused, as a notification in its session
    /// (or the most recent one when it has none).
    fn report_refusal(&mut self, client: &Connection, action: &str, reason: String) {
        let (_, drafts) = crate::notification::collect_notifications(|| super::services::refused(action, reason));
        for draft in drafts {
            match self.index_for(client.session.get()).or_else(|| self.last_index()) {
                Some(index) => self.sessions[index].raise(draft),
                None => self.notices.push(draft),
            }
        }
    }

    pub(super) fn raise(&mut self, draft: crate::notification::NotificationDraft) {
        if let Some(session) = self.last_index() {
            self.sessions[session].raise(draft);
        }
    }

    pub(super) fn begin_rpc(&mut self, line: String, reply: std::sync::mpsc::Sender<String>) {
        // A line that cannot be read is answered here: no window or session can add anything to it,
        // and sending it on would leave the asker waiting for an answer the server already has.
        let parsed = match crate::rpc::parse_rpc(&line) {
            Ok(command) => command.into_intent(&self.catalog),
            Err(error) => {
                let _ = reply.send(format!("error: {error}"));
                return;
            }
        };
        // A service's action runs where the service is, whoever is attached.
        if let Ok((intent, None)) = &parsed
            && self.services.borrow().owns(&intent.action) {
            return self.rpc_service(intent, reply);
        }
        let owned = parsed.as_ref()
            .is_ok_and(|(intent, dock)| dock.is_none()
                && crate::input::resolve_action(&intent.action, &crate::app::interaction::intent_args_as_strings(intent))
                    .is_some_and(|action| super::sessions::owns(action.kind())));
        if owned { let _ = reply.send(self.rpc(&line)); return; }
        if let Some(session) = self.recent_sessions.iter().rev()
            .filter_map(|id| self.index_for(Some(*id)))
            .find(|&index| self.sessions[index].info().windows > 0)
        {
            // Creating an unattached session must not take commands away from a live window.
            self.sessions[session].begin_rpc(line, reply);
        } else if parsed.as_ref()
            .is_ok_and(|(intent, _)| crate::actions::builtin_side(&intent.action)
                != Some(crate::actions::Side::Server))
            && let Some(client) = self.clients.iter().find(|client| !client.is_closed()
                && matches!(client.attachment.get(), super::connection::Attachment::Detached)) {
            let limits = self.config.as_ref().map(|c| c.config.settings.connection).unwrap_or_default();
            client.begin_rpc(line, reply, limits);
        } else {
            let _ = reply.send(self.rpc(&line));
        }
    }

    #[cfg(test)]
    pub(crate) fn connect(&mut self) -> super::wire::InProcessClient {
        let (client, link) = super::wire::in_process();
        self.add_client(link);
        client
    }

    pub(crate) fn add_client(&mut self, link: impl ServerLink + 'static) {
        self.add_link(Box::new(link));
    }

    pub(super) fn add_link(&mut self, link: Box<dyn ServerLink>) {
        let Some(next) = self.next_client.checked_add(1) else {
            return;
        };
        // The sole client-id allocation point, shared by every session and transport.
        self.clients
            .push(Rc::new(Connection::new(self.next_client, link)));
        self.next_client = next;
    }

    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.sessions
            .iter()
            .filter(|s| s.info().status == SessionStatus::Active)
            .filter_map(SessionRuntime::next_wake)
            .chain(
                self.clients
                    .iter()
                    .filter_map(|client| client.rpc_deadline()),
            )
            .chain(self.store.as_ref().and_then(super::persist::SessionStore::next_wake))
            .min()
    }

    pub(crate) fn pump(&mut self, now: Instant) {
        let _slow = crate::slow_steps::step("server.pump");
        let mut received = Vec::new();
        for client in &self.clients {
            loop {
                // Every message gets its server-assigned source here, never from its payload.
                match client.receive() {
                    Ok(Some(message)) => received.push((client.clone(), message)),
                    Ok(None) => break,
                    Err(_) => {
                        // Preserve messages read before EOF, including a closing window's RPC answer.
                        received.push((client.clone(), ClientMsg::Detach));
                        break;
                    }
                }
            }
        }
        for (client, message) in received {
            if client.is_closed() {
                continue;
            }
            match message {
                ClientMsg::RpcReply { request_id, answer } => client.reply_rpc(request_id, answer),
                ClientMsg::ServiceResync { service } => self.services.borrow_mut().forget(client.id, &service),
                ClientMsg::Attach => {
                    self.pump_sessions(now);
                    self.attach(client, None);
                }
                ClientMsg::AttachTo { name } => {
                    self.pump_sessions(now);
                    self.attach(client, Some(name));
                }
                ClientMsg::LeaveSession => {
                    self.pump_sessions(now);
                    if let Some(id) = client.session.take() {
                        if let Some(index) = self.index_for(Some(id)) {
                            self.sessions[index].detach_connection(client.id);
                        }
                        client.attachment.set(super::connection::Attachment::Detached);
                        client.projection_pending.set(false);
                        let _ = client.link.send(ServerMsg::SessionDetached { id });
                    }
                }
                ClientMsg::Session {
                    request_id,
                    command,
                } => {
                    self.pump_sessions(now);
                    let result = self.execute_session(command);
                    let _ = client.link.send(ServerMsg::Sessions { request_id, result });
                }
                ClientMsg::Act {
                    action, request_id, ..
                } if super::sessions::owns(action.kind()) => {
                    self.pump_sessions(now);
                    // Making, listing or renaming sessions belongs to no session: asked as such.
                    let (name, args) = crate::actions::describe(&action);
                    let verdict = self.services.borrow_mut()
                        .authorize(&Call::local(Some(client.id)), None, &name, &args, None);
                    if let Err(reason) = verdict {
                        self.report_refusal(&client, &name, reason);
                        if let Some(request_id) = request_id {
                            let _ = client.link.send(ServerMsg::Reply { request_id, changes: Vec::new() });
                        }
                        continue;
                    }
                    if matches!(action, crate::input::WmAction::KillServer) {
                        // Every window is gone with the server: nothing is left to answer or pump.
                        self.stop();
                        return;
                    }
                    let current = self
                        .index_for(client.session.get())
                        .map(|i| self.sessions[i].info().name);
                    let result = super::sessions::from_action(&action, current.as_deref())
                        .and_then(|command| self.execute_session(command));
                    let _ = client.link.send(ServerMsg::Sessions {
                        request_id: request_id.unwrap_or(0),
                        result,
                    });
                    if let Some(request_id) = request_id {
                        let _ = client.link.send(ServerMsg::Reply {
                            request_id,
                            changes: Vec::new(),
                        });
                    }
                }
                ClientMsg::Detach => {
                    self.pump_sessions(now);
                    if client.session.get().is_none() {
                        client.close();
                    } else {
                        client.queue(ClientMsg::Detach);
                    }
                }
                message => {
                    let Some(index) = self.index_for(client.session.get()) else {
                        // Session-scoped input queued before a logical detach is stale.
                        if !matches!(client.attachment.get(), super::connection::Attachment::Detached) {
                            client.close();
                        }
                        continue;
                    };
                    if self.sessions[index].info().status == SessionStatus::Terminated
                        && !matches!(message, ClientMsg::RpcReply { .. }) {
                        let _ = client.link.send(ServerMsg::SessionTerminated {
                            session: self.sessions[index].info(),
                        });
                    } else {
                        client.queue(message);
                    }
                }
            }
        }
        { let _s = crate::slow_steps::step("server.pump_sessions"); self.pump_sessions(now) };
        { let _s = crate::slow_steps::step("server.pump_services"); self.pump_services() };
        self.announce_state_changes();
        for client in &self.clients { client.expire_rpc(now); }
        self.clients.retain(|c| !c.is_closed());
        { let _s = crate::slow_steps::step("server.publish_directory"); self.publish_directory() };
        self.flush_notices();
        { let _s = crate::slow_steps::step("server.autosave"); self.autosave(now) };
    }

    fn publish_directory(&mut self) {
        let directory: Vec<_> = self.sessions.iter().map(SessionRuntime::info).collect();
        if directory == self.directory {
            return;
        }
        self.directory = directory;
        for client in &self.clients {
            if client.session.get().is_some()
                || matches!(client.attachment.get(), super::connection::Attachment::Detached) {
                let _ = client.link.send(ServerMsg::Sessions {
                    request_id: 0,
                    result: Ok(SessionReply::List(self.directory.clone())),
                });
            }
        }
    }

    fn pump_sessions(&mut self, now: Instant) {
        for session in &mut self.sessions {
            let (_, drafts) = crate::notification::collect_notifications(|| session.pump(now));
            for draft in drafts {
                session.raise(draft);
            }
        }
    }

    fn attach(&mut self, client: Rc<Connection>, name: Option<String>) {
        if name.is_none() && self.sessions.is_empty() {
            let result = dirs::home_dir()
                .ok_or("no home folder".to_owned())
                .and_then(|folder| self.create_session(folder, None, None));
            if let Err(error) = result {
                let _ = client.link.send(ServerMsg::Sessions {
                    request_id: 0,
                    result: Err(error),
                });
                return;
            }
        }
        let index = match name {
            Some(name) => self.sessions.iter().position(|s| s.info().name == name),
            None => self.last_index(),
        };
        let Some(index) = index else {
            let _ = client.link.send(ServerMsg::Sessions {
                request_id: 0,
                result: Err("session not found; create one with --new".into()),
            });
            return;
        };
        let id = self.sessions[index].info().id;
        // Attaching, even to the same session, starts the window's service projections afresh.
        self.services.borrow_mut().forget_client(client.id);
        self.last_session = Some(id);
        self.recent_sessions.retain(|recent| *recent != id);
        self.recent_sessions.push(id);
        if client.session.get() == Some(id) {
            client.queue(ClientMsg::Attach);
            return;
        }
        if let Some(previous) = self.index_for(client.session.get()) {
            self.sessions[previous].detach_connection(client.id);
        }
        client
            .attachment
            .set(super::connection::Attachment::Pending);
        client.session.set(Some(id));
        client.queue(ClientMsg::Attach);
        // A window attached to (or switched to) this session — a per-window event for the stream.
        self.events.emit(&super::StreamEvent::Session {
            window: client.id,
            session: id,
            name: self.sessions[index].info().name.clone(),
        });
        self.sessions[index].add_connection(client);
    }

    /// Deliver the services' inboxes, run what they handed in for others, and tell them how it
    /// ended; then show each attached window what changed for it.
    fn pump_services(&mut self) {
        self.services.borrow_mut().drain();
        self.run_requests();
        let mut host = self.services.borrow_mut();
        host.tell_outcomes();
        if let Some(index) = self.last_index() {
            for draft in host.take_held() { self.sessions[index].raise(draft); }
        }
        let audience: Vec<_> = self.sessions.iter().map(SessionRuntime::audience).collect();
        let limits = self.config.as_ref().map(|c| c.config.settings.connection).unwrap_or_default();
        host.publish(&audience, limits.max_frame_bytes.get());
    }

    /// Tell the services which sessions exist, after one is made, restored or deleted. Nothing is
    /// announced: [`announce_session`](Self::announce_session) is the door for a change someone
    /// made.
    pub(super) fn sessions_changed(&self) -> crate::entry::service::Sessions {
        let sessions = crate::entry::service::Sessions::new(
            self.sessions.iter().map(SessionRuntime::service_entry).collect(),
        );
        self.services.borrow_mut().know_sessions(&sessions);
        sessions
    }

    /// **`session.changed` — the one place it is raised.** The services' listeners hear it, then
    /// the scripts' stream (`heca rpc --events`) gets the same event. `session` is the session it
    /// happened to; for a deleted one, as it was just before.
    pub(super) fn announce_session(
        &self,
        kind: crate::entry::service::SessionChange,
        session: crate::entry::service::SessionEntry,
    ) {
        let sessions = self.sessions_changed();
        self.events.emit(&super::StreamEvent::SessionChanged { kind, session: session.clone() });
        let event = crate::entry::service::ServerEvent::SessionChanged(crate::entry::service::SessionEvent {
            kind,
            session,
            sessions,
        });
        self.services.borrow_mut().announce(&event);
    }

    /// **`state.changed` — the one place it is raised**, once per saved entry that changed since the
    /// last turn: the services' listeners hear it, then the scripts' stream gets the same event.
    pub(super) fn announce_state_changes(&self) {
        let changes = self.services.borrow().take_state_changes();
        for change in changes {
            self.events.emit(&super::StreamEvent::StateChanged(change.clone()));
            self.services.borrow_mut().announce(&crate::entry::service::ServerEvent::StateChanged(change));
        }
    }

    fn index_for(&self, id: Option<SessionId>) -> Option<usize> {
        self.sessions.iter().position(|s| Some(s.info().id) == id)
    }

    fn last_index(&self) -> Option<usize> {
        self.index_for(self.last_session)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod feedback_tests;
#[cfg(test)]
mod service_tests;
#[cfg(test)]
mod saved_state_tests;
#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod service_data_tests;
#[cfg(test)]
mod service_key_tests;
#[cfg(test)]
mod service_session_tests;
#[cfg(test)]
mod policy_tests;
#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod restore_tests;
#[cfg(test)]
mod resume_tests;
#[cfg(test)]
mod history_tests;
