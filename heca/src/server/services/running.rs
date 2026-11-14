//! One started service, whatever its type: what the host drives.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use heca_config::loader::AppConfig;

use crate::chrome::Intent;
use crate::entry::service::{
    ActionCx, ActionFn, ActionResult, Effects, ProjectCx, Projection, Service, ServiceCx,
    ServiceInbox, Stop,
};
use crate::server::{ServerWaker, wire::service::ServiceUpdateKind};

/// What a starting server hands every factory.
pub(crate) struct StartArgs<'a> {
    pub(crate) config: &'a AppConfig,
    pub(crate) epoch: u64,
    pub(crate) capacity: usize,
    pub(crate) wake: ServerWaker,
}

/// Who asked for an action: the session, the window if any, and the session's configuration.
pub(crate) struct Caller<'a> {
    pub(crate) session: u64,
    pub(crate) client: Option<u64>,
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
    fn schema_version(&self) -> u32;
    fn owns(&self, action: &str) -> bool;
    /// Hand the service what its inbox holds — at most one inbox's worth per turn.
    fn drain(&mut self, epoch: u64);
    fn invoke(&mut self, epoch: u64, caller: Caller<'_>, intent: &Intent) -> ActionResult;
    fn project(&self, cx: &ProjectCx<'_>) -> Result<Projected, String>;
    fn stop(&mut self, epoch: u64) -> Stop;
    fn effects(&mut self) -> &mut Effects;
}

pub(crate) struct Running<S: Service> {
    name: String,
    service: S,
    inbox: ServiceInbox<S::Message>,
    messages: Receiver<S::Message>,
    actions: HashMap<String, Rc<ActionFn<S>>>,
    effects: Effects,
    capacity: usize,
}

impl<S: Service> Running<S> {
    pub(crate) fn new(
        name: String,
        service: S,
        inbox: ServiceInbox<S::Message>,
        messages: Receiver<S::Message>,
        actions: HashMap<String, Rc<ActionFn<S>>>,
        capacity: usize,
    ) -> Self {
        Self { name, service, inbox, messages, actions, effects: Effects::default(), capacity }
    }

    fn encode(value: impl serde::Serialize) -> Result<serde_json::Value, String> {
        serde_json::to_value(value).map_err(|error| error.to_string())
    }
}

impl<S: Service> Hosted for Running<S> {
    fn name(&self) -> &str {
        &self.name
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
            let mut cx = ServiceCx { epoch, inbox: &self.inbox, effects: &mut self.effects };
            if let Err(error) = self.service.receive(&mut cx, message) {
                super::report(&self.name, "a message failed", &error);
            }
        }
        // A full turn's worth: let the rest wait for the next turn instead of starving the server.
        self.inbox.wake();
    }

    fn invoke(&mut self, epoch: u64, caller: Caller<'_>, intent: &Intent) -> ActionResult {
        let Some(run) = self.actions.get(&intent.action).cloned() else {
            return ActionResult::Refused(format!("'{}' is not an action of {}", intent.action, self.name));
        };
        let mut cx = ActionCx {
            service: ServiceCx { epoch, inbox: &self.inbox, effects: &mut self.effects },
            session: caller.session,
            client: caller.client,
            config: caller.config,
        };
        run(&mut self.service, &mut cx, intent)
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
        let mut cx = ServiceCx { epoch, inbox: &self.inbox, effects: &mut self.effects };
        self.service.stop(&mut cx)
    }

    fn effects(&mut self) -> &mut Effects {
        &mut self.effects
    }
}
