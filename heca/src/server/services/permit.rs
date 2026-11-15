//! **The one question every action is asked before it runs: may this requester do this?**
//!
//! Each declared policy is asked about each request, local or external, and the first to say no
//! wins. With no policy declared anywhere, a local request goes through and an external one never
//! does; an external request is also refused when the service that handed it in declares none.
//!
//! [`ServiceHost::authorize`] is the only place that asks. It is reached from the two doors every
//! action runs through: [`execute_server_with`](crate::actions::ActionRegistry::execute_server_with)
//! (scripts, helpers, queued requests and every follow-up of a chain, through [`gate`]) and the
//! server's receipt of a window's message.

use crate::actions::ServerGate;
use crate::chrome::{Intent, PropMap};
use crate::entry::service::{ActionRequest, Permit, Requester, Sender};
use crate::rpc::RpcError;

use super::book::Handle;
use super::{ServiceHost, SessionRef, SharedServices};

/// Who is asking, owned: [`Requester`] borrows from this.
#[derive(Clone, Debug)]
pub(crate) enum Origin {
    Local { window: Option<u64> },
    External { service: String, sender: Sender },
}

/// One call from the top, whatever it grows into: who asked, and the request it is part of when it
/// was handed in by a service. A chain's follow-ups carry the same `Call`, so none can ask for more
/// than the first did.
#[derive(Clone, Debug)]
pub(crate) struct Call {
    pub(crate) origin: Origin,
    pub(crate) request: Option<Handle>,
}

impl Call {
    /// The user, through a window or a script.
    pub(crate) fn local(window: Option<u64>) -> Self {
        Self { origin: Origin::Local { window }, request: None }
    }

    /// A request a service handed in.
    pub(crate) fn external(handle: Handle) -> Self {
        let reference = &handle.reference;
        Self {
            origin: Origin::External { service: reference.service().to_owned(), sender: reference.sender().clone() },
            request: Some(handle),
        }
    }

    pub(crate) fn requester(&self) -> Requester<'_> {
        match &self.origin {
            Origin::Local { window } => Requester::Local { window: *window },
            Origin::External { service, sender } => Requester::External { service, sender },
        }
    }
}

impl ServiceHost {
    /// **May `call` run `action`?** `Err` carries the first policy's reason.
    pub(crate) fn authorize(&mut self, call: &Call, session: Option<SessionRef>, action: &str, args: &PropMap,
        parent: Option<&str>) -> Result<(), String> {
        let judges: Vec<usize> = (0..self.services.len()).filter(|&index| self.services[index].has_policy()).collect();
        if let Origin::External { service, .. } = &call.origin
            && !judges.iter().any(|&index| self.services[index].name() == service) {
            return Err(format!("{service} declares no action policy, so requests handed in for others are refused"));
        }
        let request = ActionRequest { requester: call.requester(), action, args, session: session.map(|s| s.key), parent };
        for index in judges {
            let permit = self.services[index].permit(self.epoch, &request);
            self.settle(index);
            if let Permit::Deny(reason) = permit {
                return Err(reason);
            }
        }
        Ok(())
    }
}

/// The question as [`execute_server_with`](crate::actions::ActionRegistry::execute_server_with)
/// asks it before each action of a call and each follow-up: for `call`, in the session `session`
/// names for that action.
pub(crate) fn gate<'a, F: Fn(&Intent) -> Option<SessionRef>>(services: &'a SharedServices, call: &'a Call,
    session: F) -> Gate<'a, F> {
    Gate { services, call, session }
}

pub(crate) struct Gate<'a, F> {
    services: &'a SharedServices,
    call: &'a Call,
    session: F,
}

impl<F: Fn(&Intent) -> Option<SessionRef>> ServerGate for Gate<'_, F> {
    fn is_external(&self) -> bool {
        matches!(self.call.origin, Origin::External { .. })
    }

    fn permit(&mut self, intent: &Intent, parent: Option<&str>) -> Result<(), RpcError> {
        self.services.borrow_mut()
            .authorize(self.call, (self.session)(intent), &intent.action, &intent.args, parent)
            .map_err(RpcError::Denied)
    }
}
