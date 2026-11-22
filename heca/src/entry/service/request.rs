//! **Who is asking, whether they may, and requests a service hands in for someone else.**
//!
//! Every action a server runs is asked of the service policies first
//! ([`ServiceRegistration::action_policy`](super::ServiceRegistration::action_policy)): an action
//! from a window, a script, a chain's follow-up or an integration all reach the same check, with
//! the same [`ActionRequest`]. A service that relays what other people ask for hands it in with
//! [`ServiceCx::submit`](super::ServiceCx::submit) and hears how it ended in
//! [`Service::request_done`](super::Service::request_done).
//!
//! **What this is not.** Any process of the same user can open the socket directly. Without
//! effective isolation, such as a different OS user, permissions limit requests received through
//! cooperating integrations, not a hostile local process. Nothing more is promised.
//! [`Sender::new`] authenticates no one.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::SessionKey;
use crate::chrome::PropMap;

/// **Who an integration says it is relaying for.** An opaque name a service chose: heca keeps it
/// with the request, hands it to every policy and gives it back with the outcome.
///
/// `Sender::new(...)` authenticates no one. Whatever builds one must have got the name from
/// something it trusts, never from the content of a message.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sender(String);

impl Sender {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sender {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// **Who is asking for an action.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requester<'a> {
    /// The user, through a window (its connection number) or through a script with no window.
    Local { window: Option<u64> },
    /// A request a service handed in for `sender` with [`ServiceCx::submit`](super::ServiceCx::submit).
    /// `service` is the one that handed it in.
    External {
        service: &'a str,
        sender: &'a Sender,
    },
}

/// What a policy is asked: one action about to run, and who wants it.
///
/// A chain's follow-up is asked again with the same requester and `parent` set to the action that
/// asked for it, so a chain can never do what its requester could not.
#[derive(Clone, Copy, Debug)]
pub struct ActionRequest<'a> {
    pub(crate) requester: Requester<'a>,
    pub(crate) action: &'a str,
    pub(crate) args: &'a PropMap,
    pub(crate) session: Option<SessionKey>,
    pub(crate) parent: Option<&'a str>,
}

impl<'a> ActionRequest<'a> {
    pub fn requester(&self) -> Requester<'a> {
        self.requester
    }

    /// The action's catalog name.
    pub fn action(&self) -> &'a str {
        self.action
    }

    pub fn args(&self) -> &'a PropMap {
        self.args
    }

    /// The session it runs in. `None` for what belongs to no session, such as creating or listing
    /// sessions.
    pub fn session(&self) -> Option<SessionKey> {
        self.session
    }

    /// The action that asked for this one, when this is a chain's follow-up.
    pub fn parent(&self) -> Option<&'a str> {
        self.parent
    }
}

/// A policy's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Permit {
    Allow,
    /// Not allowed, and why. The reason is shown to the person who asked.
    Deny(String),
}

/// **A request an integration relays**: one line, as `heca rpc` takes it, to run in `session`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalRequest {
    pub sender: Sender,
    /// The session it runs in. Never guessed and never replaced by another.
    pub session: SessionKey,
    /// The caller's own name for the request. Two open requests of one service never share it.
    pub request_id: String,
    pub line: String,
}

/// **Which request an answer is for**: handed back by [`ServiceCx::submit`](super::ServiceCx::submit)
/// and again by [`Service::request_done`](super::Service::request_done).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RequestRef {
    pub(crate) service: String,
    pub(crate) sender: Sender,
    pub(crate) session: SessionKey,
    pub(crate) request_id: String,
}

impl RequestRef {
    /// The service that submitted it.
    pub fn service(&self) -> &str {
        &self.service
    }

    pub fn sender(&self) -> &Sender {
        &self.sender
    }

    pub fn session(&self) -> SessionKey {
        self.session
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }
}

/// Why [`ServiceCx::submit`](super::ServiceCx::submit) did not take a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// The service already has `[settings.services] request_queue_capacity` requests open.
    QueueFull,
    /// No session has this key. heca never picks another.
    UnknownSession,
    /// A request of this service with the same id is still open.
    DuplicateId,
    /// Not accepted, and why: the line is empty or too long, the service declares no action
    /// policy, or the server is stopping.
    Refused(String),
}

impl fmt::Display for SubmitError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueFull => out.write_str("the request queue is full"),
            Self::UnknownSession => out.write_str("no session has that key"),
            Self::DuplicateId => out.write_str("a request with that id is still open"),
            Self::Refused(reason) => out.write_str(reason),
        }
    }
}

impl std::error::Error for SubmitError {}

/// How a request ended, from [`Service::request_done`](super::Service::request_done).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestOutcome {
    /// Every action of it ran and every deferred answer came in; the last line any of them gave.
    Done(Option<String>),
    /// Refused before anything ran: by a policy, the catalog, a confirmation, or because the
    /// server stopped first.
    Refused(String),
    /// Ended with an error after something may already have run, including a refusal part way
    /// through a chain. What ran before is not undone.
    Failed(String),
    /// It was started and the server stopped before it finished: what happened cannot be known.
    /// It is never run again.
    Unknown,
}
