//! Selection changes a connection's session, never its server-assigned client identity.

use super::*;

impl ClientState {
    /// Keep the last complete frame while an attach's snapshots are still arriving.
    pub(crate) fn projection_ready(&self) -> bool {
        self.failed || self.projection_deadline.is_none()
    }

    pub(crate) fn switch_session(&mut self, name: String) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "server unavailable",
            ));
        }
        if self.switching {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "session selection is pending",
            ));
        }
        // Let the current RPC reply leave before its session is detached at the server.
        self.pending_selection = Some(ClientMsg::AttachTo { name });
        self.switching = true;
        self.wake.wake();
        Ok(())
    }

    /// **Send the selection only after the next frame is on screen** — the window stops drawing
    /// from the moment the other session is attached until it is ready, holding the last picture;
    /// the frame drawn first is the one without what the choice just closed (the palette).
    /// [`frame_presented`](Self::frame_presented) lets it go.
    ///
    /// A frame that never comes (the GPU refused one) holds it no longer than a request may take.
    pub(crate) fn hold_selection_for_a_frame(&mut self) {
        self.hold_selection = self.pending_selection.is_some().then(|| {
            Instant::now()
                + std::time::Duration::from_millis(self.connection.request_timeout_ms.get())
        });
    }

    /// A frame reached the screen: a held selection may go.
    pub(crate) fn frame_presented(&mut self) {
        if self.hold_selection.take().is_some() {
            self.wake.wake();
        }
    }

    pub(crate) fn detach_session(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "server unavailable",
            ));
        }
        if self.detached {
            return Ok(());
        }
        if self.switching {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "session selection is pending",
            ));
        }
        self.pending_selection = Some(ClientMsg::LeaveSession);
        self.switching = true;
        self.wake.wake();
        Ok(())
    }

    /// An attach is a projection boundary. Old pane ids and outboxes must not reach the new one.
    pub(super) fn reset_projection(&mut self) {
        let (outbox, requests) = outbox::channel(self.wake.clone());
        self.outbox = outbox;
        self.requests = requests;
        self.pending.clear();
        self.backends.clear();
        self.data = Default::default();
        self.images = Default::default();
        self.reads = Default::default();
        self.switching = false;
        self.pending_selection = None;
        self.projection_deadline = Some(
            Instant::now()
                + std::time::Duration::from_millis(self.connection.request_timeout_ms.get()),
        );
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
        let link = crate::server::socket::connect(
            self.connection,
            wake,
            crate::server::socket::Start::IfMissing,
        )?;
        self.reconnect_link(link, &receive)
    }

    pub(super) fn reconnect_link(
        &mut self,
        link: Box<dyn ClientLink>,
        ready: &mpsc::Receiver<()>,
    ) -> io::Result<()> {
        link.send(ClientMsg::AttachTo {
            name: self.session.name.clone(),
        })?;
        // Consume only the boundary here; leave terminal frames on the new link for normal polling.
        match crate::server::wire::await_message(link.as_ref(), ready, self.connection, || {})? {
            message @ ServerMsg::Attached { .. } => {
                if let ServerMsg::Attached { session, .. } = &message
                    && session.name != self.session.name
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "server attached a different session",
                    ));
                }
                self.link = Box::new(Reattached {
                    first: std::cell::RefCell::new(Some(message)),
                    link,
                });
                self.failed = false;
                self.reset_projection();
                // A new physical connection starts new local delivery cursors, even for the same id.
                self.feedback = Default::default();
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
    fn flush(&self) -> io::Result<()> {
        self.link.flush()
    }
    fn poll(&self) -> io::Result<Option<ServerMsg>> {
        if let Some(first) = self.first.borrow_mut().take() {
            Ok(Some(first))
        } else {
            self.link.poll()
        }
    }
}
