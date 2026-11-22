//! **One service's open requests** — what it submitted for others and has not yet heard the end of.
//!
//! A request is queued until the server's loop runs it, then running until every action of it has
//! run and every deferred answer it is waiting on has come in. It is open until one of those ends
//! it; an id that has ended is forgotten, so the same id may be used again.

use std::collections::{HashMap, VecDeque};

use crate::entry::service::{ExternalRequest, RequestOutcome, RequestRef, SessionKey, SubmitError};

/// What the loop is handed to run: the request and the line it asked for.
pub(crate) struct Queued {
    pub(crate) handle: Handle,
    pub(crate) line: String,
}

/// A request as the host follows it: what the service knows it by, and a number that is never
/// reused, so an answer arriving late can never be taken for a newer request with the same id.
#[derive(Clone, Debug)]
pub(crate) struct Handle {
    pub(crate) reference: RequestRef,
    pub(crate) serial: u64,
}

enum State {
    Queued,
    Running {
        /// Deferred answers still owed to it.
        outstanding: usize,
        /// Every action of its line has been started.
        issued: bool,
        output: Option<String>,
    },
}

struct Open {
    reference: RequestRef,
    line: String,
    state: State,
}

pub(crate) struct Requests {
    service: String,
    capacity: usize,
    max_line: usize,
    /// The service declares an action policy: without one nothing it hands in may run.
    has_policy: bool,
    /// The server is stopping: nothing more is taken.
    closed: bool,
    sessions: Vec<SessionKey>,
    next: u64,
    open: HashMap<u64, Open>,
    ids: HashMap<String, u64>,
    queue: VecDeque<u64>,
    finished: VecDeque<(RequestRef, RequestOutcome)>,
}

impl Requests {
    pub(crate) fn new(service: &str, capacity: usize, max_line: usize, has_policy: bool) -> Self {
        Self {
            service: service.to_owned(),
            capacity,
            max_line,
            has_policy,
            closed: false,
            sessions: Vec::new(),
            next: 0,
            open: HashMap::new(),
            ids: HashMap::new(),
            queue: VecDeque::new(),
            finished: VecDeque::new(),
        }
    }

    /// The sessions that exist, as the server last said.
    pub(crate) fn know_sessions(&mut self, keys: &[SessionKey]) {
        self.sessions = keys.to_vec();
    }

    /// Take a request, or say why not. Nothing runs here: the loop runs it on its next turn.
    pub(crate) fn submit(&mut self, request: ExternalRequest) -> Result<RequestRef, SubmitError> {
        let ExternalRequest {
            sender,
            session,
            request_id,
            line,
        } = request;
        if self.closed {
            return Err(SubmitError::Refused("the server is stopping".into()));
        }
        if !self.has_policy {
            return Err(SubmitError::Refused(format!(
                "{} declares no action policy, so requests handed in for others are refused",
                self.service
            )));
        }
        if line.trim().is_empty() {
            return Err(SubmitError::Refused("the request is empty".into()));
        }
        // A query answers from the catalog and runs nothing, so no policy would ever see it.
        if crate::rpc::is_query(&line) {
            return Err(SubmitError::Refused(
                "a query is not an action: only actions can be handed in".into(),
            ));
        }
        if line.len() > self.max_line {
            return Err(SubmitError::Refused(format!(
                "the request is {} bytes, over the {}-byte limit (`[settings.connection] max_frame_bytes`)",
                line.len(),
                self.max_line
            )));
        }
        if !self.sessions.contains(&session) {
            return Err(SubmitError::UnknownSession);
        }
        if self.ids.contains_key(&request_id) {
            return Err(SubmitError::DuplicateId);
        }
        if self.open.len() >= self.capacity {
            return Err(SubmitError::QueueFull);
        }
        let reference = RequestRef {
            service: self.service.clone(),
            sender,
            session,
            request_id,
        };
        let serial = self.next;
        self.next += 1;
        self.ids.insert(reference.request_id.clone(), serial);
        self.open.insert(
            serial,
            Open {
                reference: reference.clone(),
                line,
                state: State::Queued,
            },
        );
        self.queue.push_back(serial);
        Ok(reference)
    }

    /// Every request waiting to run, now started.
    pub(crate) fn start_queued(&mut self) -> Vec<Queued> {
        let mut started = Vec::new();
        while let Some(serial) = self.queue.pop_front() {
            let Some(open) = self.open.get_mut(&serial) else {
                continue;
            };
            open.state = State::Running {
                outstanding: 0,
                issued: false,
                output: None,
            };
            started.push(Queued {
                handle: Handle {
                    reference: open.reference.clone(),
                    serial,
                },
                line: std::mem::take(&mut open.line),
            });
        }
        started
    }

    /// The last action of the request has been started: it ends as soon as nothing is owed to it.
    pub(crate) fn issued(&mut self, serial: u64) {
        if let Some(State::Running { issued, .. }) = self.state_of(serial) {
            *issued = true;
        }
        self.finish_if_complete(serial);
    }

    /// An action of the request answered with `line`, or will, through a ticket.
    pub(crate) fn answered(&mut self, serial: u64, line: Option<String>) {
        if let Some(State::Running { output, .. }) = self.state_of(serial)
            && line.is_some()
        {
            *output = line;
        }
    }

    pub(crate) fn owes(&mut self, serial: u64) {
        if let Some(State::Running { outstanding, .. }) = self.state_of(serial) {
            *outstanding += 1;
        }
    }

    /// A deferred answer the request was waiting on came in. An error ends it at once.
    pub(crate) fn settled(&mut self, serial: u64, result: Result<Option<String>, String>) {
        match result {
            Err(reason) => self.end(serial, RequestOutcome::Failed(reason)),
            Ok(line) => {
                if let Some(State::Running { outstanding, .. }) = self.state_of(serial) {
                    *outstanding = outstanding.saturating_sub(1);
                }
                self.answered(serial, line);
                self.finish_if_complete(serial);
            }
        }
    }

    /// End a request. Ending one that has already ended does nothing.
    pub(crate) fn end(&mut self, serial: u64, outcome: RequestOutcome) {
        let Some(open) = self.open.remove(&serial) else {
            return;
        };
        self.ids.remove(&open.reference.request_id);
        self.queue.retain(|queued| *queued != serial);
        self.finished.push_back((open.reference, outcome));
    }

    /// The server is stopping: take nothing more, and end every open request — one that never
    /// started as refused, one that did as unknown.
    pub(crate) fn close(&mut self) {
        self.closed = true;
        let mut open: Vec<(u64, bool)> = self
            .open
            .iter()
            .map(|(serial, open)| (*serial, matches!(open.state, State::Queued)))
            .collect();
        open.sort_unstable();
        for (serial, never_started) in open {
            self.end(
                serial,
                if never_started {
                    RequestOutcome::Refused("server stopped, not executed".into())
                } else {
                    RequestOutcome::Unknown
                },
            );
        }
    }

    /// The next ended request, with how it ended.
    pub(crate) fn next_finished(&mut self) -> Option<(RequestRef, RequestOutcome)> {
        self.finished.pop_front()
    }

    fn state_of(&mut self, serial: u64) -> Option<&mut State> {
        self.open.get_mut(&serial).map(|open| &mut open.state)
    }

    fn finish_if_complete(&mut self, serial: u64) {
        let Some(State::Running {
            outstanding: 0,
            issued: true,
            output,
        }) = self.state_of(serial)
        else {
            return;
        };
        let line = output.take();
        self.end(serial, RequestOutcome::Done(line));
    }
}
