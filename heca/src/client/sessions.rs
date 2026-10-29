//! Selection changes a connection's session, never its server-assigned client identity.

use super::*;

impl ClientState {
    pub(crate) fn switch_session(&mut self, name: String) -> io::Result<()> {
        if self.failed { return Err(io::Error::new(io::ErrorKind::BrokenPipe, "server unavailable")); }
        if self.switching {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "session selection is pending",
            ));
        }
        // Let the current RPC reply leave before its session is detached at the server.
        self.pending_selection = Some(name);
        self.switching = true;
        self.wake.wake();
        Ok(())
    }

    /// An attach is a projection boundary. Old pane ids and outboxes must not reach the new one.
    pub(super) fn reset_projection(&mut self) {
        let (outbox, requests) =
            outbox::channel(self.connection.queue_capacity.get(), self.wake.clone());
        self.outbox = outbox;
        self.requests = requests;
        self.pending.clear();
        self.backends.clear();
        self.data = Default::default();
        self.images = Default::default();
        self.reads = Default::default();
        self.switching = false;
        self.pending_selection = None;
    }

    pub(crate) fn reconnect_session(&mut self) -> io::Result<()> {
        if self.local.is_some() {
            return Err(io::Error::other("the standalone server has stopped"));
        }
        let (ready, receive) = mpsc::sync_channel(1);
        let wake = self.wake.clone();
        let wake = ServerWaker::new(move || {
            wake.wake();
            let _ = ready.try_send(());
        });
        let link = transport::connect(self.connection, wake)?;
        link.send(ClientMsg::AttachTo {
            name: self.session.name.clone(),
        })?;
        // Consume only the boundary here; leave terminal frames on the new link for normal polling.
        match crate::server::wire::await_message(link.as_ref(), &receive, self.connection, || {})? {
            message @ ServerMsg::Attached { .. } => {
                self.link = Box::new(Reattached {
                    first: std::cell::RefCell::new(Some(message)),
                    link,
                });
                self.failed = false;
                self.reset_projection();
                self.wake.wake();
                Ok(())
            }
            ServerMsg::Sessions {
                result: Err(error), ..
            } => Err(io::Error::other(error)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "server did not acknowledge reconnect",
            )),
        }
    }
}

struct Reattached {
    first: std::cell::RefCell<Option<ServerMsg>>,
    link: Box<dyn ClientLink>,
}
impl ClientLink for Reattached {
    fn send(&self, message: ClientMsg) -> io::Result<()> {
        self.link.send(message)
    }
    fn flush(&self) -> io::Result<()> { self.link.flush() }
    fn poll(&self) -> io::Result<Option<ServerMsg>> {
        if let Some(first) = self.first.borrow_mut().take() {
            Ok(Some(first))
        } else {
            self.link.poll()
        }
    }
}
