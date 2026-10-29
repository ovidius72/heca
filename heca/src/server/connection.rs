//! One identity and physical link per connection. Only ServerLoop receives wire input.

use super::wire::{ClientMsg, ServerLink};
use heca_core::layout::SessionId;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
};

#[derive(Clone, Copy)]
pub(super) enum Attachment {
    Pending,
    Attached,
}

pub(super) struct Connection {
    pub id: u64,
    pub link: Box<dyn ServerLink>,
    pub attachment: Cell<Attachment>,
    pub session: Cell<Option<SessionId>>,
    pub window_ordinal: Cell<u64>,
    pub retire_at: Cell<Option<std::time::Instant>>,
    input: RefCell<VecDeque<ClientMsg>>,
    closed: Cell<bool>,
}

impl Connection {
    pub(super) fn new(id: u64, link: Box<dyn ServerLink>) -> Self {
        Self {
            id,
            link,
            attachment: Cell::new(Attachment::Pending),
            session: Cell::new(None),
            window_ordinal: Cell::new(0),
            retire_at: Cell::new(None),
            input: Default::default(),
            closed: Cell::new(false),
        }
    }

    /// The owner loop calls this at the sole physical receive point, beside this connection's id.
    pub(super) fn receive(&self) -> io::Result<Option<ClientMsg>> {
        self.link.poll()
    }

    pub(super) fn queue(&self, message: ClientMsg) {
        self.input.borrow_mut().push_back(message);
    }

    /// A session consumes already-routed messages; it does not read a socket or mint client ids.
    pub(super) fn poll(&self) -> io::Result<Option<ClientMsg>> {
        if self.closed.get() {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "client disconnected",
            ));
        }
        Ok(self.input.borrow_mut().pop_front())
    }

    pub(super) fn close(&self) {
        self.closed.set(true);
    }
    pub(super) fn is_closed(&self) -> bool {
        self.closed.get()
    }
}
