//! A window's connection and received data. The client never starts or owns a PTY.

mod outbox;
mod replica;
mod terminal;
mod transport;
mod images;
mod reads;
mod local;
pub(crate) use reads::ReadEffect;

use std::{collections::HashMap, io, rc::Rc, sync::mpsc, time::Instant};
use heca_config::{loader::AppConfig, programs::ProgramsConfig};
use heca_core::{backend::{TerminalDamage, TerminalSnapshot}, layout::{Before, SessionContent}};
use crate::{app::backend_store::{BackendStore, Ensured, SpawnError, TerminalOwner, TerminalSpec},
    chrome::panes::terminal::TerminalId,
    input::WmAction, server::{Asker, BackendPoll, Change, ServerWaker,
        wire::{ClientLink, ClientMsg, ServerMsg, in_process_with_limits}}};
use outbox::Outbox;
use replica::SharedReplica;

pub(crate) struct Answered {
    pub(crate) action: WmAction,
    pub(crate) name: String,
    pub(crate) before: Before,
    pub(crate) changes: Vec<Change>,
}

struct Pending { action: WmAction, name: String, before: Before }

#[derive(Default)]
pub(crate) struct Update {
    pub(crate) content: Option<SessionContent>,
    /// The notifications on show, when the server sent a fresh projection this poll. The window
    /// builds its own toasts from it; `None` means the set did not change.
    pub(crate) notifications: Option<Vec<crate::notification::NotificationView>>,
    pub(crate) changes: Vec<Change>,
    pub(crate) answers: Vec<Answered>,
    pub(crate) effects: BackendPoll,
    pub(crate) rpc: Vec<(u64, String)>,
    pub(crate) intents: Vec<crate::chrome::Intent>,
    pub(crate) errors: Vec<String>,
    pub(crate) exits: Vec<(heca_core::layout::PaneId, Option<i32>)>,
    pub(crate) reads: Vec<ReadEffect>,
}

pub struct ClientState {
    /// Client-side terminal views only, never emulators or process handles.
    pub backends: BackendStore,
    pub programs: Rc<ProgramsConfig>,
    link: Box<dyn ClientLink>,
    local: Option<local::LocalServer>,
    data: SharedReplica,
    outbox: Outbox,
    requests: mpsc::Receiver<ClientMsg>,
    pending: HashMap<u64, Pending>,
    next_request: u64,
    images: images::Images,
    image_limit: usize,
    reads: reads::Reads,
    failed: bool,
}

