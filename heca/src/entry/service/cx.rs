//! What a service is handed at each point of its life: starting, a message or an action, and a
//! projection for one window.

use heca_config::loader::AppConfig;

use super::{ServiceInbox, ServiceTicket};

/// What a service asked of the host during one call, collected and acted on after it returns.
#[derive(Debug, Default)]
pub(crate) struct Effects {
    /// Windows should be shown the service again.
    pub(crate) republish: bool,
    /// Deferred answers the service settled.
    pub(crate) completed: Vec<(ServiceTicket, Result<Option<String>, String>)>,
    /// The next ticket this service hands out.
    pub(crate) next_ticket: u64,
}

/// Handed to a service's factory, once per server.
pub struct StartCx<'a, M> {
    pub(crate) config: &'a AppConfig,
    pub(crate) epoch: u64,
    pub(crate) inbox: &'a ServiceInbox<M>,
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
}

/// Handed to [`Service::receive`](super::Service::receive) and [`Service::stop`](super::Service::stop).
pub struct ServiceCx<'a, M> {
    pub(crate) epoch: u64,
    pub(crate) inbox: &'a ServiceInbox<M>,
    pub(crate) effects: &'a mut Effects,
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

    /// What windows see changed: heca asks [`Service::project`](super::Service::project) again for
    /// each of them once this call returns. Say it after a change is committed, never before.
    pub fn republish(&mut self) {
        self.effects.republish = true;
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
    pub(crate) client: Option<u64>,
    pub(crate) config: &'a AppConfig,
}

impl<M> ActionCx<'_, M> {
    /// The session the action ran in.
    pub fn session(&self) -> u64 {
        self.session
    }

    /// The window's connection, when a window asked. `None` for a script with no window.
    pub fn client(&self) -> Option<u64> {
        self.client
    }

    /// The session's effective configuration — the user's, with a trusted project's on top.
    pub fn config(&self) -> &AppConfig {
        self.config
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
    pub(crate) client: u64,
    pub(crate) config: &'a AppConfig,
    pub(crate) epoch: u64,
    pub(crate) since: Option<u64>,
}

impl ProjectCx<'_> {
    /// The session the window is attached to.
    pub fn session(&self) -> u64 {
        self.session
    }

    /// The window's connection.
    pub fn client(&self) -> u64 {
        self.client
    }

    /// The session's effective configuration.
    pub fn config(&self) -> &AppConfig {
        self.config
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
