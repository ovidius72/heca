//! A window's connection and received data. The client never starts or owns a PTY.

mod outbox;
mod replica;
mod terminal;
mod transport;
mod images;
mod reads;
mod local;
mod feedback;
mod sessions;
mod titles;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod reconnect_tests;
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

/// Format this window's title from the session and its ordinal (decision e36aba0c): the folder
/// shown home-relative, or the session name when it has none, run through the configured format.
fn format_window_title(
    session: &crate::server::sessions::SessionInfo,
    ordinal: u64,
    base: &str,
    numbered: &str,
) -> String {
    let folder = session
        .folder
        .as_deref()
        .map(crate::chrome::home_relative_path)
        .unwrap_or_else(|| session.name.clone());
    crate::server::sessions::window_title(&folder, ordinal, base, numbered)
}

#[derive(Default)]
pub(crate) struct Update {
    pub(crate) session_attached: bool,
    pub(crate) session_terminated: bool,
    pub(crate) session_deleted: bool,
    pub(crate) session_detached: bool,
    pub(crate) content: Option<SessionContent>,
    /// The notifications on show, when the server sent a fresh projection this poll. The window
    /// builds its own toasts from it; `None` means the set did not change.
    pub(crate) notifications: Option<Vec<crate::notification::NotificationView>>,
    /// The session's window directory, when the server sent a fresh one this poll (join, leave or
    /// title change). The window uses it for the follow picker; `None` means it did not change.
    pub(crate) windows: Option<Vec<crate::server::sessions::WindowInfo>>,
    /// The view of the window this one follows, when a fresh one arrived this poll. The window
    /// mirrors it; `None` means no update.
    pub(crate) follow_view: Option<heca_core::layout::ViewSnapshot>,
    pub(crate) changes: Vec<Change>,
    pub(crate) answers: Vec<Answered>,
    pub(crate) effects: BackendPoll,
    pub(crate) rpc: Vec<(u64, String)>,
    pub(crate) intents: Vec<crate::chrome::Intent>,
    pub(crate) errors: Vec<String>,
    pub(crate) exits: Vec<(heca_core::layout::PaneId, Option<i32>)>,
    pub(crate) reads: Vec<ReadEffect>,
    /// What extension services sent this window this poll, in order.
    pub(crate) services: Vec<crate::server::wire::service::ServiceUpdate>,
}

pub struct ClientState {
    /// The server-assigned physical connection identity, shared by all client/session operations.
    pub client_id: u64,
    /// This connection's stable display ordinal within the selected session.
    pub window_ordinal: u64,
    /// This window's own title, formatted from the session folder and ordinal. The window shows it
    /// and the client reports it to the server for the session directory.
    pub(crate) title: String,
    /// The `[settings.session_text]` title formats, kept so a session switch reformats the title.
    title_fmt: String,
    title_fmt_numbered: String,
    /// Whether another window follows this one — while true, the window reports its view so the
    /// server can relay it to its followers.
    followed: bool,
    /// The window this one follows (mirrors), by id, or `None`.
    following: Option<u64>,
    feedback: feedback::FeedbackView,
    pub(crate) session: crate::server::sessions::SessionInfo,
    pub(crate) sessions: Vec<crate::server::sessions::SessionInfo>,
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
    pub(crate) detached: bool,
    switching: bool,
    pending_selection: Option<ClientMsg>,
    projection_deadline: Option<Instant>,
    wake: ServerWaker,
    connection: heca_config::settings::ConnectionConfig,
}