impl ClientState {
    pub(crate) fn connect(config: &AppConfig, wake: ServerWaker, standalone: bool) -> io::Result<(Self, SessionContent)> {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let app_wake = wake.clone();
        let wake = ServerWaker::new(move || { app_wake.wake(); let _ = ready_tx.try_send(()); });
        let (link, mut local): (Box<dyn ClientLink>, _) = if standalone || !cfg!(unix) {
            let mut server = local::LocalServer::new(config, wake.clone())?;
            let (client, link) = in_process_with_limits(wake.clone(), wake.clone(), config.config.settings.connection);
            server.add_client(link);
            (Box::new(client), Some(server))
        } else {
            (transport::connect(config.config.settings.connection, wake.clone())?, None)
        };
        link.send(ClientMsg::Attach)?;
        let deadline = Instant::now() + std::time::Duration::from_millis(config.config.settings.connection.request_timeout_ms.get());
        let content = loop {
            if let Some(server) = &mut local { server.pump(); }
            if let Some(message) = link.poll()? {
                if let ServerMsg::Attached { content } = message { break content; }
                return Err(io::Error::new(io::ErrorKind::InvalidData, "server did not acknowledge attach"));
            }
            ready_rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|error| io::Error::new(io::ErrorKind::TimedOut, error))?;
        };
        let (outbox, requests) = outbox::channel(config.config.settings.connection.queue_capacity.get(), wake);
        Ok((Self { backends: BackendStore::new(), programs: Rc::new(config.config.programs.clone()),
            link, local, data: Default::default(), outbox, requests, pending: HashMap::new(), next_request: 1,
            images: Default::default(), image_limit: config.config.settings.connection.max_image_bytes.get(), reads: Default::default(), failed: false }, content))
    }

    pub(crate) fn start_terminal(&mut self, owner: impl Into<TerminalOwner>, spec: TerminalSpec, cell: (f32, f32)) -> Result<Ensured, SpawnError> {
        let owner = owner.into();
        let ensured = self.ensure_view(owner.clone(), spec.clone(), cell)?;
        if matches!(ensured, Ensured::Spawned(_)) {
            self.outbox.send(ClientMsg::EnsureTerminal { owner, spec, cell });
        }
        Ok(ensured)
    }

    fn ensure_view(&mut self, owner: TerminalOwner, spec: TerminalSpec, cell: (f32, f32)) -> Result<Ensured, SpawnError> {
        let (data, outbox) = (self.data.clone(), self.outbox.clone());
        self.backends.ensure(owner.clone(), spec, move |spec| {
            Ok(Box::new(terminal::TerminalView::new(owner, spec, data, outbox, cell)))
        })
    }

    pub(crate) fn act(&mut self, action: WmAction, asker: Asker, name: String, before: Before) {
        let request_id = self.next_request;
        self.next_request += 1;
        let terminal = match &action {
            WmAction::TerminalRun { terminal, .. } | WmAction::TerminalKill { terminal, .. } => *terminal,
            _ => None,
        };
        let terminal_owner = terminal.and_then(|id| self.backends.owner_of(TerminalId(id))).cloned();
        self.outbox.send(ClientMsg::Act { action: action.clone(), asker, request_id: Some(request_id), terminal_owner });
        self.pending.insert(request_id, Pending { action, name, before });
    }

    pub(crate) fn poll(&mut self) -> io::Result<Update> {
        if self.failed { return Ok(Update::default()); }
        let result = self.poll_connected();
        if result.is_err() { self.failed = true; }
        result
    }

    fn poll_connected(&mut self) -> io::Result<Update> {
        self.outbox.check()?;
        while let Ok(message) = self.requests.try_recv() { self.link.send(message)?; }
        if let Some(server) = &mut self.local { server.pump(); }
        let mut update = Update::default();
        while let Some(message) = self.link.poll()? {
            match message {
                ServerMsg::Attached { content } | ServerMsg::Content { content } => update.content = Some(content),
                ServerMsg::Notifications(views) => update.notifications = Some(views),
                ServerMsg::Changed(change) => update.changes.push(change),
                ServerMsg::Refused(reason) => update.changes.push(Change::Refused(reason)),
                ServerMsg::Reply { request_id, changes } => {
                    if let Some(pending) = self.pending.remove(&request_id) {
                        update.answers.push(Answered { action: pending.action, name: pending.name, before: pending.before, changes });
                    }
                }
                ServerMsg::Rpc { request_id, line } => update.rpc.push((request_id, line)),
                ServerMsg::Dispatch { intents } => update.intents.extend(intents),
                ServerMsg::BackendEffects(effects) => {
                    update.effects.has_data |= effects.has_data;
                    update.effects.bell_any |= effects.bell_any;
                    update.effects.clipboard_writes.extend(effects.clipboard_writes);
                }
                ServerMsg::Terminal(frame) => {
                    for placement in &frame.screen.graphics { self.images.need(placement.image_id, &self.outbox); }
                    self.ensure_view(frame.owner.clone(), frame.spec.clone(), (frame.screen.cell_w, frame.screen.cell_h))
                        .map_err(|error| io::Error::other(error.to_string()))?;
                    self.data.lock().map_err(|error| io::Error::other(error.to_string()))?.receive(*frame);
                    update.effects.has_data = true;
                }
                ServerMsg::ImageInfo(info) => self.images.begin(info, self.image_limit, &self.outbox)?,
                ServerMsg::ImageChunk(chunk) => {
                    if let Some(image) = self.images.receive(chunk, &self.outbox)? {
                        self.data.lock().map_err(|error| io::Error::other(error.to_string()))?.receive_image(image);
                        update.effects.has_data = true;
                    }
                }
                ServerMsg::ResourceUnavailable { id, reason } => {
                    self.images.unavailable(id);
                    update.errors.push(reason);
                }
                ServerMsg::ProcessExited { pane, code } => update.exits.push((pane, code)),
                ServerMsg::Read { request_id, start, data } => {
                    if let Some(effect) = self.reads.receive(request_id, start, data, &self.outbox)? { update.reads.push(effect); }
                }
                ServerMsg::TerminalClosed { owner } => {
                    if let Some(id) = self.backends.find_owner(&owner) { self.backends.kill(id); }
                    self.data.lock().map_err(|error| io::Error::other(error.to_string()))?.terminals.retain(|(found, _), _| found != &owner);
                    update.effects.has_data = true;
                }
            }
        }
        Ok(update)
    }

    pub(crate) fn terminal_view(&mut self, id: TerminalId, scale: f32) -> Option<(TerminalSnapshot, TerminalDamage)> {
        let backend = self.backends.get_mut_by_id(id)?;
        backend.set_scale_factor(scale);
        Some((backend.terminal_snapshot()?, backend.take_terminal_damage()))
    }

    pub(crate) fn next_wake(&self) -> Option<Instant> { self.local.as_ref().and_then(local::LocalServer::next_wake) }

    pub(crate) fn poll_views(&mut self) -> BackendPoll {
        let mut result = BackendPoll::default();
        for view in self.backends.values_mut() {
            result.has_data |= view.update();
            result.terminal_animating |= view.tick_animation();
        }
        result
    }

    pub(crate) fn reply_rpc(&self, request_id: u64, answer: String) {
        self.outbox.send(ClientMsg::RpcReply { request_id, answer });
    }

    /// Ask the server to raise/dismiss/hover a notification. The server owns the queue, timers and
    /// dedup and sends back a fresh projection ([`ServerMsg::Notifications`]).
    pub(crate) fn notify(&self, command: crate::server::NotificationCommand) {
        self.outbox.send(ClientMsg::Notify(command));
    }

    /// Tell the server the config changed on disk, so it re-reads and re-applies its own terminal
    /// and notification settings. The window keeps only its own view-side config.
    pub(crate) fn reload(&self) {
        self.outbox.send(ClientMsg::Reload);
    }

    pub(crate) fn invoke(&self, intent: crate::chrome::Intent) {
        self.outbox.send(ClientMsg::Invoke { intent });
    }

    pub(crate) fn install_actions(&mut self, registry: crate::actions::ActionRegistry, catalog: crate::actions::ActionCatalog) {
        if let Some(server) = &mut self.local { server.install_actions(registry, catalog); }
    }

    pub(crate) fn read_lines(&mut self, terminal: TerminalId, start: isize, end: isize,
        complete: impl FnOnce(Vec<heca_core::backend::TerminalLine>) -> ReadEffect + 'static) {
        let Some(owner) = self.backends.owner_of(terminal).cloned() else { return };
        let Some(snapshot) = self.backends.get_by_id(terminal).and_then(|backend| backend.terminal_snapshot()) else { return };
        self.reads.lines(owner, &snapshot, start, end, complete, &self.outbox);
    }

    pub(crate) fn search(&mut self, terminal: TerminalId, query: String) {
        let Some(owner) = self.backends.owner_of(terminal).cloned() else { return };
        let Some(snapshot) = self.backends.get_by_id(terminal).and_then(|backend| backend.terminal_snapshot()) else { return };
        self.reads.search(terminal, owner, &snapshot, query, &self.outbox);
    }

    pub(crate) fn cancel_search(&mut self, terminal: TerminalId) { self.reads.cancel_search(terminal); }

    pub(crate) fn prune(&mut self, content: &SessionContent) {
        let live: std::collections::HashSet<_> = content.workspaces.iter().flat_map(|workspace| {
            workspace.columns.iter().flat_map(|column| column.panes.iter().map(|pane| pane.id))
                .chain(workspace.floating.iter().map(|floating| floating.pane.id))
        }).collect();
        let gone: Vec<_> = self.backends.owners().filter_map(|(owner, _)| match owner {
            TerminalOwner::Pane(pane) if !live.contains(&pane) => Some(pane), _ => None,
        }).collect();
        for pane in gone { self.backends.kill_for_pane(pane); }
        self.data.lock().unwrap_or_else(|error| error.into_inner()).terminals.retain(|(owner, _), _| match owner {
            TerminalOwner::Pane(pane) => live.contains(pane), TerminalOwner::Named(_) => true,
        });
    }

    pub(crate) fn local_terminal(&self, server_id: u64) -> Option<u64> {
        let data = self.data.lock().ok()?;
        let (_, frame) = data.terminals.values().find(|(_, frame)| frame.id == server_id)?;
        self.backends.find_owner(&frame.owner).map(|id| id.0)
    }

    pub(crate) fn normalize_server_intent(&self, intent: &mut crate::chrome::Intent) -> Result<(), crate::rpc::RpcError> {
        let args = crate::app::interaction::intent_args_as_strings(intent);
        let action = crate::input::resolve_action(&intent.action, &args);
        let terminal = match action {
            Some(WmAction::TerminalRun { terminal, .. } | WmAction::TerminalKill { terminal, .. }
                | WmAction::PaneTerminalFontZoom { terminal, .. }) => terminal,
            _ => None,
        };
        if let Some(server_id) = terminal {
            let local = self.local_terminal(server_id).ok_or_else(|| crate::rpc::RpcError::NotRunnable(intent.action.clone()))?;
            intent.args.insert("terminal".into(), crate::chrome::PropValue::Int(local as i64));
        }
        Ok(())
    }
}

impl Drop for ClientState {
    fn drop(&mut self) { let _ = self.link.send(ClientMsg::Detach); }
}
