//! One session's state, processes and attached clients, without a window.
//! Driven by the server's owner loop through [`SessionRuntime::pump`].
//!
//! A pump is one turn of the loop: take what each client sent, do it, send back what changed. It
//! needs no winit and no threads, so it runs and is tested headless. The headless `heca --server`
//! wraps it in a thread that pumps when its mpsc wakes (a client message, a terminal's output, a
//! timer); standalone pumps it from the window's loop. One loop body, both ways.

use std::rc::Rc;
use std::time::Instant;

use heca_core::layout::Session;

use super::connection::{Attachment, Connection};
use super::sessions::{SessionInfo, SessionStatus};
use super::wire::{ClientMsg, ServerMsg};
use super::{Asker, Change, ServerState};

mod lifecycle;
mod publish;
mod rpc;

/// Everything a session is and the clients watching it.
pub(crate) struct SessionRuntime {
    state: ServerState,
    session: Session,
    identity: SessionInfo,
    saved_terminals: Vec<super::sessions::RecordedTerminal>,
    resume_programs: bool,
    resume_harness: bool,
    /// The server end of each attached client. A detached client is dropped from here.
    clients: Vec<Rc<Connection>>,
    next_window_ordinal: u64,
    registry: crate::actions::ActionRegistry,
    catalog: crate::actions::ActionCatalog,
    store: crate::chrome::SharedChromeState,
    confirm: heca_config::confirm::ConfirmConfig,
    rpc_asker: Asker,
    pending_rpc: std::collections::HashMap<u64, PendingRpc>,
    next_rpc: u64,
    connection: heca_config::settings::ConnectionConfig,
    images: std::collections::HashMap<u64, heca_core::backend::TerminalImage>,
    known_terminals: std::collections::HashSet<crate::app::backend_store::TerminalOwner>,
}

struct PendingRpc {
    client: u64,
    reply: std::sync::mpsc::Sender<String>,
    deadline: Instant,
}

impl SessionRuntime {
    pub(crate) fn new(state: ServerState, session: Session) -> Self {
        let defaults = heca_config::settings::SettingsConfig::default();
        let registry = crate::actions::ActionRegistry::new();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let store = crate::chrome::SharedChromeState::new(0.0, 0.0);
        store.workspaces.set_programs(state.programs.clone());
        let view = heca_core::layout::WindowView::new(Default::default(), 1.0);
        let rpc_asker = Asker::seen_through(
            session.through(&view),
            None,
            heca_core::backend::DEFAULT_CELL_SIZE,
        );
        Self {
            state,
            identity: SessionInfo::unnamed(session.id, &defaults.session_name_format, None),
            saved_terminals: Vec::new(),
            resume_programs: defaults.resume_programs,
            resume_harness: defaults.resume_harness,
            session,
            clients: Vec::new(),
            next_window_ordinal: 1,
            registry,
            catalog,
            store,
            rpc_asker,
            confirm: Default::default(),
            pending_rpc: Default::default(),
            next_rpc: 1,
            connection: Default::default(),
            images: Default::default(),
            known_terminals: Default::default(),
        }
    }

    pub(crate) fn install_actions(
        &mut self,
        registry: crate::actions::ActionRegistry,
        catalog: crate::actions::ActionCatalog,
    ) {
        self.registry = registry;
        self.catalog = catalog;
    }

    pub(crate) fn shutdown(&mut self) {
        for client in &self.clients {
            client.close();
        }
        self.clients.clear();
        self.pending_rpc.clear();
        self.state.backends.clear();
    }

    pub(super) fn raise(&mut self, draft: crate::notification::NotificationDraft) {
        let changes = self
            .state
            .execute(super::ServerAction::Raise(draft), Instant::now());
        let _ = self.publish_notifications(&changes);
    }

