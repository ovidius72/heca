//! **`demo.service("rooms", Rooms::start)`** — something that lives inside the server. *Server side.*
//!
//! ```ignore
//! let demo = heca::extension("demo");
//! let rooms = demo.service("rooms", |cx| Ok(Rooms::open(cx.config(), cx.inbox())?));
//! rooms.action(demo.action("join").label("Join room"),
//!     |rooms, cx, intent| rooms.join(cx, intent));
//! let shown = rooms.client(RoomsView::default(), |view, update| view.apply(update));
//! // in a component: shown.with(|view| view.rooms.len())
//! ```
//!
//! **One per server.** The factory runs once when a server starts — the background server, or the
//! one inside a standalone window — never per window, per session or per reattach. Two servers in one
//! process each get their own. Closing a window or switching session never restarts it.
//!
//! **Nothing here crosses threads but the inbox.** The service and its factory live on the server's
//! own thread, so neither has to be `Send`. A worker the service starts talks back through its
//! [`ServiceInbox`]: a bounded queue whose send wakes the server, which hands each message to
//! [`Service::receive`].
//!
//! **What windows see is the service's to say.** After [`ServiceCx::republish`], heca asks
//! [`Service::project`] once per window — its session and its connection — and sends the answer: a
//! whole snapshot, or a delta on what that window already has. heca keeps the revisions: a window
//! that attaches, reconnects, comes back to a restarted server or misses a revision gets a snapshot.
//! On the window, the extension applies them ([`ServiceRegistration::client`]) and components read
//! the result like any signal.
//!
//! **Actions reach the service directly.** [`ServiceRegistration::action`] puts an action in the same
//! catalog every other action is in — a key, a menu, the palette and RPC find it — and its handler is
//! handed the service itself. It answers at once, or with a [`ServiceTicket`] it completes later
//! ([`ServiceCx::complete`]). A caller that times out or goes away never runs it twice and never
//! cancels it.
//!
//! **A folder of its own, and a table of its own.** [`StartCx::data_dir`] is where the service keeps
//! what it writes to disk: private to its owner, shared with no other service, the same on every
//! start of this server and different on another server running at the same time.
//! [`StartCx::extension_config`] reads the extension's `[extensions.<name>]` table as a type of its
//! own; one that does not fit is an error, and an extension can read no table but its own. Actions
//! and projections read the session's table the same way.
//!
//! **Who may run what.** [`ServiceRegistration::action_policy`] declares a rule every action the
//! server runs is put to, wherever it comes from. A service that relays what others ask for hands
//! each request in with [`ServiceCx::submit`] and hears how it ended in [`Service::request_done`].
//!
//! **Stopping.** When the server shuts down it calls [`Service::stop`] until it says
//! [`Stop::Done`], delivering inbox messages in between, for at most
//! `[settings.services] stop_timeout_ms`.
//!
//! A failing factory, message or projection is reported as a notification and leaves the server
//! running.

use std::cell::RefCell;
use std::rc::Rc;

use serde::{Serialize, de::DeserializeOwned};

use crate::actions::ActionMeta;
use crate::chrome::{Intent, StartupQueue, warn_author};

pub(crate) mod client;
mod config;
mod cx;
mod declared;
mod inbox;
mod request;
mod session_key;
mod sessions;
#[cfg(test)]
mod tests;

pub use client::ServiceState;
pub use config::ExtensionConfigError;
pub use cx::{ActionCx, ProjectCx, ServiceCx, StartCx};
pub(crate) use cx::Effects;
pub(crate) use declared::{ActionFn, Declaration, ListenFn, PolicyFn};
pub use inbox::{ServiceInbox, ServiceSendError};
pub use request::{
    ActionRequest, ExternalRequest, Permit, RequestOutcome, RequestRef, Requester, Sender, SubmitError,
};
pub use session_key::SessionKey;
pub use sessions::{ServerEvent, SessionChange, SessionEntry, SessionEvent, SessionState, Sessions};

use declared::Declared;

/// What a factory, a message or a projection can fail with. Any error type converts with `?`.
pub type ServiceResult<T> = Result<T, Box<dyn std::error::Error>>;

