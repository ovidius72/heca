//! **The services extensions run inside the server** — started once per server, fed from their
//! inboxes, asked what each window is shown, and stopped once at shutdown.
//!
//! The authoring side is [`crate::entry::service`]; this is the host half. The server owns one
//! [`ServiceHost`] and shares it with its sessions, so an action from a window reaches it from the
//! session that received it, and an action from a script from the server.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use heca_config::loader::AppConfig;
use heca_core::layout::SessionId;

use crate::chrome::Intent;
use crate::entry::service::{
    ActionResult, ProjectCx, ServerEvent, ServiceTicket, SessionKey, Sessions, Stop,
};

use super::ServerWaker;
use super::connection::{Attachment, Connection};
use super::identity::ServerIdentity;
use super::wire::{
    ServerMsg,
    service::{ServiceUpdate, ServiceUpdateKind},
};

mod book;
mod permit;
mod requests;
mod running;
pub(crate) mod saved;

pub(crate) use book::{Handle, Queued, Requests};
pub(crate) use permit::{Call, gate};
use running::Projected;
pub(crate) use running::{Brought, Caller, Home, Hosted, Running, SessionRef, StartArgs};

/// The one host, shared by the server and every session it owns.
pub(crate) type SharedServices = Rc<RefCell<ServiceHost>>;

/// Where a deferred answer goes when the service settles it.
pub(crate) enum Reply {
    /// A script waiting on the socket. Gone already is fine: nobody is told.
    Rpc(mpsc::Sender<String>),
    /// A window, which does not wait: a refusal is shown as a notification.
    Window { action: String },
}

impl Reply {
    pub(crate) fn answer(self, result: Result<Option<String>, String>) {
        match (self, result) {
            (Reply::Rpc(reply), result) => {
                let _ = reply.send(match result {
                    Ok(line) => line.unwrap_or_else(|| "ok".into()),
                    Err(reason) => format!("error: {reason}"),
                });
            }
            (Reply::Window { action }, Err(reason)) => refused(&action, reason),
            (Reply::Window { .. }, Ok(_)) => {}
        }
    }
}

/// One window's place in the session it is attached to, as the services see it.
pub(super) struct Audience<'a> {
    pub(super) session: SessionId,
    pub(super) key: SessionKey,
    /// The session's effective configuration; the server's when the session has none of its own.
    pub(super) config: Option<&'a AppConfig>,
    pub(super) clients: Vec<Rc<Connection>>,
}

/// What one window was last sent by one service, in which session.
struct Seen {
    session: SessionId,
    /// `None`: asked, and shown nothing.
    revision: Option<u64>,
}

/// Show a window that an action it asked for was refused, and why.
pub(crate) fn refused(action: &str, reason: String) {
    crate::notification::Notification::danger(format!("{action} was refused"))
        .body(reason)
        .send();
}

/// A deferred answer owed to a request a service handed in: which service's, and which request.
struct Owed {
    origin: usize,
    serial: u64,
}

#[derive(Default)]
pub(crate) struct ServiceHost {
    epoch: u64,
    config: Option<Rc<AppConfig>>,
    services: Vec<Box<dyn Hosted>>,
    seen: HashMap<(u64, usize), Seen>,
    pending: HashMap<(usize, ServiceTicket), Reply>,
    /// Deferred answers a request handed in by a service waits on.
    owed: HashMap<(usize, ServiceTicket), Owed>,
    /// Woken by every inbox send, so shutdown can sleep until a stopping service has news.
    nudge: Option<Receiver<()>>,
    /// Reports raised while starting, before anything could show them.
    held: Vec<crate::notification::NotificationDraft>,
    /// Every session, as the server last said — what each context hands a service.
    sessions: Sessions,
    /// Every extension's saved entries, global and per session — the server is the only writer.
    saved: Rc<std::cell::RefCell<saved::SavedStore>>,
}