    /// Re-read the config file and re-apply the server's own terminal and notification settings —
    /// the `prefix+Shift+r` path. The window hands nothing over: it reloads its view-side config and
    /// sends [`ClientMsg::Reload`], and the server (which runs as the same local user) reads the
    /// same file itself. A load error keeps the current settings. Returns the notification changes.
    fn reload_config(&mut self, now: Instant) -> Vec<Change> {
        let loaded = self.identity.folder.as_deref().map_or_else(
            heca_config::loader::AppConfig::try_load,
            heca_config::loader::AppConfig::try_load_for,
        );
        let Ok(config) = loaded else {
            return Vec::new();
        };
        self.configure_recovery(&config.config.settings);
        let terminals = crate::app::backend_factory::TerminalSettings::from_config(&config);
        let _ = self.state.execute(
            super::ServerAction::ConfigureTerminals(Box::new(terminals)),
            now,
        );
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

    pub(super) fn add_connection(&mut self, client: Rc<Connection>) {
        let Some(next) = self.next_window_ordinal.checked_add(1) else {
            client.close();
            return;
        };
        client.window_ordinal.set(self.next_window_ordinal);
        self.next_window_ordinal = next;
        self.clients.push(client);
    }

    pub(super) fn detach_connection(&mut self, id: u64) {
        self.clients.retain(|client| client.id != id);
        self.pending_rpc.retain(|_, request| {
            if request.client != id {
                return true;
            }
            let _ = request.reply.send("error: client switched session".into());
            false
        });
    }

    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.pending_rpc
            .values()
            .map(|request| request.deadline)
            .chain(self.state.next_wake())
            .min()
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
                match client.poll() {
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
                && !matches!(self.clients[i].attachment.get(), Attachment::Attached)
            {
                detached.push(i);
                continue;
            }
            match msg {
                ClientMsg::Attach => {
                    let content = self.session.content();
                    self.clients[i].attachment.set(Attachment::Attached);
                    if self.clients[i]
                        .link
                        .send(ServerMsg::Attached {
                            session: self.info(),
                            client_id: self.clients[i].id,
                            window_ordinal: self.clients[i].window_ordinal.get(),
                            content,
                        })
                        .is_err()
                    {
                        detached.push(i);
                    } else {
                        self.clients[i].attachment.set(Attachment::Attached);
                        if self.clients[i].link.send(ServerMsg::Feedback {
                            session_id: self.session.id,
                            panes: self.state.feedback.snapshot(),
                        }).is_err() { detached.push(i); }
                        // The attaching (or reattaching) window gets the current notifications so
                        // its toast stack comes back exactly as the session left it.
                        let views = self.state.visible_notifications();
                        if self.clients[i]
                            .link
                            .send(ServerMsg::Notifications(views))
                            .is_err()
                        {
                            detached.push(i);
                        }
                        publish_terminals = true;
                    }
                }
                ClientMsg::Detach => detached.push(i),
                ClientMsg::AttachTo { .. } | ClientMsg::Session { .. } => detached.push(i),
                ClientMsg::Act {
                    mut action,
                    asker,
                    request_id,
                    terminal_owner,
                } => {
                    if !matches!(self.clients[i].attachment.get(), Attachment::Attached)
                        || !ServerState::runs(action.kind())
                    {
                        detached.push(i);
                        continue;
                    }
                    if let Some(owner) = terminal_owner {
                        let Some(id) = self.state.backends.find_owner(&owner) else {
                            let refusal = super::Refusal::TerminalUnavailable;
                            let message = match request_id {
                                Some(request_id) => ServerMsg::Reply {
                                    request_id,
                                    changes: vec![Change::Refused(refusal)],
                                },
                                None => ServerMsg::Refused(refusal),
                            };
                            if self.clients[i].link.send(message).is_err() {
                                detached.push(i);
                            }
                            continue;
                        };
                        match &mut action {
                            crate::input::WmAction::TerminalRun { terminal, .. }
                            | crate::input::WmAction::TerminalKill { terminal, .. } => {
                                *terminal = Some(id.0)
                            }
                            _ => {}
                        }
                    }
                    let changes = self.act(action, asker);
                    match request_id {
                        Some(request_id) => {
                            detached.extend(self.broadcast_except(&changes, Some(i)));
                            if self.clients[i]
                                .link
                                .send(ServerMsg::Reply {
                                    request_id,
                                    changes,
                                })
                                .is_err()
                            {
                                detached.push(i);
                            }
                        }
                        None => detached.extend(self.broadcast(&changes)),
                    }
                    publish_terminals = true;
                }
                ClientMsg::EnsureTerminal { owner, spec, cell } => {
                    if !super::stream::valid_grid(
                        spec.grid.0,
                        spec.grid.1,
                        cell,
                        1.0,
                        self.connection.max_terminal_cells.get(),
                    ) {
                        detached.push(i);
                        continue;
                    }
                    if self.state.terminals().start(owner, spec, cell).is_err() {
                        detached.push(i);
                    }
                    publish_terminals = true;
                }
                ClientMsg::Terminal { owner, input } => {
                    if let super::wire::terminal::ProcessInput::Resize {
                        cols,
                        rows,
                        cell,
                        scale,
                    } = &input
                        && !super::stream::valid_grid(
                            *cols,
                            *rows,
                            *cell,
                            *scale,
                            self.connection.max_terminal_cells.get(),
                        )
                    {
                        detached.push(i);
                        continue;
                    }
                    self.state.process_input(self.clients[i].id, &owner, input);
                    publish_terminals = true;
                }
                ClientMsg::Viewport { owner, offset } => {
                    if let Some((frame, snapshot)) = self.state.viewport(&owner, offset) {
                        for image in snapshot.images {
                            self.images.insert(image.id, image);
                        }
                        if self.clients[i]
                            .link
                            .send(ServerMsg::Terminal(Box::new(frame)))
                            .is_err()
                        {
                            detached.push(i);
                        }
                    }
                }
                ClientMsg::RpcReply { request_id, answer } => {
                    if self
                        .pending_rpc
                        .get(&request_id)
                        .is_some_and(|request| request.client == self.clients[i].id)
                        && let Some(request) = self.pending_rpc.remove(&request_id)
                    {
                        let _ = request.reply.send(answer);
                    }
                }
                ClientMsg::Notify(command) => {
                    let changes = self.state.execute(command.into_server_action(), now);
                    detached.extend(self.publish_notifications(&changes));
                }
                ClientMsg::AcknowledgeFeedback { session_id, pane_id, revision } => {
                    if session_id == self.session.id {
                        self.state.feedback.acknowledge(pane_id, revision);
                    }
                }
                ClientMsg::RequestFeedbackSound { session_id, pane_id, revision } => {
                    if session_id == self.session.id && self.state.feedback.claim_sound(pane_id, revision) {
                        let _ = self.clients[i].link.send(ServerMsg::FeedbackSound { session_id, pane_id, revision });
                    }
                }
                ClientMsg::Reload => {
                    let changes = self.reload_config(now);
                    detached.extend(self.publish_notifications(&changes));
                    publish_terminals = true;
                }
                ClientMsg::Invoke { intent } => {
                    crate::chrome::window::signals::sync_pane_runtime_state(
                        &self.session,
                        &self.store.panes,
                    );
                    match self.registry.execute_server_dynamic(
                        &intent.action,
                        self.store.clone(),
                        &intent,
                    ) {
                        Some(intents) => {
                            if self.clients[i]
                                .link
                                .send(ServerMsg::Dispatch { intents })
                                .is_err()
                            {
                                detached.push(i);
                            }
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
                                Err(error) => ServerMsg::ResourceUnavailable {
                                    id,
                                    reason: error.to_string(),
                                },
                            }
                        }
                        None => ServerMsg::ResourceUnavailable {
                            id,
                            reason: "image is no longer available".into(),
                        },
                    };
                    if self.clients[i].link.send(message).is_err() {
                        detached.push(i);
                    }
                }
                ClientMsg::ImageChunk { id, frame, offset } => {
                    let message = match self.images.get(&id).map(|image| {
                        super::wire::image::chunk(
                            image,
                            frame,
                            offset,
                            self.connection.max_frame_bytes.get(),
                        )
                    }) {
                        Some(Ok(chunk)) => ServerMsg::ImageChunk(chunk),
                        Some(Err(error)) => ServerMsg::ResourceUnavailable {
                            id,
                            reason: error.to_string(),
                        },
                        None => ServerMsg::ResourceUnavailable {
                            id,
                            reason: "image is no longer available".into(),
                        },
                    };
                    if self.clients[i].link.send(message).is_err() {
                        detached.push(i);
                    }
                }
                ClientMsg::Read {
                    request_id,
                    owner,
                    start,
                    end,
                    cols,
                    query,
                } => {
                    let rows = end
                        .checked_sub(start)
                        .and_then(|rows| rows.checked_add(1))
                        .and_then(|rows| usize::try_from(rows).ok());
                    if !rows.is_some_and(|rows| {
                        cols > 0
                            && rows.checked_mul(cols).is_some_and(|cells| {
                                cells <= self.connection.max_terminal_cells.get()
                            })
                    }) {
                        detached.push(i);
                        continue;
                    }
                    let backend = self
                        .state
                        .backends
                        .find_owner(&owner)
                        .and_then(|id| self.state.backends.get_by_id(id));
                    let data = match query {
                        Some(query) => super::wire::terminal::ReadData::Matches(
                            backend.map_or_else(Vec::new, |backend| {
                                backend.search_stable_range(&query, cols, start, end)
                            }),
                        ),
                        None => super::wire::terminal::ReadData::Lines(
                            backend.map_or_else(Vec::new, |backend| {
                                backend.lines_in_stable_range(start, end, cols)
                            }),
                        ),
                    };
                    if self.clients[i]
                        .link
                        .send(ServerMsg::Read {
                            request_id,
                            start,
                            data,
                        })
                        .is_err()
                    {
                        detached.push(i);
                    }
                }
            }
        }