impl ClientState {
    pub(crate) fn connect(config: &AppConfig, wake: ServerWaker, launch: crate::app::cli::LaunchMode) -> io::Result<(Self, SessionContent)> {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let app_wake = wake.clone();
        let wake = ServerWaker::new(move || { app_wake.wake(); let _ = ready_tx.try_send(()); });
        let (link, mut local): (Box<dyn ClientLink>, _) = if launch == crate::app::cli::LaunchMode::Standalone || !cfg!(unix) {
            let mut server = local::LocalServer::new(config, wake.clone())?;
            let (client, link) = in_process_with_limits(wake.clone(), wake.clone(), config.config.settings.connection);
            server.add_client(link);
            (Box::new(client), Some(server))
        } else {
            (transport::connect(config.config.settings.connection, wake.clone())?, None)
        };
        let target = match launch {
            crate::app::cli::LaunchMode::Session(name) => ClientMsg::AttachTo { name },
            crate::app::cli::LaunchMode::New { path, name } => {
                let folder = crate::server::sessions::resolve_folder(path.as_deref(), &std::env::current_dir()?)?;
                link.send(ClientMsg::Session { request_id: 1, command: crate::server::sessions::SessionCommand::Create { folder, name, label: None } })?;
                match crate::server::wire::await_message(link.as_ref(), &ready_rx, config.config.settings.connection,
                    || { if let Some(server) = &mut local { server.pump(); } })? {
                    ServerMsg::Sessions { result: Ok(crate::server::sessions::SessionReply::Updated(info)), .. } => ClientMsg::AttachTo { name: info.name },
                    ServerMsg::Sessions { result: Err(error), .. } => return Err(io::Error::other(error)),
                    _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected session create reply")),
                }
            }
            _ => ClientMsg::Attach,
        };
        link.send(target)?;
        let (session, client_id, window_ordinal, content) = match crate::server::wire::await_message(link.as_ref(), &ready_rx,
            config.config.settings.connection, || { if let Some(server) = &mut local { server.pump(); } })? {
            ServerMsg::Attached { session, client_id, window_ordinal, content } => (session, client_id, window_ordinal, content),
            ServerMsg::Sessions { result: Err(error), .. } => return Err(io::Error::other(error)),
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "server did not acknowledge attach")),
        };
        let title_fmt = config.config.settings.session_text.window_title.clone();
        let title_fmt_numbered = config.config.settings.session_text.window_title_numbered.clone();
        let title = format_window_title(&session, window_ordinal, &title_fmt, &title_fmt_numbered);
        // Report this window's title so the server's session directory has it from the start.
        link.send(ClientMsg::ReportWindowTitle(title.clone()))?;
        let (outbox, requests) = outbox::channel(config.config.settings.connection.queue_capacity.get(), wake.clone());
        Ok((Self { client_id, window_ordinal, title, title_fmt, title_fmt_numbered, followed: false, following: None, feedback: Default::default(), session, sessions: Vec::new(), backends: BackendStore::new(), programs: Rc::new(config.config.programs.clone()),
            link, local, data: Default::default(), outbox, requests, pending: HashMap::new(), next_request: 1,
            images: Default::default(), image_limit: config.config.settings.connection.max_image_bytes.get(), reads: Default::default(), failed: false,
            detached: false, switching: false, pending_selection: None,
            projection_deadline: Some(Instant::now() + std::time::Duration::from_millis(config.config.settings.connection.request_timeout_ms.get())),
            wake, connection: config.config.settings.connection }, content))
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
        if self.switching || !self.projection_ready()
            || (self.detached && !crate::server::sessions::owns(action.kind())) { return; }
        let request_id = self.next_request;
        self.next_request += 1;
        let terminal = match &action {
            WmAction::TerminalRun { terminal, .. } | WmAction::TerminalKill { terminal, .. } => *terminal,
            WmAction::TerminalRenameTo { terminal, .. } => self.backends.resolve_reference(terminal).map(|id| id.0),
            _ => None,
        };
        let terminal_owner = terminal.and_then(|id| self.backends.owner_of(TerminalId(id))).cloned();
        self.outbox.send(ClientMsg::Act { action: action.clone(), asker, request_id: Some(request_id), terminal_owner });
        self.pending.insert(request_id, Pending { action, name, before });
    }

    /// Whether another window follows this one — the window reports its view while true.
    pub(crate) fn is_followed(&self) -> bool {
        self.followed
    }

    /// The window this one follows (mirrors), by id, or `None`.
    pub(crate) fn following(&self) -> Option<u64> {
        self.following
    }

    /// Follow `target` (mirror its view), or `None` to stop. Breaking is the same call with `None`.
    pub(crate) fn follow(&mut self, target: Option<u64>) {
        self.following = target;
        self.outbox.send(ClientMsg::Follow { target });
    }

    /// Report this window's current view so the server can relay it to any follower. Called only
    /// while [`is_followed`](Self::is_followed), from the window's own loop where the view lives.
    pub(crate) fn report_view(&self, view: heca_core::layout::ViewSnapshot) {
        self.outbox.send(ClientMsg::ReportView(view));
    }

    /// Report where this window is looking — its shown workspace and focused pane — so the server
    /// turns the change into `Focus`/`Workspace` events on the `--events` stream.
    pub(crate) fn report_presence(&self, workspace: usize, pane: Option<heca_core::layout::PaneId>) {
        self.outbox.send(ClientMsg::ReportPresence { workspace, pane });
    }

    pub(crate) fn poll(&mut self) -> io::Result<Update> {
        if self.failed { return Ok(Update::default()); }
        let result = self.poll_connected();
        if result.is_err() { self.failed = true; }
        result
    }

    fn poll_connected(&mut self) -> io::Result<Update> {
        self.outbox.check()?;
        for message in outbox::drain(&self.requests) {
            let allowed_detached = matches!(&message, ClientMsg::Session { .. })
                || matches!(&message, ClientMsg::Act { action, .. } if crate::server::sessions::owns(action.kind()));
            let allowed = !self.switching && (!self.detached || allowed_detached);
            if allowed || matches!(message, ClientMsg::RpcReply { .. }) { self.link.send(message)?; }
        }
        if let Some(selection) = self.pending_selection.take() { self.link.send(selection)?; }
        if let Some(server) = &mut self.local { server.pump(); }
        let mut update = Update::default();
        while let Some(message) = self.link.poll()? {
            match message {
                ServerMsg::Attached { session, client_id, window_ordinal, content } => {
                    update = Update::default();
                    if session.id != self.session.id { self.feedback = Default::default(); }
                    self.reset_projection();
                    self.detached = false;
                    self.session = session;
                    self.client_id = client_id;
                    self.window_ordinal = window_ordinal;
                    // Reformat the title for the (possibly new) session and ordinal and report it.
                    self.title = format_window_title(&self.session, self.window_ordinal, &self.title_fmt, &self.title_fmt_numbered);
                    self.link.send(ClientMsg::ReportWindowTitle(self.title.clone()))?;
                    update.content = Some(content);
                    update.session_attached = true;
                }
                ServerMsg::Content { content } => update.content = Some(content),
                ServerMsg::Windows { session_id, windows } => {
                    if session_id == self.session.id { update.windows = Some(windows); }
                }
                ServerMsg::Followed { active } => self.followed = active,
                ServerMsg::FollowView(view) => update.follow_view = Some(view),
                ServerMsg::ProjectionReady { session_id } => {
                    if session_id == self.session.id {
                        self.projection_deadline = None;
                        update.effects.has_data = true;
                    }
                },
                ServerMsg::Sessions { request_id, result } => match result {
                    Ok(crate::server::sessions::SessionReply::List(sessions)) => {
                        if self.pending.get(&request_id).is_some_and(|pending| matches!(pending.action, WmAction::ListSessions)) {
                            update.intents.push(crate::chrome::Intent::new("command_palette")
                                .arg("mode", crate::chrome::PropValue::Text("session".into())));
                        }
                        if let Some(current) = sessions.iter().find(|session| session.id == self.session.id) { self.session = current.clone(); }
                        self.sessions = sessions;
                    }
                    Ok(crate::server::sessions::SessionReply::Updated(session)) => {
                        if self.pending.get(&request_id).is_some_and(|pending| matches!(pending.action, WmAction::CreateSession { .. })) {
                            self.switch_session(session.name.clone())?;
                        }
                        if session.id == self.session.id { self.session = session; }
                    }
                    Ok(crate::server::sessions::SessionReply::Deleted(_)) => {},
                    Err(error) => { self.switching = false; update.errors.push(error); },
                },
                ServerMsg::SessionTerminated { session } => {
                    self.projection_deadline = None;
                    self.switching = false;
                    self.session = session;
                    self.pending.clear();
                    update.session_terminated = true;
                }
                ServerMsg::SessionDeleted { id } | ServerMsg::SessionDetached { id } => {
                    if id == self.session.id {
                        let deleted = matches!(message, ServerMsg::SessionDeleted { .. });
                        self.reset_projection();
                        self.projection_deadline = None;
                        self.detached = true;
                        self.window_ordinal = 0;
                        self.feedback = Default::default();
                        update = Update::default();
                        update.session_deleted = deleted;
                        update.session_detached = !deleted;
                    }
                }
                ServerMsg::Notifications(views) => update.notifications = Some(views),
                ServerMsg::Feedback { session_id, panes } => {
                    if session_id == self.session.id {
                        self.feedback.receive(panes);
                        update.effects.has_data = true;
                    }
                }
                ServerMsg::FeedbackSound { session_id, pane_id, revision } => {
                    if session_id == self.session.id {
                        self.feedback.grant_sound(pane_id, revision);
                        update.effects.has_data = true;
                    }
                }
                ServerMsg::Changed(change) => update.changes.push(change),
                ServerMsg::Refused(reason) => update.changes.push(Change::Refused(reason)),
                ServerMsg::Reply { request_id, changes } => {
                    if let Some(pending) = self.pending.remove(&request_id) {
                        update.answers.push(Answered { action: pending.action, name: pending.name, before: pending.before, changes });
                    }
                }
                ServerMsg::Rpc { request_id, line } => update.rpc.push((request_id, line)),
                ServerMsg::Dispatch { intents } => update.intents.extend(intents),
                ServerMsg::Service(service) => update.services.push(service),
                ServerMsg::BackendEffects(effects) => {
                    update.effects.has_data |= effects.has_data;
                    update.effects.bell_any |= effects.bell_any;
                    update.effects.clipboard_writes.extend(effects.clipboard_writes);
                }
                ServerMsg::Terminal(frame) => {
                    for placement in &frame.screen.graphics { self.images.need(placement.image_id, &self.outbox); }
                    let (owner, spec, cell) = (frame.owner.clone(), frame.spec.clone(), (frame.screen.cell_w, frame.screen.cell_h));
                    let display_title = frame.display_title.clone();
                    self.data.lock().map_err(|error| io::Error::other(error.to_string()))?.receive(*frame);
                    self.ensure_view(owner.clone(), spec, cell).map_err(|error| io::Error::other(error.to_string()))?;
                    if let Some(id) = self.backends.find_owner(&owner) { self.backends.set_custom_title(id, display_title); }
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
        if self.projection_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "session projection did not finish"));
        }
        self.receive_titles(&update);
        Ok(update)
    }

    pub(crate) fn terminal_view(&mut self, id: TerminalId, scale: f32) -> Option<(TerminalSnapshot, TerminalDamage)> {
        let backend = self.backends.get_mut_by_id(id)?;
        backend.set_scale_factor(scale);
        Some((backend.terminal_snapshot()?, backend.take_terminal_damage()))
    }

    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.local.as_ref().and_then(local::LocalServer::next_wake)
            .into_iter().chain(self.projection_deadline.filter(|_| !self.failed)).min()
    }

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

    /// This window's copy of `service` fell out of step: ask the server for a fresh snapshot.
    pub(crate) fn resync_service(&self, service: &str) {
        self.outbox.send(ClientMsg::ServiceResync { service: service.to_owned() });
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
            Some(WmAction::TerminalRename { terminal } | WmAction::TerminalRenameTo { terminal, .. }) => {
                // Numeric RPC references are server IDs; qualified names resolve to this view's owner.
                let local = match terminal.parse::<u64>() {
                    Ok(server_id) => self.local_terminal(server_id),
                    Err(_) => self.backends.resolve_reference(&terminal).map(|id| id.0),
                }.ok_or_else(|| crate::rpc::RpcError::NotRunnable(intent.action.clone()))?;
                intent.args.insert("terminal".into(), crate::chrome::PropValue::Int(local as i64));
                return Ok(());
            }
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
    fn drop(&mut self) {
        // A close action can end the window before the next poll flushes its RPC answer.
        while let Ok(message) = self.requests.try_recv() {
            if matches!(message, ClientMsg::RpcReply { .. }) { let _ = self.link.send(message); }
        }
        let _ = self.link.send(ClientMsg::Detach);
        let _ = self.link.flush();
    }
}
