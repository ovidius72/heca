//! One started service, whatever its type: what the host drives.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use heca_config::loader::AppConfig;

use crate::chrome::Intent;
use crate::entry::service::{
    ActionCx, ActionFn, ActionRequest, ActionResult, Effects, ListenFn, Permit, PolicyFn, ProjectCx, Projection,
    RequestOutcome, RequestRef, Requester, ServerEvent, Service, ServiceCx, ServiceInbox, SessionKey, Sessions, Stop,
};
use crate::server::{ServerWaker, identity::ServerIdentity, wire::service::ServiceUpdateKind};

/// What a starting server hands every factory.
pub(crate) struct StartArgs<'a> {
    pub(crate) config: &'a AppConfig,
    pub(crate) identity: &'a ServerIdentity,
    pub(crate) epoch: u64,
    pub(crate) capacity: usize,
    /// Requests one service may have open at once.
    pub(crate) request_capacity: usize,
    /// The longest line a request may be.
    pub(crate) max_line: usize,
    pub(crate) wake: ServerWaker,
    /// The actions this run has, for every factory to read.
    pub(crate) catalog: &'a crate::actions::ActionCatalogSnapshot,
}

/// Who a started service is and where it keeps its files.
pub(crate) struct Home {
    /// `<extension>.<service>`.
    pub(crate) name: String,
    /// The one `[extensions.<name>]` table it may read.
    pub(crate) extension: String,
    pub(crate) data_dir: PathBuf,
}

/// A session as the services are told of it: this run's number and the key that outlives it.
#[derive(Clone, Copy)]
pub(crate) struct SessionRef {
    pub(crate) id: heca_core::layout::SessionId,
    pub(crate) key: SessionKey,
    /// The workspace the asker is on: the one the asking window shows, else — a script — the one
    /// a window of the session showed last. `None` before any window said.
    pub(crate) workspace: Option<crate::entry::service::WorkspaceKey>,
}

/// Who asked for an action: the session, who is asking, and the session's configuration.
pub(crate) struct Caller<'a> {
    pub(crate) session: SessionRef,
    pub(crate) requester: Requester<'a>,
    pub(crate) config: &'a AppConfig,
}

/// A service's answer for one window, checked and turned into wire data.
#[derive(Debug, PartialEq)]
pub(crate) enum Projected {
    /// The window is shown nothing.
    Remove,
    /// The window already has the latest.
    Unchanged,
    Send { revision: u64, kind: ServiceUpdateKind },
}

