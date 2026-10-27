//! Typed, nonblocking requests from terminal views to their connection owner.

use std::{io, sync::{Arc, Mutex, mpsc::{self, Receiver, SyncSender}}};
use crate::server::{ServerWaker, wire::ClientMsg};

#[derive(Clone)]
pub(super) struct Outbox {
    sender: SyncSender<ClientMsg>,
    wake: ServerWaker,
    failure: Arc<Mutex<Option<String>>>,
}

pub(super) fn channel(capacity: usize, wake: ServerWaker) -> (Outbox, Receiver<ClientMsg>) {
    let (sender, receiver) = mpsc::sync_channel(capacity);
    (Outbox { sender, wake, failure: Arc::default() }, receiver)
}

impl Outbox {
    pub(super) fn send(&self, message: ClientMsg) {
        if let Err(error) = self.sender.try_send(message) {
            let mut failure = self.failure.lock().unwrap_or_else(|error| error.into_inner());
            if failure.is_none() { *failure = Some(error.to_string()); }
        }
        self.wake.wake();
    }

    pub(super) fn check(&self) -> io::Result<()> {
        match &*self.failure.lock().map_err(|error| io::Error::other(error.to_string()))? {
            Some(error) => Err(io::Error::new(io::ErrorKind::BrokenPipe, error.clone())),
            None => Ok(()),
        }
    }
}
