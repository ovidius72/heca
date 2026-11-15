//! What a service is handed at each point of its life: starting, a message or an action, and a
//! projection for one window.

use std::path::Path;

use heca_config::loader::AppConfig;
use serde::de::DeserializeOwned;

use super::{
    ExtensionConfigError, ExternalRequest, RequestRef, Sender, ServiceInbox, ServiceTicket, SessionKey, SubmitError, config,
};
use crate::server::services::Requests;

/// What a service asked of the host during one call, collected and acted on after it returns.
pub(crate) struct Effects {
    /// Windows should be shown the service again.
    pub(crate) republish: bool,
    /// Deferred answers the service settled.
    pub(crate) completed: Vec<(ServiceTicket, Result<Option<String>, String>)>,
    /// The next ticket this service hands out.
    pub(crate) next_ticket: u64,
    /// The requests it has handed in for others. Unlike the rest, these last across calls.
    pub(crate) requests: Requests,
}

impl Effects {
    pub(crate) fn new(requests: Requests) -> Self {
        Self { republish: false, completed: Vec::new(), next_ticket: 0, requests }
    }
}

/// Handed to a service's factory, once per server.
pub struct StartCx<'a, M> {
    pub(crate) config: &'a AppConfig,
    pub(crate) extension: &'a str,
    pub(crate) epoch: u64,
    pub(crate) inbox: &'a ServiceInbox<M>,
    pub(crate) data_dir: &'a Path,
}

impl<M> StartCx<'_, M> {
    /// The configuration the server started with.
    pub fn config(&self) -> &AppConfig {
        self.config
    }

    /// This run of the server: a new number every time a server starts, never reused.
    pub fn server_epoch(&self) -> u64 {
        self.epoch
    }

    /// The service's inbox, to hand to the workers it starts.
    pub fn inbox(&self) -> ServiceInbox<M> {
        self.inbox.clone()
    }

    /// **The folder that is this service's, on this server** — to keep what it writes to disk.
    /// It exists, only its owner can read it, no other service shares it, and it is the same on
    /// every start of this server (unlike [`server_epoch`](Self::server_epoch)). Two servers at once
    /// have different folders.
    pub fn data_dir(&self) -> &Path {
        self.data_dir
    }

    /// **The extension's own `[extensions.<name>]` table, as `T`**, from the configuration the
    /// server started with. A table that does not fit `T` is an error, never a default; a missing
    /// one reads as empty, so a `T` whose fields have `#[serde(default)]` is its defaults.
    pub fn extension_config<T: DeserializeOwned>(&self) -> Result<T, ExtensionConfigError> {
        config::read(self.config, self.extension)
    }
}

/// Handed to [`Service::receive`](super::Service::receive) and [`Service::stop`](super::Service::stop).
pub struct ServiceCx<'a, M> {
    pub(crate) epoch: u64,
    pub(crate) inbox: &'a ServiceInbox<M>,
    pub(crate) effects: &'a mut Effects,
    pub(crate) data_dir: &'a Path,
}

impl<M> ServiceCx<'_, M> {
    /// This run of the server — see [`StartCx::server_epoch`].
    pub fn server_epoch(&self) -> u64 {
        self.epoch
    }

    /// The service's inbox, to hand to a worker started now.
    pub fn inbox(&self) -> ServiceInbox<M> {
        self.inbox.clone()
    }

    /// The service's own folder — see [`StartCx::data_dir`]. Ask again when it is needed; there is
    /// nothing to store.
    pub fn data_dir(&self) -> &Path {
        self.data_dir
    }

    /// What windows see changed: heca asks [`Service::project`](super::Service::project) again for
    /// each of them once this call returns. Say it after a change is committed, never before.
    pub fn republish(&mut self) {
        self.effects.republish = true;
    }

    /// **Hand in a request for someone else.** It is answered at once, with the request's
    /// [`RequestRef`] or why it was not taken, and runs on the server's next turn: through the
    /// same parser, catalog, confirmations and execution as `heca rpc`, in `request.session`, with
    /// every declared action policy asked first. How it ended arrives in
    /// [`Service::request_done`](super::Service::request_done).
    ///
    /// Nothing is persisted: a request not finished when the server stops is not run again.
    pub fn submit(&mut self, request: ExternalRequest) -> Result<RequestRef, SubmitError> {
        let reference = self.effects.requests.submit(request)?;
        self.inbox.wake();
        Ok(reference)
    }

    /// Settle a deferred action: `Ok` with an optional line for an RPC caller, or `Err` and why. A
    /// ticket whose caller has gone is settled with no one to tell, which is fine.
    pub fn complete(&mut self, ticket: ServiceTicket, result: Result<Option<String>, String>) {
        self.effects.completed.push((ticket, result));
    }
}

