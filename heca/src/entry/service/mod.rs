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
mod cx;
mod declared;
mod inbox;
#[cfg(test)]
mod tests;

pub use client::ServiceState;
pub use cx::{ActionCx, ProjectCx, ServiceCx, StartCx};
pub(crate) use cx::Effects;
pub(crate) use declared::{ActionFn, Declaration};
pub use inbox::{ServiceInbox, ServiceSendError};

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
        let Some(name) = self.full("service", short) else {
            return ServiceRegistration { declared: None };
        };
        if SERVICES.with(|q| q.peek(|all| all.iter().any(|d| d.name() == name))) {
            warn_author(format!("[heca] service '{name}' was already declared, so this one is refused"));
            return ServiceRegistration { declared: None };
        }
        let declared = Rc::new(Declared::<S> {
            name,
            factory: Box::new(factory),
            actions: RefCell::new(Vec::new()),
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
