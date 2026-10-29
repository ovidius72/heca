//! The single physical receive point and owner of named session runtimes.

use super::{
    ServerState, ServerWaker,
    connection::Connection,
    session::SessionRuntime,
    sessions::{SessionCommand, SessionInfo, SessionReply, SessionStatus},
    wire::{ClientMsg, ServerLink, ServerMsg},
};
#[cfg(test)]
use heca_core::layout::Session;
use heca_core::layout::SessionId;
use std::{rc::Rc, time::Instant};

/// The one server-shutdown protocol word, shared with the CLI.
pub(crate) const KILL_SERVER: &str = "kill-server";
mod rpc;
mod sessions;

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
        }
    }

    #[cfg(test)]
    pub(crate) fn new(state: ServerState, session: Session) -> Self {
        let id = session.id;
        let mut server = Self::empty();
        server.sessions.push(SessionRuntime::new(state, session));
        server.last_session = Some(id);
        server.recent_sessions.push(id);
        server.next_session = id.0.saturating_add(1);
        server
    }

    pub(super) fn configure_factory(
        &mut self,
        config: heca_config::loader::AppConfig,
        wake: ServerWaker,
    ) {
        self.config = Some(config);
        self.wake = Some(wake);
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

    pub(super) fn register_extensions(&mut self) {
        let mut conflicts = crate::app::conflicts::Conflicts::default();
        crate::entry::register_queued_actions(
            &mut self.registry,
            &mut self.catalog,
            &mut conflicts,
        );
        conflicts.report();
        self.install_actions(self.registry.clone(), self.catalog.clone());
    }

    pub(crate) fn shutdown(&mut self) {
        for client in &self.clients {
            client.close();
        }
        for session in &mut self.sessions {
            session.shutdown();
        }
        self.clients.clear();
    }

    pub(super) fn raise(&mut self, draft: crate::notification::NotificationDraft) {
        if let Some(session) = self.last_index() {
            self.sessions[session].raise(draft);
        }
    }

    pub(super) fn begin_rpc(&mut self, line: String, reply: std::sync::mpsc::Sender<String>) {
        let owned = crate::rpc::parse_rpc(&line).and_then(|command| command.into_intent(&self.catalog))
            .is_ok_and(|(intent, dock)| dock.is_none()
                && crate::input::resolve_action(&intent.action, &crate::app::interaction::intent_args_as_strings(&intent))
                    .is_some_and(|action| super::sessions::owns(action.kind())));
        if owned { let _ = reply.send(self.rpc(&line)); return; }
        if line.trim() == KILL_SERVER {
            self.shutdown();
            self.stopped = true;
            let _ = reply.send("heca: server stopped\n".into());
        } else if let Some(session) = self.last_index() {
            if self.sessions[session].info().windows > 0 {
                self.sessions[session].begin_rpc(line, reply);
            } else {
                let _ = reply.send(self.rpc(&line));
            }
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
                    .filter_map(|client| client.retire_at.get()),
            )
            .min()
    }

    pub(crate) fn pump(&mut self, now: Instant) {
        let mut received = Vec::new();
        for client in &self.clients {
            if client
                .retire_at
                .get()
                .is_some_and(|deadline| now >= deadline)
            {
                client.close();
                continue;
            }
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
            if client.retire_at.get().is_some() {
                if matches!(message, ClientMsg::Detach) {
                    client.close();
                }
                continue;
            }
            match message {
                ClientMsg::Attach => {
                    self.pump_sessions(now);
                    self.attach(client, None);
                }
                ClientMsg::AttachTo { name } => {
                    self.pump_sessions(now);
                    self.attach(client, Some(name));
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
                        client.close();
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
        self.pump_sessions(now);
        self.clients.retain(|c| !c.is_closed());
        self.publish_directory();
    }

    fn publish_directory(&mut self) {
        let directory: Vec<_> = self.sessions.iter().map(SessionRuntime::info).collect();
        if directory == self.directory {
            return;
        }
        self.directory = directory;
        for client in &self.clients {
            if client.session.get().is_some() {
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
        self.sessions[index].add_connection(client);
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
mod identity_tests;