/// A started service with its type erased.
pub(crate) trait Hosted {
    fn name(&self) -> &str;
    /// The extension it was declared under.
    fn extension(&self) -> &str;
    fn schema_version(&self) -> u32;
    fn owns(&self, action: &str) -> bool;
    /// Hand the service what its inbox holds — at most one inbox's worth per turn.
    fn drain(&mut self, epoch: u64);
    fn invoke(&mut self, epoch: u64, caller: Caller<'_>, intent: &Intent) -> ActionResult;
    /// Whether it declared an action policy.
    fn has_policy(&self) -> bool;
    /// Every policy it declared, asked in turn; the first to say no is the answer.
    fn permit(&mut self, epoch: u64, request: &ActionRequest<'_>) -> Permit;
    /// How a request it handed in ended.
    fn request_done(&mut self, epoch: u64, request: &RequestRef, outcome: RequestOutcome);
    fn project(&self, cx: &ProjectCx<'_>) -> Result<Projected, String>;
    fn stop(&mut self, epoch: u64) -> Stop;
    fn effects(&mut self) -> &mut Effects;
    /// The sessions as they are now, for every context it is handed from here on.
    fn know_sessions(&mut self, sessions: &Sessions);
    /// The store every service's saved entries live in — handed once, after the services start.
    fn know_saved(&mut self, store: &Rc<std::cell::RefCell<super::saved::SavedStore>>);
    /// An event on the server: run every listener for its name, and every `"*"` one.
    fn listen(&mut self, epoch: u64, event: &ServerEvent);
}

pub(crate) struct Running<S: Service> {
    home: Home,
    service: S,
    inbox: ServiceInbox<S::Message>,
    messages: Receiver<S::Message>,
    actions: HashMap<String, Rc<ActionFn<S>>>,
    policies: Vec<Rc<PolicyFn<S>>>,
    listeners: Vec<(String, Rc<ListenFn<S>>)>,
    effects: Effects,
    capacity: usize,
    sessions: Sessions,
    /// Every extension's saved entries, shared with the host (`ServiceHost::saved`).
    saved: Rc<std::cell::RefCell<super::saved::SavedStore>>,
}

/// What a declared service brings to the host besides the service itself.
pub(crate) struct Brought<S: Service> {
    pub(crate) actions: HashMap<String, Rc<ActionFn<S>>>,
    pub(crate) policies: Vec<Rc<PolicyFn<S>>>,
    pub(crate) listeners: Vec<(String, Rc<ListenFn<S>>)>,
    pub(crate) effects: Effects,
    /// Messages one turn delivers from the inbox.
    pub(crate) capacity: usize,
}

impl<S: Service> Running<S> {
    pub(crate) fn new(
        home: Home,
        service: S,
        inbox: ServiceInbox<S::Message>,
        messages: Receiver<S::Message>,
        declared: Brought<S>,
    ) -> Self {
        let Brought { actions, policies, listeners, effects, capacity } = declared;
        Self { home, service, inbox, messages, actions, policies, listeners, effects, capacity, sessions: Sessions::default(),
            saved: Rc::default() }
    }

    /// The service, what it is handed at each call, and who it is: borrowed apart so a call can
    /// hold all three.
    fn call(&mut self, epoch: u64) -> (&mut S, ServiceCx<'_, S::Message>, &Home) {
        let cx = ServiceCx {
            epoch,
            inbox: &self.inbox,
            effects: &mut self.effects,
            data_dir: &self.home.data_dir,
            sessions: &self.sessions,
            saved: &self.saved,
            extension: &self.home.extension,
        };
        (&mut self.service, cx, &self.home)
    }

    fn encode(value: impl serde::Serialize) -> Result<serde_json::Value, String> {
        serde_json::to_value(value).map_err(|error| error.to_string())
    }
}

impl<S: Service> Hosted for Running<S> {
    fn name(&self) -> &str {
        &self.home.name
    }

    fn extension(&self) -> &str {
        &self.home.extension
    }

    fn schema_version(&self) -> u32 {
        S::SCHEMA_VERSION
    }

    fn owns(&self, action: &str) -> bool {
        self.actions.contains_key(action)
    }

    fn drain(&mut self, epoch: u64) {
        let mut delivered = 0;
        while delivered < self.capacity {
            let Ok(message) = self.messages.try_recv() else { return };
            delivered += 1;
            let (service, mut cx, home) = self.call(epoch);
            if let Err(error) = service.receive(&mut cx, message) {
                super::report(&home.name, "a message failed", &error);
            }
        }
        // A full turn's worth: let the rest wait for the next turn instead of starving the server.
        self.inbox.wake();
    }

    fn invoke(&mut self, epoch: u64, caller: Caller<'_>, intent: &Intent) -> ActionResult {
        let Some(run) = self.actions.get(&intent.action).cloned() else {
            return ActionResult::Refused(format!("'{}' is not an action of {}", intent.action, self.home.name));
        };
        let (service, cx, home) = self.call(epoch);
        let (client, sender) = match caller.requester {
            Requester::Local { window } => (window, None),
            Requester::External { sender, .. } => (None, Some(sender)),
        };
        let mut cx = ActionCx {
            service: cx,
            session: caller.session.id.0,
            session_key: caller.session.key,
            workspace: caller.session.workspace,
            client,
            sender,
            config: caller.config,
            extension: &home.extension,
        };
        run(service, &mut cx, intent)
    }

    fn has_policy(&self) -> bool {
        !self.policies.is_empty()
    }

    fn permit(&mut self, epoch: u64, request: &ActionRequest<'_>) -> Permit {
        let policies = self.policies.clone();
        let (service, mut cx, _) = self.call(epoch);
        policies.iter().map(|policy| policy(service, &mut cx, request))
            .find(|permit| matches!(permit, Permit::Deny(_)))
            .unwrap_or(Permit::Allow)
    }

    fn request_done(&mut self, epoch: u64, request: &RequestRef, outcome: RequestOutcome) {
        let (service, mut cx, _) = self.call(epoch);
        service.request_done(&mut cx, request, outcome);
    }

    fn project(&self, cx: &ProjectCx<'_>) -> Result<Projected, String> {
        let Some(projection) = self.service.project(cx) else { return Ok(Projected::Remove) };
        match (projection, cx.since) {
            (Projection::Snapshot { revision, value }, _) =>
                Ok(Projected::Send { revision, kind: ServiceUpdateKind::Snapshot(Self::encode(value)?) }),
            (Projection::Delta { base, revision, value }, Some(since)) if base == since => Ok(Projected::Send {
                revision,
                kind: ServiceUpdateKind::Delta { base_revision: base, value: Self::encode(value)? },
            }),
            (Projection::Unchanged, Some(_)) => Ok(Projected::Unchanged),
            // Not an answer for what this window has: ask for the whole thing instead.
            (_, Some(_)) => self.project(&ProjectCx { since: None, ..*cx }),
            (_, None) => Err("a window with nothing must be sent a snapshot".into()),
        }
    }

    fn stop(&mut self, epoch: u64) -> Stop {
        let (service, mut cx, _) = self.call(epoch);
        service.stop(&mut cx)
    }

    fn effects(&mut self) -> &mut Effects {
        &mut self.effects
    }

    fn know_sessions(&mut self, sessions: &Sessions) {
        self.sessions = sessions.clone();
    }

    fn know_saved(&mut self, store: &Rc<std::cell::RefCell<super::saved::SavedStore>>) {
        self.saved = store.clone();
    }

    fn listen(&mut self, epoch: u64, event: &ServerEvent) {
        let listeners: Vec<_> = self.listeners.iter()
            .filter(|(name, _)| name == "*" || name == event.name())
            .map(|(_, listen)| listen.clone())
            .collect();
        for listen in listeners {
            let (service, mut cx, _) = self.call(epoch);
            listen(service, &mut cx, event);
        }
    }
}