        let ticks = self.state.tick(now);
        detached.extend(self.publish_notifications(&ticks));
        let poll = self.state.poll_backends();
        self.state.collect_feedback(&self.session);
        let (process_changes, exits) = self.state.reconcile_processes(
            &mut self.session,
            poll.closing_panes.clone(),
            self.rpc_asker,
        );
        detached.extend(self.broadcast(&process_changes));
        detached.extend(self.publish_feedback());
        self.state.refresh_git(&mut self.session, now);
        for (pane, code) in exits {
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment.get(), Attachment::Attached)
                    && client
                        .link
                        .send(ServerMsg::ProcessExited { pane, code })
                        .is_err()
                {
                    detached.push(i);
                }
            }
        }
        publish_terminals |= poll.has_data;
        if poll.has_effects() {
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment.get(), Attachment::Attached)
                    && client
                        .link
                        .send(ServerMsg::BackendEffects(poll.clone()))
                        .is_err()
                {
                    detached.push(i);
                }
            }
        }

        if publish_terminals {
            let owners: Vec<_> = self
                .state
                .backends
                .owners()
                .map(|(owner, _)| owner)
                .collect();
            for owner in owners {
                let Some((mut frame, snapshot)) = self.state.viewport(&owner, 0) else {
                    continue;
                };
                if let crate::app::backend_store::TerminalOwner::Pane(pane) = owner
                    && let Some(pane) = self
                        .session
                        .workspaces
                        .iter()
                        .find_map(|workspace| workspace.find_pane(pane))
                {
                    frame.runtime = pane.runtime.clone();
                }
                for image in snapshot.images {
                    self.images.insert(image.id, image);
                }
                for (i, client) in self.clients.iter().enumerate() {
                    if !detached.contains(&i)
                        && matches!(client.attachment.get(), Attachment::Attached)
                        && client
                            .link
                            .send(ServerMsg::Terminal(Box::new(frame.clone())))
                            .is_err()
                    {
                        detached.push(i);
                    }
                }
            }
        }
        let current: std::collections::HashSet<_> = self
            .state
            .backends
            .owners()
            .map(|(owner, _)| owner)
            .collect();
        for owner in self.known_terminals.difference(&current) {
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment.get(), Attachment::Attached)
                    && client
                        .link
                        .send(ServerMsg::TerminalClosed {
                            owner: owner.clone(),
                        })
                        .is_err()
                {
                    detached.push(i);
                }
            }
        }
        self.known_terminals = current;

        // Drop detached clients, highest index first so the earlier ones keep their positions.
        detached.sort_unstable();
        detached.dedup();
        for i in detached.into_iter().rev() {
            let gone = self.clients.remove(i);
            gone.close();
            self.pending_rpc.retain(|_, request| {
                if request.client != gone.id {
                    return true;
                }
                let _ = request
                    .reply
                    .send("error: attached client disconnected".into());
                false
            });
        }
        self.pending_rpc.retain(|_, request| {
            if request.deadline > now {
                return true;
            }
            let _ = request
                .reply
                .send("error: attached client timed out".into());
            false
        });
    }

    /// Run one server action on the shared state for the window that asked, and say what changed.
    fn act(&mut self, action: crate::input::WmAction, asker: Asker) -> Vec<Change> {
        self.state.run(&mut self.session, asker, &action)
    }
}