/// **A service an extension runs inside the server.** Implement it on your own type and declare it
/// with [`Extension::service`](super::Extension::service).
pub trait Service: 'static {
    /// What the service's workers send it through its [`ServiceInbox`].
    type Message: Send + 'static;
    /// The whole of what one window is shown.
    type Snapshot: Serialize + DeserializeOwned + 'static;
    /// A change on top of a snapshot or an earlier delta.
    type Delta: Serialize + DeserializeOwned + 'static;
    /// The shape of [`Snapshot`](Self::Snapshot) and [`Delta`](Self::Delta). Raise it when either
    /// changes, so a window built against another shape says so instead of misreading it.
    const SCHEMA_VERSION: u32;

    /// One message from the inbox. Call [`ServiceCx::republish`] when what windows see changed.
    fn receive(&mut self, cx: &mut ServiceCx<'_, Self::Message>, message: Self::Message) -> ServiceResult<()>;

    /// What one window is shown — committed data only.
    ///
    /// [`ProjectCx::since`] is the revision this window already has. Answer a
    /// [`Projection::Delta`] on exactly that revision, a [`Projection::Snapshot`] (always accepted),
    /// or [`Projection::Unchanged`]. `None` means this window is shown nothing: whatever it was
    /// shown before is taken away.
    fn project(&self, cx: &ProjectCx<'_>) -> Option<Projection<Self::Snapshot, Self::Delta>>;

    /// The server is shutting down. Return [`Stop::Pending`] while workers are still finishing and
    /// say so through the inbox when they are; it is called again, never blocked on. The default is
    /// a service with nothing to wait for.
    fn stop(&mut self, _cx: &mut ServiceCx<'_, Self::Message>) -> Stop {
        Stop::Done
    }

    /// **A request this service handed in with [`ServiceCx::submit`] has ended**, and how. Called
    /// once per request, on the server's turn after it ended; a request still open when the server
    /// stops is ended first, as [`RequestOutcome::Refused`] if it never started and
    /// [`RequestOutcome::Unknown`] if it did. The default does nothing.
    fn request_done(&mut self, _cx: &mut ServiceCx<'_, Self::Message>, _request: &RequestRef, _outcome: RequestOutcome) {}
}

/// What a window is shown, from [`Service::project`].
#[derive(Debug, Clone, PartialEq)]
pub enum Projection<S, D> {
    /// The whole of it, at `revision`.
    Snapshot { revision: u64, value: S },
    /// A change from `base` (the revision the window has) to `revision`.
    Delta { base: u64, revision: u64, value: D },
    /// The window already has the latest.
    Unchanged,
}

/// Whether a service has finished stopping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Done,
    /// Still finishing; ask again.
    Pending,
}

/// What an action did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionResult {
    /// Done, with an optional line for an RPC caller.
    Done(Option<String>),
    /// Not done, and why.
    Refused(String),
    /// Accepted; the answer follows with [`ServiceCx::complete`] on this ticket.
    Later(ServiceTicket),
}

/// A deferred action's answer, owed by the service. Made by [`ActionCx::later`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceTicket(pub(crate) u64);

/// What a window's applier is handed — see [`ServiceRegistration::client`].
#[derive(Debug, Clone, PartialEq)]
pub enum Update<S, D> {
    /// Replace everything: the value was reset to its initial state just before.
    Snapshot(S),
    /// Apply this change.
    Delta(D),
}

thread_local! {
    /// Services declared before a server started. Every server reads them all.
    static SERVICES: StartupQueue<Rc<dyn Declaration>> = const { StartupQueue::new() };
}