impl ServiceHost {
    /// Run every declared service's factory for this server, each in its own folder under
    /// `identity`. A factory that fails — or has no folder to run in — is reported and skipped; the
    /// server starts without it.
    pub(crate) fn start(
        config: &AppConfig,
        identity: &ServerIdentity,
        wake: &ServerWaker,
        catalog: &crate::actions::ActionCatalogSnapshot,
    ) -> Self {
        let (nudge_tx, nudge) = mpsc::sync_channel(1);
        let server = wake.clone();
        let wake = ServerWaker::new(move || {
            server.wake();
            let _ = nudge_tx.try_send(());
        });
        let args = StartArgs {
            config,
            identity,
            epoch: new_epoch(),
            capacity: config.config.settings.services.inbox_capacity.get(),
            request_capacity: config.config.settings.services.request_queue_capacity.get(),
            max_line: config.config.settings.connection.max_frame_bytes.get(),
            wake,
            catalog,
        };
        // Nothing can show a notification yet — no session, and in a standalone window no sink — so
        // a failing factory's report is held for the first session.
        let (services, held) = crate::notification::collect_notifications(|| {
            crate::entry::service::declared()
                .into_iter()
                .filter_map(|declared| {
                    declared
                        .start(&args)
                        .map_err(|error| report(declared.name(), "could not start", &error))
                        .ok()
                })
                .collect()
        });
        let saved = Rc::new(std::cell::RefCell::new(saved::SavedStore::open(
            identity.own_file("saved-state.json").ok(),
            config.config.settings.services.saved_entry_max_bytes.get(),
        )));
        let mut services: Vec<Box<dyn Hosted>> = services;
        for service in &mut services {
            service.know_saved(&saved);
        }
        Self {
            epoch: args.epoch,
            config: Some(Rc::new(config.clone())),
            services,
            saved,
            nudge: Some(nudge),
            held,
            ..Self::default()
        }
    }

    /// The reports held since the server started, for the first session to show.
    pub(crate) fn take_held(&mut self) -> Vec<crate::notification::NotificationDraft> {
        std::mem::take(&mut self.held)
    }

    /// A session's saved entries, for its save.
    pub(crate) fn saved_entries(
        &self,
        session: crate::entry::service::SessionKey,
    ) -> saved::Entries {
        self.saved.borrow().session_entries(session)
    }

    /// A restored session's saved entries come back.
    pub(crate) fn adopt_saved_entries(
        &self,
        session: crate::entry::service::SessionKey,
        entries: saved::Entries,
    ) {
        self.saved.borrow_mut().adopt_session(session, entries);
    }

    /// A deleted session's saved entries go with it.
    pub(crate) fn forget_saved_entries(&self, session: crate::entry::service::SessionKey) {
        self.saved.borrow_mut().forget_session(session);
    }

    /// The saved-state changes since the last call, to announce as `state.changed`.
    ///
    /// **An extension that saved is shown again in every window**, with nothing written by it: what
    /// each window last saw of its services is forgotten, so the next publish projects them afresh
    /// and a projection that reads `cx.saved(..)` shows the new value. The author of a dock never
    /// has to say "my view changed" after a save.
    pub(crate) fn take_state_changes(&mut self) -> Vec<crate::entry::service::StateEvent> {
        let (changes, touched) = {
            let mut saved = self.saved.borrow_mut();
            (saved.take_changes(), saved.take_touched())
        };
        if !touched.is_empty() {
            let services = &self.services;
            self.seen
                .retain(|(_, index), _| !touched.contains(services[*index].extension()));
        }
        changes
    }

    /// Whether `action` belongs to one of this server's services.
    pub(crate) fn owns(&self, action: &str) -> bool {
        self.services.iter().any(|service| service.owns(action))
    }

    /// Start a service action as `call`. An immediate answer comes back now; a ticket is the
    /// service's promise to settle it later.
    fn begin(
        &mut self,
        session: SessionRef,
        call: &Call,
        config: Option<&AppConfig>,
        intent: &Intent,
    ) -> Started {
        let Some(index) = self
            .services
            .iter()
            .position(|service| service.owns(&intent.action))
        else {
            return Started::Now(Err(format!("no service runs '{}'", intent.action)));
        };
        let Some(config) = config.or(self.config.as_deref()) else {
            return Started::Now(Err("the server has no configuration".into()));
        };
        let caller = Caller {
            session,
            requester: call.requester(),
            config,
        };
        let now = match self.services[index].invoke(self.epoch, caller, intent) {
            ActionResult::Done(line) => Ok(line),
            ActionResult::Refused(reason) => Err(reason),
            // The caller keeps the ticket first, then settles: the answer may already be in.
            ActionResult::Later(ticket) => return Started::Later(index, ticket),
        };
        self.settle(index);
        Started::Now(now)
    }