/// Handed to a service's action: everything [`ServiceCx`] has, and who asked.
///
/// Who asked is only what heca knows for certain: the session the action ran in and, when it came
/// from a window, that window's connection. It is not proof of identity.
pub struct ActionCx<'a, M> {
    pub(crate) service: ServiceCx<'a, M>,
    pub(crate) session: u64,
    pub(crate) session_key: SessionKey,
    pub(crate) client: Option<u64>,
    pub(crate) sender: Option<&'a Sender>,
    pub(crate) config: &'a AppConfig,
    pub(crate) extension: &'a str,
}

impl<M> ActionCx<'_, M> {
    /// The session the action ran in, as a number for this run of the server only. To store
    /// something against the session, use [`session_key`](Self::session_key).
    pub fn session(&self) -> u64 {
        self.session
    }

    /// **The session's key** — the same for the life of the session, whatever happens to its name,
    /// its folder or its windows. See [`SessionKey`].
    pub fn session_key(&self) -> SessionKey {
        self.session_key
    }

    /// The window's connection, when a window asked. `None` for a script with no window.
    pub fn client(&self) -> Option<u64> {
        self.client
    }

    /// Who a service handed this request in for, when it came from
    /// [`ServiceCx::submit`]. `None` for the user, from a window or a script. It names whoever the
    /// integration said it was relaying for and authenticates no one.
    pub fn sender(&self) -> Option<&Sender> {
        self.sender
    }

    /// The session's effective configuration — the user's, with a trusted project's on top.
    pub fn config(&self) -> &AppConfig {
        self.config
    }

    /// The extension's own `[extensions.<name>]` table as `T`, from the session's effective
    /// configuration — see [`StartCx::extension_config`].
    pub fn extension_config<T: DeserializeOwned>(&self) -> Result<T, ExtensionConfigError> {
        config::read(self.config, self.extension)
    }

    /// Answer later: return [`ActionResult::Later`](super::ActionResult::Later) with this ticket and
    /// settle it with [`ServiceCx::complete`] once the work is committed.
    pub fn later(&mut self) -> ServiceTicket {
        let ticket = ServiceTicket(self.service.effects.next_ticket);
        self.service.effects.next_ticket = self.service.effects.next_ticket.wrapping_add(1);
        ticket
    }
}

impl<'a, M> std::ops::Deref for ActionCx<'a, M> {
    type Target = ServiceCx<'a, M>;
    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

impl<M> std::ops::DerefMut for ActionCx<'_, M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.service
    }
}

/// Handed to [`Service::project`](super::Service::project): which window is asking, and what it has.
pub struct ProjectCx<'a> {
    pub(crate) session: u64,
    pub(crate) session_key: SessionKey,
    pub(crate) client: u64,
    pub(crate) config: &'a AppConfig,
    pub(crate) extension: &'a str,
    pub(crate) epoch: u64,
    pub(crate) since: Option<u64>,
}

impl ProjectCx<'_> {
    /// The session the window is attached to, as a number for this run of the server only.
    pub fn session(&self) -> u64 {
        self.session
    }

    /// The key of the session the window is attached to — see [`ActionCx::session_key`].
    pub fn session_key(&self) -> SessionKey {
        self.session_key
    }

    /// The window's connection.
    pub fn client(&self) -> u64 {
        self.client
    }

    /// The session's effective configuration.
    pub fn config(&self) -> &AppConfig {
        self.config
    }

    /// The extension's own `[extensions.<name>]` table as `T`, from the session's effective
    /// configuration — see [`StartCx::extension_config`].
    pub fn extension_config<T: DeserializeOwned>(&self) -> Result<T, ExtensionConfigError> {
        config::read(self.config, self.extension)
    }

    /// This run of the server — see [`StartCx::server_epoch`].
    pub fn server_epoch(&self) -> u64 {
        self.epoch
    }

    /// The revision this window already has, or `None` when it has nothing and needs a snapshot.
    pub fn since(&self) -> Option<u64> {
        self.since
    }
}
