//! **What services handed in for others, as the host follows it**: started on the server's turn,
//! kept open while a deferred answer is owed, and told to the service that handed it in when it
//! ends. The book of one service is [`Requests`]; this is what joins the books to the services.

use crate::entry::service::{RequestOutcome, ServiceTicket, SessionEntry, Sessions};

use super::{Call, Handle, Owed, Queued, Requests, ServiceHost};

impl ServiceHost {
    /// The book of the service at `index`.
    pub(super) fn book(&mut self, index: usize) -> &mut Requests {
        &mut self.services[index].effects().requests
    }

    /// The service that handed `call`'s request in, and the request's number there.
    fn origin_of(&self, call: &Call) -> Option<(usize, u64)> {
        let handle = call.request.as_ref()?;
        let index = self
            .services
            .iter()
            .position(|service| service.name() == handle.reference.service())?;
        Some((index, handle.serial))
    }

    /// An action of `call`'s request answered with `line`.
    pub(super) fn answered(&mut self, call: &Call, line: Option<String>) {
        if let Some((origin, serial)) = self.origin_of(call) {
            self.book(origin).answered(serial, line);
        }
    }

    /// An action of `call`'s request will answer through `ticket` of the service at `index`; the
    /// request is not over until it does.
    pub(super) fn owe(&mut self, call: &Call, index: usize, ticket: ServiceTicket) {
        if let Some((origin, serial)) = self.origin_of(call) {
            self.book(origin).owes(serial);
            self.owed.insert((index, ticket), Owed { origin, serial });
        }
    }

    /// The sessions that exist now: what every context hands a service, and what turns a request
    /// for a session that does not exist away at once.
    pub(crate) fn know_sessions(&mut self, sessions: &Sessions) {
        let keys: Vec<_> = sessions.iter().map(SessionEntry::key).collect();
        for index in 0..self.services.len() {
            self.services[index].know_sessions(sessions);
            self.book(index).know_sessions(&keys);
        }
        self.sessions = sessions.clone();
    }

    /// Every request waiting to run, now started. The server runs them, one by one.
    pub(crate) fn take_queued(&mut self) -> Vec<Queued> {
        (0..self.services.len())
            .flat_map(|index| self.book(index).start_queued())
            .collect()
    }

    /// Every action of the request has been started: it ends when nothing is owed to it.
    pub(crate) fn request_issued(&mut self, handle: &Handle) {
        self.request_book(handle, |book| book.issued(handle.serial));
    }

    /// The request could not go on. Nothing ran before: refused. Something did: failed, and what ran
    /// is not undone.
    pub(crate) fn request_stopped(&mut self, handle: &Handle, anything_ran: bool, reason: String) {
        let outcome = if anything_ran {
            RequestOutcome::Failed(reason)
        } else {
            RequestOutcome::Refused(reason)
        };
        self.request_book(handle, |book| book.end(handle.serial, outcome));
    }

    fn request_book(&mut self, handle: &Handle, act: impl FnOnce(&mut Requests)) {
        if let Some(index) = self
            .services
            .iter()
            .position(|service| service.name() == handle.reference.service())
        {
            act(self.book(index));
        }
    }

    /// Tell each service how its ended requests ended — until nothing more ends, since a service's
    /// reply to one can settle another.
    pub(crate) fn tell_outcomes(&mut self) {
        loop {
            let mut told = false;
            for index in 0..self.services.len() {
                while let Some((request, outcome)) = self.book(index).next_finished() {
                    told = true;
                    self.services[index].request_done(self.epoch, &request, outcome);
                    self.settle(index);
                }
            }
            if !told {
                return;
            }
        }
    }
}
