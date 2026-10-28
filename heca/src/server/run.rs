//! **The server runtime** (F012, P099/T544 C4) — the shared state, the session, and the clients
//! attached to it, driven one [`pump`](ServerLoop::pump) at a time.
//!
//! A pump is one turn of the loop: take what each client sent, do it, send back what changed. It
//! needs no winit and no threads, so it runs and is tested headless. The headless `heca --server`
//! wraps it in a thread that pumps when its mpsc wakes (a client message, a terminal's output, a
//! timer); standalone pumps it from the window's loop. One loop body, both ways.

use std::time::Instant;

use heca_core::layout::Session;

use super::wire::{ClientMsg, ServerLink, ServerMsg};
#[cfg(test)]
use super::wire::{InProcessClient, in_process};
use super::{Asker, Change, ServerState};

/// The one-line request `heca --kill-server` sends over the socket. A protocol word, not a catalog
/// action, so it is defined once and read by both the CLI that sends it and the server that stops.
pub(crate) const KILL_SERVER: &str = "kill-server";

/// Everything a session is and the clients watching it.
pub(crate) struct ServerLoop {
    state: ServerState,
    session: Session,
    /// The server end of each attached client. A detached client is dropped from here.
    clients: Vec<Connection>,
    registry: crate::actions::ActionRegistry,
    catalog: crate::actions::ActionCatalog,
    store: crate::chrome::SharedChromeState,
    confirm: heca_config::confirm::ConfirmConfig,
    rpc_asker: Asker,
    pending_rpc: std::collections::HashMap<u64, PendingRpc>,
    next_rpc: u64,
    next_client: u64,
    connection: heca_config::settings::ConnectionConfig,
    images: std::collections::HashMap<u64, heca_core::backend::TerminalImage>,
    known_terminals: std::collections::HashSet<crate::app::backend_store::TerminalOwner>,
    /// Set by `heca --kill-server`: the owner loop reads it after the next pump and stops, ending
    /// the server and with it every pane and terminal (tmux's `kill-server`).
    wants_shutdown: bool,
}

enum Attachment { Pending, Attached }

struct Connection {
    id: u64,
    link: Box<dyn ServerLink>,
    attachment: Attachment,
}

struct PendingRpc {
    client: u64,
    reply: std::sync::mpsc::Sender<String>,
    deadline: Instant,
}

impl ServerLoop {
    pub(crate) fn new(state: ServerState, session: Session) -> Self {
        let registry = crate::actions::ActionRegistry::new();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let store = crate::chrome::SharedChromeState::new(0.0, 0.0);
        store.workspaces.set_programs(state.programs.clone());
        let view = heca_core::layout::WindowView::new(Default::default(), 1.0);
        let rpc_asker = Asker::seen_through(session.through(&view), None, heca_core::backend::DEFAULT_CELL_SIZE);
        Self {
            state,
            session,
            clients: Vec::new(),
            registry, catalog, store, rpc_asker,
            confirm: Default::default(),
            pending_rpc: Default::default(), next_rpc: 1, next_client: 1,
            connection: Default::default(), images: Default::default(),
            known_terminals: Default::default(),
            wants_shutdown: false,
        }
    }

    /// Whether `heca --kill-server` asked the server to stop. The owner loop reads this after each
    /// pump and ends the process; standalone ignores it (a one-off session ends with its window).
    pub(crate) fn wants_shutdown(&self) -> bool {
        self.wants_shutdown
    }

    pub(crate) fn install_actions(&mut self, registry: crate::actions::ActionRegistry, catalog: crate::actions::ActionCatalog) {
        self.registry = registry;
        self.catalog = catalog;
    }

    pub(crate) fn shutdown(&mut self) {
        self.clients.clear();
        self.pending_rpc.clear();
        self.state.backends.clear();
    }

