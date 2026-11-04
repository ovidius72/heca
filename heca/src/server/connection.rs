//! One identity and physical link per connection. Only ServerLoop receives wire input.

use super::wire::{ClientMsg, ServerLink, ServerMsg};
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
    Detached,
}

pub(super) struct Connection {
    pub id: u64,
    pub link: Box<dyn ServerLink>,
    pub attachment: Cell<Attachment>,
    pub projection_pending: Cell<bool>,
    pub session: Cell<Option<SessionId>>,
    pub window_ordinal: Cell<u64>,
    /// The title this window reported for itself, once it has. `None` until the first report (the
    /// brief gap between attach and the window formatting its title).
    pub title: RefCell<Option<String>>,
    /// The window this one follows (mirrors), by id, or `None`.
    pub follows: Cell<Option<u64>>,
    /// Whether this window was last told it is followed — so a change is sent once, not every pump.
    pub was_followed: Cell<bool>,
    /// This window's last reported view, relayed to its followers (and sent once when a new
    /// follower starts). `None` until it reports one.
    pub view: RefCell<Option<heca_core::layout::ViewSnapshot>>,
    input: RefCell<VecDeque<ClientMsg>>,
    closed: Cell<bool>,
    next_rpc: Cell<u64>,
    pending_rpc: RefCell<std::collections::HashMap<u64, PendingRpc>>,
}

struct PendingRpc {
    reply: std::sync::mpsc::Sender<String>,
    deadline: std::time::Instant,
}

impl Connection {
    pub(super) fn new(id: u64, link: Box<dyn ServerLink>) -> Self {
        Self {
            id,
            link,
            attachment: Cell::new(Attachment::Pending),
            projection_pending: Cell::new(false),
            session: Cell::new(None),
            window_ordinal: Cell::new(0),
            title: RefCell::new(None),
            follows: Cell::new(None),
            was_followed: Cell::new(false),
            view: RefCell::new(None),
            input: Default::default(),
            closed: Cell::new(false),
            next_rpc: Cell::new(1),
            pending_rpc: Default::default(),
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
        for (_, request) in self.pending_rpc.borrow_mut().drain() {
            let _ = request.reply.send("error: attached client disconnected".into());
        }
    }
    pub(super) fn is_closed(&self) -> bool {
        self.closed.get()
    }

    /// Window RPC belongs to the physical connection, including while it has no session.
    pub(super) fn begin_rpc(&self, line: String, reply: std::sync::mpsc::Sender<String>,
        limits: heca_config::settings::ConnectionConfig) {
        let request_id = self.next_rpc.get();
        let Some(next) = request_id.checked_add(1) else {
            let _ = reply.send("error: no more request identifiers".into());
            return;
        };
        self.next_rpc.set(next);
        if self.link.send(ServerMsg::Rpc { request_id, line }).is_err() {
            let _ = reply.send("error: attached client disconnected".into());
            return;
        }
        self.pending_rpc.borrow_mut().insert(request_id, PendingRpc {
            reply, deadline: std::time::Instant::now()
                + std::time::Duration::from_millis(limits.request_timeout_ms.get()),
        });
    }

    pub(super) fn reply_rpc(&self, request_id: u64, answer: String) {
        if let Some(request) = self.pending_rpc.borrow_mut().remove(&request_id) {
            let _ = request.reply.send(answer);
        }
    }

    pub(super) fn rpc_deadline(&self) -> Option<std::time::Instant> {
        self.pending_rpc.borrow().values().map(|request| request.deadline).min()
    }

    pub(super) fn expire_rpc(&self, now: std::time::Instant) {
        self.pending_rpc.borrow_mut().retain(|_, request| {
            if request.deadline > now { return true; }
            let _ = request.reply.send("error: attached client timed out".into());
            false
        });
    }
}
