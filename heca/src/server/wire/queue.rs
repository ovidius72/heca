//! Bounded transport queues. Replace pending state, never discard ordered input or effects.

use std::{collections::VecDeque, io, num::NonZeroUsize, sync::{Arc, Condvar, Mutex}};

pub(crate) trait Message {
    fn replaces(&self, _older: &Self) -> bool { false }
}

struct State<T> { messages: VecDeque<T>, closed: bool }
struct Shared<T> { state: Mutex<State<T>>, ready: Condvar, capacity: usize }
pub(super) struct Sender<T>(Arc<Shared<T>>);
pub(super) struct Receiver<T>(Arc<Shared<T>>);

pub(super) fn channel<T>(capacity: NonZeroUsize) -> (Sender<T>, Receiver<T>) {
    let shared = Arc::new(Shared { state: Mutex::new(State { messages: VecDeque::new(), closed: false }),
        ready: Condvar::new(), capacity: capacity.get() });
    (Sender(shared.clone()), Receiver(shared))
}

impl<T: Message> Sender<T> {
    pub(super) fn send(&self, message: T) -> io::Result<()> {
        let mut state = self.0.state.lock().map_err(poisoned)?;
        if state.closed { return Err(closed()); }
        if let Some(index) = state.messages.iter().position(|older| message.replaces(older)) {
            state.messages[index] = message;
            self.0.ready.notify_one();
            return Ok(());
        } else if state.messages.len() == self.0.capacity {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "connection queue full"));
        }
        state.messages.push_back(message);
        self.0.ready.notify_one();
        Ok(())
    }
}

impl<T> Receiver<T> {
    pub(super) fn poll(&self) -> io::Result<Option<T>> {
        let mut state = self.0.state.lock().map_err(poisoned)?;
        if let Some(message) = state.messages.pop_front() { return Ok(Some(message)); }
        if state.closed { return Err(closed()); }
        Ok(None)
    }

    pub(super) fn recv(&self) -> io::Result<Option<T>> {
        let mut state = self.0.state.lock().map_err(poisoned)?;
        loop {
            if let Some(message) = state.messages.pop_front() { return Ok(Some(message)); }
            if state.closed { return Ok(None); }
            state = self.0.ready.wait(state).map_err(poisoned)?;
        }
    }
}

impl<T> Shared<T> {
    fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.closed = true;
        self.ready.notify_all();
    }
}

impl<T> Drop for Sender<T> { fn drop(&mut self) { self.0.close(); } }
impl<T> Drop for Receiver<T> { fn drop(&mut self) { self.0.close(); } }
fn closed() -> io::Error { io::Error::new(io::ErrorKind::BrokenPipe, "connection closed") }
fn poisoned<T>(error: std::sync::PoisonError<T>) -> io::Error { io::Error::other(error.to_string()) }

impl Message for super::ClientMsg {}
impl Message for super::ServerMsg {
    fn replaces(&self, older: &Self) -> bool {
        use super::{Change, ServerMsg};
        match (self, older) {
            (ServerMsg::Content { .. }, ServerMsg::Content { .. }) => true,
            // A notification projection is a whole snapshot: the newest replaces a still-queued one.
            (ServerMsg::Notifications(_), ServerMsg::Notifications(_)) => true,
            (ServerMsg::Terminal(new), ServerMsg::Terminal(old)) =>
                new.owner == old.owner && new.screen.viewport_offset == old.screen.viewport_offset,
            (ServerMsg::Changed(Change::LayoutChanged), ServerMsg::Changed(Change::LayoutChanged))
            | (ServerMsg::Changed(Change::NamesChanged), ServerMsg::Changed(Change::NamesChanged))
            | (ServerMsg::Changed(Change::NotificationsChanged), ServerMsg::Changed(Change::NotificationsChanged)) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{Change, wire::{ClientMsg, ServerMsg}};

    #[test]
    fn state_coalesces_but_ordered_input_reports_backpressure() {
        let capacity = NonZeroUsize::new(1).unwrap();
        let (sender, receiver) = channel(capacity);
        sender.send(ServerMsg::Changed(Change::LayoutChanged)).unwrap();
        sender.send(ServerMsg::Changed(Change::LayoutChanged)).unwrap();
        assert!(matches!(receiver.poll().unwrap(), Some(ServerMsg::Changed(Change::LayoutChanged))));
        assert!(receiver.poll().unwrap().is_none());
        let (sender, receiver) = channel(capacity);
        sender.send(ClientMsg::Attach).unwrap();
        assert_eq!(sender.send(ClientMsg::Detach).unwrap_err().kind(), io::ErrorKind::WouldBlock);
        assert!(matches!(receiver.poll().unwrap(), Some(ClientMsg::Attach)));
    }
}