    pub(super) fn register_extensions(&mut self) {
        let mut conflicts = crate::app::conflicts::Conflicts::default();
        crate::entry::register_queued_actions(&mut self.registry, &mut self.catalog, &mut conflicts);
        conflicts.report();
    }

    pub(super) fn raise(&mut self, draft: crate::notification::NotificationDraft) {
        let changes = self.state.execute(super::ServerAction::Raise(draft), Instant::now());
        let _ = self.publish_notifications(&changes);
    }

    /// Re-read the config file and re-apply the server's own terminal and notification settings —
    /// the `prefix+Shift+r` path. The window hands nothing over: it reloads its view-side config and
    /// sends [`ClientMsg::Reload`], and the server (which runs as the same local user) reads the
    /// same file itself. A load error keeps the current settings. Returns the notification changes.
    fn reload_config(&mut self, now: Instant) -> Vec<Change> {
        let Ok(config) = heca_config::loader::AppConfig::try_load() else {
            return Vec::new();
        };
        let terminals = crate::app::backend_factory::TerminalSettings::from_config(&config);
        let _ = self.state.execute(super::ServerAction::ConfigureTerminals(Box::new(terminals)), now);
        let notifications = &config.config.settings.notification_system;
        self.state.execute(
            super::ServerAction::Configure(super::NotificationSettings {
                auto_dismiss: std::time::Duration::from_millis(notifications.auto_dismiss_ms),
                mode: notifications.mode,
                max_visible: notifications.max_visible,
            }),
            now,
        )
    }

    pub(super) fn begin_rpc(&mut self, line: String, reply: std::sync::mpsc::Sender<String>) {
        // `heca --kill-server`: stop the whole server — drop every client, end every terminal
        // (tmux's `kill-server`). Answered before any attached-client forwarding, because it is a
        // command to the server itself, not an action for a window to run.
        if line.trim() == KILL_SERVER {
            self.shutdown();
            self.wants_shutdown = true;
            let _ = reply.send("heca: server stopped\n".into());
            return;
        }
        if let Some(client) = self.clients.iter().find(|client| matches!(client.attachment, Attachment::Attached)) {
            let request_id = self.next_rpc;
            self.next_rpc += 1;
            if client.link.send(ServerMsg::Rpc { request_id, line }).is_err() {
                let _ = reply.send("error: attached client disconnected".into());
                return;
            }
            let timeout = self.connection.request_timeout_ms.get();
            self.pending_rpc.insert(request_id, PendingRpc { client: client.id, reply,
                deadline: Instant::now() + std::time::Duration::from_millis(timeout) });
        } else {
            let answer = self.rpc(&line);
            let _ = reply.send(answer);
        }
    }

    pub(super) fn configure_rpc(&mut self, asker: Asker, confirm: heca_config::confirm::ConfirmConfig, connection: heca_config::settings::ConnectionConfig) {
        self.rpc_asker = asker;
        self.confirm = confirm;
        self.connection = connection;
    }

    pub(super) fn rpc(&mut self, line: &str) -> String {
        if let Some(result) = crate::rpc::introspect(&self.catalog, line) {
            return result.unwrap_or_else(|error| format!("error: {error}"));
        }
        let result = (|| {
            let (intent, dock) = crate::rpc::parse_rpc(line)?.into_intent(&self.catalog)?;
            if dock.is_some() { return Err(crate::rpc::RpcError::NotRunnable(intent.action)); }
            self.registry.execute_server_intent(&mut crate::actions::ServerInvocation {
                state: &mut self.state, session: &mut self.session, store: self.store.clone(),
                catalog: &self.catalog, confirm: &self.confirm, asker: self.rpc_asker,
            }, &intent)
        })();
        match result {
            Ok(changes) => {
                let _ = self.broadcast(&changes);
                "ok".into()
            }
            Err(error) => format!("error: {error}"),
        }
    }

