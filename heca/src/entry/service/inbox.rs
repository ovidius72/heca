//! **The one thing that crosses threads** — a worker's way back into its service.

use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

use crate::server::ServerWaker;

/// A bounded queue into a service. Clone it into any thread; [`send`](Self::send) never blocks,
/// and wakes the server, which hands the message to [`Service::receive`](super::Service::receive).
///
/// Its size is `[settings.services] inbox_capacity`.
pub struct ServiceInbox<M> {
    sender: SyncSender<M>,
    wake: ServerWaker,
}

/// Why a message was not taken. The message comes back.
#[derive(Debug, PartialEq, Eq)]
pub enum ServiceSendError<M> {
    /// The inbox is full: the service has not caught up yet.
    Full(M),
    /// The service has stopped.
    Closed(M),
}

impl<M> ServiceInbox<M> {
    pub(crate) fn channel(capacity: usize, wake: ServerWaker) -> (Self, Receiver<M>) {
        let (sender, messages) = sync_channel(capacity);
        (Self { sender, wake }, messages)
    }

    /// Queue `message` for the service and wake the server.
    pub fn send(&self, message: M) -> Result<(), ServiceSendError<M>> {
        self.sender.try_send(message).map_err(|error| match error {
            TrySendError::Full(message) => ServiceSendError::Full(message),
            TrySendError::Disconnected(message) => ServiceSendError::Closed(message),
        })?;
        self.wake.wake();
        Ok(())
    }

    /// Bring the server round again without a message — when one turn left messages queued.
    pub(crate) fn wake(&self) {
        self.wake.wake();
    }
}

impl<M> Clone for ServiceInbox<M> {
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone(), wake: self.wake.clone() }
    }
}