    /// Run a service action. An immediate answer goes to `reply` now; a ticket keeps it until the
    /// service settles it. Whether `call` may ask is decided by the caller, before this.
    pub(crate) fn invoke(
        &mut self,
        session: SessionRef,
        call: &Call,
        config: Option<&AppConfig>,
        intent: &Intent,
        reply: Reply,
    ) {
        match self.begin(session, call, config, intent) {
            Started::Now(result) => reply.answer(result),
            Started::Later(index, ticket) => {
                self.pending.insert((index, ticket), reply);
                self.settle(index);
            }
        }
    }

    /// A service action that a chain reached (no window to hand it to): a refusal fails the chain.
    /// A deferred answer is not waited for, unless the chain is a request a service handed in —
    /// then that request is not over until it comes.
    pub(crate) fn run_step(
        &mut self,
        session: SessionRef,
        call: &Call,
        config: Option<&AppConfig>,
        intent: &Intent,
    ) -> Result<(), crate::rpc::RpcError> {
        match self.begin(session, call, config, intent) {
            Started::Now(Err(reason)) => Err(crate::rpc::RpcError::Failed(reason)),
            Started::Now(Ok(line)) => {
                self.answered(call, line);
                Ok(())
            }
            Started::Later(index, ticket) => {
                self.owe(call, index, ticket);
                self.settle(index);
                Ok(())
            }
        }
    }

    /// **An event on the server, to every service that listens for it** — the one door services
    /// hear events through.
    pub(crate) fn announce(&mut self, event: &ServerEvent) {
        for index in 0..self.services.len() {
            self.services[index].listen(self.epoch, event);
            self.settle(index);
        }
    }

    /// Hand every service what its inbox holds.
    pub(crate) fn drain(&mut self) {
        for index in 0..self.services.len() {
            self.services[index].drain(self.epoch);
            self.settle(index);
        }
    }

    /// Deliver the deferred answers a service settled during its last call.
    fn settle(&mut self, index: usize) {
        for (ticket, result) in std::mem::take(&mut self.services[index].effects().completed) {
            if let Some(reply) = self.pending.remove(&(index, ticket)) {
                reply.answer(result);
            } else if let Some(owed) = self.owed.remove(&(index, ticket)) {
                self.book(owed.origin).settled(owed.serial, result);
            }
        }
    }

    /// This window starts again from nothing: it attached, or reattached.
    pub(crate) fn forget_client(&mut self, client: u64) {
        self.seen.retain(|(id, _), _| *id != client);
    }

    /// This window lost step with `service` and asked for a fresh snapshot.
    pub(crate) fn forget(&mut self, client: u64, service: &str) {
        if let Some(index) = self
            .services
            .iter()
            .position(|hosted| hosted.name() == service)
        {
            self.seen.remove(&(client, index));
        }
    }