impl super::Extension {
    /// **A service** of this extension — `<extension>.<short>` — started once per server by
    /// `factory`.
    pub fn service<S: Service>(
        &self,
        short: &str,
        factory: impl Fn(&mut StartCx<'_, S::Message>) -> ServiceResult<S> + 'static,
    ) -> ServiceRegistration<S> {
        let (Some(extension), Some(name)) = (self.name(), self.full("service", short)) else {
            return ServiceRegistration { declared: None };
        };
        if SERVICES.with(|q| q.peek(|all| all.iter().any(|d| d.name() == name))) {
            warn_author(format!("[heca] service '{name}' was already declared, so this one is refused"));
            return ServiceRegistration { declared: None };
        }
        let declared = Rc::new(Declared::<S> {
            name,
            extension: extension.to_owned(),
            factory: Box::new(factory),
            actions: RefCell::new(Vec::new()),
            policies: RefCell::new(Vec::new()),
            listeners: RefCell::new(Vec::new()),
        });
        if let Err(late) = SERVICES.with(|q| q.add(declared.clone())) {
            warn_author(format!(
                "[heca] service '{}' was added after the app started, so it does nothing — add it \
                 before `heca::run()`",
                late.name()
            ));
            return ServiceRegistration { declared: None };
        }
        ServiceRegistration { declared: Some(declared) }
    }
}

/// A declared service: hang its actions and its window-side state off this. A refused declaration
/// still returns one, so the program keeps compiling; everything hung off it is dropped.
pub struct ServiceRegistration<S: Service> {
    declared: Option<Rc<Declared<S>>>,
}

impl<S: Service> ServiceRegistration<S> {
    /// **An action that runs on the service.** Describe it with an extension's
    /// [`action`](super::Extension::action) and [`ActionMeta`]'s methods; `run` is handed the
    /// service itself. A name already taken by any other action is refused out loud.
    pub fn action(
        &self,
        meta: ActionMeta,
        run: impl Fn(&mut S, &mut ActionCx<'_, S::Message>, &Intent) -> ActionResult + 'static,
    ) {
        let Some(declared) = &self.declared else { return };
        if !super::action::belongs_to_an_extension(&meta) {
            return;
        }
        if SERVICES.with(StartupQueue::is_closed) {
            warn_author(format!(
                "[heca] action '{}' was added after the app started, so it does nothing — add it \
                 before `heca::run()`",
                meta.name
            ));
            return;
        }
        declared.actions.borrow_mut().push((meta, Rc::new(run)));
    }

    /// **A rule about who may run what.** Every action about to run is put to every policy any
    /// service declared — an action from a window, a script, a helper or an integration's request,
    /// and every follow-up of a chain, which is asked again with the same requester — and the first
    /// [`Permit::Deny`] stops it, with its reason shown to whoever asked: a notification in a window,
    /// `error: …` to a script. Asked when the action is about to run, never earlier, so a rule that
    /// changed applies to what was already waiting.
    ///
    /// With no policy declared, the user's own requests go through and a request handed in by a
    /// service is refused. A request handed in by a service that declares no policy is refused
    /// even when another service declares one.
    ///
    /// ```ignore
    /// rooms.action_policy(|rooms, _cx, request| match request.requester() {
    ///     Requester::External { sender, .. } if !rooms.may(sender, request.action()) =>
    ///         Permit::Deny(format!("{sender} may not run {}", request.action())),
    ///     _ => Permit::Allow,
    /// });
    /// ```
    ///
    /// This limits what requests relayed through cooperating integrations can do. Any process of
    /// the same user can open the socket directly, so it is no defence against a hostile local
    /// process.
    pub fn action_policy(
        &self,
        policy: impl Fn(&mut S, &mut ServiceCx<'_, S::Message>, &ActionRequest<'_>) -> Permit + 'static,
    ) {
        let Some(declared) = &self.declared else { return };
        if SERVICES.with(StartupQueue::is_closed) {
            warn_author(format!(
                "[heca] an action policy of service '{}' was added after the app started, so it does nothing — add it \
                 before `heca::run()`",
                declared.name
            ));
            return;
        }
        declared.policies.borrow_mut().push(Rc::new(policy));
    }

    /// **Listen for an event on the server** — THE EVENTS: the same `on("name", …)` heca's own
    /// components, extensions and plugins use. `"*"` listens for every event. `listen` is handed
    /// the service itself, so it updates its own state and republishes when windows should see it.
    ///
    /// ```ignore
    /// rooms.on("session.changed", |rooms, cx, ev| {
    ///     let Some(ev) = ev.as_session() else { return };
    ///     match ev.kind() {
    ///         SessionChange::Created | SessionChange::Deleted => rooms.regroup(ev.sessions()),
    ///         SessionChange::Renamed => rooms.rename(ev.session()),
    ///         _ => return,
    ///     }
    ///     cx.republish();
    /// });
    /// ```
    ///
    /// A name heca does not have ([`ServerEvent::NAMES`]) is refused out loud, so a typo never
    /// listens for nothing in silence.
    pub fn on(
        &self,
        event: &str,
        listen: impl Fn(&mut S, &mut ServiceCx<'_, S::Message>, &ServerEvent) + 'static,
    ) {
        let Some(declared) = &self.declared else { return };
        if event != "*" && !ServerEvent::NAMES.contains(&event) {
            warn_author(format!(
                "[heca] service '{}' listens for '{event}', which is not an event — the events are {}, or \"*\"",
                declared.name,
                ServerEvent::NAMES.join(", ")
            ));
            return;
        }
        if SERVICES.with(StartupQueue::is_closed) {
            warn_author(format!(
                "[heca] a listener of service '{}' was added after the app started, so it does nothing — add it \
                 before `heca::run()`",
                declared.name
            ));
            return;
        }
        declared.listeners.borrow_mut().push((event.to_owned(), Rc::new(listen)));
    }

    /// **What this service shows a window, as a value components read.** `initial` is what a
    /// window holds before its first snapshot and after it is taken away; `apply` turns each
    /// [`Update`] into it. heca checks revisions, asks for a fresh snapshot when one is missed, and
    /// resets the value when the window changes session.
    pub fn client<C: Clone + 'static>(
        &self,
        initial: C,
        apply: impl Fn(&mut C, Update<S::Snapshot, S::Delta>) + 'static,
    ) -> ServiceState<C> {
        let state = ServiceState::new(initial.clone());
        if let Some(declared) = &self.declared {
            client::register::<S, C>(&declared.name, state, initial, Rc::new(apply));
        }
        state
    }
}

/// Every declared service, for a server that is starting. Closes the declarations.
pub(crate) fn declared() -> Vec<Rc<dyn Declaration>> {
    SERVICES.with(|q| q.close(<[_]>::to_vec))
}

/// Forget every declared service, for a test starting from a fresh program.
#[cfg(test)]
pub(crate) fn reset() {
    SERVICES.with(StartupQueue::reset);
    super::extension::reset();
    client::reset();
}