    /// Attach an in-process client and hand back its end. The socket transport adds its clients the
    /// same way, so the loop never knows which kind it is serving.
    #[cfg(test)]
    pub(crate) fn connect(&mut self) -> InProcessClient {
        let (client, server) = in_process();
        self.add_client(server);
        client
    }

    /// Both socket and in-process clients enter the same attach/action loop.
    pub(crate) fn add_client(&mut self, link: impl ServerLink + 'static) {
        self.add_link(Box::new(link));
    }

    pub(super) fn add_link(&mut self, link: Box<dyn ServerLink>) {
        let id = self.next_client;
        self.next_client += 1;
        self.clients.push(Connection { id, link, attachment: Attachment::Pending });
    }

    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.pending_rpc.values().map(|request| request.deadline).chain(self.state.next_wake()).min()
    }

    /// One turn of the loop: drain every client, do what it asked, and send back what changed;
    /// then let time pass (notification deadlines). Detached clients are dropped.
    pub(crate) fn pump(&mut self, now: Instant) {
        // Drain first (borrows the client ends), then act (borrows the state + session), so the two
        // borrows never overlap.
        let mut pending: Vec<(usize, ClientMsg)> = Vec::new();
        let mut detached: Vec<usize> = Vec::new();
        let mut publish_terminals = false;
        for (i, client) in self.clients.iter().enumerate() {
            loop {
                match client.link.poll() {
                    Ok(Some(msg)) => pending.push((i, msg)),
                    Ok(None) => break,
                    Err(_) => {
                        detached.push(i);
                        break;
                    }
                }
            }
        }

        for (i, msg) in pending {
            if detached.contains(&i) {
                continue;
            }
            if !matches!(msg, ClientMsg::Attach | ClientMsg::Detach)
                && !matches!(self.clients[i].attachment, Attachment::Attached)
            {
                detached.push(i);
                continue;
            }
            match msg {
                ClientMsg::Attach => {
                    let content = self.session.content();
                    if self.clients[i].link.send(ServerMsg::Attached { content }).is_err() {
                        detached.push(i);
                    } else {
                        self.clients[i].attachment = Attachment::Attached;
                        // The attaching (or reattaching) window gets the current notifications so
                        // its toast stack comes back exactly as the session left it — only the ones
                        // meant for it (session-wide plus any addressed to this window).
                        let views = self.state.visible_notifications_for(self.clients[i].id);
                        if self.clients[i].link.send(ServerMsg::Notifications(views)).is_err() {
                            detached.push(i);
                        }
                        publish_terminals = true;
                    }
                }
                ClientMsg::Detach => detached.push(i),
                ClientMsg::Act { mut action, asker, request_id, terminal_owner } => {
                    if !matches!(self.clients[i].attachment, Attachment::Attached)
                        || !ServerState::runs(action.kind())
                    {
                        detached.push(i);
                        continue;
                    }
                    if let Some(owner) = terminal_owner {
                        let Some(id) = self.state.backends.find_owner(&owner) else {
                            let refusal = super::Refusal::TerminalUnavailable;
                            let message = match request_id {
                                Some(request_id) => ServerMsg::Reply { request_id, changes: vec![Change::Refused(refusal)] },
                                None => ServerMsg::Refused(refusal),
                            };
                            if self.clients[i].link.send(message).is_err() { detached.push(i); }
                            continue;
                        };
                        match &mut action {
                            crate::input::WmAction::TerminalRun { terminal, .. }
                            | crate::input::WmAction::TerminalKill { terminal, .. } => *terminal = Some(id.0),
                            _ => {},
                        }
                    }
                    let changes = self.act(action, asker);
                    match request_id {
                        Some(request_id) => {
                            detached.extend(self.broadcast_except(&changes, Some(i)));
                            if self.clients[i].link.send(ServerMsg::Reply { request_id, changes }).is_err() {
                                detached.push(i);
                            }
                        }
                        None => detached.extend(self.broadcast(&changes)),
                    }
                    publish_terminals = true;
                }
                ClientMsg::EnsureTerminal { owner, spec, cell } => {
                    if !super::stream::valid_grid(spec.grid.0, spec.grid.1, cell, 1.0, self.connection.max_terminal_cells.get()) {
                        detached.push(i);
                        continue;
                    }
                    if self.state.terminals().start(owner, spec, cell).is_err() {
                        detached.push(i);
                    }
                    publish_terminals = true;
                }
                ClientMsg::Terminal { owner, input } => {
                    if let super::wire::terminal::ProcessInput::Resize { cols, rows, cell, scale } = &input
                        && !super::stream::valid_grid(*cols, *rows, *cell, *scale, self.connection.max_terminal_cells.get())
                    {
                        detached.push(i);
                        continue;
                    }
                    self.state.process_input(&owner, self.clients[i].id, input);
                    publish_terminals = true;
                }
                ClientMsg::Viewport { owner, offset } => {
                    if let Some((frame, snapshot)) = self.state.viewport(&owner, offset) {
                        for image in snapshot.images { self.images.insert(image.id, image); }
                        if self.clients[i].link.send(ServerMsg::Terminal(Box::new(frame))).is_err() { detached.push(i); }
                    }
                }
                ClientMsg::RpcReply { request_id, answer } => {
                    if self.pending_rpc.get(&request_id).is_some_and(|request| request.client == self.clients[i].id)
                        && let Some(request) = self.pending_rpc.remove(&request_id)
                    { let _ = request.reply.send(answer); }
                }
                ClientMsg::Notify(command) => {
                    let changes = self.state.execute(command.into_server_action(self.clients[i].id), now);
                    detached.extend(self.publish_notifications(&changes));
                }
                ClientMsg::Reload => {
                    let changes = self.reload_config(now);
                    detached.extend(self.publish_notifications(&changes));
                    publish_terminals = true;
                }
                ClientMsg::Invoke { intent } => {
                    crate::chrome::window::signals::sync_pane_runtime_state(&self.session, &self.store.panes);
                    match self.registry.execute_server_dynamic(&intent.action, self.store.clone(), &intent) {
                        Some(intents) => {
                            if self.clients[i].link.send(ServerMsg::Dispatch { intents }).is_err() { detached.push(i); }
                        }
                        None => detached.push(i),
                    }
                }
                ClientMsg::ImageInfo { id } => {
                    let message = match self.images.get(&id) {
                        Some(image) => {
                            let info = super::wire::image::ImageInfo::of(image);
                            match info.validate(self.connection.max_image_bytes.get()) {
                                Ok(()) => ServerMsg::ImageInfo(info),
                                Err(error) => ServerMsg::ResourceUnavailable { id, reason: error.to_string() },
                            }
                        }
                        None => ServerMsg::ResourceUnavailable { id, reason: "image is no longer available".into() },
                    };
                    if self.clients[i].link.send(message).is_err() { detached.push(i); }
                }
                ClientMsg::ImageChunk { id, frame, offset } => {
                    let message = match self.images.get(&id).map(|image| super::wire::image::chunk(image, frame, offset, self.connection.max_frame_bytes.get())) {
                        Some(Ok(chunk)) => ServerMsg::ImageChunk(chunk),
                        Some(Err(error)) => ServerMsg::ResourceUnavailable { id, reason: error.to_string() },
                        None => ServerMsg::ResourceUnavailable { id, reason: "image is no longer available".into() },
                    };
                    if self.clients[i].link.send(message).is_err() { detached.push(i); }
                }
                ClientMsg::Read { request_id, owner, start, end, cols, query } => {
                    let rows = end.checked_sub(start).and_then(|rows| rows.checked_add(1)).and_then(|rows| usize::try_from(rows).ok());
                    if !rows.is_some_and(|rows| cols > 0 && rows.checked_mul(cols).is_some_and(|cells| cells <= self.connection.max_terminal_cells.get())) {
                        detached.push(i);
                        continue;
                    }
                    let backend = self.state.backends.find_owner(&owner).and_then(|id| self.state.backends.get_by_id(id));
                    let data = match query {
                        Some(query) => super::wire::terminal::ReadData::Matches(backend.map_or_else(Vec::new, |backend| backend.search_stable_range(&query, cols, start, end))),
                        None => super::wire::terminal::ReadData::Lines(backend.map_or_else(Vec::new, |backend| backend.lines_in_stable_range(start, end, cols))),
                    };
                    if self.clients[i].link.send(ServerMsg::Read { request_id, start, data }).is_err() { detached.push(i); }
                }
            }
        }

        let ticks = self.state.tick(now);
        detached.extend(self.publish_notifications(&ticks));
        let poll = self.state.poll_backends();
        let (process_changes, exits) = self.state.reconcile_processes(&mut self.session, poll.closing_panes.clone(), self.rpc_asker);
        detached.extend(self.broadcast(&process_changes));
        self.state.refresh_git(&mut self.session, now);
        for (pane, code) in exits {
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment, Attachment::Attached)
                    && client.link.send(ServerMsg::ProcessExited { pane, code }).is_err()
                { detached.push(i); }
            }
        }
        publish_terminals |= poll.has_data;
        if poll.has_effects() {
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment, Attachment::Attached)
                    && client.link.send(ServerMsg::BackendEffects(poll.clone())).is_err()
                {
                    detached.push(i);
                }
            }
        }

        if publish_terminals {
            let owners: Vec<_> = self.state.backends.owners().map(|(owner, _)| owner).collect();
            for owner in owners {
                let Some((mut frame, snapshot)) = self.state.viewport(&owner, 0) else { continue };
                if let crate::app::backend_store::TerminalOwner::Pane(pane) = owner
                    && let Some(pane) = self.session.workspaces.iter().find_map(|workspace| workspace.find_pane(pane))
                { frame.runtime = pane.runtime.clone(); }
                for image in snapshot.images { self.images.insert(image.id, image); }
                for (i, client) in self.clients.iter().enumerate() {
                    if !detached.contains(&i) && matches!(client.attachment, Attachment::Attached)
                        && client.link.send(ServerMsg::Terminal(Box::new(frame.clone()))).is_err()
                    { detached.push(i); }
                }
            }
        }
        let current: std::collections::HashSet<_> = self.state.backends.owners().map(|(owner, _)| owner).collect();
        let closed: Vec<_> = self.known_terminals.difference(&current).cloned().collect();
        for owner in &closed {
            self.state.terminal_gone(owner);
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment, Attachment::Attached)
                    && client.link.send(ServerMsg::TerminalClosed { owner: owner.clone() }).is_err()
                { detached.push(i); }
            }
        }
        self.known_terminals = current;

        // Drop detached clients, highest index first so the earlier ones keep their positions.
        detached.sort_unstable();
        detached.dedup();
        for i in detached.into_iter().rev() {
            let gone = self.clients.remove(i);
            self.state.client_gone(gone.id);
            self.pending_rpc.retain(|_, request| {
                if request.client != gone.id { return true; }
                let _ = request.reply.send("error: attached client disconnected".into());
                false
            });
        }
        self.pending_rpc.retain(|_, request| {
            if request.deadline > now { return true; }
            let _ = request.reply.send("error: attached client timed out".into());
            false
        });
    }

    /// Run one server action on the shared state for the window that asked, and say what changed.
    fn act(&mut self, action: crate::input::WmAction, asker: Asker) -> Vec<Change> {
        self.state.run(&mut self.session, asker, &action)
    }

    /// Tell every attached client what changed — each reacts to it for itself.
    fn broadcast(&self, changes: &[Change]) -> Vec<usize> {
        self.broadcast_except(changes, None)
    }

    /// Send the current notification projection to every attached client when the visible set
    /// changed. The notification seam does **not** ride the generic [`broadcast`](Self::broadcast):
    /// its dataless `NotificationsChanged` fact would otherwise drag the whole `SessionContent`
    /// across on every notification raise and every auto-dismiss tick. The window builds its own
    /// toasts from the projection, so this is the one message the seam needs.
    fn publish_notifications(&self, changes: &[Change]) -> Vec<usize> {
        let mut disconnected = Vec::new();
        if changes.iter().any(|change| matches!(change, Change::NotificationsChanged)) {
            // Each window gets its own projection: the session-wide notifications plus any addressed
            // to it. A reload toast raised in one window reaches only that window.
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment, Attachment::Attached)
                    && client.link.send(ServerMsg::Notifications(self.state.visible_notifications_for(client.id))).is_err()
                {
                    disconnected.push(i);
                }
            }
        }
        disconnected
    }

    fn broadcast_except(&self, changes: &[Change], reply_to: Option<usize>) -> Vec<usize> {
        let mut disconnected = Vec::new();
        if !changes.is_empty() {
            let content = self.session.content();
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment, Attachment::Attached)
                    && client.link.send(ServerMsg::Content { content: content.clone() }).is_err()
                { disconnected.push(i); }
            }
        }
        for change in changes {
            for (i, client) in self.clients.iter().enumerate() {
                if reply_to != Some(i) && matches!(client.attachment, Attachment::Attached)
                    && client.link.send(ServerMsg::Changed(change.clone())).is_err()
                {
                    disconnected.push(i);
                }
            }
        }
        disconnected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::wire::ClientLink;
    use heca_core::layout::{LayoutOptions, SessionId};

    fn a_loop() -> ServerLoop {
        let session = Session::new(SessionId(1), LayoutOptions::default());
        ServerLoop::new(ServerState::for_tests(Default::default()), session)
    }

    /// A client that attaches is sent the whole content to draw from — the server's current session,
    /// exactly as [`Session::content`] projects it.
    #[test]
    fn attaching_sends_the_content() {
        let mut server = a_loop();
        let client = server.connect();
        let expected = server_content(&server);

        client.send(ClientMsg::Attach).expect("connected");
        server.pump(Instant::now());

        match client.poll().expect("connected") {
            Some(ServerMsg::Attached { content }) => assert_eq!(content, expected),
            other => panic!("attach did not return the content: {other:?}"),
        }
    }

    /// A detached client stops being sent to: after it detaches, a later change reaches no one and
    /// the loop does not panic reaching for a client that is gone.
    #[test]
    fn a_detached_client_is_dropped() {
        let mut server = a_loop();
        let client = server.connect();
        client.send(ClientMsg::Detach).expect("connected");
        server.pump(Instant::now());
        assert!(server.clients.is_empty(), "the detached client was not dropped");
    }

    #[test]
    fn missing_terminal_answers_instead_of_leaving_request_pending() {
        let mut server = a_loop();
        let client = server.connect();
        client.send(ClientMsg::Attach).unwrap();
        server.pump(Instant::now());
        assert!(matches!(client.poll().unwrap(), Some(ServerMsg::Attached { .. })));
        // Attach is also answered with the current notification projection; drain it so the next
        // poll is the reply to the action below.
        assert!(matches!(client.poll().unwrap(), Some(ServerMsg::Notifications(_))));
        client.send(ClientMsg::Act {
            action: crate::input::WmAction::TerminalKill { terminal: Some(1), pane_id: None },
            asker: test_asker(),
            request_id: Some(7),
            terminal_owner: Some(crate::app::backend_store::TerminalOwner::Pane(heca_core::layout::PaneId(999))),
        }).unwrap();
        server.pump(Instant::now());
        assert!(matches!(client.poll().unwrap(), Some(ServerMsg::Reply { request_id: 7, changes })
            if changes == vec![Change::Refused(super::super::Refusal::TerminalUnavailable)]));
        assert_eq!(server.clients.len(), 1);
    }

    #[test]
    fn kill_server_drops_every_client_and_asks_the_loop_to_stop() {
        let mut server = a_loop();
        let client = server.connect();
        client.send(ClientMsg::Attach).unwrap();
        server.pump(Instant::now());
        assert_eq!(server.clients.len(), 1);

        let (reply, answer) = std::sync::mpsc::channel();
        server.begin_rpc(super::KILL_SERVER.to_string(), reply);
        assert!(server.wants_shutdown(), "the server was asked to stop");
        assert!(server.clients.is_empty(), "kill-server drops every client");
        assert_eq!(answer.recv().unwrap(), "heca: server stopped\n");
    }

    #[test]
    fn a_dropped_client_is_removed_without_a_detach_message() {
        let mut server = a_loop();
        let client = server.connect();
        drop(client);
        server.pump(Instant::now());
        assert!(server.clients.is_empty());
    }

    #[test]
    fn an_action_before_attach_closes_the_connection() {
        let mut server = a_loop();
        let client = server.connect();
        client.send(ClientMsg::Act {
            action: crate::input::WmAction::ReloadConfig,
            asker: test_asker(),
            request_id: None,
            terminal_owner: None,
        }).unwrap();
        server.pump(Instant::now());
        assert!(server.clients.is_empty());
        assert!(client.poll().is_err());
    }

    #[test]
    fn detach_prevents_later_queued_actions_from_running() {
        let mut server = a_loop();
        let client = server.connect();
        client.send(ClientMsg::Attach).unwrap();
        client.send(ClientMsg::Detach).unwrap();
        client.send(ClientMsg::Act {
            action: crate::input::WmAction::ReloadConfig,
            asker: test_asker(),
            request_id: None,
            terminal_owner: None,
        }).unwrap();
        server.pump(Instant::now());
        assert!(server.clients.is_empty());
    }

    fn test_asker() -> super::super::Asker {
        use heca_core::layout::{Rectangle, Size};
        super::super::Asker {
            focused_pane: None, workspace: 0, column: 0,
            area: Rectangle::from_size(Size::new(800.0, 600.0)), cell: (8.0, 14.0),
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_socket_attaches_through_the_same_server_loop() {
        use std::num::NonZeroUsize;
        use std::os::unix::net::UnixStream;
        use std::time::Duration;
        use super::super::wire::socket::{SocketClient, SocketServer, TransportLimits};
        let mut server = a_loop();
        let (a, b) = UnixStream::pair().unwrap();
        let limits = TransportLimits { max_frame_bytes: NonZeroUsize::new(65536).unwrap(), queue_capacity: NonZeroUsize::new(8).unwrap(), ..Default::default() };
        let (wake_tx, wake_rx) = std::sync::mpsc::channel();
        let wake = super::super::ServerWaker::new(move || { let _ = wake_tx.send(()); });
        server.add_client(SocketServer::new(a, limits, wake).unwrap());
        let (client_tx, client_rx) = std::sync::mpsc::channel();
        let client = SocketClient::new(b, limits, super::super::ServerWaker::new(move || { let _ = client_tx.send(()); })).unwrap();
        client.send(ClientMsg::Attach).unwrap();
        wake_rx.recv_timeout(Duration::from_secs(3)).expect("server woke");
        server.pump(Instant::now());
        client_rx.recv_timeout(Duration::from_secs(3)).expect("client woke");
        match client.poll().unwrap() {
            Some(ServerMsg::Attached { content }) => assert_eq!(content, server_content(&server)),
            other => panic!("unexpected reply: {other:?}"),
        }
        client.send(ClientMsg::Detach).unwrap();
        wake_rx.recv_timeout(Duration::from_secs(3)).expect("detach woke server");
        server.pump(Instant::now());
        assert!(server.clients.is_empty());
    }

    fn server_content(server: &ServerLoop) -> heca_core::layout::SessionContent {
        server.session.content()
    }
}