    /// Send each attached window what each service shows it: everyone after a republish, and any
    /// window that has nothing yet. `max_frame_bytes` bounds one update.
    pub(super) fn publish(&mut self, audience: &[Audience<'_>], max_frame_bytes: usize) {
        let Some(server_config) = self.config.clone() else {
            return;
        };
        for index in 0..self.services.len() {
            let republish = std::mem::take(&mut self.services[index].effects().republish);
            for group in audience {
                for client in &group.clients {
                    let key = (client.id, index);
                    let seen = self
                        .seen
                        .get(&key)
                        .filter(|seen| seen.session == group.session);
                    if seen.is_some() && !republish {
                        continue;
                    }
                    let since = seen.and_then(|seen| seen.revision);
                    let cx = ProjectCx {
                        session: group.session.0,
                        session_key: group.key,
                        client: client.id,
                        config: group.config.unwrap_or(&server_config),
                        extension: self.services[index].extension(),
                        epoch: self.epoch,
                        since,
                        sessions: &self.sessions,
                        saved: &self.saved,
                    };
                    let revision = match self.services[index].project(&cx) {
                        Ok(Projected::Unchanged) => since,
                        Ok(Projected::Remove) => {
                            if since.is_some() {
                                self.send(
                                    index,
                                    client,
                                    0,
                                    ServiceUpdateKind::Removed,
                                    max_frame_bytes,
                                );
                            }
                            None
                        }
                        Ok(Projected::Send { revision, kind }) => {
                            match self.send(index, client, revision, kind, max_frame_bytes) {
                                Some(true) => Some(revision),
                                // Not sent: the next republish tries again from what it has.
                                Some(false) => since,
                                // The link refused it: start this window again from a snapshot.
                                None => {
                                    self.seen.remove(&key);
                                    continue;
                                }
                            }
                        }
                        Err(error) => {
                            report(
                                self.services[index].name(),
                                "could not show a window",
                                &error,
                            );
                            since
                        }
                    };
                    self.seen.insert(
                        key,
                        Seen {
                            session: group.session,
                            revision,
                        },
                    );
                }
            }
        }
        let live: Vec<u64> = audience
            .iter()
            .flat_map(|group| group.clients.iter().map(|client| client.id))
            .collect();
        self.seen.retain(|(client, _), _| live.contains(client));
    }

    /// `Some(true)` sent, `Some(false)` too large to send, `None` the link refused it.
    fn send(
        &self,
        index: usize,
        client: &Connection,
        revision: u64,
        kind: ServiceUpdateKind,
        max_frame_bytes: usize,
    ) -> Option<bool> {
        let hosted = &self.services[index];
        let message = ServerMsg::Service(ServiceUpdate {
            service: hosted.name().to_owned(),
            schema_version: hosted.schema_version(),
            server_epoch: self.epoch,
            revision,
            kind,
        });
        let size = serde_json::to_vec(&message).map_or(usize::MAX, |bytes| bytes.len());
        if size > max_frame_bytes {
            report(
                hosted.name(),
                "could not show a window",
                &format!("an update of {size} bytes is over the {max_frame_bytes}-byte limit"),
            );
            return Some(false);
        }
        (matches!(client.attachment.get(), Attachment::Attached)
            && client.link.send(message).is_ok())
        .then_some(true)
    }

    /// Stop every service: ask each until it is done, delivering its inbox in between, for at most
    /// `[settings.services] stop_timeout_ms`. Then answer every caller still waiting and drop the
    /// services, so a second call finds nothing to stop.
    pub(crate) fn shutdown(&mut self) {
        let timeout = self.config.as_ref().map_or(0, |config| {
            config.config.settings.services.stop_timeout_ms.get()
        });
        let deadline = Instant::now() + Duration::from_millis(timeout);
        let mut stopping: Vec<usize> = (0..self.services.len()).collect();
        loop {
            stopping.retain(|&index| {
                self.services[index].drain(self.epoch);
                self.services[index].stop(self.epoch) == Stop::Pending
            });
            for index in 0..self.services.len() {
                self.settle(index);
            }
            self.tell_outcomes();
            if stopping.is_empty() {
                break;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            let woken = self.nudge.as_ref().map(|nudge| nudge.recv_timeout(left));
            if left.is_zero() || !matches!(woken, Some(Ok(()) | Err(RecvTimeoutError::Timeout))) {
                for &index in &stopping {
                    report(
                        self.services[index].name(),
                        "did not stop",
                        &format!(
                            "still busy after {timeout} ms (`[settings.services] stop_timeout_ms`)"
                        ),
                    );
                }
                break;
            }
        }
        // What was handed in and never finished is over: refused if it never started, unknown if it
        // did. The service is told while it is still there to hear it.
        for index in 0..self.services.len() {
            self.book(index).close();
        }
        self.tell_outcomes();
        self.owed.clear();
        for (_, reply) in self.pending.drain() {
            reply.answer(Err("the server stopped".into()));
        }
        // Dropping the services closes their inboxes: a worker still sending is told so.
        self.services.clear();
    }
}

/// How a service action started.
enum Started {
    /// Answered at once.
    Now(Result<Option<String>, String>),
    /// Accepted: the service at this index owes the answer for this ticket.
    Later(usize, ServiceTicket),
}

/// Say that `service` failed, where the user and the log both see it, and carry on.
fn report(service: &str, what: &str, error: &dyn std::fmt::Display) {
    eprintln!("[heca] service {service}: {what}: {error}");
    crate::notification::Notification::danger(format!("Service {service}: {what}"))
        .body(error.to_string())
        .dedup_key(format!("service:{service}:{what}"))
        .send();
}

/// A number for this run of this server, never reused: two servers in one process differ by the
/// counter, a restarted one by the clock.
fn new_epoch() -> u64 {
    static STARTED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let count = STARTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let clock = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64);
    clock.wrapping_add(count)
}
