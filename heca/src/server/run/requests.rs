//! **Running what a service handed in for others**, in the session it names — the server's loop
//! takes each request the services queued and runs it through the one path a script's line takes.

use super::*;
use crate::server::services::{Call, Queued};

impl ServerLoop {
    /// Run every request the services have queued, once each, then let each service be told how it
    /// ended on the next delivery. A request's deferred answers are waited for by the host, not here.
    pub(super) fn run_requests(&mut self) {
        let queued = self.services.borrow_mut().take_queued();
        for request in queued {
            self.run_request(request);
        }
    }

    fn run_request(&mut self, Queued { handle, line }: Queued) {
        // The session is the one the request named, found by its key now: it may be gone since.
        let key = handle.reference.session();
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.service_ref().key == key)
        else {
            self.services.borrow_mut().request_stopped(
                &handle,
                false,
                "the session no longer exists".into(),
            );
            return;
        };
        let call = Call::external(handle.clone());
        let (result, drafts) = crate::notification::collect_notifications(|| {
            self.sessions[index].run_request(&call, &line)
        });
        for draft in drafts {
            self.sessions[index].raise(draft);
        }
        let mut host = self.services.borrow_mut();
        match result {
            Ok(()) => host.request_issued(&handle),
            Err((reason, anything_ran)) => host.request_stopped(&handle, anything_ran, reason),
        }
    }
}
